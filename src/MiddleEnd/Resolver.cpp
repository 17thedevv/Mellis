#include <iostream>
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/MiddleEnd/ScopeStack.h"
#include "mellis/FrontEnd/ASTVisitor.h"
#include <cassert>

namespace fl {

// =============================================================================
// Helper: ScopeManager
// =============================================================================
class ScopeManager {
public:
    ScopeManager(SymbolTable& table, DiagnosticEngine& diag)
        : table(table), diag(diag) {}

    void enterScope(ScopeKind kind) { 
        ScopeID parentId = scopeStack.current();
        ScopeID newScope = table.createScope(kind, parentId);
        scopeStack.push(newScope);
    }

    void enterExistingScope(ScopeID scopeId) { 
        scopeStack.push(scopeId);
    }

    void exitScope() { 
        scopeStack.pop();
    }

    SymbolID declare(std::string_view name, SymbolKind kind, SourceLocation loc, ASTNode* declNode) {
        Identifier id(name);
        ScopeID currentScope = scopeStack.current();

        if (table.containsInScope(id, currentScope)) {
            diag.error(loc, "Redeclaration of name '" + std::string(name) + "' in this scope.");
            return kInvalidSymbolID;
        }

        SymbolID symId = table.declareSymbol(id, kind, currentScope, loc, declNode);
        if (declNode) {
            if (auto* d = dynamic_cast<DeclNode*>(declNode)) {
                table.getMutableSymbol(symId).isExported = d->isExported;
            }
        }
        return symId;
    }

    SymbolID resolve(std::string_view name, SourceLocation loc) {
        Identifier id(name);
        ScopeID currentScope = scopeStack.current();

        auto optSym = table.lookup(id, currentScope);
        if (!optSym) {
            diag.error(loc, "Use of undeclared name '" + std::string(name) + "'.");
            return kInvalidSymbolID;
        }

        return *optSym;
    }

    
    SymbolID resolvePath(const std::vector<std::string_view>& path, SourceLocation loc) {
        if (path.empty()) return kInvalidSymbolID;
        SymbolID currentSym = resolve(path[0], loc);
        if (currentSym == kInvalidSymbolID) return currentSym;

        for (size_t i = 1; i < path.size(); ++i) {
            auto& sym = table.getSymbol(currentSym);
            if (sym.kind != SymbolKind::Module && sym.kind != SymbolKind::Enum) {
                diag.error(loc, "Symbol '" + std::string(path[i-1]) + "' is not a module or enum.");
                return kInvalidSymbolID;
            }
            ScopeID nextScope = kInvalidScopeID;
            if (sym.kind == SymbolKind::Module) {
                nextScope = static_cast<ModDeclNode*>(sym.decl)->bodyScopeId;
            } else if (sym.kind == SymbolKind::Enum) {
                nextScope = static_cast<EnumDeclNode*>(sym.decl)->bodyScopeId;
            }
            auto optNext = table.lookupInScope(Identifier(path[i]), nextScope);
            if (!optNext) {
                diag.error(loc, "Module/Enum '" + std::string(path[i-1]) + "' does not contain '" + std::string(path[i]) + "'.");
                return kInvalidSymbolID;
            }
            auto& nextSym = table.getSymbol(*optNext);
            if (sym.kind == SymbolKind::Module && !nextSym.isExported) {
                diag.error(loc, "Symbol '" + std::string(path[i]) + "' is private.");
                return kInvalidSymbolID;
            }
            currentSym = *optNext;
        }
        return currentSym;
    }

    SymbolTable& table;
    DiagnosticEngine& diag;
    ScopeStack scopeStack;
};

// =============================================================================
// Pass 1: Declaration Visitor
// Only visits declarations and creates symbols/scopes. Does not resolve expressions.
// =============================================================================
class DeclarationVisitor : public ASTVisitor {
    ScopeManager& sm;

public:
    DeclarationVisitor(ScopeManager& sm) : sm(sm) {}

