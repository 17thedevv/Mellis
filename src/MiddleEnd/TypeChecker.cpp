#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/MiddleEnd/MonomorphizationEngine.h"
#include "mellis/MiddleEnd/ConstEvaluator.h"
#include "mellis/MiddleEnd/OperatorRegistry.h"
#include <iostream>
#include <typeinfo>
#include <vector>

#include "mellis/MiddleEnd/Mangle.h"
#include "mellis/MiddleEnd/Semantic/ObjectSafety.h"

namespace fl {

struct TypeTableRef {
    std::vector<const Type*>& vec;
    fl::SymbolTable& table;
    fl::TypeContext& ctx;
    
    TypeTableRef(std::vector<const Type*>& v, fl::SymbolTable& t, fl::TypeContext& c) : vec(v), table(t), ctx(c) {}
    
    const fl::Type*& operator[](size_t index) {
        if (index >= vec.size()) vec.resize(table.symbolCount(), ctx.getUnknown());
        return vec[index];
    }
    const fl::Type* operator[](size_t index) const {
        if (index >= vec.size()) return ctx.getUnknown();
        return vec[index];
    }
    size_t size() const { return vec.size(); }
    operator std::vector<const fl::Type*>&() { return vec; }
    operator const std::vector<const fl::Type*>&() const { return vec; }
};


struct UnsafeContextGuard {
    bool& flag;
    bool prevState;
    UnsafeContextGuard(bool& f) : flag(f), prevState(f) { flag = true; }
    ~UnsafeContextGuard() { flag = prevState; }
};

enum class ConstraintKind { Equality, Field, MethodCall, Iterable, EnumVariantPattern, Deref, BinaryOperator, UnaryOperator, Index };

struct Constraint {
    ConstraintKind kind;
    const Type* expected;
    const Type* actual;
    std::string name;
    std::vector<const Type*> callArgs; // Used for generic args or field types
    std::vector<std::string> callArgNames; // Labels for MethodCall
    PatternNode* pattern = nullptr;
    SourceLocation loc;
    ModuleID callerModuleID = 0;
    
    Constraint(ConstraintKind k, const Type* exp, const Type* act, std::string n, std::vector<const Type*> args, std::vector<std::string> argNames, SourceLocation l)
        : kind(k), expected(exp), actual(act), name(n), callArgs(args), callArgNames(argNames), loc(l) {}
        
    Constraint(ConstraintKind k, const Type* exp, const Type* act, std::string n, SourceLocation l)
        : kind(k), expected(exp), actual(act), name(n), callArgs({}), loc(l) {}
        
