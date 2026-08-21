#include <iostream>
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/Core/Intrinsic.h"
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
    uint16_t currentFunctionDepth = 0;

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
    
    ModuleID currentModuleID() const {
        for (auto it = scopeStack.chain().rbegin(); it != scopeStack.chain().rend(); ++it) {
            if (table.getScope(*it).kind == ScopeKind::Module || table.getScope(*it).kind == ScopeKind::Global) {
                return *it;
            }
        }
        return 0;
    }

    SymbolID declare(std::string_view name, SymbolKind kind, SourceLocation loc, ASTNode* declNode) {
        Identifier id(name);
        ScopeID currentScope = scopeStack.current();

        if (table.containsInScope(id, currentScope)) {
            if (kind == SymbolKind::Function || kind == SymbolKind::TraitMethod || kind == SymbolKind::Alias) {
                auto existingSyms = table.lookupInScope(id, currentScope);
                bool allFunctions = true;
                for (SymbolID sid : existingSyms) {
                    auto& s = table.getSymbol(sid);
                    if (s.kind != SymbolKind::Function && s.kind != SymbolKind::TraitMethod && s.kind != SymbolKind::Alias) {
                        allFunctions = false;
                        break;
                    }
                }
                if (!allFunctions) {
                    std::cout << "[DEBUG] Redeclaration error: name=" << name << " kind=" << (int)kind << " scope=" << currentScope << "\n";
                    diag.error(loc, "Redeclaration of name '" + std::string(name) + "' in this scope.");
                    auto optSyms = table.lookupInScope(id, currentScope);
                    if (!optSyms.empty() && optSyms[0] != kInvalidSymbolID) {
                        auto sym = table.getSymbol(optSyms[0]);
                        if (sym.decl == declNode) return optSyms[0];
                        std::cerr << "[DEBUG] Redeclaration of name '" << name << "'! sym.decl=" << sym.decl << " decl=" << declNode << " sym.kind=" << (int)sym.kind << " newKind=" << (int)kind << "\n";
                        diag.error(loc, "Redeclaration of name '" + std::string(name) + "' in this scope.");
                    }
                    return kInvalidSymbolID;
                }
            } else {
                std::cout << "[DEBUG] Redeclaration error: name=" << name << " kind=" << (int)kind << " scope=" << currentScope << "\n";
                auto optSyms = table.lookupInScope(id, currentScope);
                if (!optSyms.empty() && optSyms[0] != kInvalidSymbolID) {
                    auto sym = table.getSymbol(optSyms[0]);
                    if (sym.decl == declNode) return optSyms[0];
                    std::cerr << "[DEBUG] Redeclaration of name '" << name << "'! sym.decl=" << sym.decl << " decl=" << declNode << " sym.kind=" << (int)sym.kind << " newKind=" << (int)kind << "\n";
                    diag.error(loc, "Redeclaration of name '" + std::string(name) + "' in this scope.");
                }
                return kInvalidSymbolID;
            }
        }

        SymbolID symId = table.declareSymbol(id, kind, currentScope, loc, declNode);
        auto& sym = table.getMutableSymbol(symId);
        sym.declaredDepth = currentFunctionDepth;
        sym.moduleID = currentModuleID();
        if (declNode) {
            if (auto* d = dynamic_cast<DeclNode*>(declNode)) {
                sym.visibility = d->visibility;
            } else if (auto* f = dynamic_cast<StructFieldNode*>(declNode)) {
                sym.visibility = f->visibility;
            }
        }
        return symId;
    }

    std::vector<SymbolID> resolve(std::string_view name, SourceLocation loc) {
        Identifier id(name);
        ScopeID currentScope = scopeStack.current();

        auto optSym = table.lookup(id, currentScope);
        if (optSym.empty()) {
            std::cout << "[DEBUG Resolver] name '" << name << "' not found in scope " << currentScope << ", falling back to Scope 0.\n";
            optSym = table.lookupInScope(id, 0); // fallback to Global Scope
            if (!optSym.empty()) {
                std::cout << "[DEBUG Resolver] FOUND '" << name << "' in Scope 0!\n";
            }
        }
        if (optSym.empty()) {
            diag.error(loc, "Use of undeclared name '" + std::string(name) + "'.");
            return {};
        }

        return optSym;
    }

    
    std::vector<SymbolID> resolvePath(const std::vector<std::string_view>& path, SourceLocation loc) {
        if (path.empty()) return {};
        std::cout << "[DEBUG resolvePath] Resolving path starting with '" << path[0] << "'\n";
        std::vector<SymbolID> currentSyms = resolve(path[0], loc);
        if (currentSyms.empty()) {
            std::cout << "[DEBUG resolvePath] resolve('" << path[0] << "') returned EMPTY\n";
            return currentSyms;
        }
        std::cout << "[DEBUG resolvePath] resolve('" << path[0] << "') returned " << currentSyms.size() << " syms. First id=" << currentSyms[0] << "\n";

        for (size_t i = 1; i < path.size(); ++i) {
            if (currentSyms.size() > 1) {
                diag.error(loc, "Ambiguous path '" + std::string(path[i-1]) + "'.");
                return {};
            }
            SymbolID currentSym = currentSyms[0];
            auto& symRef = table.getSymbol(currentSym);
            SymbolID resolvedSymId = currentSym;
            std::cout << "[DEBUG] resolvePath start: currentSym=" << currentSym << " kind=" << (int)symRef.kind << " aliasTo=" << symRef.aliasTo << "\n";
            while (table.getSymbol(resolvedSymId).kind == SymbolKind::Alias) {
                resolvedSymId = table.getSymbol(resolvedSymId).aliasTo;
                std::cout << "[DEBUG] resolvePath alias jump: resolvedSymId=" << resolvedSymId << "\n";
                if (resolvedSymId == kInvalidSymbolID) break;
            }
            if (resolvedSymId != kInvalidSymbolID) {
                currentSym = resolvedSymId;
            }
            auto& sym = table.getSymbol(currentSym);
            std::cout << "[DEBUG] resolvePath final: currentSym=" << currentSym << " kind=" << (int)sym.kind << "\n";
            
            if (sym.kind != SymbolKind::Module && sym.kind != SymbolKind::Enum && sym.kind != SymbolKind::ExternalModule) {
                diag.error(loc, "Symbol '" + std::string(path[i-1]) + "' is not a module or enum.");
                return {};
            }
            ScopeID nextScope = kInvalidScopeID;
            if (sym.kind == SymbolKind::Module) {
                if (sym.decl != nullptr && static_cast<ModDeclNode*>(sym.decl)->bodyScopeId != kInvalidScopeID) {
                    nextScope = static_cast<ModDeclNode*>(sym.decl)->bodyScopeId;
                } else {
                    nextScope = static_cast<ScopeID>(sym.mlibSymbolID);
                }
            } else if (sym.kind == SymbolKind::ExternalModule) {
                nextScope = static_cast<ScopeID>(sym.mlibSymbolID);
            } else if (sym.kind == SymbolKind::Enum) {
                if (sym.decl != nullptr && static_cast<EnumDeclNode*>(sym.decl)->bodyScopeId != kInvalidScopeID) {
                    nextScope = static_cast<EnumDeclNode*>(sym.decl)->bodyScopeId;
                } else {
                    nextScope = static_cast<ScopeID>(sym.mlibSymbolID);
                }
                std::cout << "[DEBUG] resolvePath Enum '" << sym.name.view() << "' nextScope=" << nextScope 
                          << " (sym.decl=" << (void*)sym.decl 
                          << (sym.decl ? ", bodyScopeId=" + std::to_string(static_cast<EnumDeclNode*>(sym.decl)->bodyScopeId) : "") 
                          << ", mlibSymbolID=" << sym.mlibSymbolID << ")\n";
                
                if (nextScope != kInvalidScopeID) {
                    std::cout << "[DEBUG] bindings in nextScope: ";
                    for (const auto& kv : table.getScope(nextScope).bindings) {
                        std::cout << kv.first.str() << " ";
                    }
                    std::cout << "\n";
                }
            }
            auto optNext = table.lookupInScope(Identifier(path[i]), nextScope);
            if (optNext.empty()) {
                diag.error(loc, "Module/Enum '" + std::string(path[i-1]) + "' does not contain '" + std::string(path[i]) + "'.");
                return {};
            }
            auto& nextSym = table.getSymbol(optNext[0]);
            if (sym.kind == SymbolKind::Module && nextSym.visibility != Visibility::Public) {
                diag.error(loc, "Symbol '" + std::string(path[i]) + "' is private.");
                return {};
            }
            currentSyms = optNext;
        }
        return currentSyms;
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
        SymbolKind kind = node.isMutable ? SymbolKind::Static : SymbolKind::Const;
        if (node.pattern) {
            if (auto bind = dynamic_cast<IdentifierPatternNode*>(node.pattern.get())) {
                bind->symbolId = sm.declare(bind->segments.back(), kind, node.loc, &node);
                node.symbolId = bind->symbolId;
            } else {
                sm.diag.error(node.loc, "Global variable declaration cannot use destructuring pattern.");
            }
        } else {
            node.symbolId = sm.declare(node.name, kind, node.loc, &node);
        }
    }

    void visit(FunctionDeclNode& node) override { 
        if (node.isIntrinsic) {
            auto intrinsicDecl = IntrinsicRegistry::get().lookup(std::string(node.name));
            if (intrinsicDecl) {
                node.intrinsicKind = intrinsicDecl->kind;
            } else {
                sm.diag.error(node.loc, "Unknown intrinsic function '" + std::string(node.name) + "'");
            }
        }

        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::Function, node.loc, &node);
        }
        
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
        std::cerr << "[DEBUG] Resolver::visit(StructDeclNode) name='" << node.name << "' symbolId=" << node.symbolId << " this=" << &node << " line=" << node.loc.line << "\n";
        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::Struct, node.loc, &node);
        }
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
        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::Enum, node.loc, &node);
        }
        
        node.bodyScopeId = sm.table.createScope(ScopeKind::Enum, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        std::cout << "[DEBUG] Enum '" << node.name << "' has " << node.variants.size() << " variants\n";
        for (auto& v : node.variants) {
            std::cout << "[DEBUG] Visiting variant '" << v->name << "'\n";
            v->accept(*this);
        }

        sm.exitScope();
    }

    void visit(EnumVariantNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::EnumVariant, node.loc, &node);
        node.bodyScopeId = sm.table.createScope(ScopeKind::Enum, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId); 
        int fieldIdx = 0;
        for (auto& f : node.fields) {
            if (!f->name.empty()) {
                f->symbolId = sm.declare(f->name, SymbolKind::StructField, f->loc, f.get());
            } else {
                f->symbolId = sm.declare("__field_" + std::to_string(fieldIdx), SymbolKind::StructField, f->loc, f.get());
            }
            fieldIdx++;
        }
        sm.exitScope();
    }

    void visit(TraitDeclNode& node) override { 
        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::Trait, node.loc, &node);
        }
        node.bodyScopeId = sm.table.createScope(ScopeKind::Trait, sm.scopeStack.current());
        sm.enterExistingScope(node.bodyScopeId);

        sm.declare("Self", SymbolKind::GenericParam, node.loc, nullptr);

        for (auto& param : node.genericParams) {
            param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
        }
        for (auto& at : node.associatedTypes) {
            at->accept(*this);
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
        for (auto& at : node.associatedTypes) {
            at->accept(*this);
        }
        for (auto& m : node.methods) {
            m->accept(*this);
        }

        sm.exitScope();
    }

    void visit(ModDeclNode& node) override { 
        node.symbolId = sm.declare(node.name, SymbolKind::Module, node.loc, &node);
        if (node.symbolId != kInvalidSymbolID) {
            sm.table.getMutableSymbol(node.symbolId).visibility = node.visibility;
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
        if (node.func) {
            node.func->visibility = node.visibility;
            node.func->accept(*this);
            if (node.func->symbolId != kInvalidSymbolID) {
                sm.table.getMutableSymbol(node.func->symbolId).isExternal = true;
                sm.table.getMutableSymbol(node.func->symbolId).visibility = node.visibility;
                sm.table.getFunctionInfo(node.func->symbolId).borrowCheckStatus = BorrowCheckStatus::Skipped;
            }
        }
    }

    void visit(TypeAliasDeclNode& node) override { 
        if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, SymbolKind::TypeAlias, node.loc, &node);
        }
        if (!node.genericParams.empty()) {
            node.bodyScopeId = sm.table.createScope(ScopeKind::Block, sm.scopeStack.current());
            sm.enterExistingScope(node.bodyScopeId);
            for (auto& param : node.genericParams) {
                param.symbolId = sm.declare(param.name, SymbolKind::GenericParam, param.loc, nullptr);
            }
            sm.exitScope();
        }
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
    void visit(TupleIndexExpr&) override { }
    void visit(CastExpr&) override { }
    void visit(UnsizeCastExpr&) override { }
    void visit(ArrayLiteralExpr&) override { }
    void visit(TupleLiteralExpr&) override { }
    void visit(StructInitExpr&) override { }
    void visit(MatchExpr&) override { }
    void visit(LambdaExpr&) override { }
    void visit(TryExpr&) override { }
    void visit(AwaitExpr&) override { }
    void visit(SizeofExpr&) override { }
    void visit(AlignofExpr&) override { }
    void visit(TypeofExpr&) override { }
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
            auto targets = sm.resolvePath(fullPath, tree.loc);
            for (SymbolID target : targets) {
                if (target != kInvalidSymbolID) {
                    std::string_view aliasName = tree.alias.empty() ? fullPath.back() : tree.alias;
                    Identifier aliasIdent(aliasName);
                    std::vector<SymbolID> existing = sm.table.lookupInScope(aliasIdent, sm.scopeStack.current());
                    bool alreadyImported = false;
                    for (SymbolID id : existing) {
                        if (id == target) {
                            alreadyImported = true;
                            break;
                        }
                        auto& sym = sm.table.getSymbol(id);
                        if (sym.kind == SymbolKind::Alias && sym.aliasTo == target) {
                            alreadyImported = true;
                            break;
                        }
                    }
                    if (alreadyImported) {
                        // It is already imported (e.g. root module from ImportResolver), skip
                        continue;
                    }
                    SymbolID aliasId = sm.declare(aliasName, SymbolKind::Alias, tree.loc, nullptr);
                    if (aliasId != kInvalidSymbolID) {
                        sm.table.getMutableSymbol(aliasId).aliasTo = target;
                    }
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
        if (node.isResolvedExternal) return;
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
    void visit(ComptimeStmtNode& node) override { if (node.body) node.body->accept(*this); }

    void visit(LiteralExpr&) override {}
    void visit(IdentifierExpr&) override {}
    void visit(BinaryExpr&) override {}
    void visit(UnaryExpr&) override {}
    void visit(AssignExpr&) override {}
    void visit(CallExpr&) override {}
    void visit(MethodCallExpr&) override {}
    void visit(IndexExpr&) override {}
    void visit(MemberExpr&) override {}
    void visit(TupleIndexExpr&) override {}
    void visit(CastExpr&) override {}
    void visit(UnsizeCastExpr&) override {}
    void visit(ArrayLiteralExpr&) override {}
    void visit(TupleLiteralExpr&) override {}
    void visit(StructInitExpr&) override {}
    void visit(MatchExpr&) override {}
    void visit(LambdaExpr&) override {}
    void visit(TryExpr& node) override { if (node.expr) node.expr->accept(*this); }
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}
    void visit(TypeofExpr&) override {}
};