    void visit(ProgramNode& node) override { 
        sm.enterExistingScope(sm.table.globalScopeId());
        for (auto& item : node.items) {
            item->accept(*this);
        }
        sm.exitScope();
    }

    // --- Declarations ---
    void visit(VarDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Variable, node.loc, &node);
        // We do not visit the initializer in Pass 1
    }

    void visit(FunctionDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Function, node.loc, &node);
        
        node.bodyScopeId = sm.table.createScope(ScopeKind::Function, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        for (auto& p : node.params) {
            p->accept(*this);
        }
        
        // Do not visit body block yet
        sm.exitScope();
    }

    void visit(ParamDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Parameter, node.loc, &node);
    }

    void visit(StructDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Struct, node.loc, &node);
        node.bodyScopeId = sm.table.createScope(ScopeKind::Struct, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        for (auto& field : node.fields) {
            field->symbolId = sm.declare(field->name, SymbolKind::StructField, field->loc, field.get());
            // Do not visit type yet
        }
        sm.exitScope();
    }

    void visit(StructFieldNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::StructField, node.loc, &node);
    }

    void visit(EnumDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Enum, node.loc, &node);
        
        node.bodyScopeId = sm.table.createScope(ScopeKind::Enum, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        for (auto& v : node.variants) {
            v->accept(*this);
        }

        sm.exitScope();
    }

    void visit(EnumVariantNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::EnumVariant, node.loc, &node);
        // Do not visit tupleTypes in Pass 1, only declare the symbol
    }

    void visit(TraitDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Trait, node.loc, &node);
        node.bodyScopeId = sm.table.createScope(ScopeKind::Trait, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        sm.declare("Self", SymbolKind::GenericParam, node.loc, nullptr);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        for (auto& m : node.methods) {
            m->accept(*this);
        }

        sm.exitScope();
    }

    void visit(ImplDeclNode& node) override { 
        // Impl block itself is not a named symbol, but its contents are scoped
        node.bodyScopeId = sm.table.createScope(ScopeKind::Impl, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        sm.declare("Self", SymbolKind::TypeAlias, node.loc, nullptr);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        for (auto& m : node.methods) {
            m->accept(*this);
        }

        sm.exitScope();
    }

    void visit(ModDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Module, node.loc, &node);
        if (node.symbolId != kInvalidSymbolID) {
            sm.table.getMutableSymbol(node.symbolId).isExported = node.isExported;
        }
        // We will process inner items in Pass 1 if it's an inline mod or out-of-line mod
        if (!node.decls.empty() || node.isOutlined) {
            node.bodyScopeId = sm.table.createScope(ScopeKind::Module, sm.scopeStack.current());
            sm.enterExistingScope(node.bodyScopeId);
            for (auto& i : node.decls) {
                i->accept(*this);
            }
            sm.exitScope();
        }
    }

    void visit(UseDeclNode& node) override { 
        // Handled in Pass 2 or 3 usually.
    }

    void visit(ExternDeclNode& node) override { 
        if (node.func) node.func->accept(*this);
    }

    void visit(TypeAliasDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::TypeAlias, node.loc, &node);
    }

    // --- Statements --- (We only visit statements that can contain declarations)
    void visit(BlockStmtNode& node) override { 
        node.bodyScopeId = sm.table.createScope(ScopeKind::Block, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& stmt : node.body) {
            stmt->accept(*this);
        }
        sm.exitScope();
    }
    
    void visit(IfStmtNode& node) override { 
        node.thenBranch->accept(*this);
        if (node.elseBranch) node.elseBranch->accept(*this);
    }
    
    void visit(WhileStmtNode& node) override { 
        node.body->accept(*this);
    }
    
    void visit(ForStmtNode& node) override { std::cout << "[Resolver] " << __FUNCTION__ << " line " << __LINE__ << "\n"; 
        if (node.bodyScopeId == kInvalidSymbolID) {
            node.bodyScopeId = sm.table.createScope(ScopeKind::Loop, sm.scopeStack.current());
        }
        if (node.kind == ForKind::CStyle) {
            sm.enterExistingScope(node.bodyScopeId);
            if (node.init) node.init->accept(static_cast<ASTVisitor&>(*this));
            if (node.cond) node.cond->accept(static_cast<ASTVisitor&>(*this));
            if (node.step) node.step->accept(static_cast<ASTVisitor&>(*this));
            if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
            sm.exitScope();
        } else {
            if (node.iterable) node.iterable->accept(static_cast<ASTVisitor&>(*this));
            sm.enterExistingScope(node.bodyScopeId);
            if (node.bindingId == kInvalidSymbolID) {
                node.bindingId = sm.declare(node.bindingName, SymbolKind::Variable, node.loc, &node);
            }
            if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
            sm.exitScope();
        }
    }
    
    void visit(UnsafeStmtNode& node) override { 
        if (node.body) node.body->accept(*this);
    }
    
    void visit(ComptimeStmtNode& node) override { 
        if (node.body) node.body->accept(*this);
    }
    
    void visit(ExprStmtNode& node) override { }
    void visit(ReturnStmtNode& node) override { }
    void visit(BreakStmtNode& node) override { }
    void visit(ContinueStmtNode& node) override { }

    // --- Expressions --- (Do nothing in Pass 1)
    void visit(LiteralExpr&) override { }
    void visit(IdentifierExpr&) override { }
    void visit(BinaryExpr&) override { }
    void visit(UnaryExpr&) override { }
    void visit(AssignExpr&) override { }
    void visit(CallExpr&) override { }
    void visit(MethodCallExpr&) override { }
    void visit(IndexExpr&) override { }
    void visit(MemberExpr&) override { }
    void visit(CastExpr&) override { }
    void visit(ArrayLiteralExpr&) override { }
    void visit(TupleLiteralExpr&) override { }
    void visit(StructInitExpr&) override { }
    void visit(MatchExpr&) override { }
    void visit(LambdaExpr&) override { }
    void visit(AwaitExpr&) override { }
    void visit(SizeofExpr&) override { }
    void visit(AlignofExpr&) override { }

};


