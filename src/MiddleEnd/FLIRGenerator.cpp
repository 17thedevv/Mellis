#include "fdlang/MiddleEnd/FLIRGenerator.h"
#include "fdlang/AST/ASTNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/StmtNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/AST/ProgramNode.h"
#include <cassert>
#include <iostream>

namespace fl {

FLIRGenerator::FLIRGenerator(SymbolTable& symTable, TypeChecker& typeChecker)
    : table_(symTable), typeChecker_(typeChecker) {}

std::unique_ptr<flir::Module> FLIRGenerator::generate(ProgramNode& program) {
    module_ = std::make_unique<flir::Module>();
    nextLocalId_ = 0;
    nextLabelId_ = 0;
    varAllocas_.clear();
    evalMode_ = EvalMode::RValue;

    visit(program);

    return std::move(module_);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

flir::LocalId FLIRGenerator::nextLocal() {
    return flir::LocalId{"%" + std::to_string(nextLocalId_++)};
}

flir::LabelId FLIRGenerator::nextLabel(const std::string& prefix) {
    return flir::LabelId{prefix + std::to_string(nextLabelId_++)};
}

void FLIRGenerator::terminateBlock(std::unique_ptr<flir::Terminator> term) {
    if (currentBlock_ && !currentBlock_->terminator) {
        currentBlock_->terminator = std::move(term);
    }
}

void FLIRGenerator::startBlock(flir::LabelId label) {
    auto bb = std::make_unique<flir::BasicBlock>(std::move(label));
    currentBlock_ = bb.get();
    currentFunction_->blocks.push_back(std::move(bb));
}

flir::Operand FLIRGenerator::evaluateRValue(ExprNode& expr) {
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::RValue;
    expr.accept(*this);
    evalMode_ = oldMode;
    return lastEvaluatedOperand_;
}

flir::Operand FLIRGenerator::evaluateLValue(ExprNode& expr) {
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::LValue;
    expr.accept(*this);
    evalMode_ = oldMode;
    return lastEvaluatedOperand_;
}

// ── ASTVisitor: Statements ───────────────────────────────────────────────────

void FLIRGenerator::visit(ProgramNode& node) {
    // MVP: wrap everything in @main() -> void
    auto mainFunc = std::make_unique<flir::Function>(flir::GlobalId{"@main"}, nullptr);
    currentFunction_ = mainFunc.get();
    module_->functions.push_back(std::move(mainFunc));

    // Start entry block
    startBlock(nextLabel());

    for (auto& item : node.items) {
        item->accept(*this);
    }

    // Terminate the last block of main if not already terminated
    terminateBlock(std::make_unique<flir::RetTerm>());
    
    currentFunction_ = nullptr;
    currentBlock_ = nullptr;
}

void FLIRGenerator::visit(VarDeclNode& node) {
    // Variable Declaration:
    // 1. Allocate space on stack: %id = alloca type
    // 2. If initialized, evaluate RHS and store: store val, %id

    const Type* varType = typeChecker_.typeOf(node.symbolId);
    
    flir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(
        std::make_unique<flir::AllocaInst>(ptr, varType)
    );
    
    // Save pointer location to mapping
    varAllocas_[node.symbolId] = ptr;

    if (node.initializer) {
        flir::Operand initVal = evaluateRValue(*node.initializer);
        currentBlock_->instructions.push_back(
            std::make_unique<flir::StoreInst>(initVal, ptr)
        );
    }
}

void FLIRGenerator::visit(AssignExpr& node) {
    // Assignment:
    // 1. Evaluate RHS
    // 2. Load pointer from symbol mapping
    // 3. store val, %id

    flir::Operand val = evaluateRValue(*node.value);
    flir::Operand ptr = evaluateLValue(*node.lvalue);

    currentBlock_->instructions.push_back(
        std::make_unique<flir::StoreInst>(val, ptr)
    );
    
    lastEvaluatedOperand_ = val;
}

void FLIRGenerator::visit(BlockStmtNode& node) {
    for (auto& stmt : node.body) {
        stmt->accept(*this);
    }
}

void FLIRGenerator::visit(IfStmtNode& node) {
    flir::Operand cond = evaluateRValue(*node.condition);

    flir::LabelId thenLbl = nextLabel("then");
    flir::LabelId elseLbl = node.elseBranch ? nextLabel("else") : flir::LabelId{""};
    flir::LabelId mergeLbl = nextLabel("merge");

    if (!node.elseBranch) {
        elseLbl = mergeLbl;
    }

    terminateBlock(std::make_unique<flir::BranchTerm>(cond, thenLbl, elseLbl));

    // Then block
    startBlock(thenLbl);
    node.thenBranch->accept(*this);
    terminateBlock(std::make_unique<flir::JumpTerm>(mergeLbl));

    // Else block
    if (node.elseBranch) {
        startBlock(elseLbl);
        node.elseBranch->accept(*this);
        terminateBlock(std::make_unique<flir::JumpTerm>(mergeLbl));
    }

    // Merge block
    startBlock(mergeLbl);
}

void FLIRGenerator::visit(WhileStmtNode& node) {
    flir::LabelId condLbl = nextLabel("while_cond");
    flir::LabelId bodyLbl = nextLabel("while_body");
    flir::LabelId endLbl = nextLabel("while_end");

    terminateBlock(std::make_unique<flir::JumpTerm>(condLbl));

    // Condition block
    startBlock(condLbl);
    flir::Operand cond = evaluateRValue(*node.condition);
    terminateBlock(std::make_unique<flir::BranchTerm>(cond, bodyLbl, endLbl));

    // Body block
    startBlock(bodyLbl);
    node.body->accept(*this);
    terminateBlock(std::make_unique<flir::JumpTerm>(condLbl));

    // End block
    startBlock(endLbl);
}

// ── ASTVisitor: Expressions ──────────────────────────────────────────────────

void FLIRGenerator::visit(LiteralExpr& node) {
    if (node.kind == LiteralKind::Integer) {
        lastEvaluatedOperand_ = flir::Number{std::string(node.rawText)};
    } else if (node.kind == LiteralKind::Bool) {
        lastEvaluatedOperand_ = flir::Boolean{std::string(node.rawText) == "true"};
    } else {
        lastEvaluatedOperand_ = flir::Number{"0"}; // Default stub for others
    }
}

void FLIRGenerator::visit(IdentifierExpr& node) {
    if (!varAllocas_.count(node.resolvedSymbol)) {
        std::cerr << "Missing symbolId in varAllocas_: " << node.segments.back() << " (ID: " << node.resolvedSymbol << ")\n";
        assert(false && "Identifier symbolId not allocated");
    }
    flir::LocalId ptr = varAllocas_[node.resolvedSymbol];

    if (evalMode_ == EvalMode::LValue) {
        // Return pointer directly
        lastEvaluatedOperand_ = ptr;
    } else {
        // Evaluate to value
        flir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<flir::LoadInst>(dest, ptr)
        );
        lastEvaluatedOperand_ = dest;
    }
}

void FLIRGenerator::visit(BinaryExpr& node) {
    flir::Operand left = evaluateRValue(*node.left);
    flir::Operand right = evaluateRValue(*node.right);

    flir::AluOp op;
    if (node.op == BinaryOp::Add) op = flir::AluOp::Add;
    else if (node.op == BinaryOp::Sub) op = flir::AluOp::Sub;
    else if (node.op == BinaryOp::Mul) op = flir::AluOp::Mul;
    else if (node.op == BinaryOp::Div) op = flir::AluOp::Div;
    else if (node.op == BinaryOp::Eq) op = flir::AluOp::Eq;
    else if (node.op == BinaryOp::Ne) op = flir::AluOp::Ne;
    else if (node.op == BinaryOp::Lt) op = flir::AluOp::Lt;
    else if (node.op == BinaryOp::Le) op = flir::AluOp::Le;
    else if (node.op == BinaryOp::Gt) op = flir::AluOp::Gt;
    else if (node.op == BinaryOp::Ge) op = flir::AluOp::Ge;
    else {
        assert(false && "Unknown binary operator");
        op = flir::AluOp::Add;
    }

    flir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(
        std::make_unique<flir::AluInst>(dest, op, left, right)
    );

    lastEvaluatedOperand_ = dest;
}

// ── Dummy stubs for unsupported nodes ────────────────────────────────────────

void FLIRGenerator::visit(FunctionDeclNode&) {}
void FLIRGenerator::visit(ParamDeclNode&) {}
void FLIRGenerator::visit(StructDeclNode&) {}
void FLIRGenerator::visit(StructFieldNode&) {}
void FLIRGenerator::visit(EnumDeclNode&) {}
void FLIRGenerator::visit(EnumVariantNode&) {}
void FLIRGenerator::visit(TraitDeclNode&) {}
void FLIRGenerator::visit(ImplDeclNode&) {}
void FLIRGenerator::visit(ModDeclNode&) {}
void FLIRGenerator::visit(UseDeclNode&) {}
void FLIRGenerator::visit(ExternDeclNode&) {}
void FLIRGenerator::visit(TypeAliasDeclNode&) {}

void FLIRGenerator::visit(ExprStmtNode& node) {
    evaluateRValue(*node.expr);
}

void FLIRGenerator::visit(ReturnStmtNode&) {}
void FLIRGenerator::visit(ForStmtNode&) {}
void FLIRGenerator::visit(BreakStmtNode&) {}
void FLIRGenerator::visit(ContinueStmtNode&) {}
void FLIRGenerator::visit(UnsafeStmtNode&) {}
void FLIRGenerator::visit(ComptimeStmtNode&) {}

void FLIRGenerator::visit(UnaryExpr&) {}
void FLIRGenerator::visit(CallExpr& node) {
    // If it's print(), we mock it for testing
    if (auto* ident = dynamic_cast<IdentifierExpr*>(node.callee.get())) {
        if (!ident->segments.empty() && ident->segments[0] == "print") {
            flir::Operand val = evaluateRValue(*node.args[0].value);
            std::vector<flir::Operand> args = {val};
            currentBlock_->instructions.push_back(
                std::make_unique<flir::CallInst>(std::nullopt, flir::GlobalId{"@print"}, args)
            );
            return;
        }
    }
}
void FLIRGenerator::visit(MethodCallExpr&) {}
void FLIRGenerator::visit(IndexExpr&) {}
void FLIRGenerator::visit(MemberExpr&) {}
void FLIRGenerator::visit(CastExpr&) {}
void FLIRGenerator::visit(ArrayLiteralExpr&) {}
void FLIRGenerator::visit(TupleLiteralExpr&) {}
void FLIRGenerator::visit(StructInitExpr&) {}
void FLIRGenerator::visit(LambdaExpr&) {}
void FLIRGenerator::visit(MatchExpr&) {}
void FLIRGenerator::visit(AwaitExpr&) {}
void FLIRGenerator::visit(SizeofExpr&) {}
void FLIRGenerator::visit(AlignofExpr&) {}

} // namespace fl
