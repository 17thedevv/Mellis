// =============================================================================
// mellis/MiddleEnd/ComptimeEvaluator.cpp
// =============================================================================
#include "mellis/MiddleEnd/ComptimeEvaluator.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/ProgramNode.h"
#include <iostream>
#include <charconv>

namespace fl {

ComptimeEvaluator::ComptimeEvaluator(DiagnosticEngine& diag)
    : diag_(diag) {}

bool ComptimeEvaluator::evaluateComptimeBlocks(ASTNode* root) {
    if (!root) return true;
    
    // We traverse the AST. When we encounter a ComptimeStmtNode, we evaluate it.
    root->accept(*this);
    
    return !diag_.hasErrors();
}

void ComptimeEvaluator::visit(ProgramNode& node) {
    for (auto& item : node.items) {
        if (item) item->accept(*this);
    }
}

void ComptimeEvaluator::visit(ComptimeStmtNode& node) {
    // Create a new environment for the comptime block
    ComptimeEnvironment env(currentEnv_);
    ComptimeEnvironment* oldEnv = currentEnv_;
    currentEnv_ = &env;

    if (node.body) {
        node.body->accept(*this);
    }

    currentEnv_ = oldEnv;
}

void ComptimeEvaluator::visit(BlockStmtNode& node) {
    ComptimeEnvironment env(currentEnv_);
    ComptimeEnvironment* oldEnv = currentEnv_;
    currentEnv_ = &env;

    for (auto& item : node.body) {
        if (item) item->accept(*this);
        // In a real interpreter, we'd check for returns/breaks here
    }

    currentEnv_ = oldEnv;
}

void ComptimeEvaluator::visit(VarDeclNode& node) {
    if (node.isMutable) return;

    ComptimeValue val = ComptimeValue::makeVoid();
    if (node.initializer) {
        val = evaluateExpr(node.initializer.get());
    }
    
    if (val.isError()) {
        diag_.error(node.loc, "Failed to evaluate comptime variable initializer");
        return;
    }
    
    if (currentEnv_) {
        currentEnv_->set(std::string(node.name), val);
    }
}

void ComptimeEvaluator::visit(AssignExpr& node) {
    if (auto* idNode = dynamic_cast<IdentifierExpr*>(node.lvalue.get())) {
        ComptimeValue val = evaluateExpr(node.value.get());
        if (!val.isError() && currentEnv_ && !idNode->segments.empty()) {
            std::string varName(idNode->segments.front());
            // Note: A strict interpreter should check if the variable exists and is mutable
            currentEnv_->set(varName, val);
            lastResult_ = val;
            return;
        }
    }
    diag_.error(node.loc, "Unsupported assignment in comptime block");
    lastResult_ = ComptimeValue::makeError();
}

void ComptimeEvaluator::visit(ExprStmtNode& node) {
    if (node.expr) {
        evaluateExpr(node.expr.get());
    }
}

void ComptimeEvaluator::visit(LiteralExpr& node) {
    switch(node.kind) {
        case LiteralKind::Integer: {
            int64_t v = 0;
            std::from_chars(node.rawText.data(), node.rawText.data() + node.rawText.size(), v);
            lastResult_ = ComptimeValue::makeInt(v);
            break;
        }
        case LiteralKind::Float: {
            double v = 0.0;
            // Use std::stod for simple double parsing since from_chars for double requires newer C++ versions or specific compiler flags
            v = std::stod(std::string(node.rawText));
            lastResult_ = ComptimeValue::makeFloat(v);
            break;
        }
        case LiteralKind::Bool: {
            lastResult_ = ComptimeValue::makeBool(node.rawText == "true");
            break;
        }
        case LiteralKind::Str:
        case LiteralKind::RawStr:
        case LiteralKind::Char:
        case LiteralKind::Byte:
        case LiteralKind::ByteStr:
            lastResult_ = ComptimeValue::makeString(std::string(node.rawText));
            break;
        default:
            lastResult_ = ComptimeValue::makeError();
            break;
    }
}

void ComptimeEvaluator::visit(IdentifierExpr& node) {
    if (currentEnv_ && !node.segments.empty()) {
        std::string varName(node.segments.front());
        ComptimeValue val = currentEnv_->get(varName);
        if (val.isError()) {
            diag_.error(node.loc, "Undefined comptime variable: " + varName);
        }
        lastResult_ = val;
    } else {
        lastResult_ = ComptimeValue::makeError();
    }
}

void ComptimeEvaluator::visit(BinaryExpr& node) {
    ComptimeValue left = evaluateExpr(node.left.get());
    ComptimeValue right = evaluateExpr(node.right.get());

    if (left.isError() || right.isError()) {
        lastResult_ = ComptimeValue::makeError();
        return;
    }

    if (left.kind == ComptimeValue::Kind::Int && right.kind == ComptimeValue::Kind::Int) {
        int64_t l = left.iVal;
        int64_t r = right.iVal;
        switch (node.op) {
            case BinaryOp::Add: lastResult_ = ComptimeValue::makeInt(l + r); break;
            case BinaryOp::Sub: lastResult_ = ComptimeValue::makeInt(l - r); break;
            case BinaryOp::Mul: lastResult_ = ComptimeValue::makeInt(l * r); break;
            case BinaryOp::Div: 
                if (r == 0) {
                    diag_.error(node.loc, "Division by zero in comptime evaluation");
                    lastResult_ = ComptimeValue::makeError();
                } else {
                    lastResult_ = ComptimeValue::makeInt(l / r); 
                }
                break;
            default:
                diag_.error(node.loc, "Unsupported binary operator for integers in comptime");
                lastResult_ = ComptimeValue::makeError();
        }
    } else if (left.kind == ComptimeValue::Kind::Float && right.kind == ComptimeValue::Kind::Float) {
        double l = left.fVal;
        double r = right.fVal;
        switch (node.op) {
            case BinaryOp::Add: lastResult_ = ComptimeValue::makeFloat(l + r); break;
            case BinaryOp::Sub: lastResult_ = ComptimeValue::makeFloat(l - r); break;
            case BinaryOp::Mul: lastResult_ = ComptimeValue::makeFloat(l * r); break;
            case BinaryOp::Div: lastResult_ = ComptimeValue::makeFloat(l / r); break;
            default:
                diag_.error(node.loc, "Unsupported binary operator for floats in comptime");
                lastResult_ = ComptimeValue::makeError();
        }
    } else {
        diag_.error(node.loc, "Type mismatch or unsupported types in comptime binary expression");
        lastResult_ = ComptimeValue::makeError();
    }
}

void ComptimeEvaluator::visit(FunctionDeclNode& node) {
    // If we are traversing the AST to find comptime blocks, we should visit function bodies.
    if (node.body) {
        node.body->accept(*this);
    }
}

ComptimeValue ComptimeEvaluator::evaluateExpr(ExprNode* expr) {
    if (!expr) return ComptimeValue::makeVoid();
    expr->accept(*this);
    return popResult();
}

} // namespace fl