// =============================================================================
// Pass 1.5: Use Resolution Visitor
// Resolves `use` paths into Alias symbols across modules.
// =============================================================================

class UseResolutionVisitor : public ASTVisitor {
    ScopeManager& sm;

    void processUseTree(const UseTreeNode& tree, const std::vector<std::string_view>& basePath) {
        std::vector<std::string_view> fullPath = basePath;
        fullPath.insert(fullPath.end(), tree.segments.begin(), tree.segments.end());

        if (tree.isGlob) {
            sm.diag.error(tree.loc, "Glob imports not yet supported in MVP.");
            return;
        }

        if (tree.children.empty()) {
            SymbolID target = sm.resolvePath(fullPath, tree.loc);
            if (target != kInvalidSymbolID) {
                std::string_view aliasName = tree.alias.empty() ? fullPath.back() : tree.alias;
                SymbolID aliasId = sm.declare(aliasName, SymbolKind::Alias, tree.loc, nullptr);
                if (aliasId != kInvalidSymbolID) {
                    sm.table.getMutableSymbol(aliasId).aliasTo = target;
                }
            }
        } else {
            for (const auto& child : tree.children) {
                processUseTree(child, fullPath);
            }
        }
    }

public:
    explicit UseResolutionVisitor(ScopeManager& sm) : sm(sm) {}

    void visit(ProgramNode& node) override {
        for (auto& item : node.items) item->accept(*this);
    }
    void visit(ModDeclNode& node) override {
        if (!node.decls.empty() || node.isOutlined) {
            sm.enterExistingScope(node.bodyScopeId);
            for (auto& i : node.decls) i->accept(*this);
            sm.exitScope();
        }
    }
    void visit(UseDeclNode& node) override {
        processUseTree(node.tree, {});
    }

