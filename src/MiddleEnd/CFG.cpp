#include "fdlang/MiddleEnd/CFG.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/TypeNode.h"
#include <queue>

namespace fl {

std::unique_ptr<CFG> CFGBuilder::build(FunctionDeclNode* func) {
    auto newCfg = std::make_unique<CFG>();
    cfg = newCfg.get();
    
    BasicBlock* entry = cfg->createBlock();
    BasicBlock* exit = cfg->createBlock();
    
    cfg->entry = entry;
    cfg->exit = exit;
    
    currentBlock = entry;
    
    if (func->body) {
        visit(func->body.get());
    }
    
    // If the last block didn't unconditionally jump/return, link it to exit
    if (currentBlock && !currentBlock->hasReturn) {
        currentBlock->addSuccessor(exit);
    }
    
    return newCfg;
}

void CFGBuilder::visit(ASTNode* node) {
    if (!node || !currentBlock) return;
    
    if (auto* block = dynamic_cast<BlockStmtNode*>(node)) visitBlock(block);
    else if (auto* ifs = dynamic_cast<IfStmtNode*>(node)) visitIf(ifs);
    else if (auto* whiles = dynamic_cast<WhileStmtNode*>(node)) visitWhile(whiles);
    else if (auto* fors = dynamic_cast<ForStmtNode*>(node)) visitFor(fors);
    else if (auto* rets = dynamic_cast<ReturnStmtNode*>(node)) visitReturn(rets);
    else if (auto* brks = dynamic_cast<BreakStmtNode*>(node)) visitBreak(brks);
    else if (auto* cont = dynamic_cast<ContinueStmtNode*>(node)) visitContinue(cont);
    else if (auto* match = dynamic_cast<MatchExpr*>(node)) visitMatch(match);
    else if (auto* exprStmt = dynamic_cast<ExprStmtNode*>(node)) {
        currentBlock->addElement(node);
        visit(exprStmt->expr.get());
    }
    else {
        currentBlock->addElement(node);
    }
}

void CFGBuilder::visitBlock(BlockStmtNode* node) {
    for (auto& item : node->body) {
        visit(item.get());
        if (!currentBlock) break; // Unreachable code after this
    }
}

void CFGBuilder::visitIf(IfStmtNode* node) {
    visit(node->condition.get());
    
    BasicBlock* thenBlock = cfg->createBlock();
    BasicBlock* elseBlock = cfg->createBlock();
    BasicBlock* mergeBlock = cfg->createBlock();
    
    currentBlock->addSuccessor(thenBlock);
    currentBlock->addSuccessor(elseBlock);
    
    // Then branch
    currentBlock = thenBlock;
    visit(node->thenBranch.get());
    if (currentBlock) currentBlock->addSuccessor(mergeBlock);
    
    // Else branch
    currentBlock = elseBlock;
    if (node->elseBranch) {
        visit(node->elseBranch.get());
    }
    if (currentBlock) currentBlock->addSuccessor(mergeBlock);
    
    // Continue from merge block
    currentBlock = mergeBlock;
}

void CFGBuilder::visitWhile(WhileStmtNode* node) {
    BasicBlock* condBlock = cfg->createBlock();
    BasicBlock* bodyBlock = cfg->createBlock();
    BasicBlock* exitBlock = cfg->createBlock();
    
    currentBlock->addSuccessor(condBlock);
    
    currentBlock = condBlock;
    visit(node->condition.get());
    currentBlock->addSuccessor(bodyBlock);
    currentBlock->addSuccessor(exitBlock);
    
    breakTargets.push_back(exitBlock);
    continueTargets.push_back(condBlock);
    
    currentBlock = bodyBlock;
    visit(node->body.get());
    if (currentBlock) currentBlock->addSuccessor(condBlock);
    
    breakTargets.pop_back();
    continueTargets.pop_back();
    
    currentBlock = exitBlock;
}

void CFGBuilder::visitFor(ForStmtNode* node) {
    BasicBlock* condBlock = cfg->createBlock();
    BasicBlock* bodyBlock = cfg->createBlock();
    BasicBlock* stepBlock = cfg->createBlock();
    BasicBlock* exitBlock = cfg->createBlock();
    
    if (node->init) visit(node->init.get());
    currentBlock->addSuccessor(condBlock);
    
    currentBlock = condBlock;
    if (node->cond) visit(node->cond.get());
    currentBlock->addSuccessor(bodyBlock);
    currentBlock->addSuccessor(exitBlock);
    
    breakTargets.push_back(exitBlock);
    continueTargets.push_back(stepBlock);
    
    currentBlock = bodyBlock;
    visit(node->body.get());
    if (currentBlock) currentBlock->addSuccessor(stepBlock);
    
    currentBlock = stepBlock;
    if (node->step) visit(node->step.get());
    currentBlock->addSuccessor(condBlock);
    
    breakTargets.pop_back();
    continueTargets.pop_back();
    
    currentBlock = exitBlock;
}

void CFGBuilder::visitMatch(MatchExpr* node) {
    visit(node->subject.get());
    
    BasicBlock* mergeBlock = cfg->createBlock();
    BasicBlock* prevBlock = currentBlock;
    
    for (auto& arm : node->arms) {
        BasicBlock* armBlock = cfg->createBlock();
        prevBlock->addSuccessor(armBlock);
        
        currentBlock = armBlock;
        if (arm.body) visit(arm.body.get());
        if (currentBlock) currentBlock->addSuccessor(mergeBlock);
    }
    
    currentBlock = mergeBlock;
}

void CFGBuilder::visitReturn(ReturnStmtNode* node) {
    if (node->value) visit(node->value.get());
    currentBlock->hasReturn = true;
    currentBlock->addSuccessor(cfg->exit);
    currentBlock = nullptr; // Next statements are unreachable
}

void CFGBuilder::visitBreak(BreakStmtNode* node) {
    if (breakTargets.empty()) {
        diag.error(node->loc, "'break' outside of loop");
    } else {
        currentBlock->addSuccessor(breakTargets.back());
    }
    currentBlock = nullptr;
}

void CFGBuilder::visitContinue(ContinueStmtNode* node) {
    if (continueTargets.empty()) {
        diag.error(node->loc, "'continue' outside of loop");
    } else {
        currentBlock->addSuccessor(continueTargets.back());
    }
    currentBlock = nullptr;
}

bool ControlFlowAnalyzer::analyze(FunctionDeclNode* func) {
    CFGBuilder builder(diag);
    auto cfg = builder.build(func);
    
    // Mark reachable blocks using BFS
    std::queue<BasicBlock*> q;
    q.push(cfg->entry);
    cfg->entry->isReachable = true;
    
    while (!q.empty()) {
        BasicBlock* curr = q.front();
        q.pop();
        for (auto* succ : curr->successors) {
            if (!succ->isReachable) {
                succ->isReachable = true;
                q.push(succ);
            }
        }
    }
    
    // Check missing return
    // A function needs a return if its return type is not 'void'
    bool needsReturn = false;
    if (func->returnType) {
        if (auto* rt = dynamic_cast<BuiltinTypeNode*>(func->returnType.get())) {
            if (rt->kind != BuiltinKind::Void) needsReturn = true;
        } else {
            needsReturn = true; // Not a simple 'void' type
        }
    }
    
    if (needsReturn) {
        // If the exit block is reachable from a path that doesn't end in 'return'
        // wait, how to check this? 
        // Any predecessor of 'exit' that doesn't have 'hasReturn = true' means it just fell through!
        bool missingReturn = false;
        for (auto* pred : cfg->exit->predecessors) {
            if (pred->isReachable && !pred->hasReturn) {
                missingReturn = true;
                break;
            }
        }
        
        if (missingReturn) {
            diag.error(func->loc, "Function '" + std::string(func->name) + "' does not return a value on all paths");
            return false;
        }
    }
    
    return true;
}

} // namespace fl