// =============================================================================
// Pass 2: Resolution Visitor
// Resolves expressions, type annotations, and function bodies.
// =============================================================================
class ResolutionVisitor : public ASTVisitor, public TypeVisitor, public PatternVisitor {
    ScopeManager& sm;
    DiagnosticEngine& diag;
    std::vector<std::pair<uint16_t, LambdaExpr*>> activeLambdas;
    SymbolKind currentVarKind = SymbolKind::Variable;
    
    struct LoopInfo {
        std::string label;
        ASTNode* node;
    };
    std::vector<LoopInfo> loopStack_;

public:
    ResolutionVisitor(ScopeManager& sm, DiagnosticEngine& diag)
        : sm(sm), diag(diag) {}

    void visit(ProgramNode& node) override { 
        sm.enterExistingScope(sm.table.globalScopeId());
        for (auto& item : node.items) {
            item->accept(static_cast<ASTVisitor&>(*this));
        }
        sm.exitScope();
    }

    // --- Declarations ---
    void visit(VarDeclNode& node) override { 
        SymbolKind oldKind = currentVarKind;
        currentVarKind = node.isMutable ? SymbolKind::Variable : SymbolKind::Const;
        if (node.pattern) {
            node.pattern->accept(static_cast<PatternVisitor&>(*this));
            if (auto idPat = dynamic_cast<IdentifierPatternNode*>(node.pattern.get())) {
                node.symbolId = idPat->symbolId;
            }
        } else if (node.symbolId == kInvalidSymbolID) {
            node.symbolId = sm.declare(node.name, currentVarKind, node.loc, &node);
        }
        currentVarKind = oldKind;
        if (node.typeAnnot) node.typeAnnot->accept(static_cast<TypeVisitor&>(*this));
        if (node.initializer) node.initializer->accept(static_cast<ASTVisitor&>(*this));
    }

