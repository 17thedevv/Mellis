#include "mellis/MiddleEnd/MonomorphizationEngine.h"
#include "mellis/MiddleEnd/Mangle.h"
#include "mellis/MiddleEnd/SubstitutionVisitor.h"
#include "mellis/AST/TypeNode.h"
#include <stdexcept>
#include <iostream>

namespace fl {

// Helper to convert a semantic Type back to a TypeNode AST for the SubstitutionVisitor
static std::unique_ptr<TypeNode> typeToAST(const Type* type, const SymbolTable& symTable) {
    if (!type) return nullptr;
    
    switch (type->getKind()) {
        case TypeKind::Primitive: {
            auto* p = dynamic_cast<const PrimitiveType*>(type);
            auto node = std::make_unique<BuiltinTypeNode>();
            node->kind = p->builtinKind;
            return node;
        }
        case TypeKind::Struct: {
            auto* s = dynamic_cast<const StructType*>(type);
            auto node = std::make_unique<NamedTypeNode>();
            const Symbol& sym = symTable.getSymbol(s->structSymbolId);
            node->segments.push_back(sym.name.view());
            node->symbolId = s->structSymbolId;
            for (const auto* arg : s->genericArgs) {
                node->genericArgs.push_back(typeToAST(arg, symTable));
            }
            return node;
        }
        case TypeKind::Pointer: {
            auto* pt = dynamic_cast<const PointerType*>(type);
            auto node = std::make_unique<PointerTypeNode>();
            node->isMutable = pt->isMutable;
            node->inner = typeToAST(pt->pointee, symTable);
            return node;
        }
        case TypeKind::Reference: {
            auto* rt = dynamic_cast<const ReferenceType*>(type);
            auto node = std::make_unique<ReferenceTypeNode>();
            node->isMutable = rt->isMutable;
            node->inner = typeToAST(rt->pointee, symTable);
            return node;
        }
        // ... Enum, Tuple, Array can be mapped similarly
        default:
            // Fallback for simple generic parameters that might not be fully specialized yet
            if (auto* gp = dynamic_cast<const GenericParamType*>(type)) {
                auto node = std::make_unique<NamedTypeNode>();
                node->segments.push_back(gp->name);
                return node;
            }
            if (auto* ltTy = dynamic_cast<const LifetimeType*>(type)) {
                auto node = std::make_unique<LifetimeNode>();
                if (ltTy->lt.kind == LifetimeKind::Static) {
                    node->name = "'static";
                } else if (ltTy->lt.kind == LifetimeKind::Anonymous) {
                    node->name = "'_";
                } else {
                    // String literal from interned or just keep it?
                    // Actually ltTy->lt.name is std::string, but node->name is std::string_view.
                    // We might need to copy it or leak it, but for now we just assign it. 
                    // To be safe we should use a global string pool or just leak it.
                    // Let's use new std::string to ensure lifetime, but memory leak is okay for this compiler phase during monomorphization.
                    auto* str = new std::string(ltTy->lt.name);
                    node->name = *str;
                }
                return node;
            }
            throw std::runtime_error("Unsupported type kind for AST conversion in Monomorphization Engine: " + std::to_string(static_cast<int>(type->getKind())));
    }
}

SymbolID MonomorphizationEngine::requestSpecialization(
    const FunctionDeclNode* genericTemplate,
    const std::vector<const Type*>& genericArgs,
    SourceLocation loc,
    const ImplDeclNode* parentImpl,
    const Type* selfType
) {
    if (currentDepth >= kMaxDepth) {
        diag.error(loc, "Maximum generic instantiation depth exceeded. Infinite recursion detected.");
        return kInvalidSymbolID;
    }
    currentDepth++;
    // 1. Check Generic Bounds First
    for (size_t i = 0; i < genericTemplate->genericParams.size(); ++i) {
        if (i >= genericArgs.size()) break;
        auto& param = genericTemplate->genericParams[i];
        const Type* argType = genericArgs[i];
        
        for (auto& boundNode : param.bounds) {
            auto* named = dynamic_cast<NamedTypeNode*>(boundNode.get());
            if (named && named->symbolId != kInvalidSymbolID) {
                // named->symbolId should point to the Trait
                // Use typeChecker to check if argType implements this Trait
                if (!typeChecker.implementsTrait(argType, named->symbolId)) {
                    diag.error(loc, "Generic bounds check failed: Type does not implement required trait");
                    currentDepth--;
                    return kInvalidSymbolID;
                }
            }
        }
    }

    // S7.4d Check BorrowCheckStatus
    if (genericTemplate->symbolId != kInvalidSymbolID) {
        auto status = symTable.getFunctionInfo(genericTemplate->symbolId).borrowCheckStatus;
        if (status != BorrowCheckStatus::Checked && status != BorrowCheckStatus::Skipped) {
            if (symTable.getSymbol(genericTemplate->symbolId).isExternal) {
                diag.error(loc, "Cannot instantiate generic function that has not passed borrow checking.");
                currentDepth--;
                return kInvalidSymbolID;
            }
        }
    }

    // 2. Generate Mangled Name
    std::string mangledName = Mangle::mangleFunction(genericTemplate->name, genericArgs, symTable);

    // 3. Check Cache & Get stable string reference
    auto it = specializedRegistry.find(mangledName);
    if (it != specializedRegistry.end()) {
        currentDepth--;
        return it->second;
    }
    
    // We insert a placeholder to get a stable std::string reference for the AST's string_view
    auto [insertedIt, inserted] = specializedRegistry.insert({std::move(mangledName), kInvalidSymbolID});
    const std::string& stableMangledName = insertedIt->first;

    // 3. Cycle Detection
    if (inProgress.count(stableMangledName)) {
        diag.error(loc, "Infinite generic recursion detected for: " + stableMangledName);
        currentDepth--;
        return kInvalidSymbolID;
    }
    
    // 4. Mark In-Progress
    inProgress.insert(stableMangledName);

    // 5. Clone AST
    auto specializedAST = genericTemplate->cloneAs<FunctionDeclNode>();
    
    // 6. Rename AST to mangled name
    specializedAST->name = stableMangledName;
    specializedAST->genericParams.clear(); // The new function is NO LONGER generic!

    // 7. Prepare Substitution Map
    GenericSubstitution subs;
    size_t argIdx = 0;
    for (size_t i = 0; i < genericTemplate->genericParams.size(); ++i) {
        if (argIdx < genericArgs.size()) {
            std::string paramName = std::string(genericTemplate->genericParams[i].name);
            auto astNode = typeToAST(genericArgs[argIdx], symTable);
            
            if (genericTemplate->genericParams[i].kind == GenericParamKind::Type) {
                subs.typeSubstitutions[paramName] = std::move(astNode);
                std::cerr << "[DEBUG] Setting type substitution for '" << paramName << "' to a concrete type.\n";
            } else if (genericTemplate->genericParams[i].kind == GenericParamKind::Lifetime) {
                if (auto* ltNode = dynamic_cast<LifetimeNode*>(astNode.get())) {
                    std::unique_ptr<LifetimeNode> ownedLt(static_cast<LifetimeNode*>(astNode.release()));
                    subs.lifetimeSubstitutions[paramName] = std::move(ownedLt);
                    std::cerr << "[DEBUG] Setting lifetime substitution for '" << paramName << "'.\n";
                }
            }
            argIdx++;
        }
    }

    // 8. Run SubstitutionVisitor
    SubstitutionVisitor visitor(std::move(subs));
    visitor.substitute(*specializedAST);

    // 9. Re-run Resolver & TypeChecker
    const Symbol& origSym = symTable.getSymbol(genericTemplate->symbolId);
    resolver.resolve(specializedAST.get(), origSym.declaredInScope);
    typeChecker.check(specializedAST.get(), origSym.moduleID);

    // 10. Register
    SymbolID newId = specializedAST->symbolId;
    insertedIt->second = newId; // Update the placeholder with the real ID
    inProgress.erase(stableMangledName);
    specializedASTs.push_back(std::move(specializedAST));

    currentDepth--;
    return newId;
}

SymbolID MonomorphizationEngine::requestStructSpecialization(
    const StructDeclNode* genericTemplate,
    const std::vector<const Type*>& genericArgs,
    SourceLocation loc
) {
    if (currentDepth >= kMaxDepth) {
        diag.error(loc, "Maximum generic instantiation depth exceeded. Infinite recursion detected.");
        return kInvalidSymbolID;
    }
    currentDepth++;

    std::string mangledName = Mangle::mangleStruct(genericTemplate->name, genericArgs, symTable);

    auto it = specializedRegistry.find(mangledName);
    if (it != specializedRegistry.end()) {
        currentDepth--;
        return it->second;
    }
    
    auto [insertedIt, inserted] = specializedRegistry.insert({std::move(mangledName), kInvalidSymbolID});
    const std::string& stableMangledName = insertedIt->first;

    if (inProgress.count(stableMangledName)) {
        diag.error(loc, "Infinite generic recursion detected for: " + stableMangledName);
        currentDepth--;
        return kInvalidSymbolID;
    }
    inProgress.insert(stableMangledName);

    auto specializedAST = genericTemplate->cloneAs<StructDeclNode>();
    specializedAST->name = stableMangledName;
    specializedAST->genericParams.clear();

    GenericSubstitution subs;
    size_t argIdx = 0;
    for (size_t i = 0; i < genericTemplate->genericParams.size(); ++i) {
        if (argIdx < genericArgs.size()) {
            std::string paramName = std::string(genericTemplate->genericParams[i].name);
            auto astNode = typeToAST(genericArgs[argIdx], symTable);
            
            if (genericTemplate->genericParams[i].kind == GenericParamKind::Type) {
                subs.typeSubstitutions[paramName] = std::move(astNode);
            } else if (genericTemplate->genericParams[i].kind == GenericParamKind::Lifetime) {
                if (auto* ltNode = dynamic_cast<LifetimeNode*>(astNode.get())) {
                    std::unique_ptr<LifetimeNode> ownedLt(static_cast<LifetimeNode*>(astNode.release()));
                    subs.lifetimeSubstitutions[paramName] = std::move(ownedLt);
                }
            }
            argIdx++;
        }
    }

    SubstitutionVisitor visitor(std::move(subs));
    visitor.substitute(*specializedAST);

    const Symbol& origSym = symTable.getSymbol(genericTemplate->symbolId);
    resolver.resolve(specializedAST.get(), origSym.declaredInScope);
    typeChecker.check(specializedAST.get(), origSym.moduleID);

    SymbolID newId = specializedAST->symbolId;
    insertedIt->second = newId;
    
    // Set originalTemplateId on the new StructType
    const Type* specType = typeChecker.typeOf(newId);
    if (auto* mutSt = const_cast<StructType*>(dynamic_cast<const StructType*>(specType))) {
        mutSt->originalTemplateId = genericTemplate->symbolId;
        mutSt->specializedArgs = genericArgs;
    }
    
    inProgress.erase(stableMangledName);
    specializedASTs.push_back(std::move(specializedAST));

    // Specialize associated Impl blocks
    auto implIt = genericImpls.find(genericTemplate->symbolId);
    if (implIt != genericImpls.end()) {
        for (const auto* implNode : implIt->second) {
            auto specializedImpl = implNode->cloneAs<ImplDeclNode>();
            specializedImpl->genericParams.clear(); // Now concrete!
            
            GenericSubstitution implSubs;
            for (size_t i = 0; i < implNode->genericParams.size() && i < genericArgs.size(); ++i) {
                std::string paramName = std::string(implNode->genericParams[i].name);
                auto astNode = typeToAST(genericArgs[i], symTable);
                
                if (implNode->genericParams[i].kind == GenericParamKind::Type) {
                    implSubs.typeSubstitutions[paramName] = std::move(astNode);
                } else if (implNode->genericParams[i].kind == GenericParamKind::Lifetime) {
                    if (auto* ltNode = dynamic_cast<LifetimeNode*>(astNode.get())) {
                        std::unique_ptr<LifetimeNode> ownedLt(static_cast<LifetimeNode*>(astNode.release()));
                        implSubs.lifetimeSubstitutions[paramName] = std::move(ownedLt);
                    }
                }
            }
            
            SubstitutionVisitor implVisitor(std::move(implSubs));
            implVisitor.substitute(*specializedImpl);
            
            const Symbol& origSym = symTable.getSymbol(genericTemplate->symbolId);
            resolver.resolve(specializedImpl.get(), origSym.declaredInScope);
            typeChecker.check(specializedImpl.get(), origSym.moduleID);
            
            specializedASTs.push_back(std::move(specializedImpl));
        }
    }

    currentDepth--;
    return newId;
}

SymbolID MonomorphizationEngine::requestEnumSpecialization(
    const EnumDeclNode* genericTemplate,
    const std::vector<const Type*>& genericArgs,
    SourceLocation loc
) {
    if (currentDepth >= kMaxDepth) {
        diag.error(loc, "Maximum generic instantiation depth exceeded. Infinite recursion detected.");
        return kInvalidSymbolID;
    }
    currentDepth++;

    std::string mangledName = Mangle::mangleStruct(genericTemplate->name, genericArgs, symTable);
    std::cerr << "[DEBUG] IN requestEnumSpecialization! mangledName=" << mangledName << std::endl;

    auto it = specializedRegistry.find(mangledName);
    if (it != specializedRegistry.end()) {
        std::cerr << "[DEBUG] Already specialized: " << it->second << std::endl;
        currentDepth--;
        return it->second;
    }
    
    auto [insertedIt, inserted] = specializedRegistry.insert({std::move(mangledName), kInvalidSymbolID});
    const std::string& stableMangledName = insertedIt->first;

    if (inProgress.count(stableMangledName)) {
        diag.error(loc, "Infinite generic recursion detected for: " + stableMangledName);
        currentDepth--;
        return kInvalidSymbolID;
    }
    inProgress.insert(stableMangledName);

    auto specializedAST = genericTemplate->cloneAs<EnumDeclNode>();
    specializedAST->name = stableMangledName;
    specializedAST->genericParams.clear();

    GenericSubstitution subs;
    size_t argIdx = 0;
    for (size_t i = 0; i < genericTemplate->genericParams.size(); ++i) {
        if (genericTemplate->genericParams[i].kind == GenericParamKind::Type) {
            if (argIdx < genericArgs.size()) {
                subs.typeSubstitutions[std::string(genericTemplate->genericParams[i].name)] = typeToAST(genericArgs[argIdx], symTable);
                argIdx++;
            }
        }
    }

    SubstitutionVisitor visitor(std::move(subs));
    visitor.substitute(*specializedAST);

    const Symbol& origSym = symTable.getSymbol(genericTemplate->symbolId);
    resolver.resolve(specializedAST.get(), origSym.declaredInScope);
    typeChecker.check(specializedAST.get(), origSym.moduleID);

    SymbolID newId = specializedAST->symbolId;
    insertedIt->second = newId;
    inProgress.erase(stableMangledName);
    specializedASTs.push_back(std::move(specializedAST));

    currentDepth--;
    return newId;
}


void MonomorphizationEngine::registerGenericImpl(SymbolID targetStructId, const ImplDeclNode* implNode) {
    if (targetStructId != kInvalidSymbolID) {
        genericImpls[targetStructId].push_back(implNode);
    }
}
} // namespace fl
