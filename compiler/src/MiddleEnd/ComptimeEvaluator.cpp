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

ComptimeEvaluator::ComptimeEvaluator(DiagnosticEngine& diag, const SymbolTable& symbolTable)
    : diag_(diag), symbolTable_(symbolTable) {}

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
    bool oldExecuting = isExecuting_;
    isExecuting_ = true;

    // Create a new environment for the comptime block
    ComptimeEnvironment env(currentEnv_);
    ComptimeEnvironment* oldEnv = currentEnv_;
    currentEnv_ = &env;

    if (node.body) {
        node.body->accept(*this);
    }

    currentEnv_ = oldEnv;
    isExecuting_ = oldExecuting;
}

void ComptimeEvaluator::visit(BlockStmtNode& node) {
    if (!isExecuting_) {
        for (auto& item : node.body) {
            if (item) item->accept(*this);
        }
        return;
    }

    ComptimeEnvironment env(currentEnv_);
    ComptimeEnvironment* oldEnv = currentEnv_;
    currentEnv_ = &env;

    for (auto& item : node.body) {
        if (item) item->accept(*this);
        if (hasReturned_ || hasBroken_ || hasContinued_) break; // Stop executing current block
    }

    currentEnv_ = oldEnv;
}

void ComptimeEvaluator::visit(VarDeclNode& node) {
    if (!isExecuting_) {
        if (node.initializer) node.initializer->accept(*this);
        return;
    }

    mvir::ConstantValue val = mvir::ConstantValue::makeVoid();
    if (node.initializer) {
        val = evaluateExpr(node.initializer.get());
    }
    
    if (val.isError()) {
        diag_.error(node.loc, "Failed to evaluate comptime variable initializer");
        return;
    }
    
    if (currentEnv_) {
        currentEnv_->set(std::string(node.name), val);
        std::cout << "[COMPTIME] Set " << node.name << " = " << val.toString() << "\n";
    }
}

void ComptimeEvaluator::visit(AssignExpr& node) {
    if (auto* idNode = dynamic_cast<IdentifierExpr*>(node.lvalue.get())) {
        mvir::ConstantValue val = evaluateExpr(node.value.get());
        if (!val.isError() && currentEnv_ && !idNode->segments.empty()) {
            std::string varName(idNode->segments.front());
            // Note: A strict interpreter should check if the variable exists and is mutable
            currentEnv_->set(varName, val);
            lastResult_ = val;
            return;
        }
    }
    // diag_.error(node.loc, "Unsupported assignment in comptime block");
    lastResult_ = mvir::ConstantValue::makeError();
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
            lastResult_ = mvir::ConstantValue::makeInt(v);
            break;
        }
        case LiteralKind::Float: {
            double v = 0.0;
            // Use std::stod for simple double parsing since from_chars for double requires newer C++ versions or specific compiler flags
            v = std::stod(std::string(node.rawText));
            lastResult_ = mvir::ConstantValue::makeFloat(v);
            break;
        }
        case LiteralKind::Bool: {
            lastResult_ = mvir::ConstantValue::makeBool(node.rawText == "true");
            break;
        }
        case LiteralKind::Str:
        case LiteralKind::RawStr:
        case LiteralKind::Char:
        case LiteralKind::Byte:
        case LiteralKind::ByteStr:
            lastResult_ = mvir::ConstantValue::makeString(std::string(node.rawText));
            break;
        default:
            lastResult_ = mvir::ConstantValue::makeError();
            break;
    }
}

void ComptimeEvaluator::visit(IdentifierExpr& node) {
    if (currentEnv_ && !node.segments.empty()) {
        std::string varName(node.segments.front());
        mvir::ConstantValue val = currentEnv_->get(varName);
        if (val.isError()) {
             std::cout << "[COMPTIME ERROR] Undefined variable: " << varName << "\n";
        }
        lastResult_ = val;
    } else {
        std::cout << "[COMPTIME ERROR] IdentifierExpr without env or empty segments\n";
        lastResult_ = mvir::ConstantValue::makeError();
    }
}

