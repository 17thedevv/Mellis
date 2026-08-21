#include "mellis/FrontEnd/PlaceholderReplacer.h"

namespace fl {

void PlaceholderReplacer::visit(PlaceholderExpr& node) {
    auto it = args_.find(node.data.name);
    if (it != args_.end()) {
        const PlaceholderBinding& binding = it->second;
        if (binding.variadic) {
            diag_.error(node.data.loc, "variadic placeholder @" + node.data.name + " cannot be used in a single expression context");
            return;
        }
        if (binding.nodes.empty()) return;
        
        auto replacement = cloner_.clone(*binding.nodes[0]);
        result_ = std::move(replacement);
    } else {
        diag_.error(node.data.loc, "placeholder @" + node.data.name + " not bound");
    }
}

void PlaceholderReplacer::visit(PlaceholderStmt& node) {
    auto it = args_.find(node.data.name);
    if (it != args_.end()) {
        const PlaceholderBinding& binding = it->second;
        if (binding.variadic) {
            diag_.error(node.data.loc, "variadic placeholder @" + node.data.name + " cannot be used in a single statement context");
            return;
        }
        if (binding.nodes.empty()) return;

        auto replacement = cloner_.clone(*binding.nodes[0]);
        result_ = std::move(replacement);
    } else {
        diag_.error(node.data.loc, "placeholder @" + node.data.name + " not bound");
    }
}

std::vector<std::unique_ptr<ItemNode>> PlaceholderReplacer::transformItemList(std::vector<std::unique_ptr<ItemNode>> list) {
    std::vector<std::unique_ptr<ItemNode>> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (!item) continue;
        if (auto exprStmt = dynamic_cast<ExprStmtNode*>(item.get())) {
            if (auto placeholder = dynamic_cast<PlaceholderExpr*>(exprStmt->expr.get())) {
                auto it = args_.find(placeholder->data.name);
                if (it != args_.end() && it->second.variadic) {
                    // SPLICE!
                    for (const ASTNode* boundNode : it->second.nodes) {
                        auto cloned = cloner_.clone(*boundNode);
                        if (auto stmt = dynamic_cast<StmtNode*>(cloned.get())) {
                            cloned.release();
                            newList.push_back(std::unique_ptr<ItemNode>(stmt));
                        } else if (auto expr = dynamic_cast<ExprNode*>(cloned.get())) {
                            auto newExprStmt = std::make_unique<ExprStmtNode>();
                            newExprStmt->loc = expr->loc;
                            cloned.release();
                            newExprStmt->expr = std::unique_ptr<ExprNode>(expr);
                            newList.push_back(std::move(newExprStmt));
                        }
                    }
                    continue;
                }
            }
        }

                if (auto placeholder = dynamic_cast<PlaceholderStmt*>(item.get())) {
            auto it = args_.find(placeholder->data.name);
            if (it != args_.end() && it->second.variadic) {
                // SPLICE!
                for (const ASTNode* boundNode : it->second.nodes) {
                    auto cloned = cloner_.clone(*boundNode);
                    if (auto stmt = dynamic_cast<StmtNode*>(cloned.get())) {
                        cloned.release();
                        newList.push_back(std::unique_ptr<ItemNode>(stmt));
                    } else if (auto expr = dynamic_cast<ExprNode*>(cloned.get())) {
                        auto exprStmt = std::make_unique<ExprStmtNode>();
                        exprStmt->loc = expr->loc;
                        cloned.release();
                        exprStmt->expr = std::unique_ptr<ExprNode>(expr);
                        newList.push_back(std::move(exprStmt));
                    }
                }
                continue;
            }
        }

        if (auto forStmt = dynamic_cast<MacroExpandForStmt*>(item.get())) {
            auto it = args_.find(forStmt->listName);
            if (it == args_.end() || !it->second.variadic) {
                diag_.error(forStmt->loc, "placeholder @" + forStmt->listName + " is not bound to a variadic list");
                continue; // Skip this node
            }
            const PlaceholderBinding& binding = it->second;

            // Expand body for each item in the variadic list
            for (const ASTNode* boundNode : binding.nodes) {
                // We create a new Replacer context!
                // Wait! We can just modify our args map temporarily? But args_ is const ref!
                // So we copy the map, update it, and run a new PlaceholderReplacer on the cloned body!
                std::unordered_map<std::string, PlaceholderBinding> newArgs = args_;
                newArgs[forStmt->iterName] = PlaceholderBinding{false, {boundNode}};
                
                PlaceholderReplacer subReplacer(newArgs, expId_, diag_);
                auto clonedBody = cloner_.clone(*forStmt->body);
                
                // The body is a BlockStmtNode which contains a list of items
                auto transformedBody = subReplacer.transformNode(std::move(clonedBody));
                if (auto block = dynamic_cast<BlockStmtNode*>(transformedBody.get())) {
                    for (auto& st : block->body) {
                        newList.push_back(std::move(st));
                    }
                }
            }
        } else {
            auto transformed = transformNode(std::move(item));
            if (transformed) {
                newList.push_back(std::unique_ptr<ItemNode>(static_cast<ItemNode*>(transformed.release())));
            }
        }
    }
    return newList;
}


