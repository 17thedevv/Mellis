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
            throw std::runtime_error("Unsupported type kind for AST conversion in Monomorphization Engine: " + std::to_string(static_cast<int>(type->getKind())));
    }
}

SymbolID MonomorphizationEngine::requestSpecialization(
    const FunctionDeclNode* genericTemplate,
    const std::vector<const Type*>& genericArgs,
    SourceLocation loc
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
    std::unordered_map<std::string, std::unique_ptr<TypeNode>> subs;
    for (size_t i = 0; i < genericTemplate->genericParams.size() && i < genericArgs.size(); ++i) {
        subs[std::string(genericTemplate->genericParams[i].name)] = typeToAST(genericArgs[i], symTable);
    }

    // 8. Run SubstitutionVisitor
    SubstitutionVisitor visitor(std::move(subs));
    visitor.substitute(*specializedAST);

    // 9. Re-run Resolver & TypeChecker
    // We assume resolver and typeChecker have `resolve` and `check` methods taking ASTNode*
    // In actual implementation, they might need to be hooked up to the Global Scope
    resolver.resolve(specializedAST.get());
    typeChecker.check(specializedAST.get());

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

    std::unordered_map<std::string, std::unique_ptr<TypeNode>> subs;
    for (size_t i = 0; i < genericTemplate->genericParams.size() && i < genericArgs.size(); ++i) {
        subs[std::string(genericTemplate->genericParams[i].name)] = typeToAST(genericArgs[i], symTable);
    }

    SubstitutionVisitor visitor(std::move(subs));
    visitor.substitute(*specializedAST);

    resolver.resolve(specializedAST.get());
    typeChecker.check(specializedAST.get());

    SymbolID newId = specializedAST->symbolId;
    insertedIt->second = newId;
    inProgress.erase(stableMangledName);
    specializedASTs.push_back(std::move(specializedAST));

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

    auto specializedAST = genericTemplate->cloneAs<EnumDeclNode>();
    specializedAST->name = stableMangledName;
    specializedAST->genericParams.clear();

    std::unordered_map<std::string, std::unique_ptr<TypeNode>> subs;
    for (size_t i = 0; i < genericTemplate->genericParams.size() && i < genericArgs.size(); ++i) {
        subs[std::string(genericTemplate->genericParams[i].name)] = typeToAST(genericArgs[i], symTable);
    }

    SubstitutionVisitor visitor(std::move(subs));
    visitor.substitute(*specializedAST);

    resolver.resolve(specializedAST.get());
    typeChecker.check(specializedAST.get());

    SymbolID newId = specializedAST->symbolId;
    insertedIt->second = newId;
    inProgress.erase(stableMangledName);
    specializedASTs.push_back(std::move(specializedAST));

    currentDepth--;
    return newId;
}

} // namespace fl
