#include "fdlang/MiddleEnd/Resolver.h"
#include "fdlang/AST/ASTNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/StmtNode.h"
#include "fdlang/AST/TypeNode.h"
#include "fdlang/AST/PatternNode.h"
#include "fdlang/AST/ProgramNode.h"
#include "fdlang/MiddleEnd/ScopeStack.h"
#include "fdlang/FrontEnd/ASTVisitor.h"
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

        return table.declareSymbol(id, kind, currentScope, loc, declNode);
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

        for (auto& param : node.genericParams) {}
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
        // We will process inner items in Pass 1 if it's an inline mod
        if (!node.decls.empty()) {
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
    
    void visit(ForStmtNode& node) override {
        node.bodyScopeId = sm.table.createScope(ScopeKind::Loop, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);
        // We will declare pattern variables in pass 2 since they require RHS evaluation.
        // Actually ForStmtNode has bindingName
        node.bindingId = sm.declare(node.bindingName, SymbolKind::Variable, node.loc, &node);
        node.body->accept(*this);
        sm.exitScope();
    }
    
    void visit(UnsafeStmtNode& node) override {
        if (node.body) node.body->accept(*this);
    }
    
    void visit(ComptimeStmtNode& node) override {
        if (node.body) node.body->accept(*this);
    }
    
    void visit(ExprStmtNode& node) override {}
    void visit(ReturnStmtNode& node) override {}
    void visit(BreakStmtNode& node) override {}
    void visit(ContinueStmtNode& node) override {}

    // --- Expressions --- (Do nothing in Pass 1)
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
        if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
        
        for (auto& p : node.params) {
            if (p->type) p->type->accept(static_cast<TypeVisitor&>(*this));
        }
        
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
        sm.exitScope();
    }

    void visit(ParamDeclNode& node) override {} // Handled in FunctionDeclNode

    void visit(StructDeclNode& node) override {
        sm.enterExistingScope(node.bodyScopeId);
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
        for (auto& m : node.methods) {
            m->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(ImplDeclNode& node) override {
        sm.enterExistingScope(node.bodyScopeId);
        node.selfType->accept(static_cast<TypeVisitor&>(*this));
        if (node.traitType) node.traitType->accept(static_cast<TypeVisitor&>(*this));
        
        for (auto& m : node.methods) {
            m->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(ModDeclNode& node) override {
        if (!node.decls.empty()) {
            sm.enterExistingScope(node.bodyScopeId);
            for (auto& i : node.decls) {
                i->accept(static_cast<ASTVisitor&>(*this));
            }
            sm.exitScope();
        }
    }

    void visit(UseDeclNode& node) override {}
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
        node.iterable->accept(static_cast<ASTVisitor&>(*this));
        if (node.bodyScopeId == kInvalidSymbolID) {
            node.bodyScopeId = sm.table.createScope(ScopeKind::Loop, sm.scopeStack.current());
        }
        sm.enterExistingScope(node.bodyScopeId);
        // node.pat->accept(static_cast<PatternVisitor&>(*this)); // Bind pattern vars
        node.body->accept(static_cast<ASTVisitor&>(*this));
        sm.exitScope();
    }
    
    void visit(ReturnStmtNode& node) override {
        if (node.value) node.value->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(BreakStmtNode& node) override {
    }
    
    void visit(ContinueStmtNode& node) override {}
    
    void visit(UnsafeStmtNode& node) override {
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(ComptimeStmtNode& node) override {
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
    }

    // --- Expressions ---
    void visit(LiteralExpr&) override {}
    
    void visit(IdentifierExpr& node) override {
        // Resolve the first segment as a symbol in the current scope
        if (!node.segments.empty()) {
            node.resolvedSymbol = sm.resolve(node.segments[0], node.loc);
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
    void visit(BuiltinTypeNode&) override {}
    void visit(NamedTypeNode& node) override {
        if (!node.segments.empty()) {
            node.symbolId = sm.resolve(node.segments[0], node.loc);
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
    void visit(NeverTypeNode&) override {}
    void visit(TraitObjectTypeNode& node) override {
        if (node.trait) node.trait->accept(static_cast<TypeVisitor&>(*this));
    }

    // --- Patterns ---
    void visit(WildcardPatternNode&) override {}
    void visit(LiteralPatternNode&) override {}
    void visit(IdentifierPatternNode& node) override {
        // Pattern binding creates a new variable in the current scope
        if (!node.segments.empty()) {
            node.symbolId = sm.declare(node.segments[0], SymbolKind::Variable, node.loc, &node);
        }
    }
    void visit(EnumPatternNode& node) override {
        if (!node.path.empty()) {
            SymbolID enumId = sm.resolve(node.path[0], node.loc);
            if (enumId != kInvalidSymbolID) {
                auto sym = sm.table.getSymbol(enumId);
                if (sym.kind == SymbolKind::Enum && sym.decl) {
                    auto* enumDecl = static_cast<EnumDeclNode*>(sym.decl);
                    if (node.path.size() > 1) {
                        node.variantSymbolId = sm.table.lookup(Identifier(node.path[1]), enumDecl->bodyScopeId).value_or(kInvalidSymbolID);
                        if (node.variantSymbolId == kInvalidSymbolID) {
                            diag.error(node.loc, "Variant '" + std::string(node.path[1]) + "' not found in enum '" + std::string(node.path[0]) + "'");
                        }
                    } else {
                        diag.error(node.loc, "Expected enum variant path (e.g., EnumName::VariantName)");
                    }
                } else {
                    diag.error(node.loc, "'" + std::string(node.path[0]) + "' is not an enum");
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
    sm.declare("void", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    sm.declare("i32", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    sm.declare("f32", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    sm.declare("bool", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    sm.declare("string", SymbolKind::TypeAlias, SourceLocation{}, nullptr);
    sm.exitScope();
    
    DeclarationVisitor pass1(sm);
    root->accept(pass1);
    
    if (diag_.hasErrors()) return false;
    
    ResolutionVisitor pass2(sm, diag_);
    root->accept(pass2);
    
    return !diag_.hasErrors();
}

} // namespace fl