std::vector<std::unique_ptr<ExprNode>> PlaceholderReplacer::transformExprList(std::vector<std::unique_ptr<ExprNode>> list) {
    std::vector<std::unique_ptr<ExprNode>> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (!item) continue;
        
        if (auto placeholder = dynamic_cast<PlaceholderExpr*>(item.get())) {
            auto it = args_.find(placeholder->data.name);
            if (it != args_.end() && it->second.variadic) {
                // SPLICE!
                for (const ASTNode* boundNode : it->second.nodes) {
                    auto cloned = cloner_.clone(*boundNode);
                    if (auto expr = dynamic_cast<ExprNode*>(cloned.get())) {
                        cloned.release();
                        newList.push_back(std::unique_ptr<ExprNode>(expr));
                    }
                }
                continue;
            }
        }

        auto transformed = transformExpr(std::move(item));
        if (transformed) {
            newList.push_back(std::move(transformed));
        }
    }
    return newList;
}

std::vector<std::unique_ptr<StmtNode>> PlaceholderReplacer::transformStmtList(std::vector<std::unique_ptr<StmtNode>> list) {
    std::vector<std::unique_ptr<StmtNode>> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (!item) continue;
        if (auto exprStmt = dynamic_cast<ExprStmtNode*>(item.get())) {
            if (auto placeholder = dynamic_cast<PlaceholderExpr*>(exprStmt->expr.get())) {
                auto it = args_.find(placeholder->data.name);
                if (it != args_.end() && it->second.variadic) {
                    // SPLICE!
                    for (const ASTNode* boundNode : it->second.nodes) {
                        auto cloned = cloner_.clone(*boundNode);
                        if (auto stmt = dynamic_cast<StmtNode*>(cloned.get())) {
                            cloned.release();
                            newList.push_back(std::unique_ptr<StmtNode>(stmt));
                        } else if (auto expr = dynamic_cast<ExprNode*>(cloned.get())) {
                            auto newExprStmt = std::make_unique<ExprStmtNode>();
                            newExprStmt->loc = expr->loc;
                            cloned.release();
                            newExprStmt->expr = std::unique_ptr<ExprNode>(expr);
                            newList.push_back(std::move(newExprStmt));
                        }
                    }
                    continue;
                }
            }
        }
        
        if (auto placeholder = dynamic_cast<PlaceholderStmt*>(item.get())) {
            auto it = args_.find(placeholder->data.name);
            if (it != args_.end() && it->second.variadic) {
                // SPLICE!
                for (const ASTNode* boundNode : it->second.nodes) {
                    auto cloned = cloner_.clone(*boundNode);
                    if (auto stmt = dynamic_cast<StmtNode*>(cloned.get())) {
                        cloned.release();
                        newList.push_back(std::unique_ptr<StmtNode>(stmt));
                    } else if (auto expr = dynamic_cast<ExprNode*>(cloned.get())) {
                        auto exprStmt = std::make_unique<ExprStmtNode>();
                        exprStmt->loc = expr->loc;
                        cloned.release();
                        exprStmt->expr = std::unique_ptr<ExprNode>(expr);
                        newList.push_back(std::move(exprStmt));
                    }
                }
                continue;
            }
        }

        auto transformed = transformStmt(std::move(item));
        if (transformed) {
            newList.push_back(std::move(transformed));
        }
    }
    return newList;
}

std::vector<MacroCallArgNode> PlaceholderReplacer::transformMacroCallArgList(std::vector<MacroCallArgNode> list) {
    std::vector<MacroCallArgNode> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (item.node) {
            if (auto placeholder = dynamic_cast<PlaceholderExpr*>(item.node.get())) {
                auto it = args_.find(placeholder->data.name);
                if (it != args_.end() && it->second.variadic) {
                    // SPLICE!
                    for (const ASTNode* boundNode : it->second.nodes) {
                        auto cloned = cloner_.clone(*boundNode);
                        newList.push_back(MacroCallArgNode{std::move(cloned)});
                    }
                    continue;
                }
            } else if (auto placeholderStmt = dynamic_cast<PlaceholderStmt*>(item.node.get())) {
                auto it = args_.find(placeholderStmt->data.name);
                if (it != args_.end() && it->second.variadic) {
                    // SPLICE!
                    for (const ASTNode* boundNode : it->second.nodes) {
                        auto cloned = cloner_.clone(*boundNode);
                        newList.push_back(MacroCallArgNode{std::move(cloned)});
                    }
                    continue;
                }
            }

            item.node = transformNode(std::move(item.node));
            if (item.node) newList.push_back(std::move(item));
        }
    }
    return newList;
}
} // namespace fl