void ComptimeEvaluator::visit(BinaryExpr& node) {
    mvir::ConstantValue left = evaluateExpr(node.left.get());
    mvir::ConstantValue right = evaluateExpr(node.right.get());

    if (left.isError()) {
        std::cout << "[COMPTIME ERROR] BinaryExpr left operand failed.\n";
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }
    if (right.isError()) {
        std::cout << "[COMPTIME ERROR] BinaryExpr right operand failed.\n";
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }

    if (left.kind == mvir::ConstantValue::Kind::Int && right.kind == mvir::ConstantValue::Kind::Int) {
        int64_t l = left.iVal;
        int64_t r = right.iVal;
        switch (node.op) {
            case BinaryOp::Add: lastResult_ = mvir::ConstantValue::makeInt(l + r); break;
            case BinaryOp::Sub: lastResult_ = mvir::ConstantValue::makeInt(l - r); break;
            case BinaryOp::Mul: lastResult_ = mvir::ConstantValue::makeInt(l * r); break;
            case BinaryOp::Div: 
                if (r == 0) {
                    diag_.error(node.loc, "Division by zero in comptime evaluation");
                    lastResult_ = mvir::ConstantValue::makeError();
                } else {
                    lastResult_ = mvir::ConstantValue::makeInt(l / r); 
                }
                break;
            case BinaryOp::Mod:
                if (r == 0) {
                    diag_.error(node.loc, "Modulo by zero in comptime evaluation");
                    lastResult_ = mvir::ConstantValue::makeError();
                } else {
                    lastResult_ = mvir::ConstantValue::makeInt(l % r);
                }
                break;
            case BinaryOp::BitAnd: lastResult_ = mvir::ConstantValue::makeInt(l & r); break;
            case BinaryOp::BitOr: lastResult_ = mvir::ConstantValue::makeInt(l | r); break;
            case BinaryOp::BitXor: lastResult_ = mvir::ConstantValue::makeInt(l ^ r); break;
            case BinaryOp::LShift: lastResult_ = mvir::ConstantValue::makeInt(l << r); break;
            case BinaryOp::RShift: lastResult_ = mvir::ConstantValue::makeInt(l >> r); break;
            case BinaryOp::Eq: lastResult_ = mvir::ConstantValue::makeBool(l == r); break;
            case BinaryOp::Ne: lastResult_ = mvir::ConstantValue::makeBool(l != r); break;
            case BinaryOp::Lt: lastResult_ = mvir::ConstantValue::makeBool(l < r); break;
            case BinaryOp::Le: lastResult_ = mvir::ConstantValue::makeBool(l <= r); break;
            case BinaryOp::Gt: lastResult_ = mvir::ConstantValue::makeBool(l > r); break;
            case BinaryOp::Ge: lastResult_ = mvir::ConstantValue::makeBool(l >= r); break;
            default:
                diag_.error(node.loc, "Unsupported binary operator for integers in comptime");
                lastResult_ = mvir::ConstantValue::makeError();
        }
    } else if (left.kind == mvir::ConstantValue::Kind::Float && right.kind == mvir::ConstantValue::Kind::Float) {
        double l = left.fVal;
        double r = right.fVal;
        switch (node.op) {
            case BinaryOp::Add: lastResult_ = mvir::ConstantValue::makeFloat(l + r); break;
            case BinaryOp::Sub: lastResult_ = mvir::ConstantValue::makeFloat(l - r); break;
            case BinaryOp::Mul: lastResult_ = mvir::ConstantValue::makeFloat(l * r); break;
            case BinaryOp::Div: 
                if (r == 0.0) {
                    diag_.error(node.loc, "Division by zero in comptime evaluation");
                    lastResult_ = mvir::ConstantValue::makeError();
                } else {
                    lastResult_ = mvir::ConstantValue::makeFloat(l / r);
                }
                break;
            case BinaryOp::Eq: lastResult_ = mvir::ConstantValue::makeBool(l == r); break;
            case BinaryOp::Ne: lastResult_ = mvir::ConstantValue::makeBool(l != r); break;
            case BinaryOp::Lt: lastResult_ = mvir::ConstantValue::makeBool(l < r); break;
            case BinaryOp::Le: lastResult_ = mvir::ConstantValue::makeBool(l <= r); break;
            case BinaryOp::Gt: lastResult_ = mvir::ConstantValue::makeBool(l > r); break;
            case BinaryOp::Ge: lastResult_ = mvir::ConstantValue::makeBool(l >= r); break;
            default:
                diag_.error(node.loc, "Unsupported binary operator for floats in comptime");
                lastResult_ = mvir::ConstantValue::makeError();
        }
    } else {
        diag_.error(node.loc, "Type mismatch or unsupported types in comptime binary expression");
        lastResult_ = mvir::ConstantValue::makeError();
    }
}

void ComptimeEvaluator::visit(FunctionDeclNode& node) {
    // If we are traversing the AST to find comptime blocks, we should visit function bodies.
    if (node.body) {
        node.body->accept(*this);
    }
}

void ComptimeEvaluator::visit(ReturnStmtNode& node) {
    if (!isExecuting_) return;
    hasReturned_ = true;
    if (node.value) {
        returnVal_ = evaluateExpr(node.value.get());
    } else {
        returnVal_ = mvir::ConstantValue::makeVoid();
    }
}

