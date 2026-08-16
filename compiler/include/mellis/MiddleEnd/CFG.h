#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/Support/Diagnostic.h"
#include <vector>
#include <memory>

namespace fl {

class FunctionDeclNode;
class MatchExpr;
class BlockStmtNode;
class IfStmtNode;
class WhileStmtNode;
class ForStmtNode;
class ReturnStmtNode;
class BreakStmtNode;
class ContinueStmtNode;

class BasicBlock {
public:
    unsigned id;
    std::vector<ASTNode*> elements;
    std::vector<BasicBlock*> successors;
    std::vector<BasicBlock*> predecessors;

    // True if this block terminates the function (e.g., contains a 'return' statement)
    bool hasReturn = false;
    
    // True if this block is unreachable from the entry block
    bool isReachable = false;

    explicit BasicBlock(unsigned id) : id(id) {}

    void addSuccessor(BasicBlock* succ) {
        successors.push_back(succ);
        succ->predecessors.push_back(this);
    }
    
    void addElement(ASTNode* node) {
        elements.push_back(node);
    }
};

class CFG {
public:
    std::vector<std::unique_ptr<BasicBlock>> blocks;
    BasicBlock* entry = nullptr;
    BasicBlock* exit = nullptr;

    BasicBlock* createBlock() {
        auto block = std::make_unique<BasicBlock>(static_cast<unsigned>(blocks.size()));
        BasicBlock* ptr = block.get();
        blocks.push_back(std::move(block));
        return ptr;
    }
};

class CFGBuilder {
    DiagnosticEngine& diag;
    CFG* cfg;
    BasicBlock* currentBlock;
    
    // Loop control stacks
    std::vector<BasicBlock*> breakTargets;
    std::vector<BasicBlock*> continueTargets;
    
public:
    CFGBuilder(DiagnosticEngine& diag) : diag(diag), cfg(nullptr), currentBlock(nullptr) {}
    
    std::unique_ptr<CFG> build(FunctionDeclNode* func);

private:
    void visit(ASTNode* node);
    void visitBlock(BlockStmtNode* node);
    void visitIf(IfStmtNode* node);
    void visitWhile(WhileStmtNode* node);
    void visitFor(ForStmtNode* node);
    void visitMatch(MatchExpr* node);
    void visitReturn(ReturnStmtNode* node);
    void visitBreak(BreakStmtNode* node);
    void visitContinue(ContinueStmtNode* node);
};

class ControlFlowAnalyzer {
    DiagnosticEngine& diag;
public:
    ControlFlowAnalyzer(DiagnosticEngine& diag) : diag(diag) {}
    bool analyze(FunctionDeclNode* func);
};

} // namespace fl
