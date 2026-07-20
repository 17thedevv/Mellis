#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/MiddleEnd/MonomorphizationEngine.h"
#include "mellis/MiddleEnd/ConstEvaluator.h"
#include <iostream>
#include <typeinfo>
#include <vector>

namespace fl {

struct UnsafeContextGuard {
    bool& flag;
    bool prevState;
    UnsafeContextGuard(bool& f) : flag(f), prevState(f) { flag = true; }
    ~UnsafeContextGuard() { flag = prevState; }
};

enum class ConstraintKind { Equality, Field, MethodCall, Iterable, EnumVariantPattern, Deref };

struct Constraint {
    ConstraintKind kind;
    const Type* expected;
    const Type* actual;
    std::string name;
    std::vector<const Type*> callArgs; // Used for generic args or field types
    std::vector<std::string> callArgNames; // Labels for MethodCall
    PatternNode* pattern = nullptr;
    SourceLocation loc;
    
    Constraint(ConstraintKind k, const Type* exp, const Type* act, std::string n, std::vector<const Type*> args, std::vector<std::string> argNames, SourceLocation l)
        : kind(k), expected(exp), actual(act), name(n), callArgs(args), callArgNames(argNames), loc(l) {}
        
    Constraint(ConstraintKind k, const Type* exp, const Type* act, std::string n, SourceLocation l)
        : kind(k), expected(exp), actual(act), name(n), callArgs({}), loc(l) {}
        