    void visit(VarDeclNode&) override {}
    void visit(FunctionDeclNode&) override {}
    void visit(StructDeclNode&) override {}
    void visit(EnumDeclNode&) override {}
    void visit(TraitDeclNode&) override {}
    void visit(ImplDeclNode&) override {}
    void visit(ExternDeclNode&) override {}
    void visit(TypeAliasDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(StructFieldNode&) override {}
    void visit(ParamDeclNode&) override {}
    void visit(ExprStmtNode&) override {}
    void visit(BlockStmtNode&) override {}
    void visit(IfStmtNode&) override {}
    void visit(WhileStmtNode&) override {}
    void visit(ForStmtNode&) override {}
    void visit(ReturnStmtNode&) override {}
    void visit(BreakStmtNode&) override {}
    void visit(ContinueStmtNode&) override {}
    void visit(UnsafeStmtNode&) override {}
    void visit(ComptimeStmtNode&) override {}

    void visit(LiteralExpr&) override {}
    void visit(IdentifierExpr&) override {}
    void visit(BinaryExpr&) override {}
    void visit(UnaryExpr&) override {}
    void visit(AssignExpr&) override {}
    void visit(CallExpr&) override {}
    void visit(MethodCallExpr&) override {}
    void visit(IndexExpr&) override {}
    void visit(MemberExpr&) override {}
    void visit(CastExpr&) override {}
    void visit(ArrayLiteralExpr&) override {}
    void visit(TupleLiteralExpr&) override {}
    void visit(StructInitExpr&) override {}
    void visit(MatchExpr&) override {}
    void visit(LambdaExpr&) override {}
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}
};

// =============================================================================
// Pass 2: Resolution Visitor
// Resolves expressions, type annotations, and function bodies.
// =============================================================================
class ResolutionVisitor : public ASTVisitor, public TypeVisitor, public PatternVisitor {
    ScopeManager& sm;
    DiagnosticEngine& diag;

public:
    ResolutionVisitor(ScopeManager& sm, DiagnosticEngine& diag) : sm(sm), diag(diag) {}

    void visit(ProgramNode& node) override { 
        sm.enterExistingScope(sm.table.globalScopeId());
        for (auto& item : node.items) {
            item->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    // --- Declarations ---
    void visit(VarDeclNode& node) override { 
        // Local variables are declared sequentially in Pass 2
        // If it's a global variable, it was already declared in Pass 1, but declare() handles redeclaration.
        // Wait, if it's global, we don't want to re-declare it.
        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::Variable, node.loc, &node);
        }
        if (node.typeAnnot) node.typeAnnot->accept(static_cast<TypeVisitor&>(*this));
        if (node.initializer) node.initializer->accept(static_cast<ASTVisitor&>(*this));
    }

    void visit(FunctionDeclNode& node) override { 
        sm.enterExistingScope(node.bodyScopeId);
        
        for (auto& gp : node.genericParams) {
            for (auto& bound : gp.bounds) {
                bound->accept(static_cast<TypeVisitor&>(*this));
            }
        }
        
        if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
        
        for (auto& p : node.params) {
            p->accept(static_cast<ASTVisitor&>(*this));
        }
        
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
        sm.exitScope();
    }