    void visit(FunctionDeclNode& node) override { 
        std::cerr << "[DEBUG] Resolving function: " << node.name << std::endl;
        sm.currentFunctionDepth++;
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
        sm.currentFunctionDepth--;
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
        sm.enterExistingScope(node.bodyScopeId); 
        // tupleTypes not implemented in parser yet? Wait, EnumVariantNode has `fields`
        for (auto& f : node.fields) {
            if (f->type) f->type->accept(static_cast<TypeVisitor&>(*this));
        }
        sm.exitScope();
    }

    void visit(TraitDeclNode& node) override { 
        sm.enterExistingScope(node.bodyScopeId);
        for (auto& gp : node.genericParams) {
            for (auto& bound : gp.bounds) {
                bound->accept(static_cast<TypeVisitor&>(*this));
            }
        }
        for (auto& at : node.associatedTypes) {
            at->accept(static_cast<ASTVisitor&>(*this));
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
        
        for (auto& at : node.associatedTypes) {
            at->accept(static_cast<ASTVisitor&>(*this));
        }
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
        if (node.func) {
            node.func->visibility = node.visibility;
            node.func->accept(*this);
            if (node.func->symbolId != kInvalidSymbolID) {
                sm.table.getMutableSymbol(node.func->symbolId).isExternal = true;
                sm.table.getMutableSymbol(node.func->symbolId).visibility = node.visibility;
                sm.table.getFunctionInfo(node.func->symbolId).borrowCheckStatus = BorrowCheckStatus::Skipped;
            }
        }
    }
    void visit(TypeAliasDeclNode& node) override { 
        if (!node.genericParams.empty()) sm.enterExistingScope(node.bodyScopeId);
        if (node.aliasedType) node.aliasedType->accept(static_cast<TypeVisitor&>(*this));
        if (!node.genericParams.empty()) sm.exitScope();
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
        if (node.tailExpr) {
            node.tailExpr->accept(static_cast<ASTVisitor&>(*this));
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
        loopStack_.push_back({node.label, &node});
        node.condition->accept(static_cast<ASTVisitor&>(*this));
        node.body->accept(static_cast<ASTVisitor&>(*this));
        loopStack_.pop_back();
    }
    
    void visit(ForStmtNode& node) override { 
        loopStack_.push_back({node.label, &node});
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
            if (node.pattern) {
                node.pattern->accept(static_cast<PatternVisitor&>(*this));
                if (auto idPat = dynamic_cast<IdentifierPatternNode*>(node.pattern.get())) {
                    node.bindingId = idPat->symbolId;
                }
            } else if (node.bindingId == kInvalidSymbolID) {
                node.bindingId = sm.declare(node.bindingName, SymbolKind::Variable, node.loc, &node);
            }
            if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
            sm.exitScope();
        }
        loopStack_.pop_back();
    }
    
    void visit(ReturnStmtNode& node) override { 
        if (node.value) node.value->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(BreakStmtNode& node) override { 
        if (loopStack_.empty()) {
            diag.error(node.loc, "'break' outside of loop");
        } else if (node.label.empty()) {
            node.targetLoop = loopStack_.back().node;
        } else {
            bool found = false;
            for (auto it = loopStack_.rbegin(); it != loopStack_.rend(); ++it) {
                if (it->label == node.label) {
                    node.targetLoop = it->node;
                    found = true;
                    break;
                }
            }
            if (!found) {
                diag.error(node.loc, "Use of undeclared label '" + node.label + "'");
            }
        }
    }
    
    void visit(ContinueStmtNode& node) override {
        if (loopStack_.empty()) {
            diag.error(node.loc, "'continue' outside of loop");
        } else if (node.label.empty()) {
            node.targetLoop = loopStack_.back().node;
        } else {
            bool found = false;
            for (auto it = loopStack_.rbegin(); it != loopStack_.rend(); ++it) {
                if (it->label == node.label) {
                    node.targetLoop = it->node;
                    found = true;
                    break;
                }
            }
            if (!found) {
                diag.error(node.loc, "Use of undeclared label '" + node.label + "'");
            }
        }
    }
    
    void visit(UnsafeStmtNode& node) override { 
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
    }
    
    void visit(ComptimeStmtNode& node) override { 
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
    }

    // --- Expressions ---
    void visit(LiteralExpr&) override { }
    
    void visit(IdentifierExpr& node) override { 
        std::cerr << "[DEBUG] Resolving identifier: " << (node.segments.empty() ? "<empty>" : node.segments[0]) << std::endl;
        if (!node.segments.empty()) {
            std::vector<SymbolID> ids = sm.resolvePath(node.segments, node.loc);
            std::cerr << "[DEBUG] Finished resolvePath for " << (node.segments.empty() ? "<empty>" : node.segments[0]) << std::endl;
            if (node.segments.size() > 0 && node.segments.back() == "c") {
                std::cerr << "[DEBUG] Resolver: resolvePath for 'c' returned " << ids.size() << " ids.\n";
            }
            std::vector<SymbolID> finalIds;
            for (SymbolID id : ids) {
                if (node.segments.size() > 0 && node.segments.back() == "c") {
                    std::cerr << "[DEBUG] Resolver: id=" << id << "\n";
                }
                while (id != kInvalidSymbolID) {
                    auto& sym = sm.table.getSymbol(id);
                    if (sym.kind == SymbolKind::Alias) {
                        id = sym.aliasTo;
                    } else {
                        break;
                    }
                }
                if (id != kInvalidSymbolID) {
                    finalIds.push_back(id);
                }
            }
            node.overloadCandidates = finalIds;
            if (node.segments.size() > 0 && node.segments.back() == "c") {
                std::cerr << "[DEBUG] Resolver: finalIds size for 'c' is " << finalIds.size() << "\n";
            }
            
            // --- Closure Capture Detection ---
            if (finalIds.size() == 1 && !activeLambdas.empty()) {
                SymbolID resolvedId = finalIds[0];
                const auto& sym = sm.table.getSymbol(resolvedId);
                if (sym.kind == SymbolKind::Variable || sym.kind == SymbolKind::Parameter) {
                    // Propagate capture up the closure chain
                    for (auto& [lambdaDepth, lambdaNode] : activeLambdas) {
                        if (sym.declaredDepth < lambdaDepth) {
                            bool alreadyCaptured = false;
                            for (auto& c : lambdaNode->captures) {
                                if (c.symbolId == resolvedId) { alreadyCaptured = true; break; }
                            }
                            if (!alreadyCaptured) {
                                lambdaNode->captures.push_back(LambdaCaptureRef{resolvedId});
                            }
                        }
                    }
                }
            }
            // ---------------------------------
        }
        for (auto& arg : node.genericArgs) {
            if (arg) arg->accept(static_cast<TypeVisitor&>(*this));
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
    
    void visit(TupleIndexExpr& node) override {
        node.object->accept(static_cast<ASTVisitor&>(*this));
        // index is a literal uint32_t — no symbol resolution needed
    }
    
    void visit(CastExpr& node) override { 
        node.expr->accept(static_cast<ASTVisitor&>(*this));
        node.targetType->accept(static_cast<TypeVisitor&>(*this));
    }
    void visit(UnsizeCastExpr& node) override { 
        node.expr->accept(static_cast<ASTVisitor&>(*this));
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
            auto syms = sm.resolvePath(node.path, node.loc);
            if (!syms.empty()) {
                node.structId = syms[0];
            }
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
        sm.currentFunctionDepth++;
        sm.enterScope(ScopeKind::Function);
        
        // Register this lambda as active for capture analysis
        activeLambdas.push_back({sm.currentFunctionDepth, &node});
        
        for (auto& p : node.params) {
            p->accept(static_cast<ASTVisitor&>(*this)); // Bind lambda params
        }
        if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
        if (node.body) node.body->accept(static_cast<ASTVisitor&>(*this));
        
        activeLambdas.pop_back();
        
        sm.exitScope();
        sm.currentFunctionDepth--;
    }
    
    void visit(TryExpr& node) override {
        if (node.expr) node.expr->accept(static_cast<ASTVisitor&>(*this));
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
    void visit(TypeofExpr& node) override {
        if (node.expr) node.expr->accept(*this);
    }

    // --- Types ---
    void visit(BuiltinTypeNode&) override { }
    void visit(NamedTypeNode& node) override { 
        if (node.symbolId == kInvalidSymbolID && !node.segments.empty()) {
            auto ids = sm.resolve(node.segments[0], node.loc);
            if (!ids.empty()) {
                SymbolID currentSym = ids[0];
                size_t i = 1;
                while (i < node.segments.size()) {
                    auto& sym = sm.table.getSymbol(currentSym);
                    if (sym.kind != SymbolKind::Module && sym.kind != SymbolKind::Enum && sym.kind != SymbolKind::ExternalModule) {
                        break;
                    }
                    ScopeID nextScope = kInvalidScopeID;
                    if (sym.kind == SymbolKind::Module) {
                        if (sym.decl != nullptr && static_cast<ModDeclNode*>(sym.decl)->bodyScopeId != kInvalidScopeID) {
                            nextScope = static_cast<ModDeclNode*>(sym.decl)->bodyScopeId;
                        } else {
                            nextScope = static_cast<ScopeID>(sym.mlibSymbolID);
                        }
                    } else if (sym.kind == SymbolKind::ExternalModule) {
                        nextScope = static_cast<ScopeID>(sym.mlibSymbolID);
                    } else if (sym.kind == SymbolKind::Enum) {
                        if (sym.decl != nullptr && static_cast<EnumDeclNode*>(sym.decl)->bodyScopeId != kInvalidScopeID) {
                            nextScope = static_cast<EnumDeclNode*>(sym.decl)->bodyScopeId;
                        } else {
                            nextScope = static_cast<ScopeID>(sym.mlibSymbolID);
                        }
                    }
                    if (nextScope == kInvalidScopeID) break;
                    
                    auto optNext = sm.table.lookupInScope(Identifier(node.segments[i]), nextScope);
                    if (optNext.empty()) break;
                    currentSym = optNext[0];
                    i++;
                }
                
                while (currentSym != kInvalidSymbolID) {
                    auto& sym = sm.table.getSymbol(currentSym);
                    if (sym.kind == SymbolKind::Alias) {
                        currentSym = sym.aliasTo;
                    } else {
                        break;
                    }
                }
                std::cerr << "[DEBUG Resolver NamedTypeNode] Resolved path length for segment[0] '" << node.segments[0] << "': i=" << i << ", currentSym=" << currentSym << "\n";
                node.symbolId = currentSym;
                node.resolvedPathLength = i;
            }
        }
        for (auto& arg : node.genericArgs) {
            if (arg) arg->accept(static_cast<TypeVisitor&>(*this));
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
    void visit(PlaceholderTypeNode&) override { }
    void visit(LifetimeNode&) override { }

    // --- Patterns ---
    void visit(WildcardPatternNode&) override { }
    void visit(LiteralPatternNode&) override { }
    void visit(IdentifierPatternNode& node) override { 
        // Pattern binding creates a new variable in the current scope
        if (!node.segments.empty()) {
            if (node.symbolId == kInvalidSymbolID) {
                node.symbolId = sm.declare(node.segments[0], currentVarKind, node.loc, &node);
            }
        }
    }
    void visit(EnumPatternNode& node) override { 
        if (!node.path.empty()) {
            auto varIds = sm.resolvePath(node.path, node.loc);
            if (!varIds.empty()) {
                SymbolID varId = varIds[0];
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
    void visit(StructPatternNode& node) override {
        // Struct name (path)
        if (!node.path.empty()) {
            auto symIds = sm.resolvePath(node.path, node.loc);
            if (!symIds.empty()) {
                node.structSymbolId = symIds[0];
            }
        }
        for (auto& f : node.fields) {
            if (f.pattern) {
                f.pattern->accept(static_cast<PatternVisitor&>(*this));
            }
        }
    }
};


// =============================================================================
// Resolver Implementation
// =============================================================================

Resolver::Resolver(SymbolTable& table, DiagnosticEngine& diag)
    : table_(table), diag_(diag) {}

bool Resolver::resolve(ASTNode* root, ScopeID parentScope) {
    if (!root) return false;

    ScopeManager sm(table_, diag_);
    
    const bool isTopLevel = (parentScope == kInvalidScopeID || 
                             parentScope == sm.table.globalScopeId());

    // --- Seed ScopeStack with ancestor chain ---
    if (!isTopLevel) {
        // Resolving a specialized sub-AST: preserve full module scope context
        std::vector<ScopeID> ancestors;
        ScopeID curr = parentScope;
        while (curr != kInvalidScopeID) {
            ancestors.push_back(curr);
            curr = sm.table.getScope(curr).parentId;
        }
        for (auto it = ancestors.rbegin(); it != ancestors.rend(); ++it) {
            sm.enterExistingScope(*it);
        }
    } else {
        sm.enterExistingScope(sm.table.globalScopeId());
    }

    // --- Pass 0: Builtins (always in global scope) ---
    if (!sm.table.containsInScope(Identifier(std::string_view("void")), sm.table.globalScopeId())) {
        sm.table.declareSymbol(Identifier(std::string_view("void")), SymbolKind::TypeAlias, sm.table.globalScopeId(), SourceLocation{}, nullptr);
    }
    if (!sm.table.containsInScope(Identifier(std::string_view("str")), sm.table.globalScopeId())) {
        sm.table.declareSymbol(Identifier(std::string_view("str")), SymbolKind::TypeAlias, sm.table.globalScopeId(), SourceLocation{}, nullptr);
    }

    if (isTopLevel) {
        // For top-level resolution, reset to global before passes (original behavior)
        sm.exitScope();
        sm.enterExistingScope(sm.table.globalScopeId());
    }
    
    if (diag_.hasErrors()) {
        std::cerr << "[DEBUG] Resolver::resolve: diag already has errors BEFORE pass1: " << diag_.errorCount() << " size: " << diag_.allDiagnostics().size() << "\n";
        for (const auto& d : diag_.allDiagnostics()) {
            std::cerr << "RAW ERROR: " << d.message << "\n";
        }
    }
    
    DeclarationVisitor pass1(sm);
    root->accept(pass1);
    
    if (diag_.hasErrors()) { 
        std::cerr << "[DEBUG] Resolver::resolve: Aborting after pass1 due to previous errors (" << diag_.errorCount() << ").\n";
        for (const auto& d : diag_.allDiagnostics()) {
            std::cerr << "RAW ERROR: " << d.message << "\n";
        }
        sm.exitScope(); return false; 
    }
    
    UseResolutionVisitor pass1_5(sm);
    root->accept(pass1_5);
    
    if (diag_.hasErrors()) { 
        std::cerr << "[DEBUG] Resolver::resolve: Aborting after pass1_5 due to previous errors.\n";
        sm.exitScope(); return false; 
    }
    
    std::cerr << "[DEBUG] Resolver::resolve: Running pass2.\n";
    ResolutionVisitor pass2(sm, diag_);
    root->accept(pass2);
    
    sm.exitScope();
    
    return !diag_.hasErrors();
}


} // namespace fl
