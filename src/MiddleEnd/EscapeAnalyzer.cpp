#include "mellis/MiddleEnd/EscapeAnalyzer.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/Core/CompilerSession.h"
#include <iostream>

namespace fl {

void EscapeAnalyzer::analyze(ASTNode* root) {
    if (root) root->accept(*this);
}

void EscapeAnalyzer::visit(ProgramNode& node) {
    for (auto& item : node.items) {
        if (item) item->accept(*this);
    }
}

void EscapeAnalyzer::visit(FunctionDeclNode& node) {
    if (node.body) node.body->accept(*this);
}

void EscapeAnalyzer::visit(BlockStmtNode& node) {
    for (auto& stmt : node.body) {
        if (stmt) stmt->accept(*this);
    }
}

void EscapeAnalyzer::visit(IfStmtNode& node) {
    if (node.condition) node.condition->accept(*this);
    if (node.thenBranch) node.thenBranch->accept(*this);
    if (node.elseBranch) node.elseBranch->accept(*this);
}

void EscapeAnalyzer::visit(WhileStmtNode& node) {
    if (node.condition) node.condition->accept(*this);
    if (node.body) node.body->accept(*this);
}

void EscapeAnalyzer::visit(ReturnStmtNode& node) {
    if (node.value) {
        markEscape(node.value.get());
        node.value->accept(*this);
    }
}

void EscapeAnalyzer::visit(AssignExpr& node) {
    // If assigning a closure to something that is not a local variable of the same scope.
    // For now, conservatively assume assigning to anything is an escape.
    if (node.value) {
        markEscape(node.value.get());
        node.value->accept(*this);
    }
    if (node.lvalue) {
        node.lvalue->accept(*this);
    }
}

void EscapeAnalyzer::visit(CallExpr& node) {
    if (node.callee) {
        node.callee->accept(*this);
    }
    
    // Check if arguments escape based on parameter EscapeBehavior
    const FunctionType* fnTy = nullptr;
    if (node.callee && node.callee->inferredType) {
        if (auto* t = dynamic_cast<const FunctionType*>(node.callee->inferredType)) {
            fnTy = t;
        } else if (auto* ct = dynamic_cast<const ClosureType*>(node.callee->inferredType)) {
            fnTy = ct->signature;
        }
    }
    
    for (size_t i = 0; i < node.args.size(); ++i) {
        bool escapes = true; // Conservatively assume escape
        if (fnTy && i < fnTy->paramEscapeBehaviors.size()) {
            escapes = (fnTy->paramEscapeBehaviors[i] == EscapeBehavior::Escaping);
        }
        
        if (escapes) {
            markEscape(node.args[i].value.get());
        }
        node.args[i].value->accept(*this);
    }
}

void EscapeAnalyzer::visit(VarDeclNode& node) {
    if (node.initializer) {
        node.initializer->accept(*this);
    }
}

void EscapeAnalyzer::visit(LambdaExpr& node) {
    // Record current lambda
    if (auto* cTy = dynamic_cast<const ClosureType*>(node.inferredType)) {
        // By default, assume it does not escape until proven otherwise
        // (This happens in EscapeAnalyzer.h/cpp when used in Return/Assign)
        
        // Check 0-capture copyability
        bool allCopy = true;
        for (const auto& cap : cTy->captures) {
            // Check if cap.envType is copyable. For now, we assume raw types are copyable.
            // But borrowed references (&T, &mut T) are NOT copyable implicitly in closures 
            if (cap.mode == CaptureMode::Borrow || cap.mode == CaptureMode::BorrowMut) {
                allCopy = false;
                break;
            }
        }
        
        // This mutation is safe because we own the ClosureType allocated in TypeTable
        // ClosureType* mutClosureTy = const_cast<ClosureType*>(cTy);
        // mutClosureTy->isCopy = allCopy; // TBD: more precise copyability
    }
    
    if (node.body) {
        node.body->accept(*this);
    }
}

void EscapeAnalyzer::visit(MatchExpr& node) {
    if (node.subject) node.subject->accept(*this);
    for (auto& arm : node.arms) {
        if (arm.body) arm.body->accept(*this);
    }
}

void EscapeAnalyzer::markEscape(ExprNode* expr) {
    if (!expr || !expr->inferredType) return;
    
    if (auto* closureTy = dynamic_cast<const ClosureType*>(expr->inferredType)) {
        markClosureEscaping(const_cast<ClosureType*>(closureTy), expr->loc);
    } else if (auto* ident = dynamic_cast<IdentifierExpr*>(expr)) {
        // If it's returning a variable that holds a closure, we need to mark that closure as escaping.
        // We can look up the symbol type in the type table/context.
        if (auto* cTy = dynamic_cast<const ClosureType*>(ident->inferredType)) {
            markClosureEscaping(const_cast<ClosureType*>(cTy), expr->loc);
        }
    }

    // TODO: Handle returning struct/tuple containing closures
}

void EscapeAnalyzer::markClosureEscaping(ClosureType* closureTy, SourceLocation loc) {
    if (!closureTy) return;
    
    storageMap_[closureTy] = ClosureStorageKind::Heap;
    
    checkBorrowEscape(closureTy, loc);
    propagateNestedCaptures(closureTy, loc);
}

void EscapeAnalyzer::propagateNestedCaptures(ClosureType* closureTy, SourceLocation loc) {
    // If this closure captures another closure by value (Move/Copy), the inner closure 
    // is now part of the outer's heap environment, so its lifetime is extended.
    // It must also be marked as escaping.
    
    for (auto& cap : closureTy->captures) {
        if (auto* innerTy = dynamic_cast<const ClosureType*>(cap.envType)) {
            if (cap.mode == CaptureMode::Move || cap.mode == CaptureMode::Copy) {
                markClosureEscaping(const_cast<ClosureType*>(innerTy), loc);
            }
        }
    }
}

void EscapeAnalyzer::checkBorrowEscape(const ClosureType* closureTy, SourceLocation loc) {
    if (storageMap_[closureTy] != ClosureStorageKind::Heap) return;
    
    for (const auto& cap : closureTy->captures) {
        if (cap.mode == CaptureMode::Borrow || cap.mode == CaptureMode::BorrowMut) {
            diag_.error(loc, "Closure captures local variable by reference but escapes its scope.");
        }
    }
}

} // namespace fl