    void visit(ParamDeclNode& node) override {
        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::Variable, node.loc, &node);
        }
        if (node.type) node.type->accept(static_cast<TypeVisitor&>(*this));
    }

    void visit(StructDeclNode& node) override { 
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& gp : node.genericParams) {
            for (auto& bound : gp.bounds) {
                bound->accept(static_cast<TypeVisitor&>(*this));
            }
        }
        for (auto& f : node.fields) {
            f->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(StructFieldNode& node) override { 
        if (node.type) node.type->accept(static_cast<TypeVisitor&>(*this));
    }

    void visit(EnumDeclNode& node) override { 
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& gp : node.genericParams) {
            for (auto& bound : gp.bounds) {
                bound->accept(static_cast<TypeVisitor&>(*this));
            }
        }
        for (auto& v : node.variants) {
            v->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(EnumVariantNode& node) override { 
        // tupleTypes not implemented in parser yet? Wait, EnumVariantNode has `fields`
        for (auto& f : node.fields) {
            if (f->type) f->type->accept(static_cast<TypeVisitor&>(*this));
        }
    }

    void visit(TraitDeclNode& node) override { 
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& gp : node.genericParams) {
            for (auto& bound : gp.bounds) {
                bound->accept(static_cast<TypeVisitor&>(*this));
            }
        }
        for (auto& m : node.methods) {
            m->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(ImplDeclNode& node) override { 
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& gp : node.genericParams) {
            for (auto& bound : gp.bounds) {
                bound->accept(static_cast<TypeVisitor&>(*this));
            }
        }
        node.selfType->accept(static_cast<TypeVisitor&>(*this));
        if (node.traitType) node.traitType->accept(static_cast<TypeVisitor&>(*this));
        
        for (auto& m : node.methods) {
            m->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(ModDeclNode& node) override { 
        if (!node.decls.empty() || node.isOutlined) {
            sm.enterExistingScope(node.bodyScopeId);
            for (auto& i : node.decls) {
                i->accept(static_cast<ASTVisitor&>(*this));
            }
            sm.exitScope();
        }
    }

    void visit(UseDeclNode& node) override { }
    void visit(ExternDeclNode& node) override { 
        if (node.func) node.func->accept(static_cast<ASTVisitor&>(*this));
    }
    void visit(TypeAliasDeclNode& node) override { 
        if (node.aliasedType) node.aliasedType->accept(static_cast<TypeVisitor&>(*this));
    }

    // --- Statements ---
    void visit(BlockStmtNode& node) override { 
        if (node.bodyScopeId == kInvalidSymbolID) {
            node.bodyScopeId = sm.table.createScope(ScopeKind::Block, sm.scopeStack.current());
        }
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& stmt : node.body) {
            stmt->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }
    
    void visit(ExprStmtNode& node) override { 
        if (node.expr) node.expr->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(IfStmtNode& node) override { 
        node.condition->accept(static_cast<ASTVisitor&>(*this));
        node.thenBranch->accept(static_cast<ASTVisitor&>(*this));
        if (node.elseBranch) node.elseBranch->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(WhileStmtNode& node) override { 
        node.condition->accept(static_cast<ASTVisitor&>(*this));
        node.body->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(ForStmtNode& node) override { 
        if (node.bodyScopeId == kInvalidSymbolID) {
            node.bodyScopeId = sm.table.createScope(ScopeKind::Loop, sm.scopeStack.current());
        }
        if (node.kind == ForKind::CStyle) {
            sm.enterExistingScope(node.bodyScopeId);
            if (node.init) node.init->accept(static_cast<ASTVisitor&>(*this));
            if (node.cond) node.cond->accept(static_cast<ASTVisitor&>(*this));
            if (node.step) node.step->accept(static_cast<ASTVisitor&>(*this));
            if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
            sm.exitScope();
        } else {
            if (node.iterable) node.iterable->accept(static_cast<ASTVisitor&>(*this));
            sm.enterExistingScope(node.bodyScopeId);
            if (node.bindingId == kInvalidSymbolID) {
                node.bindingId = sm.declare(node.bindingName, SymbolKind::Variable, node.loc, &node);
            }
            if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
            sm.exitScope();
        }
    }
    
    void visit(ReturnStmtNode& node) override { 
        if (node.value) node.value->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(BreakStmtNode& node) override { 
    }
    
    void visit(ContinueStmtNode& node) override { }
    
    void visit(UnsafeStmtNode& node) override { 
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(ComptimeStmtNode& node) override { 
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
    }

    // --- Expressions ---
    void visit(LiteralExpr&) override { }
    
    void visit(IdentifierExpr& node) override { 
        if (!node.segments.empty()) {
            SymbolID id = sm.resolvePath(node.segments, node.loc);
            while (id != kInvalidSymbolID) {
                auto& sym = sm.table.getSymbol(id);
                if (sym.kind == SymbolKind::Alias) {
                    id = sym.aliasTo;
                } else {
                    break;
                }
            }
            node.resolvedSymbol = id; std::cout << "[DEBUG] Resolved '" << node.segments.back() << "' to ID " << id << "\n";
        }
    }
    
    void visit(BinaryExpr& node) override { 
        node.left->accept(static_cast<ASTVisitor&>(*this));
        node.right->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(UnaryExpr& node) override { 
        node.operand->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(AssignExpr& node) override { 
        node.lvalue->accept(static_cast<ASTVisitor&>(*this));
        node.value->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(CallExpr& node) override { 
        node.callee->accept(static_cast<ASTVisitor&>(*this));
        for (auto& arg : node.args) {
            if (arg.value) arg.value->accept(static_cast<ASTVisitor&>(*this));
        }
    }
    
    void visit(MethodCallExpr& node) override { 
        node.object->accept(static_cast<ASTVisitor&>(*this));
        for (auto& arg : node.args) {
            if (arg.value) arg.value->accept(static_cast<ASTVisitor&>(*this));
        }
    }
    
    void visit(IndexExpr& node) override { 
        node.base->accept(static_cast<ASTVisitor&>(*this));
        node.index->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(MemberExpr& node) override { 
        node.object->accept(static_cast<ASTVisitor&>(*this));
        // node.member is resolved in TypeChecker (since it depends on object's type)
    }
    
    void visit(CastExpr& node) override { 
        node.expr->accept(static_cast<ASTVisitor&>(*this));
        node.targetType->accept(static_cast<TypeVisitor&>(*this));
    }
    
    void visit(ArrayLiteralExpr& node) override { 
        for (auto& e : node.elements) {
            e->accept(static_cast<ASTVisitor&>(*this));
        }
    }
    
    void visit(TupleLiteralExpr& node) override { 
        for (auto& e : node.elements) {
            e->accept(static_cast<ASTVisitor&>(*this));
        }
    }
    
    void visit(StructInitExpr& node) override { 
        if (!node.path.empty()) {
            node.structId = sm.resolve(node.path[0], node.loc);
        }
        for (auto& f : node.fields) {
            if (f.value) f.value->accept(static_cast<ASTVisitor&>(*this));
        }
    }
    
    void visit(MatchExpr& node) override { 
        node.subject->accept(static_cast<ASTVisitor&>(*this));
        for (auto& arm : node.arms) {
            sm.enterScope(ScopeKind::Block);
            if (arm.pattern) arm.pattern->accept(static_cast<PatternVisitor&>(*this)); // Bind pattern vars
            if (arm.body) arm.body->accept(static_cast<ASTVisitor&>(*this));
            sm.exitScope();
        }
    }
    
    void visit(LambdaExpr& node) override { 
        sm.enterScope(ScopeKind::Function);
        for (auto& p : node.params) {
            p->accept(static_cast<ASTVisitor&>(*this)); // Bind lambda params
        }
        if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
        node.body->accept(static_cast<ASTVisitor&>(*this));
        sm.exitScope();
    }
    
    void visit(AwaitExpr& node) override { 
        node.expr->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(SizeofExpr& node) override { 
        node.targetType->accept(static_cast<TypeVisitor&>(*this));
    }
    
    void visit(AlignofExpr& node) override { 
        node.targetType->accept(static_cast<TypeVisitor&>(*this));
    }

    // --- Types ---
    void visit(BuiltinTypeNode&) override { }
    void visit(NamedTypeNode& node) override { 
        if (!node.segments.empty()) {
            SymbolID id = sm.resolve(node.segments[0], node.loc);
            while (id != kInvalidSymbolID) {
                auto& sym = sm.table.getSymbol(id);
                if (sym.kind == SymbolKind::Alias) {
                    id = sym.aliasTo;
                } else {
                    break;
                }
            }
            node.symbolId = id;
        }
    }
    void visit(ReferenceTypeNode& node) override { if (node.inner) node.inner->accept(static_cast<TypeVisitor&>(*this)); }
    void visit(PointerTypeNode& node) override { if (node.inner) node.inner->accept(static_cast<TypeVisitor&>(*this)); }
    void visit(ArrayTypeNode& node) override { 
        if (node.elementType) node.elementType->accept(static_cast<TypeVisitor&>(*this));
        if (node.size) node.size->accept(static_cast<ASTVisitor&>(*this));
    }
    void visit(TupleTypeNode& node) override { 
        for (auto& t : node.elements) { t->accept(static_cast<TypeVisitor&>(*this)); }
    }
    void visit(FunctionTypeNode& node) override { 
        for (auto& p : node.params) { p->accept(static_cast<TypeVisitor&>(*this)); }
        if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
    }
    void visit(NeverTypeNode&) override { }
    void visit(TraitObjectTypeNode& node) override { 
        if (node.trait) node.trait->accept(static_cast<TypeVisitor&>(*this));
    }

    // --- Patterns ---
    void visit(WildcardPatternNode&) override { }
    void visit(LiteralPatternNode&) override { }
    void visit(IdentifierPatternNode& node) override { 
        // Pattern binding creates a new variable in the current scope
        if (!node.segments.empty()) {
            node.symbolId = sm.declare(node.segments[0], SymbolKind::Variable, node.loc, &node);
        }
    }
    void visit(EnumPatternNode& node) override { 
        if (!node.path.empty()) {
            SymbolID varId = sm.resolvePath(node.path, node.loc);
            if (varId != kInvalidSymbolID) {
                auto sym = sm.table.getSymbol(varId);
                if (sym.kind == SymbolKind::EnumVariant) {
                    node.variantSymbolId = varId;
                } else {
                    sm.diag.error(node.loc, "'" + std::string(node.path.back()) + "' is not an enum variant");
                }
            }
        }
        for (auto& p : node.fields) { p->accept(static_cast<PatternVisitor&>(*this)); }
    }
    void visit(TuplePatternNode& node) override { 
        for (auto& p : node.elements) { p->accept(static_cast<PatternVisitor&>(*this)); }
    }
};


// =============================================================================
// Resolver Implementation
// =============================================================================

Resolver::Resolver(SymbolTable& table, DiagnosticEngine& diag)
    : table_(table), diag_(diag) {}

bool Resolver::resolve(ASTNode* root) {
    if (!root) return false;

    ScopeManager sm(table_, diag_);

    // --- Pass 0: Builtins ---
    sm.enterExistingScope(sm.table.globalScopeId());
    if (!sm.table.containsInScope(Identifier(std::string("void")), sm.table.globalScopeId())) {
        sm.declare("void", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    }
    if (!sm.table.containsInScope(Identifier(std::string("str")), sm.table.globalScopeId())) {
        sm.declare("str", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    }
    sm.exitScope();
    
    std::cout << "[DEBUG] Entering resolve\n";
    sm.enterExistingScope(sm.table.globalScopeId());
    
    DeclarationVisitor pass1(sm);
    std::cout << "[DEBUG] pass1\n";
    root->accept(pass1);
    
    if (diag_.hasErrors()) { sm.exitScope(); return false; }

    UseResolutionVisitor pass1_5(sm);
    std::cout << "[DEBUG] pass1_5\n";
    root->accept(pass1_5);
    
    if (diag_.hasErrors()) { sm.exitScope(); return false; }
    
    ResolutionVisitor pass2(sm, diag_);
    std::cout << "[DEBUG] pass2\n";
    root->accept(pass2);
    
    sm.exitScope();
    std::cout << "[DEBUG] Exiting resolve\n";
    
    return !diag_.hasErrors();
}

} // namespace fl
