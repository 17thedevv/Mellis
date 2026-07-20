// =============================================================================
// mellis/FrontEnd/ImportResolver.cpp
//
// ImportResolver — bridges `use` declarations to the live SymbolTable.
//
// Logic for visit(UseDeclNode&):
//   1. Walk the UseTreeNode, building the full path.
//   2. If path[0] matches a locally-declared `mod` name → skip (handled by
//      UseResolutionVisitor in Pass 1.5 of the Resolver).
//   3. Otherwise → external path: ask ModuleLoader to load the module,
//      creating a Virtual Scope. Then walk sub-paths within that scope and
//      register SymbolKind::Alias entries in the current scope.
// =============================================================================

#include "mellis/FrontEnd/ImportResolver.h"
#include "mellis/MiddleEnd/Symbol.h"

namespace fl {

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

// Returns true if `name` is already declared as a Module in the global scope.
static bool isLocalModule(SymbolTable& st, std::string_view name) {
    auto optSym = st.lookupInScope(name, /* global scope = */ 0);
    if (!optSym) return false;
    return st.getSymbol(*optSym).kind == SymbolKind::Module;
}

// ─────────────────────────────────────────────────────────────────────────────
// Process a single resolved leaf: declare an Alias in the current scope.
// ─────────────────────────────────────────────────────────────────────────────

void processLeaf(SymbolTable& st,
                 ScopeID currentScope,
                 SymbolID targetID,
                 std::string_view localName,
                 SourceLocation loc) {
    Identifier aliasIdent(localName);
    if (st.containsInScope(aliasIdent, currentScope)) return; // already declared

    SymbolID aliasID = st.declareSymbol(aliasIdent, SymbolKind::Alias,
                                        currentScope, loc, nullptr);
    if (aliasID != kInvalidSymbolID) {
        st.getMutableSymbol(aliasID).aliasTo = targetID;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Recursive tree walker for external paths
// ─────────────────────────────────────────────────────────────────────────────

static void walkExternalTree(SymbolTable& st,
                             DiagnosticEngine& diag,
                             ScopeID currentScope,
                             ScopeID virtualScope,
                             const UseTreeNode& tree,
                             const std::vector<std::string_view>& basePath) {
    std::vector<std::string_view> fullPath = basePath;
    fullPath.insert(fullPath.end(), tree.segments.begin(), tree.segments.end());

    if (tree.isGlob) {
        diag.error(tree.loc, "Glob imports ('*') not yet supported.");
        return;
    }

    if (tree.children.empty()) {
        // Leaf node: lookup the final name in the virtual scope.
        if (fullPath.empty()) return;
        std::string_view leafName = fullPath.back();

        auto optSym = st.lookupInScope(leafName, virtualScope);
        if (!optSym) {
            diag.error(tree.loc, "Symbol '" + std::string(leafName)
                       + "' not found in external module.");
            return;
        }

        std::string_view localName = tree.alias.empty() ? leafName : tree.alias;
        processLeaf(st, currentScope, *optSym, localName, tree.loc);
    } else {
        for (const auto& child : tree.children) {
            walkExternalTree(st, diag, currentScope, virtualScope, child, fullPath);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Visitor implementations
// ─────────────────────────────────────────────────────────────────────────────

void ImportResolver::visit(ProgramNode& node) {
    for (const auto& item : node.items) {
        if (item) item->accept(*this);
    }
}

void ImportResolver::visit(ModDeclNode& node) {
    for (const auto& decl : node.decls) {
        if (decl) decl->accept(*this);
    }
}

void ImportResolver::visit(UseDeclNode& node) {
    const UseTreeNode& tree = node.tree;
    if (tree.segments.empty()) return;

    std::string_view rootName = tree.segments[0];

    // Is the root a local module already declared in scope?
    // If so, the existing UseResolutionVisitor (Pass 1.5) handles it.
    if (isLocalModule(symbolTable_, rootName)) return;

    // External path: ask ModuleLoader for the virtual scope.
    ScopeID virtualScope = moduleLoader_.loadModule(rootName, tree.loc);
    if (virtualScope == kInvalidScopeID) return; // error already emitted

    // Register an alias for the top-level module name itself in local scope.
    // (so `std` can be used as `std::Vec` in expressions)
    {
        Identifier moduleIdent(rootName);
        if (!symbolTable_.containsInScope(moduleIdent, currentScope_)) {
            // Declare as ExternalModule kind so Resolver knows to look it up
            // in its virtual scope rather than walking the scope chain.
            SymbolID modSymID = symbolTable_.declareSymbol(
                moduleIdent, SymbolKind::ExternalModule, currentScope_,
                tree.loc, nullptr);
            if (modSymID != kInvalidSymbolID) {
                // Store the virtual scope ID as mlibSymbolID (reusing the field
                // to carry scope information to TypeChecker).
                symbolTable_.getMutableSymbol(modSymID).mlibSymbolID =
                    static_cast<uint32_t>(virtualScope);
                symbolTable_.getMutableSymbol(modSymID).isExternal = true;
            }
        }
    }

    // Now walk the sub-tree to register individual symbol aliases.
    // Build a starting path that skips the root module name.
    UseTreeNode subtree = tree;
    subtree.segments.clear(); // strip root; the virtualScope IS the root

    // If there are no further children/segments, we've imported the whole
    // module (just the name is enough for qualified access).
    if (tree.children.empty() && tree.segments.size() == 1) return;

    walkExternalTree(symbolTable_, diag_, currentScope_, virtualScope,
                     tree, {});
}

} // namespace fl