    Constraint(ConstraintKind k, const Type* exp, SymbolID vId, PatternNode* pat, SourceLocation l)
        : kind(k), expected(exp), actual(nullptr), name(std::to_string(vId)), callArgs({}), pattern(pat), loc(l) {}
};

TypeChecker::TypeChecker(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, MonomorphizationEngine* monoEngine)
    : table_(table), diag_(diag), ctx_(ctx), monoEngine_(monoEngine) {}

void TypeChecker::MethodResolver::addMethod(const Type* receiverType, const std::string& name, SymbolID methodId, const FunctionType* type, TypeContext& ctx, const Type* selfTypeOverride) {
    const Type* actualSelf = selfTypeOverride ? selfTypeOverride : receiverType;
    auto replaceSelf = [&](const Type* t) -> const Type* {
        if (!t) return t;
        if (auto* gp = dynamic_cast<const GenericParamType*>(t)) {
            if (gp->name == "Self") return actualSelf;
        }
        if (auto* ref = dynamic_cast<const ReferenceType*>(t)) {
            if (auto* gp = dynamic_cast<const GenericParamType*>(ref->pointee)) {
                if (gp->name == "Self") return ctx.getReferenceType(actualSelf, ref->isMutable);
            }
        }
        if (auto* ptr = dynamic_cast<const PointerType*>(t)) {
            if (auto* gp = dynamic_cast<const GenericParamType*>(ptr->pointee)) {
                if (gp->name == "Self") return ctx.getPointerType(actualSelf, ptr->isMutable);
            }
        }
        return t;
    };
    
    std::vector<const Type*> newParamTypes;
    for (auto* pt : type->paramTypes) {
        newParamTypes.push_back(replaceSelf(pt));
    }
    const Type* newRetType = replaceSelf(type->returnType);
    const FunctionType* newFnTy = ctx.getFunctionType(type->paramNames, std::move(newParamTypes), newRetType, type->isCallSite, type->isVariadic);
    implMap[receiverType][name] = {methodId, newFnTy};
}

bool TypeChecker::MethodResolver::probe(const Type* receiverType, const std::string& name, MethodInfo& outMethod) {
    auto checkType = [&](const Type* t) -> bool {
        auto it = implMap.find(t);
        if (it != implMap.end()) {
            auto methodIt = it->second.find(name);
            if (methodIt != it->second.end()) {
                outMethod = methodIt->second;
                return true;
            }
        }
        return false;
    };
    
    if (checkType(receiverType)) return true;
    
    if (auto* ref = dynamic_cast<const ReferenceType*>(receiverType)) {
        if (checkType(ref->pointee)) return true;
    }
    if (auto* ptr = dynamic_cast<const PointerType*>(receiverType)) {
        if (checkType(ptr->pointee)) return true;
    }
    
    return false;
}

bool TypeChecker::check(ASTNode* root) {
    if (!root) return false;
    typeTable_.resize(table_.symbolCount(), ctx_.getUnknown());

    class TypePrePass : public ASTVisitor, public TypeVisitor {
        SymbolTable& table;
        TypeContext& ctx;
        std::vector<const Type*>& typeTable;
        MethodResolver& methodResolver;
        std::unordered_map<const Type*, std::unordered_set<SymbolID>>& implementedTraits;
        const Type* evaluatedType = nullptr;
        MonomorphizationEngine* monoEngine;
        DiagnosticEngine& diag;

    public:
        const Type* evaluateTypeNode(TypeNode* node) {
            if (!node) return ctx.getVoid();
            evaluatedType = nullptr;
            node->accept(static_cast<TypeVisitor&>(*this));
            return evaluatedType ? evaluatedType : ctx.getUnknown();
        }

        TypePrePass(SymbolTable& table, TypeContext& ctx, std::vector<const Type*>& typeTable, MethodResolver& methodResolver, std::unordered_map<const Type*, std::unordered_set<SymbolID>>& implementedTraits, MonomorphizationEngine* monoEngine, DiagnosticEngine& diag)
              : table(table), ctx(ctx), typeTable(typeTable), methodResolver(methodResolver), implementedTraits(implementedTraits), monoEngine(monoEngine), diag(diag) {}

        void visit(ProgramNode& node) override { for (auto& item : node.items) item->accept(*this); }
        void visit(ModDeclNode& node) override { for (auto& d : node.decls) d->accept(*this); }
        void visit(StructDeclNode& node) override {
            const StructType* stType = ctx.getStructType(node.symbolId);
            typeTable[node.symbolId] = stType;
            StructType* mutSt = const_cast<StructType*>(stType);
            for (size_t i = 0; i < node.fields.size(); ++i) {
                auto& field = node.fields[i];
                mutSt->fieldIndices[std::string(field->name)] = i;
                field->symbolId = table.lookup(Identifier(field->name), node.bodyScopeId).value_or(kInvalidSymbolID);
                if (field->symbolId != kInvalidSymbolID) {
                    typeTable[field->symbolId] = evaluateTypeNode(field->type.get());
                }
            }
        }
        void visit(EnumDeclNode& node) override {
            std::vector<const Type*> genericArgs;
            for (auto& param : node.genericParams) {
                genericArgs.push_back(ctx.getGenericParamType(param.symbolId, param.name));
                if (param.symbolId != kInvalidSymbolID) typeTable[param.symbolId] = genericArgs.back();
            }
            const Type* enumTy = genericArgs.empty() ? ctx.getEnumType(node.symbolId) : ctx.getEnumType(node.symbolId, genericArgs);
            typeTable[node.symbolId] = enumTy;
            for (auto& variant : node.variants) {
                if (variant->symbolId != kInvalidSymbolID) {
                    if (variant->fields.empty()) {
                        typeTable[variant->symbolId] = enumTy;
                    } else {
                        std::vector<const Type*> fieldTypes;
                        std::vector<std::string> paramNames;
                        for (auto& field : variant->fields) {
                            fieldTypes.push_back(evaluateTypeNode(field->type.get()));
                            paramNames.push_back(std::string(field->name));
                        }
                        typeTable[variant->symbolId] = ctx.getFunctionType(std::move(paramNames), std::move(fieldTypes), enumTy, false);
                    }
                }
            }
        }
        void visit(FunctionDeclNode& node) override {
            std::vector<std::string> paramNames;
            std::vector<const Type*> paramTypes;
            for (auto& param : node.params) {
                const Type* pt = evaluateTypeNode(param->type.get());
                if (param->symbolId != kInvalidSymbolID) typeTable[param->symbolId] = pt;
                paramNames.push_back(std::string(param->name));
                paramTypes.push_back(pt);
            }
            const Type* retType = evaluateTypeNode(node.returnType.get());
            if (node.isAsync) {
                  retType = ctx.create<FutureType>(retType);
              }
              typeTable[node.symbolId] = ctx.getFunctionType(std::move(paramNames), std::move(paramTypes), retType, false, node.isVariadic);
            
            // Register methods from generic bounds
            for (auto& gp : node.genericParams) {
                if (gp.symbolId != kInvalidSymbolID) {
                    const Type* gpType = ctx.getGenericParamType(gp.symbolId, gp.name);
                    for (auto& bound : gp.bounds) {
                        const Type* boundType = evaluateTypeNode(bound.get());
                        if (auto* traitTy = dynamic_cast<const TraitType*>(boundType)) {
                            implementedTraits[gpType].insert(traitTy->traitId);
                            const auto& traitSym = table.getSymbol(traitTy->traitId);
                            if (traitSym.kind == SymbolKind::Trait && traitSym.decl) {
                                auto* traitDecl = static_cast<TraitDeclNode*>(traitSym.decl);
                                for (auto& m : traitDecl->methods) {
                                    if (m->symbolId != kInvalidSymbolID) {
                                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                                            methodResolver.addMethod(gpType, std::string(m->name), m->symbolId, fnTy, ctx);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        void visit(VarDeclNode& node) override {}
        void visit(ParamDeclNode& node) override {}
        void visit(StructFieldNode& node) override {}
        void visit(EnumVariantNode& node) override {}
        void visit(TraitDeclNode& node) override {
            if (node.symbolId != kInvalidSymbolID) {
                typeTable[node.symbolId] = ctx.getTraitType(node.symbolId);
            }
            for (auto& method : node.methods) {
                method->accept(*this);
            }
            if (node.symbolId != kInvalidSymbolID) {
                const Type* dynTraitTy = ctx.getTraitObjectType(node.symbolId);
                const Type* refDynTraitTy = ctx.getReferenceType(dynTraitTy, false);
                const Type* mutRefDynTraitTy = ctx.getReferenceType(dynTraitTy, true);
                const Type* ptrDynTraitTy = ctx.getPointerType(dynTraitTy, false);
                const Type* mutPtrDynTraitTy = ctx.getPointerType(dynTraitTy, true);
                for (auto& m : node.methods) {
                    if (m->symbolId != kInvalidSymbolID) {
                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                            methodResolver.addMethod(dynTraitTy, std::string(m->name), m->symbolId, fnTy, ctx, dynTraitTy);
                            methodResolver.addMethod(refDynTraitTy, std::string(m->name), m->symbolId, fnTy, ctx, dynTraitTy);
                            methodResolver.addMethod(mutRefDynTraitTy, std::string(m->name), m->symbolId, fnTy, ctx, dynTraitTy);
                            methodResolver.addMethod(ptrDynTraitTy, std::string(m->name), m->symbolId, fnTy, ctx, dynTraitTy);
                            methodResolver.addMethod(mutPtrDynTraitTy, std::string(m->name), m->symbolId, fnTy, ctx, dynTraitTy);
                        }
                    }
                }
            }
            // Register methods for Generic Params in Trait
            for (auto& gp : node.genericParams) {
                if (gp.symbolId != kInvalidSymbolID) {
                    const Type* gpType = ctx.getGenericParamType(gp.symbolId, gp.name);
                    for (auto& bound : gp.bounds) {
                        const Type* boundType = evaluateTypeNode(bound.get());
                        if (auto* traitTy = dynamic_cast<const TraitType*>(boundType)) {
                            implementedTraits[gpType].insert(traitTy->traitId);
                            const auto& traitSym = table.getSymbol(traitTy->traitId);
                            if (traitSym.kind == SymbolKind::Trait && traitSym.decl) {
                                auto* traitDecl = static_cast<TraitDeclNode*>(traitSym.decl);
                                for (auto& m : traitDecl->methods) {
                                    if (m->symbolId != kInvalidSymbolID) {
                                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                                            methodResolver.addMethod(gpType, std::string(m->name), m->symbolId, fnTy, ctx);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        void visit(ImplDeclNode& node) override {
            if (!node.genericParams.empty()) {
                if (auto* nt = dynamic_cast<NamedTypeNode*>(node.selfType.get())) {
                    if (monoEngine) monoEngine->registerGenericImpl(nt->symbolId, &node);
                }
                return;
            }
            const Type* selfType = evaluateTypeNode(node.selfType.get());
            if (!selfType) return;

            auto optSelfId = table.lookup(Identifier(std::string("Self")), node.bodyScopeId);
            if (optSelfId) {
                typeTable[*optSelfId] = selfType;
            }
            
            // Register methods for Generic Params in Impl
            for (auto& gp : node.genericParams) {
                if (gp.symbolId != kInvalidSymbolID) {
                    const Type* gpType = ctx.getGenericParamType(gp.symbolId, gp.name);
                    for (auto& bound : gp.bounds) {
                        const Type* boundType = evaluateTypeNode(bound.get());
                        if (auto* traitTy = dynamic_cast<const TraitType*>(boundType)) {
                            implementedTraits[gpType].insert(traitTy->traitId);
                            const auto& traitSym = table.getSymbol(traitTy->traitId);
                            if (traitSym.kind == SymbolKind::Trait && traitSym.decl) {
                                auto* traitDecl = static_cast<TraitDeclNode*>(traitSym.decl);
                                for (auto& m : traitDecl->methods) {
                                    if (m->symbolId != kInvalidSymbolID) {
                                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                                            methodResolver.addMethod(gpType, std::string(m->name), m->symbolId, fnTy, ctx);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            const TraitType* traitTy = nullptr;
            if (node.traitType) {
                const Type* ty = evaluateTypeNode(node.traitType.get());
                traitTy = dynamic_cast<const TraitType*>(ty);
                if (!traitTy) {
                    // Note: diag needs to be accessible, but TypePrePass doesn't have it.
                    // We might need to pass DiagnosticEngine to TypePrePass.
                }
            }

            for (auto& method : node.methods) {
                method->accept(*this);
                if (method->symbolId != kInvalidSymbolID) {
                    if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[method->symbolId])) {
                        methodResolver.addMethod(selfType, std::string(method->name), method->symbolId, fnTy, ctx);
                    }
                }
            }
            
            if (traitTy) {
                implementedTraits[selfType].insert(traitTy->traitId);
                const auto& sym = table.getSymbol(traitTy->traitId);
                if (sym.kind == SymbolKind::Trait && sym.decl) {
                    auto* traitDecl = static_cast<TraitDeclNode*>(sym.decl);
                    // Minimal check: verify all trait methods are implemented
                    // A proper check would unify their types.
                    for (auto& tMethod : traitDecl->methods) {
                        bool found = false;
                        for (auto& iMethod : node.methods) {
                            if (iMethod->name == tMethod->name) {
                                found = true;
                                break;
                            }
                        }
                        if (!found) {
                            // Diagnostics needed
                        }
                    }
                }
            }
        }
        void visit(TypeAliasDeclNode& node) override {}
        void visit(UseDeclNode& node) override {}
        void visit(ExternDeclNode& node) override { if (node.func) node.func->accept(*this); }
        
        void visit(BlockStmtNode& node) override {}
        void visit(ExprStmtNode& node) override {}
        void visit(IfStmtNode& node) override {}
        void visit(WhileStmtNode& node) override {}
        void visit(ForStmtNode& node) override {}
        void visit(ReturnStmtNode& node) override {}
        void visit(BreakStmtNode& node) override {}
        void visit(ContinueStmtNode& node) override {}
        void visit(UnsafeStmtNode& node) override {}
        void visit(ComptimeStmtNode& node) override {}

        void visit(LiteralExpr& node) override {}
        void visit(IdentifierExpr& node) override {}
        void visit(BinaryExpr& node) override {}
        void visit(UnaryExpr& node) override {}
        void visit(AssignExpr& node) override {}
        void visit(CallExpr& node) override {}
        void visit(MethodCallExpr& node) override {}
        void visit(IndexExpr& node) override {}
        void visit(MemberExpr& node) override {}
        void visit(TupleIndexExpr& node) override {}
        void visit(CastExpr& node) override {}
        void visit(UnsizeCastExpr& node) override {}
        void visit(ArrayLiteralExpr& node) override {}
        void visit(TupleLiteralExpr& node) override {}
        void visit(StructInitExpr& node) override {}
        void visit(MatchExpr& node) override {}
        void visit(LambdaExpr& node) override {}
        void visit(AwaitExpr& node) override {
              if (node.expr) node.expr->accept(*this);
          }
        void visit(SizeofExpr& node) override {}
        void visit(AlignofExpr& node) override {}

        void visit(BuiltinTypeNode& node) override { 
            evaluatedType = ctx.getPrimitive(node.kind); 
        }
        void visit(NamedTypeNode& node) override {
            printf("DEBUG: NamedTypeNode visited (segments[0] = %s, symbolId = %u)\n", node.segments.empty() ? "empty" : std::string(node.segments[0]).c_str(), node.symbolId);
            if (node.symbolId != kInvalidSymbolID) {
                printf("DEBUG: Inside IF block\n");
                const auto& sym = table.getSymbol(node.symbolId);
                std::vector<const Type*> args;
                for (auto& argNode : node.genericArgs) {
                    args.push_back(evaluateTypeNode(argNode.get()));
                }
                if (sym.kind == SymbolKind::Struct) {
                    bool allConcrete = true;
                    for (auto* a : args) {
                        if (a->getKind() == TypeKind::InferenceVar || dynamic_cast<const GenericParamType*>(a)) {
                            allConcrete = false; break;
                        }
                    }
                    if (allConcrete && monoEngine && sym.decl) {
                        auto* structDecl = static_cast<const StructDeclNode*>(sym.decl);
                        if (!structDecl->genericParams.empty()) {
                            SymbolID specId = monoEngine->requestStructSpecialization(structDecl, args, node.loc);
                            if (specId != kInvalidSymbolID) {
                                node.symbolId = specId;
                                args.clear();
                                node.genericArgs.clear();
                            }
                        }
                    }
                    evaluatedType = ctx.getStructType(node.symbolId, args);
                }
                else if (sym.kind == SymbolKind::Enum) {
                    bool allConcrete = true;
                    for (auto* a : args) {
                        if (a->getKind() == TypeKind::InferenceVar || dynamic_cast<const GenericParamType*>(a)) {
                            allConcrete = false; break;
                        }
                    }
                    if (allConcrete && monoEngine && sym.decl) {
                        auto* enumDecl = static_cast<const EnumDeclNode*>(sym.decl);
                        if (!enumDecl->genericParams.empty()) {
                            SymbolID specId = monoEngine->requestEnumSpecialization(enumDecl, args, node.loc);
                            if (specId != kInvalidSymbolID) {
                                node.symbolId = specId;
                                args.clear();
                                node.genericArgs.clear();
                            }
                        }
                    }
                    evaluatedType = ctx.getEnumType(node.symbolId, args);
                }
                else if (sym.kind == SymbolKind::GenericParam) {
                    evaluatedType = ctx.getGenericParamType(node.symbolId, sym.name.view());
                } else if (sym.kind == SymbolKind::TypeAlias && sym.decl == nullptr) {
                    std::string name = std::string(sym.name.view());
                    if (name == "i32") evaluatedType = ctx.getPrimitive(BuiltinKind::I32);
                    else if (name == "u32") evaluatedType = ctx.getPrimitive(BuiltinKind::U32);
                    else if (name == "i64") evaluatedType = ctx.getPrimitive(BuiltinKind::I64);
                    else if (name == "u64") evaluatedType = ctx.getPrimitive(BuiltinKind::U64);
                    else if (name == "i128") evaluatedType = ctx.getPrimitive(BuiltinKind::I128);
                    else if (name == "u128") evaluatedType = ctx.getPrimitive(BuiltinKind::U128);
                    else if (name == "i16") evaluatedType = ctx.getPrimitive(BuiltinKind::I16);
                    else if (name == "u16") evaluatedType = ctx.getPrimitive(BuiltinKind::U16);
                    else if (name == "i8") evaluatedType = ctx.getPrimitive(BuiltinKind::I8);
                    else if (name == "u8") evaluatedType = ctx.getPrimitive(BuiltinKind::U8);
                    else if (name == "i4") evaluatedType = ctx.getPrimitive(BuiltinKind::I4);
                    else if (name == "u4") evaluatedType = ctx.getPrimitive(BuiltinKind::U4);
                    else if (name == "f32") evaluatedType = ctx.getPrimitive(BuiltinKind::F32);
                    else if (name == "f64") evaluatedType = ctx.getPrimitive(BuiltinKind::F64);
                    else if (name == "bool") evaluatedType = ctx.getPrimitive(BuiltinKind::Bool);
                    else if (name == "char") evaluatedType = ctx.getPrimitive(BuiltinKind::Char);
                    else if (name == "str") evaluatedType = ctx.getPrimitive(BuiltinKind::Str);
                    else evaluatedType = ctx.getUnknown();
                } else {
                    evaluatedType = typeTable[node.symbolId]; 
                }
            } else {
                evaluatedType = ctx.getUnknown();
            }
        }
        void visit(PointerTypeNode& node) override {
            const Type* inner = evaluateTypeNode(node.inner.get());
            evaluatedType = ctx.getPointerType(inner, node.isMutable);
        }
        void visit(ReferenceTypeNode& node) override {
            const Type* inner = evaluateTypeNode(node.inner.get());
            evaluatedType = ctx.getReferenceType(inner, node.isMutable);
        }
        void visit(ArrayTypeNode& node) override {
            const Type* elem = evaluateTypeNode(node.elementType.get());
            if (node.size) {
                auto evaluatedSize = ConstEvaluator::evaluate(node.size.get(), &table);
                if (!evaluatedSize.has_value()) {
                    diag.error(node.loc, "Array size must be a constant expression evaluated at compile-time");
                    node.resolvedSize = 0;
                } else {
                    node.resolvedSize = evaluatedSize.value();
                }
            }
            evaluatedType = ctx.getArrayType(elem, node.resolvedSize); 
        }
        void visit(TupleTypeNode& node) override {
            std::vector<const Type*> elements;
            for (auto& elem : node.elements) {
                elements.push_back(evaluateTypeNode(elem.get()));
            }
            evaluatedType = ctx.getTupleType(std::move(elements));
        }
        void visit(FunctionTypeNode& node) override {
            std::vector<const Type*> paramTypes;
            for (auto& param : node.params) {
                paramTypes.push_back(evaluateTypeNode(param.get()));
            }
            const Type* retType = evaluateTypeNode(node.returnType.get());
            evaluatedType = ctx.getFunctionType(std::move(paramTypes), retType);
        }
        void visit(NeverTypeNode& node) override {
            evaluatedType = ctx.getNever();
        }
        void visit(TraitObjectTypeNode& node) override {
            const Type* traitType = evaluateTypeNode(node.trait.get());
            if (auto* tr = dynamic_cast<const TraitType*>(traitType)) {
                evaluatedType = ctx.getTraitObjectType(tr->traitId);
            } else {
                evaluatedType = ctx.getUnknown();
            }
        }
    };
    
    class PatternConstraintVisitor : public PatternVisitor {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        std::vector<const Type*>& typeTable;
        std::vector<Constraint>& constraints;
        const Type* expectedType;
        
    public:
        PatternConstraintVisitor(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, std::vector<const Type*>& typeTable, std::vector<Constraint>& constraints, const Type* expected)
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), constraints(constraints), expectedType(expected) {}
            
        void visit(WildcardPatternNode& node) override {
            node.inferredType = expectedType;
        }
        
        void visit(LiteralPatternNode& node) override {
            node.inferredType = expectedType;
        }
        
        void visit(IdentifierPatternNode& node) override {
            if (node.symbolId != kInvalidSymbolID) {
                typeTable[node.symbolId] = expectedType;
                node.inferredType = expectedType;
            }
        }
        
        void visit(EnumPatternNode& node) override {
            if (node.variantSymbolId != kInvalidSymbolID) {
                const auto& sym = table.getSymbol(node.variantSymbolId);
                if (auto* variantFn = dynamic_cast<const FunctionType*>(typeTable[node.variantSymbolId])) {
                    if (node.fields.size() != variantFn->paramTypes.size()) {
                        diag.error(node.loc, "Variant '" + sym.name.str() + "' requires " + std::to_string(variantFn->paramTypes.size()) + " fields, but " + std::to_string(node.fields.size()) + " were provided");
                    } else {
                        std::unordered_map<SymbolID, const Type*> substitutionMap;
                        std::vector<const Type*> freshArgs;
                        const EnumType* fnRetEnum = dynamic_cast<const EnumType*>(variantFn->returnType);
                        if (fnRetEnum) {
                            const auto& enumSym = table.getSymbol(fnRetEnum->enumSymbolId);
                            if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                                auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                                for (const auto& param : enumDecl->genericParams) {
                                    const Type* freshVar = ctx.getInferenceVar(ctx.newVar());
                                    substitutionMap[param.symbolId] = freshVar;
                                    freshArgs.push_back(freshVar);
                                }
                            }
                        }
                        
                        if (fnRetEnum) {
                            const Type* specializedEnum = freshArgs.empty() ? fnRetEnum : ctx.getEnumType(fnRetEnum->enumSymbolId, std::move(freshArgs));
                            constraints.push_back(Constraint(ConstraintKind::Equality, specializedEnum, expectedType, "", node.loc));
                        }

                        for (size_t i = 0; i < node.fields.size(); ++i) {
                            const Type* fieldExpected = variantFn->paramTypes[i];
                            if (!substitutionMap.empty()) {
                                fieldExpected = ctx.substitute(fieldExpected, substitutionMap);
                            }
                            PatternConstraintVisitor fieldVisitor(table, diag, ctx, typeTable, constraints, fieldExpected);
                            node.fields[i]->accept(fieldVisitor);
                        }
                    }
                } else if (typeTable[node.variantSymbolId]->getKind() == TypeKind::Enum) {
                    if (!node.fields.empty()) {
                        diag.error(node.loc, "Variant '" + sym.name.str() + "' does not take any fields");
                    }
                    std::vector<const Type*> freshArgs;
                    const EnumType* variantEnum = dynamic_cast<const EnumType*>(typeTable[node.variantSymbolId]);
                    if (variantEnum) {
                        const auto& enumSym = table.getSymbol(variantEnum->enumSymbolId);
                        if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                            auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                            for (const auto& param : enumDecl->genericParams) {
                                freshArgs.push_back(ctx.getInferenceVar(ctx.newVar()));
                            }
                        }
                    }
                    const Type* specializedEnum = freshArgs.empty() ? typeTable[node.variantSymbolId] : ctx.getEnumType(variantEnum->enumSymbolId, std::move(freshArgs));
                    constraints.push_back(Constraint(ConstraintKind::Equality, specializedEnum, expectedType, "", node.loc));
                }
            }
            node.inferredType = expectedType;
        }
        
        void visit(TuplePatternNode& node) override {
            std::vector<const Type*> elementTypes;
            for (auto& elem : node.elements) {
                const Type* elemVar = ctx.getInferenceVar(ctx.newVar());
                elementTypes.push_back(elemVar);
                PatternConstraintVisitor elemVisitor(table, diag, ctx, typeTable, constraints, elemVar);
                elem->accept(elemVisitor);
            }
            const Type* tupleType = ctx.getTupleType(elementTypes);
            constraints.push_back(Constraint(ConstraintKind::Equality, tupleType, expectedType, "", node.loc));
            node.inferredType = tupleType;
        }
    };

    class ConstraintGenerator : public ASTVisitor, public TypeVisitor {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        std::vector<const Type*>& typeTable;
        std::vector<Constraint>& constraints;
        MethodResolver& methodResolver;
        std::unordered_map<const Type*, std::unordered_set<SymbolID>>& implementedTraits;
        const Type* currentReturnType = nullptr;
        MonomorphizationEngine* monoEngine;
        MLibMetadataCache* metadataCache; // nullable
        bool isUnsafeContext_ = false;
    bool isAsyncContext_ = false;

        const Type* evaluateTypeNode(TypeNode* node) {
            if (!node) return nullptr;
            TypePrePass pre(table, ctx, typeTable, methodResolver, implementedTraits, monoEngine, diag);
            return pre.evaluateTypeNode(node);
        }
    public:
        ConstraintGenerator(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, std::vector<const Type*>& typeTable, std::vector<Constraint>& constraints, MethodResolver& methodResolver, std::unordered_map<const Type*, std::unordered_set<SymbolID>>& implementedTraits, MonomorphizationEngine* monoEngine, MLibMetadataCache* metadataCache = nullptr)
              : table(table), diag(diag), ctx(ctx), typeTable(typeTable), constraints(constraints), methodResolver(methodResolver), implementedTraits(implementedTraits), monoEngine(monoEngine), metadataCache(metadataCache) {}

        void visit(ProgramNode& node) override { for (auto& item : node.items) item->accept(*this); }
        void visit(ModDeclNode& node) override { for (auto& d : node.decls) d->accept(*this); }
        void visit(ExternDeclNode& node) override { if (node.func) node.func->accept(*this); }
        
        void visit(BlockStmtNode& node) override {
            for (auto& s : node.body) s->accept(*this);
            if (node.tailExpr) {
                node.tailExpr->accept(*this);
                if (node.tailExpr->inferredType) {
                    node.inferredType = node.tailExpr->inferredType;
                }
            } else {
                node.inferredType = ctx.getVoid();
            }
        }
        
        void visit(VarDeclNode& node) override {
            const Type* annotType = nullptr;
            if (node.typeAnnot) {
                annotType = evaluateTypeNode(node.typeAnnot.get()); 
                if (annotType && !annotType->isSized()) {
                    diag.error(node.loc, "Variable must have a statically known size. Cannot use dynamically sized type directly.");
                    annotType = ctx.getUnknown();
                }
            }
            
            if (node.initializer) {
                node.initializer->accept(*this);
            }

            const Type* varType = annotType ? annotType : 
                (node.initializer && node.initializer->inferredType ? node.initializer->inferredType : ctx.getInferenceVar(ctx.newVar()));
            typeTable[node.symbolId] = varType;

            if (node.initializer && node.initializer->inferredType && varType != node.initializer->inferredType) {
                constraints.push_back({ConstraintKind::Equality, varType, node.initializer->inferredType, "", node.loc});
            }
        }
        void visit(ParamDeclNode& node) override {}
        void visit(StructInitExpr& node) override {
            const Type* structType = ctx.getUnknown();
            if (node.structId != kInvalidSymbolID) {
                const auto& sym = table.getSymbol(node.structId);
                if (sym.kind == SymbolKind::Struct && sym.decl) {
                    auto* structDecl = static_cast<const StructDeclNode*>(sym.decl);
                    if (!structDecl->genericParams.empty()) {
                        std::vector<const Type*> args;
                        for (size_t i = 0; i < structDecl->genericParams.size(); ++i) {
                            if (i < node.genericArgs.size()) {
                                TypePrePass pre(table, ctx, typeTable, methodResolver, implementedTraits, monoEngine, diag);
                                args.push_back(pre.evaluateTypeNode(node.genericArgs[i].get()));
                            } else {
                                args.push_back(ctx.getInferenceVar(ctx.newVar()));
                            }
                        }
                        bool allConcrete = true;
                        for (auto* a : args) {
                            if (a->getKind() == TypeKind::InferenceVar || dynamic_cast<const GenericParamType*>(a)) {
                                allConcrete = false; break;
                            }
                        }
                        if (allConcrete && monoEngine) {
                            SymbolID specId = monoEngine->requestStructSpecialization(structDecl, args, node.loc);
                            if (specId != kInvalidSymbolID) {
                                node.structId = specId;
                                args.clear();
                                node.genericArgs.clear();
                            }
                        }
                        structType = ctx.getStructType(node.structId, args);
                    } else {
                        structType = typeTable[node.structId];
                    }
                } else {
                    structType = typeTable[node.structId];
                }
            }
            node.inferredType = structType;
            for (auto& field : node.fields) {
                if (field.value) field.value->accept(*this);
            }
            
            if (auto* st = dynamic_cast<const StructType*>(structType)) {
                if (node.structId != kInvalidSymbolID) {
                    const auto& sym = table.getSymbol(node.structId);
                    if (sym.kind == SymbolKind::Struct && sym.decl) {
                        auto* structDecl = static_cast<const StructDeclNode*>(sym.decl);
                        std::unordered_map<SymbolID, const Type*> subst;
                        for (size_t i = 0; i < structDecl->genericParams.size(); ++i) {
                            if (i < st->genericArgs.size()) {
                                subst[structDecl->genericParams[i].symbolId] = st->genericArgs[i];
                            }
                        }
                        
                        for (auto& field : node.fields) {
                            if (!field.value) continue;
                            auto it = st->fieldIndices.find(std::string(field.name));
                            if (it != st->fieldIndices.end()) {
                                SymbolID fId = structDecl->fields[it->second]->symbolId;
                                const Type* fTy = typeTable[fId];
                                const Type* instTy = ctx.substitute(fTy, subst);
                                constraints.push_back(Constraint(ConstraintKind::Equality, instTy, field.value->inferredType, "field type mismatch", node.loc));
                            }
                        }
                    }
                }
            }
        }
        void visit(FunctionDeclNode& node) override {
            const Type* oldRet = currentReturnType;
            if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[node.symbolId])) {
                if (fnTy->returnType && !fnTy->returnType->isSized()) {
                    diag.error(node.loc, "Function return type must have a statically known size. Cannot return dynamically sized type.");
                }
                for (const Type* paramType : fnTy->paramTypes) {
                    if (paramType && !paramType->isSized()) {
                        diag.error(node.loc, "Function parameter must have a statically known size. Cannot pass dynamically sized type by value.");
                    }
                }
                currentReturnType = fnTy->returnType;
            }
            bool oldAsync = isAsyncContext_;
              isAsyncContext_ = node.isAsync;
              std::unique_ptr<UnsafeContextGuard> guard;
            if (node.isUnsafe) {
                guard = std::make_unique<UnsafeContextGuard>(isUnsafeContext_);
            }
            if (node.body) node.body->accept(*this);
            isAsyncContext_ = oldAsync;
              currentReturnType = oldRet;
        }
        
        void visit(UnsafeStmtNode& node) override {
            UnsafeContextGuard guard(isUnsafeContext_);
            if (node.body) node.body->accept(*this);
        }
        void visit(StructDeclNode& node) override {}
        void visit(StructFieldNode& node) override {}
        void visit(EnumDeclNode& node) override {}
        void visit(EnumVariantNode& node) override {}
        void visit(TraitDeclNode& node) override {}
        void visit(ImplDeclNode& node) override {
            if (!node.genericParams.empty()) return;
              for (auto& method : node.methods) method->accept(*this);
        }
        void visit(TypeAliasDeclNode& node) override {}
        void visit(UseDeclNode& node) override {}
        
        void visit(ExprStmtNode& node) override { node.expr->accept(*this); }
        void visit(IfStmtNode& node) override {
            node.condition->accept(*this);
            if (node.condition->inferredType) {
                constraints.push_back(Constraint(ConstraintKind::Equality, ctx.getPrimitive(BuiltinKind::Bool), node.condition->inferredType, "", node.loc));
            }
            node.thenBranch->accept(*this);
            if (node.elseBranch) node.elseBranch->accept(*this);
        }
        void visit(WhileStmtNode& node) override {
            node.condition->accept(*this);
            if (node.condition->inferredType) {
                constraints.push_back(Constraint(ConstraintKind::Equality, ctx.getPrimitive(BuiltinKind::Bool), node.condition->inferredType, "", node.loc));
            }
            node.body->accept(*this);
        }
        void visit(ForStmtNode& node) override {
            if (node.kind == ForKind::CStyle) {
                if (node.init) node.init->accept(*this);
                if (node.cond) {
                    node.cond->accept(*this);
                    if (node.cond->inferredType) {
                        constraints.push_back(Constraint(ConstraintKind::Equality, ctx.getPrimitive(BuiltinKind::Bool), node.cond->inferredType, "", node.loc));
                    }
                }
                if (node.step) node.step->accept(*this);
            } else {
                node.iterable->accept(*this);
                const Type* elemType = ctx.getInferenceVar(ctx.newVar());
                constraints.push_back(Constraint(ConstraintKind::Iterable, node.iterable->inferredType, elemType, "", node.loc));
                
                if (node.bindingId != kInvalidSymbolID) {
                    typeTable[node.bindingId] = elemType;
                }
            }
            if (node.body) node.body->accept(*this);
        }
        void visit(ReturnStmtNode& node) override {
            if (node.value) {
                node.value->accept(*this);
                const Type* expectedRet = currentReturnType;
                if (isAsyncContext_) {
                    if (auto* futTy = dynamic_cast<const FutureType*>(ctx.unificationTable.deepResolve(expectedRet, ctx))) {
                        expectedRet = futTy->innerType;
                    }
                }
                constraints.push_back(Constraint(ConstraintKind::Equality, expectedRet, node.value->inferredType, "Return value must match function return type", node.loc));
            }
        }
        void visit(BreakStmtNode& node) override {}
        void visit(ContinueStmtNode& node) override {}

        void visit(ComptimeStmtNode& node) override {}

        void visit(LiteralExpr& node) override {
            switch (node.kind) {
                case LiteralKind::Integer: node.inferredType = ctx.getPrimitive(BuiltinKind::I32); break;
                case LiteralKind::Float:   node.inferredType = ctx.getPrimitive(BuiltinKind::F32); break;
                case LiteralKind::Bool:    node.inferredType = ctx.getPrimitive(BuiltinKind::Bool); break;
                case LiteralKind::Str:     node.inferredType = ctx.getPrimitive(BuiltinKind::Str); break;
                case LiteralKind::Char:    node.inferredType = ctx.getPrimitive(BuiltinKind::Char); break;
                default:                   node.inferredType = ctx.getUnknown(); break;
            }
        }
        void visit(IdentifierExpr& node) override {
            if (node.resolvedSymbol != kInvalidSymbolID) {
                const Type* baseType = typeTable[node.resolvedSymbol];
                
                const EnumType* enumTy = dynamic_cast<const EnumType*>(baseType);
                if (!enumTy) {
                    if (auto* fnTy = dynamic_cast<const FunctionType*>(baseType)) {
                        enumTy = dynamic_cast<const EnumType*>(fnTy->returnType);
                    }
                }
                
                if (enumTy) {
                    const auto& sym = table.getSymbol(enumTy->enumSymbolId);
                    if (sym.kind == SymbolKind::Enum && sym.decl) {
                        auto* enumDecl = static_cast<EnumDeclNode*>(sym.decl);
                        if (!enumDecl->genericParams.empty()) {
                            std::unordered_map<SymbolID, const Type*> substitutionMap;
                            std::vector<const Type*> freshArgs;
                            for (size_t i = 0; i < enumDecl->genericParams.size(); ++i) {
                                const Type* argTy = nullptr;
                                if (i < node.genericArgs.size()) {
                                    TypePrePass pre(table, ctx, typeTable, methodResolver, implementedTraits, monoEngine, diag);
                                    argTy = pre.evaluateTypeNode(node.genericArgs[i].get());
                                } else {
                                    argTy = ctx.getInferenceVar(ctx.newVar());
                                }
                                substitutionMap[enumDecl->genericParams[i].symbolId] = argTy;
                                freshArgs.push_back(argTy);
                            }
                            node.inferredType = ctx.substitute(baseType, substitutionMap);
                            return;
                        }
                    }
                }
                
                node.inferredType = baseType;
            } else {
                node.inferredType = ctx.getUnknown();
            }
        }
        void visit(BinaryExpr& node) override {
            node.left->accept(*this);
            node.right->accept(*this);
            if (node.left->inferredType && node.right->inferredType) {
                constraints.push_back({ConstraintKind::Equality, node.left->inferredType, node.right->inferredType, "", node.loc});
            }
            if (node.op == BinaryOp::Eq || node.op == BinaryOp::Ne || 
                node.op == BinaryOp::Lt || node.op == BinaryOp::Le || 
                node.op == BinaryOp::Gt || node.op == BinaryOp::Ge ||
                node.op == BinaryOp::LogicAnd || node.op == BinaryOp::LogicOr) {
                node.inferredType = ctx.getPrimitive(BuiltinKind::Bool);
            } else {
                node.inferredType = node.left->inferredType ? node.left->inferredType : ctx.getUnknown();
            }
        }
        void visit(UnaryExpr& node) override {
            node.operand->accept(*this);
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
            if (node.op == UnaryOp::Ref || node.op == UnaryOp::RefMut) {
                bool isMut = (node.op == UnaryOp::RefMut);
                const Type* expected = ctx.getReferenceType(node.operand->inferredType, isMut);
                constraints.push_back(Constraint(ConstraintKind::Equality, node.inferredType, expected, "", node.loc));
            } else if (node.op == UnaryOp::Deref) {
                constraints.push_back(Constraint(ConstraintKind::Deref, node.operand->inferredType, node.inferredType, "", node.loc));
            } else if (node.op == UnaryOp::Not) {
                constraints.push_back(Constraint(ConstraintKind::Equality, ctx.getPrimitive(BuiltinKind::Bool), node.operand->inferredType, "", node.loc));
                constraints.push_back(Constraint(ConstraintKind::Equality, ctx.getPrimitive(BuiltinKind::Bool), node.inferredType, "", node.loc));
            } else if (node.op == UnaryOp::Neg || node.op == UnaryOp::BitNot) {
                // Should be numeric, for now just unify with operand
                constraints.push_back(Constraint(ConstraintKind::Equality, node.inferredType, node.operand->inferredType, "", node.loc));
            }
        }
        void visit(AssignExpr& node) override {
            node.lvalue->accept(*this);
            node.value->accept(*this);
            if (node.lvalue->inferredType && node.value->inferredType) {
                constraints.push_back({ConstraintKind::Equality, node.lvalue->inferredType, node.value->inferredType, "", node.loc});
            }
            node.inferredType = node.lvalue->inferredType ? node.lvalue->inferredType : ctx.getUnknown();
        }
        void visit(CallExpr& node) override {
            node.callee->accept(*this);
            std::vector<std::string> argNames;
            std::vector<const Type*> argTypes;
            for (auto& arg : node.args) {
                argNames.push_back(std::string(arg.label));
                if (arg.value) {
                    arg.value->accept(*this);
                    argTypes.push_back(arg.value->inferredType);
                } else {
                    argTypes.push_back(ctx.getUnknown());
                }
            }
            node.inferredType = ctx.getInferenceVar(ctx.newVar());

            if (auto* ident = dynamic_cast<IdentifierExpr*>(node.callee.get())) {
                if (ident->resolvedSymbol != kInvalidSymbolID) {
                    const auto& sym = table.getSymbol(ident->resolvedSymbol);

                    // ── External Symbol path (loaded from .mlib) ──────────────
                    if (sym.isExternal && metadataCache) {
                        const Type* extType = metadataCache->getType(ident->resolvedSymbol);
                        if (extType && extType->getKind() != TypeKind::Unknown) {
                            // The cache has the full FunctionType: use it directly.
                            ident->inferredType = extType;
                            node.callee->inferredType = extType;
                            // Constrain the inferred return type
                            if (auto* ft = dynamic_cast<const FunctionType*>(extType)) {
                                constraints.push_back(Constraint(ConstraintKind::Equality,
                                    node.inferredType, ft->returnType, "", node.loc));
                            }
                            return;
                        }
                    }

                    // ── Local AST symbol path (original logic) ────────────────
                    if (sym.kind == SymbolKind::Function && sym.decl) {
                        auto* fnDecl = static_cast<FunctionDeclNode*>(sym.decl);
                        if (!fnDecl->genericParams.empty()) {
                            if (!node.genericArgs.empty()) {
                                for (auto& argNode : node.genericArgs) {
                                    node.inferredGenericArgs.push_back(evaluateTypeNode(argNode.get()));
                                }
                            } else {
                                for (size_t i = 0; i < fnDecl->genericParams.size(); ++i) {
                                    node.inferredGenericArgs.push_back(ctx.getInferenceVar(ctx.newVar()));
                                }
                            }
                            
                            std::unordered_map<SymbolID, const Type*> substitutionMap;
                            for (size_t i = 0; i < fnDecl->genericParams.size() && i < node.inferredGenericArgs.size(); ++i) {
                                substitutionMap[fnDecl->genericParams[i].symbolId] = node.inferredGenericArgs[i];
                            }
                            node.callee->inferredType = ctx.substitute(node.callee->inferredType, substitutionMap);
                        }
                    }
                }
            }

            const Type* expectedFnType = ctx.getFunctionType(std::move(argNames), std::move(argTypes), node.inferredType, true /* isCallSite */);
            constraints.push_back(Constraint(ConstraintKind::Equality, expectedFnType, node.callee->inferredType, "", node.loc));
        }
        
        void visit(MethodCallExpr& node) override {
            node.object->accept(*this);
            std::vector<std::string> argNames;
            std::vector<const Type*> argTypes;
            for (auto& arg : node.args) {
                argNames.push_back(std::string(arg.label));
                if (arg.value) {
                    arg.value->accept(*this);
                    argTypes.push_back(arg.value->inferredType);
                } else {
                    argTypes.push_back(ctx.getUnknown());
                }
            }
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
            // Note: method call arguments don't include 'self' in the AST arg list, so this is fine.
            constraints.push_back(Constraint(ConstraintKind::MethodCall, node.object->inferredType, node.inferredType, std::string(node.methodName), argTypes, argNames, node.loc));
        }
        void visit(IndexExpr& node) override {
            node.base->accept(*this);
            node.index->accept(*this);
            if (node.index->inferredType) {
                constraints.push_back(Constraint(ConstraintKind::Equality, ctx.getPrimitive(BuiltinKind::I32), node.index->inferredType, "", node.loc));
            }
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
            constraints.push_back(Constraint(ConstraintKind::Iterable, node.base->inferredType, node.inferredType, "", node.loc));
        }
        void visit(MemberExpr& node) override {
            node.object->accept(*this);
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
            constraints.push_back(Constraint(ConstraintKind::Field, node.object->inferredType, node.inferredType, std::string(node.member), node.loc));
        }
        void visit(TupleIndexExpr& node) override {
            node.object->accept(*this);
            const Type* objTy = ctx.unificationTable.deepResolve(node.object->inferredType, ctx);
            if (auto* tup = dynamic_cast<const TupleType*>(objTy)) {
                if (node.index < tup->elements.size()) {
                    node.inferredType = tup->elements[node.index];
                    return;
                }
            }
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
        }
        void visit(CastExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            const Type* targetTy = evaluateTypeNode(node.targetType.get());
            node.inferredType = targetTy;
        }
        void visit(UnsizeCastExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            node.inferredType = node.targetTypePtr;
        }
        void visit(ArrayLiteralExpr& node) override {
            for (auto& el : node.elements) {
                el->accept(*this);
            }
            if (node.elements.empty()) {
                node.inferredType = ctx.getArrayType(ctx.getUnknown(), 0);
            } else {
                node.inferredType = ctx.getArrayType(node.elements[0]->inferredType, node.elements.size());
                for (size_t i = 1; i < node.elements.size(); ++i) {
                    constraints.push_back(Constraint(ConstraintKind::Equality, node.elements[0]->inferredType, node.elements[i]->inferredType, "", node.loc));
                }
            }
        }
        void visit(TupleLiteralExpr& node) override {
            std::vector<const Type*> elementTypes;
            for (auto& elem : node.elements) {
                elem->accept(*this);
                elementTypes.push_back(elem->inferredType);
            }
            node.inferredType = ctx.getTupleType(elementTypes);
        }
        void visit(MatchExpr& node) override {
            node.subject->accept(*this);
            node.inferredType = ctx.getInferenceVar(ctx.newVar()); // T_ret
            for (auto& arm : node.arms) {
                if (arm.pattern) {
                    PatternConstraintVisitor patVisitor(table, diag, ctx, typeTable, constraints, node.subject->inferredType);
                    arm.pattern->accept(patVisitor);
                }
                if (arm.body) {
                    arm.body->accept(*this);
                    if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(arm.body.get())) {
                        constraints.push_back(Constraint(ConstraintKind::Equality, node.inferredType, exprStmt->expr->inferredType, "", node.loc));
                    }
                    // TODO: When BlockStmtNode supports tail expressions, unify arm.body's type with node.inferredType
                }
            }
        }
        void visit(LambdaExpr& node) override {
            std::vector<const Type*> paramTypes;
            std::vector<std::string> paramNames;
            for (auto& p : node.params) {
                if (p->type) {
                    p->type->accept(*this);
                }
                printf("DEBUG: visit(LambdaExpr) param=%s has_type=%d\n", std::string(p->name).c_str(), p->type != nullptr);
                const Type* pType = typeTable[p->symbolId];
                printf("DEBUG: param=%s pType_is_null=%d kind=%d\n", std::string(p->name).c_str(), pType == nullptr, pType ? (int)pType->getKind() : -1);
                if (!pType || pType->getKind() == TypeKind::Unknown) {
                    if (p->type) {
                        pType = evaluateTypeNode(p->type.get());
                        printf("DEBUG: evaluateTypeNode returned kind=%d for param %s\n", (int)pType->getKind(), std::string(p->name).c_str());
                    }
                    else pType = ctx.getInferenceVar(ctx.newVar());
                    typeTable[p->symbolId] = pType;
                }
                paramTypes.push_back(pType);
                paramNames.push_back(std::string(p->name));
            }
            
            const Type* retType = nullptr;
            if (node.returnType) {
                retType = evaluateTypeNode(node.returnType.get());
            } else {
                retType = ctx.getInferenceVar(ctx.newVar());
            }
            
            const Type* oldReturnType = currentReturnType;
            currentReturnType = retType;
            if (node.body) {
                node.body->accept(*this);
                // Simplified return type checking for MVP
                if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(node.body.get())) {
                    constraints.push_back(Constraint(ConstraintKind::Equality, retType, exprStmt->expr->inferredType, "Lambda body expression return type", node.loc));
                }
            }
            currentReturnType = oldReturnType;
            
            auto* sig = ctx.create<FunctionType>(paramNames, paramTypes, retType);
            
            node.generatedStructId = table.declareSymbol(Identifier("__Closure_" + std::to_string(reinterpret_cast<uintptr_t>(&node))), SymbolKind::Struct, table.globalScopeId(), node.loc, nullptr);
            node.generatedFuncId = table.declareSymbol(Identifier("__LambdaFunc_" + std::to_string(reinterpret_cast<uintptr_t>(&node))), SymbolKind::Function, table.globalScopeId(), node.loc, nullptr);
            
            auto* closureTy = const_cast<ClosureType*>(ctx.create<ClosureType>(node.generatedStructId, node.generatedFuncId, sig, node.capturedSymbols));
            closureTy->fieldTypes.push_back(ctx.getPointerType(sig, false)); // First field is func ptr
            for (auto capId : node.capturedSymbols) {
                const Type* capTy = typeTable[capId];
                if (!capTy) capTy = ctx.getUnknown();
                closureTy->fieldTypes.push_back(capTy);
            }
            
            node.inferredType = closureTy;
        }
        void visit(AwaitExpr& node) override {
              if (node.expr) node.expr->accept(*this);
              
              if (!isAsyncContext_) {
                  diag.error(node.loc, "`await` is only allowed inside `async` functions.");
              }
              
              const Type* innerT = ctx.getInferenceVar(ctx.newVar());
              const Type* futT = ctx.create<FutureType>(innerT);
              if (node.expr) {
                  constraints.push_back(Constraint(ConstraintKind::Equality, node.expr->inferredType, futT, "await expression must be a Future", node.loc));
              }
              node.inferredType = innerT;
          }
        void visit(SizeofExpr& node) override {
            node.evaluatedTargetType = evaluateTypeNode(node.targetType.get());
            node.inferredType = ctx.getPrimitive(BuiltinKind::U64);
        }
        void visit(AlignofExpr& node) override {
            node.evaluatedTargetType = evaluateTypeNode(node.targetType.get());
            node.inferredType = ctx.getPrimitive(BuiltinKind::U64);
        }

        void visit(BuiltinTypeNode& node) override {}
        void visit(NamedTypeNode& node) override {}
        void visit(PointerTypeNode& node) override {}
        void visit(ReferenceTypeNode& node) override {}
        void visit(ArrayTypeNode& node) override {}
        void visit(TupleTypeNode& node) override {}
        void visit(FunctionTypeNode& node) override {}
        void visit(NeverTypeNode& node) override {}
        void visit(TraitObjectTypeNode& node) override {}
    };


    class UnificationEngine {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        std::vector<const Type*>& typeTable;
        MethodResolver& methodResolver;
    public:
        UnificationEngine(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, std::vector<const Type*>& typeTable, MethodResolver& methodResolver) 
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), methodResolver(methodResolver) {}

        bool unify(const Type* t1, const Type* t2, SourceLocation loc) {
            t1 = ctx.unificationTable.deepResolve(t1, ctx);
            t2 = ctx.unificationTable.deepResolve(t2, ctx);
            if (!t1 || !t2) return false;
            if (t1->getKind() == TypeKind::Error || t2->getKind() == TypeKind::Error) return true;
            if (t1 == t2 || t1->equals(t2)) return true;
            
            auto* inf1 = dynamic_cast<const InferenceVarType*>(t1);
            auto* inf2 = dynamic_cast<const InferenceVarType*>(t2);

            if (inf1) {
                ctx.unificationTable.unify(inf1->varId, t2);
                return true;
            }
            if (inf2) {
                ctx.unificationTable.unify(inf2->varId, t1);
                return true;
            }

            if (auto* fut1 = dynamic_cast<const FutureType*>(t1)) {
                if (auto* fut2 = dynamic_cast<const FutureType*>(t2)) {
                    return unify(fut1->innerType, fut2->innerType, loc);
                }
            }

            if (auto* clos1 = dynamic_cast<const ClosureType*>(t1)) {
                if (auto* fn2 = dynamic_cast<const FunctionType*>(t2)) {
                    if (fn2->isCallSite) return unify(clos1->signature, t2, loc);
                }
            }
            if (auto* fn1 = dynamic_cast<const FunctionType*>(t1)) {
                if (auto* clos2 = dynamic_cast<const ClosureType*>(t2)) {
                    if (fn1->isCallSite) return unify(t1, clos2->signature, loc);
                }
                if (auto* fn2 = dynamic_cast<const FunctionType*>(t2)) {
                    
                    const FunctionType* callSite = nullptr;
                    const FunctionType* defSite = nullptr;
                    
                    if (fn1->isCallSite) { callSite = fn1; defSite = fn2; }
                    else if (fn2->isCallSite) { callSite = fn2; defSite = fn1; }
                    
                    if (callSite && defSite) {
                        bool hasNames = false;
                        for (const auto& n : callSite->paramNames) if (!n.empty()) hasNames = true;
                        
                        if (defSite->isVariadic) {
                            if (callSite->paramTypes.size() < defSite->paramTypes.size()) goto mismatch;
                        } else {
                            if (callSite->paramTypes.size() != defSite->paramTypes.size()) goto mismatch;
                        }

                        if (hasNames) {
                            bool seenNamedArg = false;
                            std::vector<bool> provided(defSite->paramTypes.size(), false);
                            for (size_t i = 0; i < callSite->paramNames.size(); ++i) {
                                if (callSite->paramNames[i].empty()) { // Positional
                                    if (seenNamedArg) {
                                        diag.error(loc, "Positional argument cannot follow named arguments");
                                        return false;
                                    }
                                    if (i < provided.size() && provided[i]) {
                                        diag.error(loc, "Parameter provided multiple times");
                                        return false;
                                    }
                                    if (i < provided.size()) provided[i] = true;
                                    
                                    const Type* defType = (defSite->isVariadic && i >= defSite->paramTypes.size()) ? callSite->paramTypes[i] : defSite->paramTypes[i];
                                    if (!unify(callSite->paramTypes[i], defType, loc)) return false;
                                } else { // Named
                                    seenNamedArg = true;
                                    bool found = false;
                                    for (size_t j = 0; j < defSite->paramNames.size(); ++j) {
                                        if (defSite->paramNames[j] == callSite->paramNames[i]) {
                                            if (provided[j]) {
                                                diag.error(loc, "Parameter '" + callSite->paramNames[i] + "' provided multiple times");
                                                return false;
                                            }
                                            provided[j] = true;
                                            if (!unify(callSite->paramTypes[i], defSite->paramTypes[j], loc)) {
                                                return false;
                                            }
                                            found = true;
                                            break;
                                        }
                                    }
                                    if (!found) {
                                        diag.error(loc, "Named argument '" + callSite->paramNames[i] + "' not found in function signature");
                                        return false;
                                    }
                                }
                            }
                        } else {
                            for (size_t i = 0; i < callSite->paramTypes.size(); ++i) {
                                const Type* defType = (defSite->isVariadic && i >= defSite->paramTypes.size()) ? callSite->paramTypes[i] : defSite->paramTypes[i];
                                if (!unify(callSite->paramTypes[i], defType, loc)) return false;
                            }
                        }
                    } else {
                        if (fn1->isVariadic != fn2->isVariadic || fn1->paramTypes.size() != fn2->paramTypes.size()) goto mismatch;
                        for (size_t i = 0; i < fn1->paramTypes.size(); ++i) {
                            if (!unify(fn1->paramTypes[i], fn2->paramTypes[i], loc)) return false;
                        }
                    }
                    return unify(fn1->returnType, fn2->returnType, loc);
                }
            }
            if (auto* prim1 = dynamic_cast<const PrimitiveType*>(t1)) {
                if (prim1->builtinKind == BuiltinKind::Str) {
                    if (auto* ptr2 = dynamic_cast<const PointerType*>(t2)) {
                        if (auto* prim2 = dynamic_cast<const PrimitiveType*>(ptr2->pointee)) {
                            if (prim2->builtinKind == BuiltinKind::I8) return true;
                        }
                    }
                }
            }
            if (auto* ptr1 = dynamic_cast<const PointerType*>(t1)) {
                if (auto* prim2 = dynamic_cast<const PrimitiveType*>(t2)) {
                    if (prim2->builtinKind == BuiltinKind::Str) {
                        if (auto* prim1 = dynamic_cast<const PrimitiveType*>(ptr1->pointee)) {
                            if (prim1->builtinKind == BuiltinKind::I8) return true;
                        }
                    }
                }
                if (auto* ptr2 = dynamic_cast<const PointerType*>(t2)) {
                    if (ptr1->isMutable != ptr2->isMutable) goto mismatch;
                    return unify(ptr1->pointee, ptr2->pointee, loc);
                }
            }

            if (auto* ref1 = dynamic_cast<const ReferenceType*>(t1)) {
                if (auto* ref2 = dynamic_cast<const ReferenceType*>(t2)) {
                    if (ref1->isMutable != ref2->isMutable) goto mismatch;
                    return unify(ref1->pointee, ref2->pointee, loc);
                }
            }
            if (auto* st1 = dynamic_cast<const StructType*>(t1)) {
                if (auto* st2 = dynamic_cast<const StructType*>(t2)) {
                    if (st1->structSymbolId != st2->structSymbolId) goto mismatch;
                    if (st1->genericArgs.size() != st2->genericArgs.size()) goto mismatch;
                    for (size_t i = 0; i < st1->genericArgs.size(); ++i) {
                        if (!unify(st1->genericArgs[i], st2->genericArgs[i], loc)) return false;
                    }
                    return true;
                }
            } else if (auto* e1 = dynamic_cast<const EnumType*>(t1)) {
                if (auto* e2 = dynamic_cast<const EnumType*>(t2)) {
                    if (e1->enumSymbolId != e2->enumSymbolId) goto mismatch;
                    if (e1->genericArgs.size() != e2->genericArgs.size()) goto mismatch;
                    for (size_t i = 0; i < e1->genericArgs.size(); ++i) {
                        if (!unify(e1->genericArgs[i], e2->genericArgs[i], loc)) return false;
                    }
                    return true;
                }
            }

        mismatch:
            diag.error(loc, "Type mismatch: expected '" + t1->toString() + "', found '" + t2->toString() + "'");
            return false;
        }

        void solve(std::vector<Constraint> constraints) {
            bool changed = true;
            int iterations = 0;
            while (changed && iterations++ < 10) {
                changed = false;
                for (const auto& c : constraints) {
                    if (c.kind == ConstraintKind::Equality) {
                        printf("Solving constraint: %s == %s\n", c.expected->toString().c_str(), c.actual->toString().c_str());
                        if (unify(c.expected, c.actual, c.loc)) changed = true;
                    } else if (c.kind == ConstraintKind::Field) {
                        const Type* objType = ctx.unificationTable.deepResolve(c.expected, ctx);
                        if (auto* ptr = dynamic_cast<const PointerType*>(objType)) objType = ptr->pointee;
                        if (auto* ref = dynamic_cast<const ReferenceType*>(objType)) objType = ref->pointee;
                        if (auto* st = dynamic_cast<const StructType*>(objType)) {
                            const auto& sym = table.getSymbol(st->structSymbolId);
                            if (sym.kind == SymbolKind::Struct && sym.decl) {
                                auto* structDecl = static_cast<StructDeclNode*>(sym.decl);
                                for (auto& field : structDecl->fields) {
                                    if (field->name == c.name) {
                                        if (field->symbolId != kInvalidSymbolID) {
                                            const Type* fTy = typeTable[field->symbolId];
                                            std::unordered_map<SymbolID, const Type*> subst;
                                            for (size_t j = 0; j < structDecl->genericParams.size(); ++j) {
                                                if (j < st->genericArgs.size()) {
                                                    subst[structDecl->genericParams[j].symbolId] = st->genericArgs[j];
                                                }
                                            }
                                            fTy = ctx.substitute(fTy, subst);
                                            if (unify(c.actual, fTy, c.loc)) changed = true;
                                        }
                                    }
                                }
                            }
                        }
                    } else if (c.kind == ConstraintKind::MethodCall) {
                        const Type* objType = ctx.unificationTable.deepResolve(c.expected, ctx);
                        if (objType->getKind() == TypeKind::InferenceVar) {
                            // Wait for the object type to be inferred
                        } else {
                            MethodInfo mInfo;
                            if (methodResolver.probe(objType, c.name, mInfo)) {
                                // 1. Construct the synthetic call site function type.
                                // The receiver is inherently positional and prepended to the user's call args.
                                std::vector<std::string> callArgNames;
                                callArgNames.push_back(""); // self has no label at call site (it's the receiver)
                                for (auto& name : c.callArgNames) callArgNames.push_back(name);
                                
                                std::vector<const Type*> callArgTypes;
                                callArgTypes.push_back(objType); // Receiver type (to be checked against param 0)
                                for (auto& argTy : c.callArgs) callArgTypes.push_back(argTy);
                                
                                const Type* expectedFnType = ctx.getFunctionType(std::move(callArgNames), std::move(callArgTypes), c.actual, true /* isCallSite */);
                                
                                // 2. Unify the synthetic call site with the method's definition signature
                                if (unify(expectedFnType, mInfo.type, c.loc)) {
                                    changed = true;
                                }
                            } else {
                                diag.error(c.loc, "No method named '" + c.name + "' found for type '" + objType->toString() + "'");
                                changed = true; // Error reported, don't loop forever
                            }
                        }
                    } else if (c.kind == ConstraintKind::Iterable) {
                        const Type* t1 = ctx.unificationTable.deepResolve(c.expected, ctx); // The iterable
                        const Type* t2 = ctx.unificationTable.deepResolve(c.actual, ctx);   // The element type (inference var)
                        
                        if (t1->getKind() == TypeKind::InferenceVar) {
                            // Wait for iterable to be inferred
                        } else if (auto* arr = dynamic_cast<const ArrayType*>(t1)) {
                            if (unify(arr->elementType, t2, c.loc)) changed = true;
                        } else if (auto* sl = dynamic_cast<const SliceType*>(t1)) {
                            if (unify(sl->elementType, t2, c.loc)) changed = true;
                        } else if (t1->getKind() != TypeKind::Unknown) {
                            diag.error(c.loc, "Type '" + t1->toString() + "' is not iterable");
                        }
                    } else if (c.kind == ConstraintKind::EnumVariantPattern) {
                        // Dead code, now handled directly in PatternConstraintVisitor
                        changed = true;
                    } else if (c.kind == ConstraintKind::Deref) {
                        const Type* ptrType = ctx.unificationTable.deepResolve(c.expected, ctx);
                        const Type* valType = ctx.unificationTable.deepResolve(c.actual, ctx);
                        if (ptrType->getKind() == TypeKind::InferenceVar) {
                            // Wait
                        } else if (auto* ptr = dynamic_cast<const PointerType*>(ptrType)) {
                            if (unify(ptr->pointee, valType, c.loc)) changed = true;
                        } else if (auto* ref = dynamic_cast<const ReferenceType*>(ptrType)) {
                            if (unify(ref->pointee, valType, c.loc)) changed = true;
                        } else if (ptrType->getKind() != TypeKind::Unknown) {
                            diag.error(c.loc, "Type '" + ptrType->toString() + "' cannot be dereferenced");
                            changed = true;
                        }
                    }
                }
            }
        }
    };
      class TypeResolver : public ASTVisitor {
          SymbolTable& table;
          DiagnosticEngine& diag;
          TypeContext& ctx;
          std::vector<const Type*>& typeTable;
          MonomorphizationEngine* monoEngine;
          MethodResolver& methodResolver;
          const Type* currentReturnType = nullptr;
          bool isUnsafeContext_ = false;
          bool isAsyncContext_ = false;
      public:
          TypeResolver(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, std::vector<const Type*>& typeTable, MonomorphizationEngine* monoEngine, MethodResolver& methodResolver) 
              : table(table), diag(diag), ctx(ctx), typeTable(typeTable), monoEngine(monoEngine), methodResolver(methodResolver) {}
              
          class PatternResolverVisitor : public PatternVisitor {
              TypeResolver& resolver;
          public:
              PatternResolverVisitor(TypeResolver& r) : resolver(r) {}
              void visit(WildcardPatternNode& node) override { resolver.resolve(node.inferredType, node.loc); }
              void visit(LiteralPatternNode& node) override { resolver.resolve(node.inferredType, node.loc); }
              void visit(IdentifierPatternNode& node) override { resolver.resolve(node.inferredType, node.loc); }
              void visit(TuplePatternNode& node) override {
                  resolver.resolve(node.inferredType, node.loc);
                  for (auto& e : node.elements) if (e) e->accept(*this);
              }
              void visit(EnumPatternNode& node) override {
                  resolver.resolve(node.inferredType, node.loc);
                  for (auto& f : node.fields) if (f) f->accept(*this);
              }
          };

        void resolve(const Type*& t, SourceLocation loc) {
            if (!t) return;
            t = ctx.unificationTable.deepResolve(t, ctx);
            if (t->getKind() == TypeKind::InferenceVar) {
                diag.error(loc, "Type annotation needed");
            }
        }
        
        void coerce(std::unique_ptr<ExprNode>& exprPtr, const Type* expected) {
            if (!exprPtr || !expected) return;
            const Type* actual = exprPtr->inferredType;
            if (!actual) return;
            if (auto* refExpected = dynamic_cast<const ReferenceType*>(expected)) {
                if (dynamic_cast<const TraitObjectType*>(refExpected->pointee)) {
                    if (auto* refActual = dynamic_cast<const ReferenceType*>(actual)) {
                        if (!dynamic_cast<const TraitObjectType*>(refActual->pointee)) {
                            auto unsize = std::make_unique<UnsizeCastExpr>();
                            unsize->expr = std::move(exprPtr);
                            unsize->targetTypePtr = expected;
                            unsize->inferredType = expected;
                            exprPtr = std::move(unsize);
                        }
                    }
                }
            }
        }

        void coerceArgs(std::vector<CallArgNode>& args, const FunctionType* fnTy) {
            if (!fnTy) return;
            for (size_t i = 0; i < args.size(); ++i) {
                if (!args[i].value) continue;
                if (args[i].label.empty()) {
                    if (i < fnTy->paramTypes.size()) {
                        coerce(args[i].value, fnTy->paramTypes[i]);
                    } else if (fnTy->isVariadic && !fnTy->paramTypes.empty()) {
                        coerce(args[i].value, fnTy->paramTypes.back());
                    }
                } else {
                    for (size_t j = 0; j < fnTy->paramNames.size(); ++j) {
                        if (fnTy->paramNames[j] == args[i].label) {
                            coerce(args[i].value, fnTy->paramTypes[j]);
                            break;
                        }
                    }
                }
            }
        }
        
        // Helper to deeply resolve generic arguments and request specialization
        void resolveGenericsAndSpecialize(CallExpr& node) {
            if (node.inferredGenericArgs.empty()) return;
            
            auto* ident = dynamic_cast<IdentifierExpr*>(node.callee.get());
            if (!ident || ident->resolvedSymbol == kInvalidSymbolID) return;
            
            std::vector<const Type*> concreteArgs;
            for (auto* t : node.inferredGenericArgs) {
                const Type* resolved = ctx.unificationTable.deepResolve(t, ctx);
                if (resolved->getKind() == TypeKind::InferenceVar) {
                    diag.error(node.loc, "Cannot infer type for generic parameter");
                    return;
                }
                concreteArgs.push_back(resolved);
            }
            node.inferredGenericArgs = concreteArgs;
            
            if (monoEngine) {
                const auto& sym = table.getSymbol(ident->resolvedSymbol);
                if (sym.kind == SymbolKind::Function && sym.decl) {
                    auto* fnDecl = static_cast<FunctionDeclNode*>(sym.decl);
                    try {
                        SymbolID specializedId = monoEngine->requestSpecialization(fnDecl, concreteArgs, ident->loc);
                        if (specializedId != kInvalidSymbolID) {
                            ident->resolvedSymbol = specializedId; // Update Call site!
                            const auto& specSym = table.getSymbol(specializedId);
                            ident->segments.back() = specSym.name.str();
                        }
                        node.resolvedFn = specializedId;
                    } catch (const std::exception& e) {
                        diag.error(node.loc, e.what());
                    }
                }
            }
        }

        void visit(ProgramNode& node) override { for (auto& item : node.items) item->accept(*this); }
        void visit(ModDeclNode& node) override { for (auto& d : node.decls) d->accept(*this); }
        void visit(ExternDeclNode& node) override { if (node.func) node.func->accept(*this); }
        void visit(VarDeclNode& node) override {
            if (node.initializer) {
                node.initializer->accept(*this);
                const Type* varTy = typeTable[node.symbolId]; 
                varTy = ctx.unificationTable.deepResolve(varTy, ctx);
                coerce(node.initializer, varTy);
            }
        }
        void visit(ParamDeclNode& node) override {}
        void visit(FunctionDeclNode& node) override { 
            if (node.body) {
                const Type* oldRet = currentReturnType;
                if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[node.symbolId])) {
                    currentReturnType = ctx.unificationTable.deepResolve(fnTy->returnType, ctx);
                }
                bool oldAsync = isAsyncContext_;
                  isAsyncContext_ = node.isAsync;
                  std::unique_ptr<UnsafeContextGuard> guard;
                if (node.isUnsafe) {
                    guard = std::make_unique<UnsafeContextGuard>(isUnsafeContext_);
                }
                node.body->accept(*this);
                isAsyncContext_ = oldAsync;
                  currentReturnType = oldRet;
            }
        }
        
        void visit(UnsafeStmtNode& node) override {
            UnsafeContextGuard guard(isUnsafeContext_);
            if (node.body) node.body->accept(*this);
        }
        void visit(StructDeclNode& node) override {}
        void visit(StructFieldNode& node) override {}
        void visit(EnumDeclNode& node) override {}
        void visit(EnumVariantNode& node) override {}
        void visit(TraitDeclNode& node) override {}
        void visit(ImplDeclNode& node) override {
            if (!node.genericParams.empty()) return;
              for (auto& method : node.methods) method->accept(*this);
        }
        void visit(TypeAliasDeclNode& node) override {}
        void visit(UseDeclNode& node) override {}

        void visit(BlockStmtNode& node) override { 
            for (auto& s : node.body) s->accept(*this); 
            if (node.tailExpr) {
                node.tailExpr->accept(*this);
            }
            resolve(node.inferredType, node.loc);
        }
        
        void visit(ExprStmtNode& node) override { node.expr->accept(*this); }
        void visit(IfStmtNode& node) override {
            node.condition->accept(*this);
            node.thenBranch->accept(*this);
            if (node.elseBranch) node.elseBranch->accept(*this);
        }
        void visit(WhileStmtNode& node) override {
            node.condition->accept(*this);
            node.body->accept(*this);
        }
        void visit(ForStmtNode& node) override {
            if (node.init) node.init->accept(*this);
            if (node.cond) node.cond->accept(*this);
            if (node.step) node.step->accept(*this);
            if (node.iterable) node.iterable->accept(*this);
            if (node.body) node.body->accept(*this);
        }
        void visit(ReturnStmtNode& node) override { 
            if (node.value) {
                node.value->accept(*this);
                if (currentReturnType) {
                    const Type* expectedRet = currentReturnType;
                    if (isAsyncContext_) {
                        if (auto* futTy = dynamic_cast<const FutureType*>(ctx.unificationTable.deepResolve(expectedRet, ctx))) {
                            expectedRet = futTy->innerType;
                        }
                    }
                    coerce(node.value, expectedRet);
                }
            }
        }
        void visit(BreakStmtNode& node) override {}
        void visit(ContinueStmtNode& node) override {}

        void visit(ComptimeStmtNode& node) override {}

        void visit(LiteralExpr& node) override { resolve(node.inferredType, node.loc); }
        void visit(IdentifierExpr& node) override { resolve(node.inferredType, node.loc); }
        void visit(BinaryExpr& node) override {
            node.left->accept(*this);
            node.right->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(UnaryExpr& node) override {
            node.operand->accept(*this);
            
            if (node.op == UnaryOp::Deref) {
                if (auto* ptrTy = dynamic_cast<const PointerType*>(node.operand->inferredType)) {
                    if (!isUnsafeContext_) {
                        diag.error(node.loc, "Dereferencing a raw pointer requires an unsafe block.");
                    }
                }
            }
            
            resolve(node.inferredType, node.loc);
        }
        void visit(AssignExpr& node) override {
            node.lvalue->accept(*this);
            node.value->accept(*this);
            coerce(node.value, node.lvalue->inferredType);
            resolve(node.inferredType, node.loc);
        }
        void visit(CallExpr& node) override {
            node.callee->accept(*this);
            
            if (auto* idExpr = dynamic_cast<IdentifierExpr*>(node.callee.get())) {
                if (idExpr->resolvedSymbol != kInvalidSymbolID) {
                    const auto& sym = table.getSymbol(idExpr->resolvedSymbol);
                    if (sym.decl) {
                        if (auto* fnDecl = dynamic_cast<const FunctionDeclNode*>(sym.decl)) {
                            if (fnDecl->isUnsafe && !isUnsafeContext_) {
                                diag.error(node.loc, "Call to unsafe function requires an unsafe block.");
                            }
                        }
                    }
                }
            }
            
            for (auto& arg : node.args) {
                if (arg.value) arg.value->accept(*this);
            }
            resolve(node.inferredType, node.loc);
            resolveGenericsAndSpecialize(node);
            
            if (auto* fnTy = dynamic_cast<const FunctionType*>(node.callee->inferredType)) {
                coerceArgs(node.args, fnTy);
            } else if (auto* closTy = dynamic_cast<const ClosureType*>(node.callee->inferredType)) {
                coerceArgs(node.args, closTy->signature);
                node.isClosureCall = true;
            }
        }
        void visit(MethodCallExpr& node) override {
            node.object->accept(*this);
            for (auto& arg : node.args) {
                if (arg.value) arg.value->accept(*this);
            }
            resolve(node.inferredType, node.loc);
            
            MethodInfo mInfo;
            if (methodResolver.probe(node.object->inferredType, std::string(node.methodName), mInfo)) {
                node.resolvedFn = mInfo.methodId;
                if (mInfo.type) {
                    coerceArgs(node.args, mInfo.type);
                }
            }
        }
        void visit(IndexExpr& node) override {
            node.base->accept(*this);
            node.index->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(MemberExpr& node) override {
            node.object->accept(*this);
            resolve(node.inferredType, node.loc);
            
            const Type* objTy = node.object->inferredType;
            if (auto* refTy = dynamic_cast<const ReferenceType*>(objTy)) objTy = refTy->pointee;
            else if (auto* ptrTy = dynamic_cast<const PointerType*>(objTy)) objTy = ptrTy->pointee;
            
            if (auto* st = dynamic_cast<const StructType*>(objTy)) {
                const auto& sym = table.getSymbol(st->structSymbolId);
                if (sym.kind == SymbolKind::Struct && sym.decl) {
                    auto* structDecl = static_cast<StructDeclNode*>(sym.decl);
                    for (size_t i = 0; i < structDecl->fields.size(); ++i) {
                        if (structDecl->fields[i]->name == node.member) {
                            if (sym.isExternal && !structDecl->fields[i]->isPublic) {
                                diag.error(node.loc, "Field '" + std::string(node.member) + "' of struct '" + sym.name.str() + "' is private.");
                            }
                            node.resolvedFieldIndex = i;
                            break;
                        }
                    }
                }
            }
        }
        void visit(TupleIndexExpr& node) override {
            node.object->accept(*this);
            const Type* objTy = ctx.unificationTable.deepResolve(node.object->inferredType, ctx);
            if (auto* tup = dynamic_cast<const TupleType*>(objTy)) {
                if (node.index < tup->elements.size()) {
                    node.inferredType = tup->elements[node.index];
                } else {
                    diag.error(node.loc, "Tuple index " + std::to_string(node.index) +
                                          " out of bounds (tuple has " +
                                          std::to_string(tup->elements.size()) + " elements)");
                    node.inferredType = ctx.getUnknown();
                }
            } else {
                node.inferredType = ctx.getUnknown();
            }
        }
        void visit(CastExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(UnsizeCastExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(ArrayLiteralExpr& node) override {}
        void visit(TupleLiteralExpr& node) override {
            for (auto& elem : node.elements) elem->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(StructInitExpr& node) override {
            resolve(node.inferredType, node.loc);
            
            if (node.structId != kInvalidSymbolID) {
                const auto& sym = table.getSymbol(node.structId);
                if (sym.isExternal && sym.kind == SymbolKind::Struct && sym.decl) {
                    auto* structDecl = static_cast<const StructDeclNode*>(sym.decl);
                    for (auto& field : structDecl->fields) {
                        if (!field->isPublic) {
                            diag.error(node.loc, "Struct '" + sym.name.str() + "' has private field '" + std::string(field->name) + "' and cannot be initialized directly.");
                            break;
                        }
                    }
                }
            }
            
            for (auto& field : node.fields) {
                if (field.value) field.value->accept(*this);
            }
        }
          void visit(MatchExpr& node) override {
              node.subject->accept(*this);
              PatternResolverVisitor patRes(*this);
              for (auto& arm : node.arms) {
                  if (arm.pattern) arm.pattern->accept(patRes);
                  if (arm.body) arm.body->accept(*this);
              }
              resolve(node.inferredType, node.loc);
          }
        void visit(LambdaExpr& node) override {}
        void visit(AwaitExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(SizeofExpr& node) override {}
        void visit(AlignofExpr& node) override {}
    };

    TypePrePass pre(table_, ctx_, typeTable_, methodResolver_, implementedTraits_, monoEngine_, diag_);
    root->accept(pre);

    std::vector<Constraint> constraints;
    ConstraintGenerator gen(table_, diag_, ctx_, typeTable_, constraints, methodResolver_, implementedTraits_, monoEngine_, metadataCache_);
    root->accept(gen);

    UnificationEngine solver(table_, diag_, ctx_, typeTable_, methodResolver_);
    solver.solve(constraints);

    for (auto& t : typeTable_) {
        if (t) t = ctx_.unificationTable.deepResolve(t, ctx_);
    }

    TypeResolver resolver(table_, diag_, ctx_, typeTable_, monoEngine_, methodResolver_);
    root->accept(resolver);

    return !diag_.hasErrors();
}


const Type* TypeChecker::typeOf(SymbolID sym) const {
    // For external symbols (loaded from .mlib), the typeTable_ entry is
    // ctx_.getUnknown() by default. Check the metadata cache first.
    if (metadataCache_) {
        const Symbol& s = table_.getSymbol(sym);
        if (s.isExternal) {
            const Type* extType = metadataCache_->getType(sym);
            if (extType && extType->getKind() != TypeKind::Unknown) {
                return extType;
            }
        }
    }
    if (sym < typeTable_.size()) return typeTable_[sym];
    return nullptr;
}

void TypeChecker::registerImpl(const Type* type, SymbolID traitId) {
    if (type) implementedTraits_[type].insert(traitId);
}

bool TypeChecker::implementsTrait(const Type* type, SymbolID traitId) const {
    if (!type) return false;
    auto it = implementedTraits_.find(type);
    if (it != implementedTraits_.end()) {
        return it->second.count(traitId) > 0;
    }
    return false;
}

} // namespace fl