    Constraint(ConstraintKind k, const Type* exp, SymbolID vId, PatternNode* pat, SourceLocation l)
        : kind(k), expected(exp), actual(nullptr), name(std::to_string(vId)), callArgs({}), pattern(pat), loc(l) {}
};

TypeChecker::TypeChecker(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, MonomorphizationEngine* monoEngine)
    : table_(table), diag_(diag), ctx_(ctx), monoEngine_(monoEngine), methodResolver_(), traitSolver_(ctx, table, diag, typeTable_, methodResolver_) {}


static void propagateLValue(ExprNode* expr) {
    if (!expr) return;
    expr->valueCategory = ValueCategory::LValue;
    if (auto* member = dynamic_cast<MemberExpr*>(expr)) propagateLValue(member->object.get());
    else if (auto* index = dynamic_cast<IndexExpr*>(expr)) propagateLValue(index->base.get());
    else if (auto* tup = dynamic_cast<TupleIndexExpr*>(expr)) propagateLValue(tup->object.get());
}

bool TypeChecker::check(ASTNode* root, ModuleID currentModule) {
    if (!root) return false;
    typeTable_.resize(table_.symbolCount(), ctx_.getUnknown());

    class TypePrePass : public ASTVisitor, public TypeVisitor {
        SymbolTable& table;
        TypeContext& ctx;
        TypeTableRef typeTable;
        MethodResolver& methodResolver;
        TraitSolver& traitSolver;
        
        const Type* evaluatedType = nullptr;
        MonomorphizationEngine* monoEngine;
        DiagnosticEngine& diag;
        std::set<std::pair<SymbolID, const Type*>> implCache_;

    public:
        const Type* evaluateTypeNode(TypeNode* node) {
            if (!node) return ctx.getVoid();
            std::cerr << "[DEBUG] evaluateTypeNode called on " << typeid(*node).name() << std::endl;
            evaluatedType = nullptr;
            node->accept(static_cast<TypeVisitor&>(*this));
            return evaluatedType ? evaluatedType : ctx.getUnknown();
        }

        TypePrePass(SymbolTable& table, TypeContext& ctx, TypeTableRef typeTable, MethodResolver& methodResolver, TraitSolver& traitSolver, MonomorphizationEngine* monoEngine, DiagnosticEngine& diag)
              : table(table), ctx(ctx), typeTable(typeTable), methodResolver(methodResolver), traitSolver(traitSolver), monoEngine(monoEngine), diag(diag) {}

        void visit(ProgramNode& node) override {
    std::cerr << "[DEBUG] TypePrePass visiting ProgramNode with " << node.items.size() << " items" << std::endl;
    for (auto& item : node.items) {
        std::cerr << "[DEBUG] item kind: " << typeid(*item).name() << std::endl;
        item->accept(*this);
    }
}
        void visit(ModDeclNode& node) override { for (auto& d : node.decls) d->accept(*this); }
        void visit(StructDeclNode& node) override {
            const StructType* stType = ctx.getStructType(node.symbolId);
            typeTable[node.symbolId] = stType;
            StructType* mutSt = const_cast<StructType*>(stType);
            for (size_t i = 0; i < node.fields.size(); ++i) {
                auto& field = node.fields[i];
                mutSt->fieldIndices[std::string(field->name)] = i;
                auto optId = table.lookup(Identifier(field->name), node.bodyScopeId);
                field->symbolId = optId.empty() ? kInvalidSymbolID : optId[0];
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
                            const Type* fTy = evaluateTypeNode(field->type.get());
                            fieldTypes.push_back(fTy);
                            paramNames.push_back(std::string(field->name));
                            if (field->symbolId != kInvalidSymbolID) {
                                typeTable[field->symbolId] = fTy;
                            }
                        }
                        typeTable[variant->symbolId] = ctx.getFunctionType(std::move(paramNames), std::move(fieldTypes), enumTy, false);
                    }
                }
            }
        }
        void visit(FunctionDeclNode& node) override {
            // Register generic params so typeTable[gp.symbolId] = GenericParamType(paramId=gp.symbolId)
            for (auto& gp : node.genericParams) {
                if (gp.symbolId != kInvalidSymbolID) {
                    typeTable[gp.symbolId] = ctx.getGenericParamType(gp.symbolId, gp.name);
                }
            }
            std::vector<std::string> paramNames;
            std::vector<const Type*> paramTypes;
            for (auto& param : node.params) {
                const Type* pt = nullptr;
                if (!param->type && param->name == "self") {
                    auto selfSyms = table.lookup(Identifier(std::string("Self")), node.bodyScopeId);
                    if (!selfSyms.empty()) {
                        pt = typeTable[selfSyms[0]];
                        std::cerr << "[DEBUG self] Found Self alias, symbolId=" << selfSyms[0] << ", typeKind=" << (pt ? std::to_string((int)pt->getKind()) : "NULL") << "\n";
                    } else {
                        std::cerr << "[DEBUG self] Could not find Self alias in scope " << node.bodyScopeId << "\n";
                    }
                    if (!pt) pt = ctx.getUnknown();
                } else {
                    pt = evaluateTypeNode(param->type.get());
                    std::cerr << "[DEBUG self] Evaluated param->type for " << param->name << " -> " << (pt ? std::to_string((int)pt->getKind()) : "NULL") << "\n";
                }
                if (param->symbolId != kInvalidSymbolID) typeTable[param->symbolId] = pt;
                paramNames.push_back(std::string(param->name));
                paramTypes.push_back(pt);
            }
            const Type* retType = evaluateTypeNode(node.returnType.get());
            if (node.name == "display") {
                std::cerr << "[DEBUG] display retType: " << (retType ? std::to_string((int)retType->getKind()) : "null") << std::endl;
            }
            if (node.isAsync) {
                retType = ctx.create<FutureType>(retType);
            }
            typeTable[node.symbolId] = ctx.getFunctionType(std::move(paramNames), std::move(paramTypes), retType, false, node.isVariadic);

            // Mangle name if overloaded, and check for extern overload error
            auto& sym = table.getMutableSymbol(node.symbolId);
            auto existingSyms = table.lookupInScope(Identifier(node.name), sym.declaredInScope);
            
            if (existingSyms.size() > 1) {
                bool hasExtern = false;
                for (SymbolID sid : existingSyms) {
                    if (table.getSymbol(sid).isExternal) hasExtern = true;
                }
                if (hasExtern) {
                    diag.error(node.loc, "Cannot overload external function '" + std::string(node.name) + "'");
                }
                
                if (!sym.isExternal && std::string(node.name) != "main") {
                    if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[node.symbolId])) {
                        sym.mangledName = Mangle::mangleOverloadedFunction(node.name, fnTy->paramTypes, table);
                    }
                }
            }
            
            // Register methods from generic bounds
            for (auto& gp : node.genericParams) {
                if (gp.symbolId != kInvalidSymbolID) {
                    std::cerr << "[DEBUG] Processing generic param: " << gp.name << " with " << gp.bounds.size() << " bounds" << std::endl;
                    const Type* gpType = ctx.getGenericParamType(gp.symbolId, gp.name);
                    for (auto& bound : gp.bounds) {
                        const Type* boundType = evaluateTypeNode(bound.get());
                        std::cerr << "[DEBUG] Bound type evaluated to kind: " << (boundType ? std::to_string((int)boundType->getKind()) : "null") << std::endl;
                        if (boundType && boundType->getKind() == TypeKind::Trait) {
                            auto* traitTy = static_cast<const TraitType*>(boundType);
                            TraitBound tb;
                            tb.selfType = gpType;
                            tb.traitId = traitTy->traitId;
                            traitSolver.addBound(tb);
                            std::cerr << "[DEBUG] Added trait bound to solver: self=" << gpType->toString() << " traitId=" << traitTy->traitId << std::endl;
                            
                            // implementedTraits[gpType].insert(traitTy->traitId);
                            const auto& traitSym = table.getSymbol(traitTy->traitId);
                            if (traitSym.kind == SymbolKind::Trait && traitSym.decl) {
                                auto* traitDecl = static_cast<TraitDeclNode*>(traitSym.decl);
                                for (auto& m : traitDecl->methods) {
                                    if (m->symbolId != kInvalidSymbolID) {
                                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                                            // methodResolver.addMethod(gpType, std::string(m->name), m->symbolId, fnTy, ctx);
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
                const Type* dynTraitTy = ctx.getTraitObjectType({node.symbolId});
                const Type* refDynTraitTy = ctx.getReferenceType(dynTraitTy, false);
                const Type* mutRefDynTraitTy = ctx.getReferenceType(dynTraitTy, true);
                const Type* ptrDynTraitTy = ctx.getPointerType(dynTraitTy, false);
                const Type* mutPtrDynTraitTy = ctx.getPointerType(dynTraitTy, true);
                for (auto& m : node.methods) {
                    if (m->symbolId != kInvalidSymbolID) {
                        auto& sym = table.getMutableSymbol(m->symbolId);
                        if (sym.mangledName.empty()) {
                            sym.mangledName = std::string(m->name) + "_" + std::to_string(m->symbolId);
                        }
                        std::cerr << "[DEBUG TraitDeclNode] Method '" << m->name << "' symbolId=" << m->symbolId << "\n";
                        if (typeTable.size() > m->symbolId) {
                            std::cerr << "[DEBUG TraitDeclNode] typeTable[" << m->symbolId << "] = " << (typeTable[m->symbolId] ? std::to_string((int)typeTable[m->symbolId]->getKind()) : "NULL") << "\n";
                        } else {
                            std::cerr << "[DEBUG TraitDeclNode] typeTable size " << typeTable.size() << " is too small for " << m->symbolId << "\n";
                        }
                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                            methodResolver.addTraitMethod(std::string(m->name), node.symbolId, m->symbolId, fnTy);
                            table.registerTraitMethod(node.symbolId, m->symbolId);
                            std::cerr << "[DEBUG TraitDeclNode] Registered trait method '" << m->name << "'\n";
                        } else {
                            std::cerr << "[DEBUG TraitDeclNode] Failed to cast typeTable[" << m->symbolId << "] to FunctionType\n";
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
                            // implementedTraits[gpType].insert(traitTy->traitId);
                            const auto& traitSym = table.getSymbol(traitTy->traitId);
                            if (traitSym.kind == SymbolKind::Trait && traitSym.decl) {
                                auto* traitDecl = static_cast<TraitDeclNode*>(traitSym.decl);
                                for (auto& m : traitDecl->methods) {
                                    if (m->symbolId != kInvalidSymbolID) {
                                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                                            // methodResolver.addMethod(gpType, std::string(m->name), m->symbolId, fnTy, ctx);
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
            std::cerr << "[DEBUG] visit(ImplDeclNode) genericParams size: " << node.genericParams.size() << std::endl;
            if (!node.genericParams.empty()) {
                if (auto* nt = dynamic_cast<NamedTypeNode*>(node.selfType.get())) {
                    if (monoEngine) monoEngine->registerGenericImpl(nt->symbolId, &node);
                }
                return;
            }
            const Type* selfType = evaluateTypeNode(node.selfType.get());
        std::cerr << "[DEBUG] ImplDeclNode evaluating selfType: " << (selfType ? std::to_string((int)selfType->getKind()) : "null") << std::endl;
        if (!selfType) return;
        
        auto optSelfId = table.lookup(Identifier(std::string("Self")), node.bodyScopeId);
            if (!optSelfId.empty()) {
                typeTable[optSelfId[0]] = selfType;
                std::cerr << "[DEBUG] ImplDeclNode Self symbolId=" << optSelfId[0] << " selfType=" << (selfType ? std::to_string((int)selfType->getKind()) : "null") << std::endl;
            }
            
            // Register methods for Generic Params in Impl
            for (auto& gp : node.genericParams) {
                if (gp.symbolId != kInvalidSymbolID) {
                    const Type* gpType = ctx.getGenericParamType(gp.symbolId, gp.name);
                    for (auto& bound : gp.bounds) {
                        const Type* boundType = evaluateTypeNode(bound.get());
                        if (auto* traitTy = dynamic_cast<const TraitType*>(boundType)) {
                            // implementedTraits[gpType].insert(traitTy->traitId);
                            const auto& traitSym = table.getSymbol(traitTy->traitId);
                            if (traitSym.kind == SymbolKind::Trait && traitSym.decl) {
                                auto* traitDecl = static_cast<TraitDeclNode*>(traitSym.decl);
                                for (auto& m : traitDecl->methods) {
                                    if (m->symbolId != kInvalidSymbolID) {
                                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[m->symbolId])) {
                                            // methodResolver.addMethod(gpType, std::string(m->name), m->symbolId, fnTy, ctx);
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
                    auto& sym = table.getMutableSymbol(method->symbolId);
                    if (sym.mangledName.empty()) {
                        sym.mangledName = std::string(method->name) + "_" + std::to_string(method->symbolId);
                    }
                    if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[method->symbolId])) {
                        // methodResolver.addMethod(selfType, std::string(method->name), method->symbolId, fnTy, ctx);
                    }
                }
            }
            
            if (traitTy) {
                // implementedTraits[selfType].insert(traitTy->traitId);
                const auto& sym = table.getSymbol(traitTy->traitId);
                
                if (sym.name == "Drop") {
                    if (auto* st = dynamic_cast<StructType*>(const_cast<Type*>(selfType))) {
                        st->hasDrop = true;
                    }
                }
                
                bool isDuplicate = false;
                if (sym.kind == SymbolKind::Trait && sym.decl) {
                    // DUPLICATE IMPL CHECK
                    auto implPair = std::make_pair(traitTy->traitId, selfType);
                    if (implCache_.find(implPair) != implCache_.end()) {
                        diag.error(node.loc, "duplicate implementation of trait for this type", "E-TRAIT-DUPLICATE-IMPL");
                        isDuplicate = true;
                    } else {
                        implCache_.insert(implPair);
                    }

                    auto* traitDecl = static_cast<TraitDeclNode*>(sym.decl);
                    // MISSING METHOD & SIGNATURE CHECK
                    for (auto& tMethod : traitDecl->methods) {
                        bool found = false;
                        for (auto& iMethod : node.methods) {
                            if (iMethod->name == tMethod->name) {
                                found = true;
                                auto* tFnTy = dynamic_cast<const FunctionType*>(typeTable[tMethod->symbolId]);
                                auto* iFnTy = dynamic_cast<const FunctionType*>(typeTable[iMethod->symbolId]);
                                if (tFnTy && iFnTy) {
                                    if (tFnTy->paramTypes.size() != iFnTy->paramTypes.size()) {
                                        diag.error(iMethod->loc, "method signature mismatch: parameter count differs from trait", "E-TRAIT-SIGNATURE")
                                            .addNote(tMethod->loc, "method declared in trait here");
                                    }
                                }
                                break;
                            }
                        }
                        if (!found) {
                            diag.error(node.loc, "missing method '" + std::string(tMethod->name) + "' in implementation", "E-TRAIT-MISSING-METHOD")
                                .addNote(tMethod->loc, "method declared in trait here");
                        }
                    }
                }
                
                if (isDuplicate) return; // Do not register duplicate impl to avoid solver issues
                
                TraitClause clause;
                clause.implNode = &node;
                clause.traitId = traitTy->traitId;
                clause.selfType = selfType;
                // clause.genericArgs = ...
                // clause.genericParamIds = ...
                // clause.obligations = ...

                for (const auto& at : node.associatedTypes) {
                    if (at && at->aliasedType) {
                        const Type* resolved = evaluateTypeNode(at->aliasedType.get());
                        if (resolved) {
                            clause.associatedBindings[std::string(at->name)] = resolved;
                        }
                    }
                }

                traitSolver.addClause(std::move(clause));
                
                if (sym.kind == SymbolKind::Trait && sym.decl) {
                    for (auto& method : node.methods) {
                        if (method->symbolId != kInvalidSymbolID) {
                            // Trait methods are already registered from the TraitDeclNode.
                            // We do not register impl trait methods to avoid shadowing and signature mismatches during generic checking.
                        }
                    }
                }
            } else {
                for (auto& method : node.methods) {
                    if (method->symbolId != kInvalidSymbolID) {
                        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[method->symbolId])) {
                            methodResolver.addInherentMethod(std::string(method->name), &node, method->symbolId, fnTy);
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
        void visit(UnaryExpr& node) {
            if (node.op == UnaryOp::RefMut) {
                propagateLValue(node.operand.get());
            }
        }
        void visit(AssignExpr& node) override {
            propagateLValue(node.lvalue.get());
        }
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
        void visit(TryExpr& node) override {}
        void visit(LambdaExpr& node) override {
            if (node.body) node.body->accept(*this);
            std::vector<CaptureInfo> semCaptures;
            auto* closureTy = const_cast<ClosureType*>(ctx.create<ClosureType>(node.generatedStructId, node.generatedFuncId, nullptr, semCaptures));
            node.inferredType = closureTy;
        }
        void visit(AwaitExpr& node) override {
            if (node.expr) node.expr->accept(*this);
          }
        void visit(SizeofExpr& node) override {}
        void visit(AlignofExpr& node) override {}

        void visit(BuiltinTypeNode& node) override { 
            evaluatedType = ctx.getPrimitive(node.kind); 
        }
        void visit(NamedTypeNode& node) override {
            if (!node.segments.empty()) {
                std::cerr << "[DEBUG] NamedTypeNode segment[0]: '" << node.segments[0] << "' symbolId=" << node.symbolId << std::endl;
                if (node.segments[0] == "void") { evaluatedType = ctx.getVoid(); return; }
                if (node.segments[0] == "int_32") { evaluatedType = ctx.getPrimitive(BuiltinKind::I32); return; }
                if (node.segments[0] == "int_64") { evaluatedType = ctx.getPrimitive(BuiltinKind::I64); return; }
                if (node.segments[0] == "uint_32") { evaluatedType = ctx.getPrimitive(BuiltinKind::U32); return; }
                if (node.segments[0] == "uint_64") { evaluatedType = ctx.getPrimitive(BuiltinKind::U64); return; }
                if (node.segments[0] == "float_32") { evaluatedType = ctx.getPrimitive(BuiltinKind::F32); return; }
                if (node.segments[0] == "float_64") { evaluatedType = ctx.getPrimitive(BuiltinKind::F64); return; }
                if (node.segments[0] == "bool") { evaluatedType = ctx.getPrimitive(BuiltinKind::Bool); return; }
                if (node.segments[0] == "char") { evaluatedType = ctx.getPrimitive(BuiltinKind::Char); return; }
                if (node.segments[0] == "string") { evaluatedType = ctx.getPrimitive(BuiltinKind::Str); return; }
            }
            if (node.symbolId != kInvalidSymbolID) {
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
                    if (sym.name.view() == "Option") {
                        std::cerr << "[DEBUG] Checking Option: allConcrete=" << allConcrete << " monoEngine=" << (monoEngine != nullptr) << " sym.decl=" << (sym.decl != nullptr) << std::endl;
                        if (sym.decl) {
                            auto* enumDecl = static_cast<const EnumDeclNode*>(sym.decl);
                            std::cerr << "[DEBUG] enumDecl->genericParams.empty()=" << enumDecl->genericParams.empty() << std::endl;
                        }
                    }
                    if (allConcrete && monoEngine && sym.decl) {
                        auto* enumDecl = static_cast<const EnumDeclNode*>(sym.decl);
                        if (!enumDecl->genericParams.empty()) {
                            std::cerr << "[DEBUG] evaluateTypeNode calling requestEnumSpecialization for " << sym.name.view() << std::endl;
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
                else if (sym.kind == SymbolKind::Trait) {
                    evaluatedType = ctx.getTraitType(node.symbolId);
                }
                else if (sym.kind == SymbolKind::GenericParam) {
            evaluatedType = ctx.getGenericParamType(node.symbolId, sym.name.view());
        } else if (sym.kind == SymbolKind::TypeAlias) {
            if (node.symbolId < typeTable.size() && typeTable[node.symbolId]) {
                evaluatedType = typeTable[node.symbolId];
            } else if (sym.decl == nullptr) {
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
            evaluatedType = typeTable[node.symbolId];
        }

        if (evaluatedType && node.segments.size() > 1) {
            for (size_t i = 1; i < node.segments.size(); ++i) {
                evaluatedType = ctx.getAssociatedProjection(evaluatedType, kInvalidSymbolID, std::string(node.segments[i]));
            }
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
                evaluatedType = ctx.getTraitObjectType({tr->traitId});
            } else {
                evaluatedType = ctx.getUnknown();
            }
        }
        void visit(LifetimeNode& node) override {
            Lifetime lt;
            if (node.name == "'static") {
                lt.kind = LifetimeKind::Static;
            } else if (node.name == "'_") {
                lt.kind = LifetimeKind::Anonymous;
            } else {
                lt.kind = LifetimeKind::Named;
                lt.name = std::string(node.name);
            }
            evaluatedType = ctx.getLifetimeType(lt);
        }
    };
    
    class PatternConstraintVisitor : public PatternVisitor {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        TypeTableRef typeTable;
        std::vector<Constraint>& constraints;
        const Type* expectedType;
        ModuleID callerModuleID;

    public:
        void addConstraint(Constraint c) { c.callerModuleID = callerModuleID; constraints.push_back(std::move(c)); }

        PatternConstraintVisitor(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, TypeTableRef typeTable, std::vector<Constraint>& constraints, const Type* expected, ModuleID callerModuleID)
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), constraints(constraints), expectedType(expected), callerModuleID(callerModuleID) {}
            
        void visit(WildcardPatternNode& node) override {
            node.inferredType = expectedType;
        }
        
        void visit(LiteralPatternNode& node) override {
            node.inferredType = expectedType;
        }
        
        void visit(StructPatternNode& node) override {}
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
                            addConstraint(Constraint(ConstraintKind::Equality, specializedEnum, expectedType, "", node.loc));
                        }

                        for (size_t i = 0; i < node.fields.size(); ++i) {
                            const Type* fieldExpected = variantFn->paramTypes[i];
                            if (!substitutionMap.empty()) {
                                fieldExpected = ctx.substitute(fieldExpected, substitutionMap);
                            }
                            PatternConstraintVisitor fieldVisitor(table, diag, ctx, typeTable, constraints, fieldExpected, callerModuleID);
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
                    addConstraint(Constraint(ConstraintKind::Equality, specializedEnum, expectedType, "", node.loc));
                }
            }
            node.inferredType = expectedType;
        }
        
        void visit(TuplePatternNode& node) override {
            std::vector<const Type*> elementTypes;
            for (auto& elem : node.elements) {
                const Type* elemVar = ctx.getInferenceVar(ctx.newVar());
                elementTypes.push_back(elemVar);
                PatternConstraintVisitor elemVisitor(table, diag, ctx, typeTable, constraints, elemVar, callerModuleID);
                elem->accept(elemVisitor);
            }
            const Type* tupleType = ctx.getTupleType(elementTypes);
            addConstraint(Constraint(ConstraintKind::Equality, tupleType, expectedType, "", node.loc));
            node.inferredType = tupleType;
        }
    };

    class ConstraintGenerator : public ASTVisitor, public TypeVisitor {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        TypeTableRef typeTable;
        std::vector<Constraint>& constraints;
        MethodResolver& methodResolver;
        TraitSolver& traitSolver;
        const Type* currentReturnType = nullptr;
        MonomorphizationEngine* monoEngine;
        MLibMetadataCache* metadataCache; // nullable
        bool isUnsafeContext_ = false;
        bool isAsyncContext_ = false;
        ModuleID currentModuleID = 0;

        const Type* evaluateTypeNode(TypeNode* node) {
            if (!node) return nullptr;
            TypePrePass pre(table, ctx, typeTable, methodResolver, traitSolver, monoEngine, diag);
            return pre.evaluateTypeNode(node);
        }
    public:
        ConstraintGenerator(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, TypeTableRef typeTable, std::vector<Constraint>& constraints, MethodResolver& methodResolver, TraitSolver& traitSolver, MonomorphizationEngine* monoEngine, MLibMetadataCache* metadataCache = nullptr)
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), constraints(constraints), methodResolver(methodResolver), traitSolver(traitSolver), monoEngine(monoEngine), metadataCache(metadataCache) {}

        void visit(ProgramNode& node) override { std::cerr << "[DEBUG] CG visiting ProgramNode\n"; for (auto& item : node.items) { std::cerr << "[DEBUG] CG visiting item: " << typeid(*item).name() << "\n"; item->accept(*this); } }
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
            
            if (node.symbolId != kInvalidSymbolID) {
                typeTable[node.symbolId] = varType;
            }

            if (node.pattern) {
                PatternConstraintVisitor patVis(table, diag, ctx, typeTable, constraints, varType, currentModuleID);
                node.pattern->accept(patVis);
            }

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
                        TypePrePass pre(table, ctx, typeTable, methodResolver, traitSolver, monoEngine, diag);
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
            if (node.overloadCandidates.size() == 1) {
                SymbolID resolvedSymbol = node.overloadCandidates[0];
                const Type* baseType = typeTable[resolvedSymbol];
                
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
                                    TypePrePass pre(table, ctx, typeTable, methodResolver, traitSolver, monoEngine, diag);
                                    argTy = pre.evaluateTypeNode(node.genericArgs[i].get());
                                } else {
                                    argTy = ctx.getInferenceVar(ctx.newVar());
                                }
                                freshArgs.push_back(argTy);
                                substitutionMap[enumDecl->genericParams[i].symbolId] = argTy;
                            }
                            
                            node.inferredType = ctx.substitute(baseType, substitutionMap);
                            std::cerr << "[DEBUG IdentifierExpr] Substituted generic enum variant '" << node.segments.back() << "', new type: " << node.inferredType->toString() << "\n";
                            return;
                        }
                    }
                } else {
                    const auto& sym = table.getSymbol(resolvedSymbol);
                    if (sym.kind == SymbolKind::Function && sym.decl) {
                        auto* fnDecl = static_cast<FunctionDeclNode*>(sym.decl);
                        if (!fnDecl->genericParams.empty()) {
                            std::unordered_map<SymbolID, const Type*> substitutionMap;
                            std::vector<const Type*> freshArgs;
                            for (size_t i = 0; i < fnDecl->genericParams.size(); ++i) {
                                const Type* argTy = nullptr;
                                if (i < node.genericArgs.size()) {
                                    TypePrePass pre(table, ctx, typeTable, methodResolver, traitSolver, monoEngine, diag);
                                    argTy = pre.evaluateTypeNode(node.genericArgs[i].get());
                                } else {
                                    argTy = ctx.getInferenceVar(ctx.newVar());
                                }
                                substitutionMap[fnDecl->genericParams[i].symbolId] = argTy;
                                freshArgs.push_back(argTy);
                            }
                            
                            node.inferredType = ctx.substitute(baseType, substitutionMap);
                            return;
                        }
                    }
                }
                
                node.inferredType = baseType;
            } else if (node.overloadCandidates.size() > 1) {
                // Type is unknown until we resolve the overload in CallExpr
                node.inferredType = ctx.getUnknown();
            } else {
                node.inferredType = ctx.getUnknown();
            }
        }
        void visit(BinaryExpr& node) override {
            node.left->accept(*this);
            node.right->accept(*this);
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
            constraints.push_back(Constraint(ConstraintKind::BinaryOperator, node.inferredType, node.right->inferredType, std::to_string((int)node.op), {node.left->inferredType}, {""}, node.loc));
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
            } else {
                constraints.push_back(Constraint(ConstraintKind::UnaryOperator, node.inferredType, node.operand->inferredType, std::to_string((int)node.op), {node.operand->inferredType}, {""}, node.loc));
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
                if (ident->overloadCandidates.size() > 1) {
                    std::vector<SymbolID> validCandidates;
                    for (SymbolID candidate : ident->overloadCandidates) {
                        const Type* candType = typeTable[candidate];
                        if (auto* fnTy = dynamic_cast<const FunctionType*>(candType)) {
                            if (fnTy->paramTypes.size() != argTypes.size() && !fnTy->isVariadic) continue;
                            if (fnTy->isVariadic && fnTy->paramTypes.size() > argTypes.size()) continue;

                            bool match = true;
                            for (size_t i = 0; i < argTypes.size(); ++i) {
                                const Type* pTy = (fnTy->isVariadic && i >= fnTy->paramTypes.size()) ? fnTy->paramTypes.back() : fnTy->paramTypes[i];
                                const Type* aTy = ctx.unificationTable.deepResolve(argTypes[i], ctx);
                                if (aTy->getKind() == TypeKind::InferenceVar) continue; 
                                if (aTy->getKind() == TypeKind::Unknown) { match = false; break; }
                                if (pTy->equals(aTy)) continue;
                                if (aTy->getKind() == TypeKind::Primitive && static_cast<const PrimitiveType*>(aTy)->builtinKind == BuiltinKind::Str) {
                                    if (auto* ptr = dynamic_cast<const PointerType*>(pTy)) {
                                        if (auto* prim = dynamic_cast<const PrimitiveType*>(ptr->pointee)) {
                                            if (prim->builtinKind == BuiltinKind::I8) continue;
                                        }
                                    }
                                }
                                match = false;
                                break;
                            }
                            if (match) validCandidates.push_back(candidate);
                        }
                    }

                    if (validCandidates.size() == 1) {
                        ident->overloadCandidates = { validCandidates[0] };
                    } else if (validCandidates.size() > 1) {
                        diag.error(node.loc, "Ambiguous call to overloaded function");
                    } else {
                        diag.error(node.loc, "No matching function for call");
                    }
                }

                if (ident->overloadCandidates.size() == 1) {
                    SymbolID resolvedSymbol = ident->overloadCandidates[0];
                    node.resolvedFn = resolvedSymbol;
                    const auto& sym = table.getSymbol(resolvedSymbol);

                    // Update inferred type now that overload is resolved
                    if (sym.kind == SymbolKind::Function || ident->inferredType == nullptr || ident->inferredType->getKind() == TypeKind::Unknown) {
                        ident->inferredType = typeTable[resolvedSymbol];
                        node.callee->inferredType = ident->inferredType;
                    }

                    // ── External Symbol path (loaded from .mlib) ──────────────
                    if (sym.isExternal && metadataCache) {
                        const Type* extType = metadataCache->getType(resolvedSymbol);
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
                            auto* ident = dynamic_cast<IdentifierExpr*>(node.callee.get());
                            const auto& providedArgs = (ident && !ident->genericArgs.empty()) ? ident->genericArgs : node.genericArgs;
                            
                            if (!providedArgs.empty()) {
                                for (auto& argNode : providedArgs) {
                                    node.inferredGenericArgs.push_back(evaluateTypeNode(argNode.get()));
                                }
                            } else {
                                for (size_t i = 0; i < fnDecl->genericParams.size(); ++i) {
                                    node.inferredGenericArgs.push_back(ctx.getInferenceVar(ctx.newVar()));
                                }
                            }
                            
                std::unordered_map<SymbolID, const Type*> substitutionMap;
                std::cerr << "[DEBUG CallExpr] fnDecl name: '" << fnDecl->name << "' symbolId: " << fnDecl->symbolId << std::endl;
                
                bool explicitArgs = false;
                if (auto* identExpr = dynamic_cast<IdentifierExpr*>(node.callee.get())) {
                    if (!identExpr->genericArgs.empty()) explicitArgs = true;
                }
                if (!node.genericArgs.empty()) explicitArgs = true;

                if (!explicitArgs) {
                    for (size_t i = 0; i < fnDecl->genericParams.size() && i < node.inferredGenericArgs.size(); ++i) {
                        SymbolID gpSymId = fnDecl->genericParams[i].symbolId;
                        substitutionMap[gpSymId] = node.inferredGenericArgs[i];
                    }
                } else {
                    size_t argIdx = 0;
                    std::vector<const Type*> fullArgs;
                    for (size_t i = 0; i < fnDecl->genericParams.size(); ++i) {
                        SymbolID gpSymId = fnDecl->genericParams[i].symbolId;
                        const Type* mappedType = nullptr;
                        
                        if (argIdx < node.inferredGenericArgs.size()) {
                            const Type* arg = node.inferredGenericArgs[argIdx];
                            bool isArgLifetime = (arg->getKind() == TypeKind::Lifetime);
                            bool isParamLifetime = (fnDecl->genericParams[i].kind == GenericParamKind::Lifetime);
                            
                            if (isArgLifetime == isParamLifetime) {
                                mappedType = arg;
                                argIdx++;
                            } else if (isParamLifetime && !isArgLifetime) {
                                Lifetime lt;
                                lt.kind = LifetimeKind::Anonymous;
                                lt.name = "'_";
                                mappedType = ctx.getLifetimeType(lt);
                            } else {
                                diag.error(node.loc, "Generic argument kind mismatch");
                                mappedType = ctx.getUnknown();
                            }
                        } else {
                            if (fnDecl->genericParams[i].kind == GenericParamKind::Lifetime) {
                                Lifetime lt;
                                lt.kind = LifetimeKind::Anonymous;
                                lt.name = "'_";
                                mappedType = ctx.getLifetimeType(lt);
                            } else {
                                mappedType = ctx.getInferenceVar(ctx.newVar());
                            }
                        }
                        substitutionMap[gpSymId] = mappedType;
                        fullArgs.push_back(mappedType);
                    }
                    node.inferredGenericArgs = fullArgs;
                }
                
                // Deep-resolve the callee type (it may be an InferenceVar pointing to a FunctionType)
                const Type* resolvedCalleeType = ctx.unificationTable.deepResolve(node.callee->inferredType, ctx);
                std::cerr << "[DEBUG CallExpr] resolvedCalleeType kind=" << (int)resolvedCalleeType->getKind() << ": " << resolvedCalleeType->toString() << std::endl;
                
                // Walk the resolved fn signature: for each type generic param, find the matching
                // GenericParamType (by name) in the param list and return type, and add its paramId to the map
                if (auto* fnTy = dynamic_cast<const FunctionType*>(resolvedCalleeType)) {
                    for (size_t i = 0; i < fnDecl->genericParams.size() && i < node.inferredGenericArgs.size(); ++i) {
                        std::string_view gpName = fnDecl->genericParams[i].name;
                        // Walk all param types
                        for (auto* p : fnTy->paramTypes) {
                            const Type* inner = p;
                            if (auto* r = dynamic_cast<const ReferenceType*>(inner)) inner = r->pointee;
                            if (auto* gp = dynamic_cast<const GenericParamType*>(inner)) {
                                std::cerr << "[DEBUG CallExpr] walk: gp->name='" << gp->name << "' paramId=" << gp->paramId << " vs gpName='" << gpName << "'" << std::endl;
                                if (gp->name == gpName) {
                                    substitutionMap[gp->paramId] = node.inferredGenericArgs[i];
                                    std::cerr << "[DEBUG CallExpr] fn param '" << gpName << "' paramId=" << gp->paramId << " -> " << node.inferredGenericArgs[i]->toString() << std::endl;
                                }
                            }
                        }
                        // Also check return type AssociatedTypeProjection's selfType
                        if (auto* proj = dynamic_cast<const AssociatedTypeProjection*>(fnTy->returnType)) {
                            if (auto* gp = dynamic_cast<const GenericParamType*>(proj->selfType)) {
                                if (gp->name == gpName) {
                                    substitutionMap[gp->paramId] = node.inferredGenericArgs[i];
                                    std::cerr << "[DEBUG CallExpr] return proj '" << gpName << "' paramId=" << gp->paramId << " mapped" << std::endl;
                                }
                            }
                        }
                    }
                }
                
                std::cerr << "[DEBUG CallExpr] before substitute: " << node.callee->inferredType->toString() << std::endl;
                // Substitute on the resolved canonical type, then assign back
                node.callee->inferredType = ctx.substitute(resolvedCalleeType, substitutionMap);
                std::cerr << "[DEBUG CallExpr] after substitute: " << node.callee->inferredType->toString() << std::endl;
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
            node.inferredType = ctx.getInferenceVar(ctx.newVar());
            constraints.push_back(Constraint(ConstraintKind::Index, node.base->inferredType, node.index->inferredType, std::to_string((int)node.valueCategory), {node.inferredType}, {""}, node.loc));
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

        void visit(TryExpr& node) override {
            if (node.expr) node.expr->accept(*this);
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
                    PatternConstraintVisitor patVisitor(table, diag, ctx, typeTable, constraints, node.subject->inferredType, currentModuleID);
                    arm.pattern->accept(patVisitor);
                }
                if (arm.body) {
                    arm.body->accept(*this);
                    if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(arm.body.get())) {
                        constraints.push_back(Constraint(ConstraintKind::Equality, node.inferredType, exprStmt->expr->inferredType, "", node.loc));
                    } else if (dynamic_cast<BlockStmtNode*>(arm.body.get())) {
                        constraints.push_back(Constraint(ConstraintKind::Equality, node.inferredType, ctx.getPrimitive(BuiltinKind::Void), "", node.loc));
                    }
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
                const Type* pType = typeTable[p->symbolId];
                if (!pType || pType->getKind() == TypeKind::Unknown) {
                    if (p->type) {
                        pType = evaluateTypeNode(p->type.get());
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
            
            std::vector<CaptureInfo> semCaptures;
            auto* closureTy = const_cast<ClosureType*>(ctx.create<ClosureType>(node.generatedStructId, node.generatedFuncId, sig, semCaptures));
            closureTy->fieldTypes.push_back(ctx.getPointerType(sig, false)); // First field is func ptr
            for (auto capRef : node.captures) {
                auto capId = capRef.symbolId;
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

        void visit(LifetimeNode& node) override {}
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
        TypeTableRef typeTable;
        MethodResolver& methodResolver;
        TraitSolver& traitSolver;
    public:
        UnificationEngine(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, TypeTableRef typeTable, MethodResolver& methodResolver, TraitSolver& traitSolver) 
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), methodResolver(methodResolver), traitSolver(traitSolver) {}

        bool unify(const Type* t1, const Type* t2, SourceLocation loc) {
            t1 = ctx.unificationTable.deepResolve(t1, ctx);
            t2 = ctx.unificationTable.deepResolve(t2, ctx);
            if (!t1 || !t2) return false;
            
            if (auto* proj1 = dynamic_cast<const AssociatedTypeProjection*>(t1)) {
                if (const Type* res1 = traitSolver.resolveProjection(proj1)) {
                    return unify(res1, t2, loc);
                }
            }
            if (auto* proj2 = dynamic_cast<const AssociatedTypeProjection*>(t2)) {
                if (const Type* res2 = traitSolver.resolveProjection(proj2)) {
                    return unify(t1, res2, loc);
                }
            }

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
                    if (st1->structSymbolId != st2->structSymbolId) {
                        if (st1->originalTemplateId != kInvalidSymbolID && st1->originalTemplateId == st2->structSymbolId) {
                            if (st1->specializedArgs.size() != st2->genericArgs.size()) goto mismatch;
                            for (size_t i = 0; i < st1->specializedArgs.size(); ++i) {
                                if (!unify(st1->specializedArgs[i], st2->genericArgs[i], loc)) return false;
                            }
                            return true;
                        } else if (st2->originalTemplateId != kInvalidSymbolID && st2->originalTemplateId == st1->structSymbolId) {
                            if (st2->specializedArgs.size() != st1->genericArgs.size()) goto mismatch;
                            for (size_t i = 0; i < st2->specializedArgs.size(); ++i) {
                                if (!unify(st1->genericArgs[i], st2->specializedArgs[i], loc)) return false;
                            }
                            return true;
                        }
                        goto mismatch;
                    }
                    if (st1->genericArgs.size() != st2->genericArgs.size()) goto mismatch;
                    for (size_t i = 0; i < st1->genericArgs.size(); ++i) {
                        if (!unify(st1->genericArgs[i], st2->genericArgs[i], loc)) return false;
                    }
                    return true;
                }
            }
            if (auto* tup1 = dynamic_cast<const TupleType*>(t1)) {
                if (auto* tup2 = dynamic_cast<const TupleType*>(t2)) {
                    if (tup1->elements.size() != tup2->elements.size()) goto mismatch;
                    for (size_t i = 0; i < tup1->elements.size(); ++i) {
                        if (!unify(tup1->elements[i], tup2->elements[i], loc)) return false;
                    }
                    return true;
                }
            }
            if (auto* arr1 = dynamic_cast<const ArrayType*>(t1)) {
                if (auto* arr2 = dynamic_cast<const ArrayType*>(t2)) {
                    if (arr1->length != arr2->length) goto mismatch;
                    return unify(arr1->elementType, arr2->elementType, loc);
                }
            }
            if (auto* e1 = dynamic_cast<const EnumType*>(t1)) {
                if (auto* e2 = dynamic_cast<const EnumType*>(t2)) {
                    if (e1->enumSymbolId != e2->enumSymbolId) {
                        if (e1->originalTemplateId != kInvalidSymbolID && e1->originalTemplateId == e2->enumSymbolId) {
                            if (e1->specializedArgs.size() != e2->genericArgs.size()) goto mismatch;
                            for (size_t i = 0; i < e1->specializedArgs.size(); ++i) {
                                if (!unify(e1->specializedArgs[i], e2->genericArgs[i], loc)) return false;
                            }
                            return true;
                        } else if (e2->originalTemplateId != kInvalidSymbolID && e2->originalTemplateId == e1->enumSymbolId) {
                            if (e2->specializedArgs.size() != e1->genericArgs.size()) goto mismatch;
                            for (size_t i = 0; i < e2->specializedArgs.size(); ++i) {
                                if (!unify(e1->genericArgs[i], e2->specializedArgs[i], loc)) return false;
                            }
                            return true;
                        }
                        goto mismatch;
                    }
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
            std::vector<bool> solved(constraints.size(), false);
            bool changed = true;
            int iterations = 0;
            while (changed && iterations++ < 10) {
                changed = false;
                for (size_t i = 0; i < constraints.size(); ++i) {
                    if (solved[i]) continue;
                    const auto& c = constraints[i];
                    if (c.kind == ConstraintKind::Equality) {
                        printf("Solving constraint: %s == %s\n", c.expected->toString().c_str(), c.actual->toString().c_str());
                        unify(c.expected, c.actual, c.loc);
                        solved[i] = true;
                        changed = true;
                    } else if (c.kind == ConstraintKind::Field) {
                        const Type* objType = ctx.unificationTable.deepResolve(c.expected, ctx);
                        if (auto* ptr = dynamic_cast<const PointerType*>(objType)) objType = ptr->pointee;
                        if (auto* ref = dynamic_cast<const ReferenceType*>(objType)) objType = ref->pointee;
                        if (auto* st = dynamic_cast<const StructType*>(objType)) {
                            const auto& sym = table.getSymbol(st->structSymbolId);
                            std::cerr << "[DEBUG Field] Found StructType symbol=" << st->structSymbolId << " name=" << sym.name.str() << "\n";
                            if (sym.kind == SymbolKind::Struct && sym.decl) {
                                auto* structDecl = static_cast<StructDeclNode*>(sym.decl);
                                bool fieldFound = false;
                                for (auto& field : structDecl->fields) {
                                    std::cerr << "[DEBUG Field] Checking struct field: " << field->name << "\n";
                                    if (field->name == c.name) {
                                        fieldFound = true;
                                        if (field->symbolId != kInvalidSymbolID) {
                                            const Type* fTy = typeTable[field->symbolId];
                                            std::unordered_map<SymbolID, const Type*> subst;
                                            for (size_t j = 0; j < structDecl->genericParams.size(); ++j) {
                                                if (j < st->genericArgs.size()) {
                                                    subst[structDecl->genericParams[j].symbolId] = st->genericArgs[j];
                                                }
                                            }
                                            fTy = ctx.substitute(fTy, subst);
                                            unify(c.actual, fTy, c.loc);
                                            solved[i] = true;
                                            changed = true;
                                            std::cerr << "[DEBUG Field] Solved field constraint!\n";
                                        } else {
                                            std::cerr << "[DEBUG Field] Field symbolId is Invalid!\n";
                                        }
                                    }
                                }
                                if (!fieldFound) {
                                    std::cerr << "[DEBUG Field] Field " << c.name << " not found in struct " << sym.name.str() << "\n";
                                }
                            } else {
                                std::cerr << "[DEBUG Field] Struct symbol is not Struct or decl is null!\n";
                            }
                        }
                    } else if (c.kind == ConstraintKind::MethodCall) {
                        const Type* objType = ctx.unificationTable.deepResolve(c.expected, ctx);
                        if (objType->getKind() == TypeKind::InferenceVar) {
                            // Wait for the object type to be inferred
                        } else {
                            if (auto* ptr = dynamic_cast<const PointerType*>(objType->unwrapAlias())) {
                                if (c.name == "add" || c.name == "sub" || c.name == "offset") {
                                    if (unify(c.actual, ptr, c.loc)) changed = true;
                                    if (c.callArgs.size() > 0) {
                                        if (unify(c.callArgs[0], ctx.getPrimitive(BuiltinKind::I32), c.loc)) changed = true;
                                    }
                                    solved[i] = true;
                                    continue;
                                }
                            }
                            MethodInfo mInfo;
                            if (methodResolver.probe(objType, c.name, mInfo, traitSolver, ctx, table, typeTable, c.callerModuleID, &diag, c.loc)) {
                                // 1. Construct the synthetic call site function type.
                                // The receiver is inherently positional and prepended to the user's call args.
                                std::vector<std::string> callArgNames;
                                callArgNames.push_back(""); // self has no label at call site (it's the receiver)
                                for (auto& name : c.callArgNames) callArgNames.push_back(name);
                                
                                const Type* adjustedObjType = objType;
                                if (auto* mFnTy = dynamic_cast<const FunctionType*>(mInfo.type)) {
                                    if (!mFnTy->paramTypes.empty()) {
                                        const Type* expectedSelfTy = mFnTy->paramTypes[0];
                                        if (expectedSelfTy->getKind() == TypeKind::Reference && adjustedObjType->getKind() != TypeKind::Reference) {
                                            bool isMutable = static_cast<const ReferenceType*>(expectedSelfTy)->isMutable;
                                            adjustedObjType = ctx.getReferenceType(adjustedObjType, isMutable);
                                        } else if (expectedSelfTy->getKind() != TypeKind::Reference && adjustedObjType->getKind() == TypeKind::Reference) {
                                            adjustedObjType = static_cast<const ReferenceType*>(adjustedObjType)->pointee;
                                        }
                                    }
                                }
                                
                                std::vector<const Type*> callArgTypes;
                                callArgTypes.push_back(adjustedObjType); // Receiver type (to be checked against param 0)
                                for (auto& argTy : c.callArgs) callArgTypes.push_back(argTy);
                                
                                const Type* expectedFnType = ctx.getFunctionType(std::move(callArgNames), std::move(callArgTypes), c.actual, true /* isCallSite */);
                                
                                std::cerr << "[DEBUG MethodCall] unifying expectedFnType: " << expectedFnType->toString() << " with mInfo.type: " << mInfo.type->toString() << std::endl;
                                
                                // 2. Unify the synthetic call site with the method's definition signature
                                unify(expectedFnType, mInfo.type, c.loc);
                                solved[i] = true;
                                changed = true;
                            } else {
                                diag.error(c.loc, "No method named '" + c.name + "' found for type '" + objType->toString() + "'");
                                solved[i] = true;
                                changed = true; // Error reported, don't loop forever
                            }
                        }
                    } else if (c.kind == ConstraintKind::Iterable) {
                        const Type* t1 = ctx.unificationTable.deepResolve(c.expected, ctx); // The iterable
                        const Type* t2 = ctx.unificationTable.deepResolve(c.actual, ctx);   // The element type (inference var)
                        
                        if (t1->getKind() == TypeKind::InferenceVar) {
                            // Wait for iterable to be inferred
                        } else if (auto* arr = dynamic_cast<const ArrayType*>(t1)) {
                            unify(arr->elementType, t2, c.loc);
                            solved[i] = true;
                            changed = true;
                        } else if (auto* sl = dynamic_cast<const SliceType*>(t1)) {
                            unify(sl->elementType, t2, c.loc);
                            solved[i] = true;
                            changed = true;
                        } else if (t1->getKind() != TypeKind::Unknown) {
                            diag.error(c.loc, "Type '" + t1->toString() + "' is not iterable");
                            solved[i] = true;
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
                        solved[i] = true;
                    } else if (c.kind == ConstraintKind::Index) {
                        const Type* baseTy = ctx.unificationTable.deepResolve(c.expected, ctx);
                        const Type* idxTy = ctx.unificationTable.deepResolve(c.actual, ctx);
                        if (baseTy->getKind() == TypeKind::InferenceVar || idxTy->getKind() == TypeKind::InferenceVar) {
                            // Wait
                        } else {
                            ValueCategory vc = (ValueCategory)std::stoi(c.name);
                            if (auto* arr = dynamic_cast<const ArrayType*>(baseTy->unwrapAlias())) {
                                if (unify(c.callArgs[0], arr->elementType, c.loc)) changed = true;
                                solved[i] = true;
                            } else if (auto* sl = dynamic_cast<const SliceType*>(baseTy->unwrapAlias())) {
                                if (unify(c.callArgs[0], sl->elementType, c.loc)) changed = true;
                                solved[i] = true;
                            } else {
                                auto traitInfo = OperatorRegistry::getIndexOperatorTrait(vc == ValueCategory::LValue);
                                Constraint methC(ConstraintKind::MethodCall, baseTy, c.callArgs[0], traitInfo.methodName, {idxTy}, {""}, c.loc);
                                methC.callerModuleID = c.callerModuleID;
                                constraints.push_back(methC);
                                solved[i] = true;
                                changed = true;
                            }
                        }
                    } else if (c.kind == ConstraintKind::BinaryOperator) {
                        const Type* leftTy = ctx.unificationTable.deepResolve(c.callArgs[0], ctx);
                        const Type* rightTy = ctx.unificationTable.deepResolve(c.actual, ctx);
                        if (leftTy->getKind() == TypeKind::InferenceVar || rightTy->getKind() == TypeKind::InferenceVar) {
                            // Wait
                        } else {
                            BinaryOp op = (BinaryOp)std::stoi(c.name);
                            TokenType tOp = mapBinaryOpToTokenType(op);
                            if (leftTy->getKind() == TypeKind::Primitive && rightTy->getKind() == TypeKind::Primitive) {
                                bool isComp = (op == BinaryOp::Eq || op == BinaryOp::Ne || op == BinaryOp::Lt || op == BinaryOp::Le || op == BinaryOp::Gt || op == BinaryOp::Ge);
                                if (isComp) {
                                    if (unify(c.expected, ctx.getPrimitive(BuiltinKind::Bool), c.loc)) changed = true;
                                } else {
                                    if (unify(c.expected, leftTy, c.loc)) changed = true;
                                }
                                solved[i] = true;
                            } else {
                                auto traitInfo = OperatorRegistry::getBinaryOperatorTrait(tOp);
                                if (traitInfo) {
                                    Constraint methC(ConstraintKind::MethodCall, leftTy, c.callArgs[0], traitInfo->methodName, {rightTy}, {""}, c.loc);
                                    methC.callerModuleID = c.callerModuleID;
                                    constraints.push_back(methC);
                                } else {
                                    diag.error(c.loc, "Operator not supported for type " + leftTy->toString());
                                }
                                solved[i] = true;
                                changed = true;
                            }
                        }
                    } else if (c.kind == ConstraintKind::UnaryOperator) {
                        const Type* opTy = ctx.unificationTable.deepResolve(c.actual, ctx);
                        if (opTy->getKind() == TypeKind::InferenceVar) {
                            // Wait
                        } else {
                            UnaryOp op = (UnaryOp)std::stoi(c.name);
                            TokenType tOp = mapUnaryOpToTokenType(op);
                            if (opTy->getKind() == TypeKind::Primitive) {
                                if (op == UnaryOp::Not) {
                                    if (unify(c.expected, ctx.getPrimitive(BuiltinKind::Bool), c.loc)) changed = true;
                                    if (unify(c.actual, ctx.getPrimitive(BuiltinKind::Bool), c.loc)) changed = true;
                                } else {
                                    if (unify(c.expected, opTy, c.loc)) changed = true;
                                    if (unify(c.actual, opTy, c.loc)) changed = true;
                                }
                                solved[i] = true;
                            } else {
                                auto traitInfo = OperatorRegistry::getUnaryOperatorTrait(tOp);
                                if (traitInfo) {
                                    Constraint methC(ConstraintKind::MethodCall, opTy, c.actual, traitInfo->methodName, {}, {}, c.loc);
                                    methC.callerModuleID = c.callerModuleID;
                                    constraints.push_back(methC);
                                } else {
                                    diag.error(c.loc, "Unary operator not supported for type " + opTy->toString());
                                }
                                solved[i] = true;
                                changed = true;
                            }
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
          TypeTableRef typeTable;
          MonomorphizationEngine* monoEngine;
          MethodResolver& methodResolver;
          TraitSolver& traitSolver;
          const Type* currentReturnType = nullptr;
          bool isUnsafeContext_ = false;
          bool isAsyncContext_ = false;
      public:
          ModuleID currentModuleID = 0;
          TypeResolver(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, TypeTableRef typeTable, MonomorphizationEngine* monoEngine, MethodResolver& methodResolver, TraitSolver& traitSolver) 
              : table(table), diag(diag), ctx(ctx), typeTable(typeTable), monoEngine(monoEngine), methodResolver(methodResolver), traitSolver(traitSolver) {}
              
          class PatternResolverVisitor : public PatternVisitor {
              TypeResolver& resolver;
          public:
              PatternResolverVisitor(TypeResolver& r) : resolver(r) {}
              void visit(StructPatternNode& node) override { 
                  resolver.resolve(node.inferredType, node.loc);
                  for (auto& f : node.fields) if (f.pattern) f.pattern->accept(*this);
              }
              void visit(WildcardPatternNode& node) override { resolver.resolve(node.inferredType, node.loc); }
              void visit(LiteralPatternNode& node) override { resolver.resolve(node.inferredType, node.loc); }
                void visit(EnumPatternNode& node) override {
                    resolver.resolve(node.inferredType, node.loc);
                    if (node.variantSymbolId != kInvalidSymbolID) {
                        const auto& origSym = resolver.table.getSymbol(node.variantSymbolId);
                        if (auto* resolvedEnum = dynamic_cast<const EnumType*>(node.inferredType)) {
                            const auto& enumSym = resolver.table.getSymbol(resolvedEnum->enumSymbolId);
                            if (auto* enumDecl = dynamic_cast<const EnumDeclNode*>(enumSym.decl)) {
                                for (auto& var : enumDecl->variants) {
                                    if (var->name == origSym.name.view()) {
                                        node.variantSymbolId = var->symbolId;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    for (auto& f : node.fields) if (f) f->accept(*this);
                }
                void visit(IdentifierPatternNode& node) override { 
                    resolver.resolve(node.inferredType, node.loc);
                    if (node.symbolId != kInvalidSymbolID) {
                        resolver.typeTable[node.symbolId] = node.inferredType;
                    }
                }
              void visit(TuplePatternNode& node) override {
                  resolver.resolve(node.inferredType, node.loc);
                  for (auto& e : node.elements) if (e) e->accept(*this);
              }

          };

        void resolve(const Type*& t, SourceLocation loc) {
        if (!t) return;
        t = ctx.unificationTable.deepResolve(t, ctx);
        
        if (t->getKind() == TypeKind::InferenceVar) {
            std::cerr << "[DEBUG] InferenceVar at loc: " << loc.line << ":" << loc.column << " type: " << t->toString() << "\n";
            diag.error(loc, "Type annotation needed");
        }
        
        if (auto* proj = dynamic_cast<const AssociatedTypeProjection*>(t)) {
            if (const Type* res = traitSolver.resolveProjection(proj)) {
                t = res;
            }
        }
        
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
                            // Check object safety!
                            bool isSafe = true;
                            if (auto* traitObjTy = dynamic_cast<const TraitObjectType*>(refExpected->pointee)) {
                                for (auto id : traitObjTy->traitIds) {
                                    Goal goal;
                                    goal.kind = GoalKind::ObjectSafety;
                                    goal.traitId = id;
                                    if (traitSolver.solve(goal).result != SolverResult::Success) {
                                        isSafe = false;
                                    }
                                }
                            }
                            if (!isSafe) {
                                exprPtr->inferredType = ctx.getUnknown();
                                return;
                            }
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

        void visit(ProgramNode& node) override { std::cerr << "[DEBUG] CG visiting ProgramNode\n"; for (auto& item : node.items) { std::cerr << "[DEBUG] CG visiting item: " << typeid(*item).name() << "\n"; item->accept(*this); } }
        void visit(ModDeclNode& node) override {
            ModuleID oldModuleID = currentModuleID;
            if (node.bodyScopeId != kInvalidSymbolID) {
                currentModuleID = node.bodyScopeId;
            }
            for (auto& d : node.decls) d->accept(*this);
            currentModuleID = oldModuleID;
        }
        void visit(ExternDeclNode& node) override { if (node.func) node.func->accept(*this); }
        void visit(VarDeclNode& node) override {
            if (node.initializer) {
                node.initializer->accept(*this);
                if (node.symbolId != kInvalidSymbolID) {
                    const Type* varTy = typeTable[node.symbolId]; 
                    varTy = ctx.unificationTable.deepResolve(varTy, ctx);
                    coerce(node.initializer, varTy);
                }
            }
            if (node.pattern) {
                PatternResolverVisitor patRes(*this);
                node.pattern->accept(patRes);
            }
        }
        void visit(ParamDeclNode& node) override {}
        void visit(FunctionDeclNode& node) override { 
            if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[node.symbolId])) {
                bool changed = false;
                const Type* resolvedRet = fnTy->returnType;
                resolve(resolvedRet, node.loc);
                if (resolvedRet != fnTy->returnType) changed = true;

                std::vector<const Type*> resolvedParams = fnTy->paramTypes;
                for (size_t i = 0; i < resolvedParams.size(); ++i) {
                    const Type* pTy = resolvedParams[i];
                    resolve(pTy, node.loc);
                    if (pTy != resolvedParams[i]) {
                        resolvedParams[i] = pTy;
                        changed = true;
                    }
                }

                if (changed) {
                    typeTable[node.symbolId] = ctx.getFunctionType(
                        std::vector<std::string>(fnTy->paramNames),
                        resolvedParams,
                        resolvedRet,
                        fnTy->isVariadic
                    );
                }
            }

            if (node.body) {
                const Type* oldRet = currentReturnType;
                if (auto* fnTy = dynamic_cast<const FunctionType*>(typeTable[node.symbolId])) {
                    currentReturnType = fnTy->returnType;
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

            // For ForEach (for-in), resolve iteration protocol after all types are finalized.
            // Array/Slice paths are handled in MVIRGenerator directly.
            // For struct/custom types, probe `iter()` and `next()` via MethodResolver.
            if (node.kind == ForKind::ForEach && node.iterable) {
                const Type* iterTy = ctx.unificationTable.deepResolve(node.iterable->inferredType, ctx);
                // Skip intrinsic paths — they don't need iterMethodId/nextMethodId
                if (!dynamic_cast<const ArrayType*>(iterTy) && !dynamic_cast<const SliceType*>(iterTy)) {
                    // Try to probe iter() first (IntoIterator pattern)
                    MethodInfo iterInfo;
                    if (methodResolver.probe(iterTy, "iter", iterInfo, traitSolver, ctx, table, typeTable, 0, &diag, node.loc)) {
                        node.iterMethodId = iterInfo.id;
                        // The return type of iter() is the iterator type
                        const Type* iteratorTy = iterInfo.type ? iterInfo.type->returnType : nullptr;
                        if (iteratorTy) {
                            MethodInfo nextInfo;
                            if (methodResolver.probe(iteratorTy, "next", nextInfo, traitSolver, ctx, table, typeTable, 0, &diag, node.loc)) {
                                node.nextMethodId = nextInfo.id;
                            } else {
                                diag.error(node.loc, "Iterator type returned by iter() does not implement next()");
                            }
                        }
                    } else {
                        // Try direct next() (Iterator pattern — struct is already an iterator)
                        MethodInfo nextInfo;
                        if (methodResolver.probe(iterTy, "next", nextInfo, traitSolver, ctx, table, typeTable, 0, &diag, node.loc)) {
                            node.nextMethodId = nextInfo.id;
                            // iterMethodId stays kInvalidSymbolID — struct is iterated directly
                        } else if (iterTy->getKind() != TypeKind::Unknown && iterTy->getKind() != TypeKind::InferenceVar) {
                            diag.error(node.loc, "Type '" + iterTy->toString() + "' is not iterable: no iter() or next() method found");
                        }
                    }
                }
            }
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
        void visit(IdentifierExpr& node) override {
            resolve(node.inferredType, node.loc);
            if (node.resolvedSymbol == kInvalidSymbolID && node.overloadCandidates.size() == 1) {
                node.resolvedSymbol = node.overloadCandidates[0];
            }
        }
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
                const Type* resolvedRet = fnTy->returnType;
                resolve(resolvedRet, node.loc);
                if (resolvedRet != fnTy->returnType) {
                    node.callee->inferredType = ctx.getFunctionType(
                        std::vector<std::string>(fnTy->paramNames),
                        std::vector<const Type*>(fnTy->paramTypes),
                        resolvedRet,
                        fnTy->isVariadic
                    );
                    fnTy = static_cast<const FunctionType*>(node.callee->inferredType);
                }
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
            const Type* resolvedObjTy = ctx.unificationTable.deepResolve(node.object->inferredType, ctx);
            
            if (auto* ptrTy = dynamic_cast<const PointerType*>(resolvedObjTy->unwrapAlias())) {
                if (node.methodName == "add" || node.methodName == "sub" || node.methodName == "offset") {
                    if (!isUnsafeContext_) {
                        diag.error(node.loc, "Pointer arithmetic requires an unsafe block.");
                    }
                    if (node.args.size() != 1) {
                        diag.error(node.loc, "Pointer arithmetic takes exactly 1 argument.");
                    } else {
                        coerce(node.args[0].value, ctx.getPrimitive(BuiltinKind::I32));
                    }
                    if (node.methodName == "add") node.intrinsic = IntrinsicKind::PtrAdd;
                    else if (node.methodName == "sub") node.intrinsic = IntrinsicKind::PtrSub;
                    else node.intrinsic = IntrinsicKind::PtrOffset;
                    return;
                }
            }
            
            const Type* objTy = resolvedObjTy->unwrapAlias();
            if (auto* refTy = dynamic_cast<const ReferenceType*>(objTy)) objTy = refTy->pointee->unwrapAlias();
            else if (auto* ptrTy = dynamic_cast<const PointerType*>(objTy)) objTy = ptrTy->pointee->unwrapAlias();
            
            if (methodResolver.probe(objTy, std::string(node.methodName), mInfo, traitSolver, ctx, table, typeTable, currentModuleID, &diag, node.loc)) {
                node.resolvedFn = mInfo.id;
                
                // Check visibility
                const auto& sym = table.getSymbol(mInfo.id);
                if ((sym.isExternal || sym.moduleID != currentModuleID) && sym.visibility != Visibility::Public) {
                    diag.error(node.loc, "Method '" + std::string(node.methodName) + "' is private.");
                }

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
                            if ((sym.isExternal || sym.moduleID != currentModuleID) && structDecl->fields[i]->visibility != Visibility::Public) {
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
            } else if (objTy->getKind() != TypeKind::Unknown && objTy->getKind() != TypeKind::InferenceVar) {
                diag.error(node.loc, "Cannot index into a non-tuple type");
                node.inferredType = ctx.getUnknown();
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
                if (sym.kind == SymbolKind::Struct && sym.decl) {
                    auto* structDecl = static_cast<const StructDeclNode*>(sym.decl);
                    if (!structDecl->genericParams.empty() && monoEngine) {
                        if (auto* stTy = dynamic_cast<const StructType*>(node.inferredType)) {
                            bool allConcrete = true;
                            for (auto* a : stTy->genericArgs) {
                                if (a->getKind() == TypeKind::InferenceVar || dynamic_cast<const GenericParamType*>(a)) {
                                    allConcrete = false; break;
                                }
                            }
                            if (allConcrete) {
                                SymbolID specId = monoEngine->requestStructSpecialization(structDecl, stTy->genericArgs, node.loc);
                                if (specId != kInvalidSymbolID) {
                                    node.structId = specId;
                                }
                            }
                        }
                    }
                    
                    if (sym.isExternal) {
                        for (auto& field : structDecl->fields) {
                            if (field->visibility != fl::Visibility::Public) {
                                diag.error(node.loc, "Struct '" + sym.name.str() + "' has private field '" + std::string(field->name) + "' and cannot be initialized directly.");
                                break;
                            }
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
        void visit(TryExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(LambdaExpr& node) override {
            if (node.body) node.body->accept(*this);
            auto* closureTy = const_cast<ClosureType*>(dynamic_cast<const ClosureType*>(node.inferredType));
            if (!closureTy) return;
            struct Usage { bool read=false, mutate=false, consume=false; };
            std::unordered_map<SymbolID, Usage> usage;
            std::function<void(ASTNode*, bool)> analyzeUsage = [&](ASTNode* n, bool isMutateCtx) {
                if (!n) return;
                if (auto* id = dynamic_cast<IdentifierExpr*>(n)) {
                    usage[id->resolvedSymbol].read = true;
                    if (isMutateCtx) usage[id->resolvedSymbol].mutate = true;
                } else if (auto* call = dynamic_cast<CallExpr*>(n)) {
                    if (auto* calleeId = dynamic_cast<IdentifierExpr*>(call->callee.get())) {
                        if (table.getSymbol(calleeId->resolvedSymbol).name == "consume") {
                            if (call->args.size() == 1) {
                                if (auto* argId = dynamic_cast<IdentifierExpr*>(call->args[0].value.get())) usage[argId->resolvedSymbol].consume = true;
                            }
                        }
                    }
                    analyzeUsage(call->callee.get(), false);
                    for (auto& arg : call->args) if (arg.value) analyzeUsage(arg.value.get(), false);
                } else if (auto* assign = dynamic_cast<AssignExpr*>(n)) {
                    analyzeUsage(assign->lvalue.get(), true);
                    analyzeUsage(assign->value.get(), false);
                } else if (auto* bin = dynamic_cast<BinaryExpr*>(n)) {
                    analyzeUsage(bin->left.get(), false);
                    analyzeUsage(bin->right.get(), false);
                } else if (auto* block = dynamic_cast<BlockStmtNode*>(n)) {
                    for (auto& s : block->body) analyzeUsage(s.get(), false);
                    if (block->tailExpr) analyzeUsage(block->tailExpr.get(), false);
                } else if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(n)) {
                    if (exprStmt->expr) analyzeUsage(exprStmt->expr.get(), false);
                } else if (auto* decl = dynamic_cast<VarDeclNode*>(n)) {
                    if (decl->initializer) analyzeUsage(decl->initializer.get(), false);
                } else if (auto* fa = dynamic_cast<MemberExpr*>(n)) {
                    analyzeUsage(fa->object.get(), isMutateCtx);
                }
            };
            if (node.body) analyzeUsage(node.body.get(), false);
            auto isTypeCopyable = [](const Type* t) {
                if (!t) return false;
                auto k = t->getKind();
                return k == TypeKind::Primitive || k == TypeKind::Pointer || k == TypeKind::Function || k == TypeKind::Tuple || k == TypeKind::Unknown;
            };
            closureTy->captures.clear();
            closureTy->fieldTypes.clear();
            closureTy->fieldTypes.push_back(ctx.getPointerType(closureTy->signature, false));
            for (auto& capRef : node.captures) {
                SymbolID capId = capRef.symbolId;
                const Type* capTy = typeTable[capId];
                if (!capTy) capTy = ctx.getUnknown();
                CaptureMode mode = CaptureMode::Borrow;
                auto& u = usage[capId];
                if (node.isMove) {
                    mode = isTypeCopyable(capTy) ? CaptureMode::Copy : CaptureMode::Move;
                } else {
                    if (u.consume) mode = CaptureMode::Move;
                    else if (u.mutate) mode = CaptureMode::BorrowMut;
                    else if (isTypeCopyable(capTy) && u.read && !u.mutate && !u.consume) mode = CaptureMode::Copy;
                    else mode = CaptureMode::Borrow;
                }
                const Type* envType = capTy;
                if (mode == CaptureMode::Borrow) envType = ctx.getPointerType(capTy, false);
                else if (mode == CaptureMode::BorrowMut) envType = ctx.getPointerType(capTy, true);
                closureTy->captures.push_back(CaptureInfo{capId, mode, CaptureSource::Direct, capTy, envType});
                closureTy->fieldTypes.push_back(envType);
            }
        }
        void visit(AwaitExpr& node) override {
            if (node.expr) node.expr->accept(*this);
            resolve(node.inferredType, node.loc);
        }
        void visit(SizeofExpr& node) override {}
        void visit(AlignofExpr& node) override {}
    };

    TypePrePass pre(table_, ctx_, TypeTableRef(typeTable_, table_, ctx_), methodResolver_, traitSolver_, monoEngine_, diag_);
    root->accept(pre);

    std::vector<Constraint> constraints;
    std::cerr << "[DEBUG] Starting ConstraintGenerator\n";
    ConstraintGenerator gen(table_, diag_, ctx_, TypeTableRef(typeTable_, table_, ctx_), constraints, methodResolver_, traitSolver_, monoEngine_, metadataCache_);
    root->accept(gen);

    std::cerr << "[DEBUG] Starting UnificationEngine\n";
    UnificationEngine solver(table_, diag_, ctx_, TypeTableRef(typeTable_, table_, ctx_), methodResolver_, traitSolver_);
    solver.solve(constraints);

    for (auto& t : typeTable_) {
        if (t) t = ctx_.unificationTable.deepResolve(t, ctx_);
    }

    std::cerr << "[DEBUG] Starting TypeResolver\n";
    TypeResolver resolver(table_, diag_, ctx_, TypeTableRef(typeTable_, table_, ctx_), monoEngine_, methodResolver_, traitSolver_);
    root->accept(resolver);

    return !diag_.hasErrors();
}


const Type* TypeChecker::typeOf(SymbolID sym) const {
    if (sym == kInvalidSymbolID) {
        std::cerr << "[DEBUG TypeChecker] typeOf called with kInvalidSymbolID!" << std::endl;
        return nullptr;
    }
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

void TypeChecker::registerImpl(const ImplDeclNode* implNode) {}

SolverResult TypeChecker::implementsTrait(const Type* type, SymbolID traitId, const std::vector<const Type*>& genericArgs) const {
    std::cerr << "[DEBUG implementsTrait] type=" << (type ? type->toString() : "null") << " traitId=" << traitId << std::endl;
    if (!type || traitId == kInvalidSymbolID) return SolverResult::Failure;
    Goal goal;
    goal.kind = GoalKind::Trait;
    goal.selfType = type;
    goal.traitId = traitId;
    goal.genericArgs = genericArgs;
    auto res = traitSolver_.solve(goal);
    std::cerr << "[DEBUG implementsTrait] solve result=" << (int)res.result << std::endl;
    return res.result;
}

} // namespace fl
