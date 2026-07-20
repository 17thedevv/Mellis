#pragma once
#include "mellis/FrontEnd/ASTTransformer.h"
#include "mellis/Core/Types.h"
#include "mellis/Support/Diagnostic.h"
#include "mellis/FrontEnd/ASTCloner.h"
#include <unordered_map>
#include <string>

namespace fl {

struct PlaceholderBinding {
    bool variadic = false;
    std::vector<const ASTNode*> nodes;
};

class PlaceholderReplacer : public ASTTransformer {
public:
    PlaceholderReplacer(const std::unordered_map<std::string, PlaceholderBinding>& args, 
                        ExpansionID expId, 
                        DiagnosticEngine& diag)
        : args_(args), expId_(expId), diag_(diag) {}

    // When we visit a node, we should set its expansion ID.
    std::unique_ptr<ASTNode> transformNode(std::unique_ptr<ASTNode> node) {
        if (!node) return nullptr;
        node->expansionID = expId_;
        return ASTTransformer::transformNode(std::move(node));
    }

    // List Transformers for Variadic Splicing
    std::vector<std::unique_ptr<ItemNode>> transformItemList(std::vector<std::unique_ptr<ItemNode>> list) override;
    std::vector<std::unique_ptr<ExprNode>> transformExprList(std::vector<std::unique_ptr<ExprNode>> list) override;
    std::vector<std::unique_ptr<StmtNode>> transformStmtList(std::vector<std::unique_ptr<StmtNode>> list) override;
    std::vector<MacroCallArgNode> transformMacroCallArgList(std::vector<MacroCallArgNode> list) override;

    void visit(PlaceholderExpr& node) override;
    void visit(PlaceholderStmt& node) override;
    // We can also override visit(PlaceholderTypeNode&) if needed.

private:
    const std::unordered_map<std::string, PlaceholderBinding>& args_;
    ExpansionID expId_;
    DiagnosticEngine& diag_;
    ASTCloner cloner_;
};

} // namespace fl