void ComptimeEvaluator::visit(BreakStmtNode& node) {
    if (!isExecuting_) return;
    hasBroken_ = true;
}

void ComptimeEvaluator::visit(ContinueStmtNode& node) {
    if (!isExecuting_) return;
    hasContinued_ = true;
}

void ComptimeEvaluator::visit(UnsafeStmtNode& node) {
    if (!isExecuting_) {
        if (node.body) node.body->accept(*this);
        return;
    }
    diag_.error(node.loc, "Unsafe blocks are not allowed in comptime context.");
    lastResult_ = mvir::ConstantValue::makeError();
}

void ComptimeEvaluator::visit(CallExpr& node) {
    if (!isExecuting_) {
        for (auto& arg : node.args) {
            if (arg.value) arg.value->accept(*this);
        }
        return;
    }
    // We must evaluate the callee
    // For now, we only support calling named functions directly (IdentifierExpr)
    auto idNode = dynamic_cast<IdentifierExpr*>(node.callee.get());
    if (!idNode || idNode->segments.empty()) {
        diag_.error(node.loc, "Comptime evaluator currently only supports direct function calls by name.");
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }

    FunctionDeclNode* funcDecl = nullptr;
    std::string funcName = "<unknown>";

    if (node.resolvedFn != kInvalidSymbolID) {
        const Symbol& sym = symbolTable_.getSymbol(node.resolvedFn);
        if (sym.kind == SymbolKind::Function && sym.decl) {
            funcDecl = dynamic_cast<FunctionDeclNode*>(sym.decl);
            funcName = sym.name.str();
        }
    }

    if (!funcDecl) {
        diag_.error(node.loc, "Could not resolve function for comptime call.");
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }

    if (!funcDecl->isComptime) {
        diag_.error(node.loc, "Cannot call non-comptime function '" + funcName + "' at compile time.");
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }

    // Evaluate arguments
    std::vector<mvir::ConstantValue> argVals;
    for (auto& arg : node.args) {
        mvir::ConstantValue v = evaluateExpr(arg.value.get());
        if (v.isError()) {
            diag_.error(arg.value->loc, "Failed to evaluate argument for comptime call.");
            lastResult_ = mvir::ConstantValue::makeError();
            return;
        }
        argVals.push_back(v);
    }

    if (argVals.size() != funcDecl->params.size()) {
        diag_.error(node.loc, "Arity mismatch in comptime call to '" + funcName + "'.");
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }

    ComptimeEnvironment callEnv; // Root environment for function call
    for (size_t i = 0; i < argVals.size(); ++i) {
        std::string paramName(funcDecl->params[i]->name);
        callEnv.set(paramName, argVals[i]);
        std::cout << "[COMPTIME] Bind param " << paramName << " = " << argVals[i].toString() << "\n";
    }

    ComptimeEnvironment* oldEnv = currentEnv_;
    currentEnv_ = &callEnv;

    bool oldHasReturned = hasReturned_;
    hasReturned_ = false;
    mvir::ConstantValue oldReturnVal = returnVal_;

    // Execute body
    if (funcDecl->body) {
        funcDecl->body->accept(*this);
    }

    lastResult_ = hasReturned_ ? returnVal_ : mvir::ConstantValue::makeVoid();
    std::cout << "[COMPTIME] Call to '" << funcName << "' returned " << lastResult_.toString() << "\n";

    // Restore state
    currentEnv_ = oldEnv;
    hasReturned_ = oldHasReturned;
    returnVal_ = oldReturnVal;
}

void ComptimeEvaluator::visit(IfStmtNode& node) {
    if (!isExecuting_) {
        if (node.condition) node.condition->accept(*this);
        if (node.thenBranch) node.thenBranch->accept(*this);
        if (node.elseBranch) node.elseBranch->accept(*this);
        return;
    }

    // Evaluate the condition
    mvir::ConstantValue cond = evaluateExpr(node.condition.get());
    
    if (cond.isError()) {
        diag_.error(node.loc, "Failed to evaluate if condition in comptime");
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }
    
    // Treat non-zero/non-empty values as true
    bool condVal = false;
    if (cond.kind == mvir::ConstantValue::Kind::Bool) {
        condVal = cond.bVal;
    } else if (cond.kind == mvir::ConstantValue::Kind::Int) {
        condVal = (cond.iVal != 0);
    } else if (cond.kind == mvir::ConstantValue::Kind::Float) {
        condVal = (cond.fVal != 0.0);
    }
    
    if (condVal) {
        if (node.thenBranch) {
            node.thenBranch->accept(*this);
        }
    } else if (node.elseBranch) {
        node.elseBranch->accept(*this);
    }
}

void ComptimeEvaluator::visit(WhileStmtNode& node) {
    if (!isExecuting_) {
        if (node.condition) node.condition->accept(*this);
        if (node.body) node.body->accept(*this);
        return;
    }

    while (true) {
        if (!node.condition) break;
        mvir::ConstantValue condVal = evaluateExpr(node.condition.get());
        if (condVal.isError()) {
            lastResult_ = condVal;
            return;
        }
        
        bool isTrue = false;
        if (condVal.kind == mvir::ConstantValue::Kind::Bool) {
            isTrue = condVal.bVal;
        } else if (condVal.kind == mvir::ConstantValue::Kind::Int) {
            isTrue = condVal.iVal != 0;
        } else {
            diag_.error(node.loc, "Condition in 'while' loop must be boolean.");
            lastResult_ = mvir::ConstantValue::makeError();
            return;
        }
        
        if (!isTrue) break;
        
        if (node.body) {
            node.body->accept(*this);
        }
        
        if (hasBroken_) {
            hasBroken_ = false;
            break;
        }
        if (hasContinued_) {
            hasContinued_ = false;
        }
        
        if (hasReturned_) break;
    }
}

void ComptimeEvaluator::visit(UnaryExpr& node) {
    mvir::ConstantValue val = evaluateExpr(node.operand.get());
    
    if (val.isError()) {
        lastResult_ = mvir::ConstantValue::makeError();
        return;
    }
    
    switch (node.op) {
        case UnaryOp::Not:
            if (val.kind == mvir::ConstantValue::Kind::Bool) {
                lastResult_ = mvir::ConstantValue::makeBool(!val.bVal);
            } else if (val.kind == mvir::ConstantValue::Kind::Int) {
                lastResult_ = mvir::ConstantValue::makeBool(val.iVal == 0);
            } else {
                diag_.error(node.loc, "Unsupported operand type for logical NOT in comptime");
                lastResult_ = mvir::ConstantValue::makeError();
            }
            break;
            
        case UnaryOp::Neg:
            if (val.kind == mvir::ConstantValue::Kind::Int) {
                lastResult_ = mvir::ConstantValue::makeInt(-val.iVal);
            } else if (val.kind == mvir::ConstantValue::Kind::Float) {
                lastResult_ = mvir::ConstantValue::makeFloat(-val.fVal);
            } else {
                diag_.error(node.loc, "Unsupported operand type for negation in comptime");
                lastResult_ = mvir::ConstantValue::makeError();
            }
            break;
            
        case UnaryOp::BitNot:
            if (val.kind == mvir::ConstantValue::Kind::Int) {
                lastResult_ = mvir::ConstantValue::makeInt(~val.iVal);
            } else {
                diag_.error(node.loc, "Bitwise NOT requires integer operand in comptime");
                lastResult_ = mvir::ConstantValue::makeError();
            }
            break;
            
        default:
            diag_.error(node.loc, "Unsupported unary operator in comptime");
            lastResult_ = mvir::ConstantValue::makeError();
    }
}

void ComptimeEvaluator::visit(ArrayLiteralExpr& node) {
    // For simplicity, store array literals as string representation
    std::string arrayStr = "[";
    bool first = true;
    for (auto& elem : node.elements) {
        if (!first) arrayStr += ", ";
        mvir::ConstantValue elemVal = evaluateExpr(elem.get());
        if (elemVal.isError()) {
            diag_.error(node.loc, "Failed to evaluate array element in comptime");
            lastResult_ = mvir::ConstantValue::makeError();
            return;
        }
        arrayStr += elemVal.toString();
        first = false;
    }
    arrayStr += "]";
    lastResult_ = mvir::ConstantValue::makeString(arrayStr);
}

void ComptimeEvaluator::visit(TupleLiteralExpr& node) {
    // For simplicity, store tuple literals as string representation
    std::string tupleStr = "(";
    bool first = true;
    for (auto& elem : node.elements) {
        if (!first) tupleStr += ", ";
        mvir::ConstantValue elemVal = evaluateExpr(elem.get());
        if (elemVal.isError()) {
            diag_.error(node.loc, "Failed to evaluate tuple element in comptime");
            lastResult_ = mvir::ConstantValue::makeError();
            return;
        }
        tupleStr += elemVal.toString();
        first = false;
    }
    tupleStr += ")";
    lastResult_ = mvir::ConstantValue::makeString(tupleStr);
}

mvir::ConstantValue ComptimeEvaluator::evaluateExpr(ExprNode* expr) {
    if (!expr) return mvir::ConstantValue::makeVoid();
    expr->accept(*this);
    return popResult();
}

} // namespace fl
