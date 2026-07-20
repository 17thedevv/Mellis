#include "mellis/MiddleEnd/MVIRGenerator.h"
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/PatternNode.h"
#include <cassert>
#include <iostream>

namespace fl {

MVIRGenerator::MVIRGenerator(SymbolTable& symTable, TypeChecker& typeChecker)
    : table_(symTable), typeChecker_(typeChecker) {}

std::unique_ptr<mvir::Module> MVIRGenerator::generate(ProgramNode& program) {
    module_ = std::make_unique<mvir::Module>();
    nextLocalId_ = 0;
    nextLabelId_ = 0;
    varAllocas_.clear();
    evalMode_ = EvalMode::RValue;

    visit(program);

    return std::move(module_);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

mvir::LocalId MVIRGenerator::nextLocal() {
    return mvir::LocalId{"%" + std::to_string(nextLocalId_++)};
}

mvir::LabelId MVIRGenerator::nextLabel(const std::string& prefix) {
    return mvir::LabelId{prefix + std::to_string(nextLabelId_++)};
}

void MVIRGenerator::terminateBlock(std::unique_ptr<mvir::Terminator> term) {
    if (currentBlock_ && !currentBlock_->terminator) {
        currentBlock_->terminator = std::move(term);
    }
}

void MVIRGenerator::startBlock(mvir::LabelId label) {
    auto bb = std::make_unique<mvir::BasicBlock>(std::move(label));
    currentBlock_ = bb.get();
    currentFunction_->blocks.push_back(std::move(bb));
}

void MVIRGenerator::resetFunctionState() {
    nextLocalId_ = 0;
    nextLabelId_ = 0;
    varAllocas_.clear();
    currentBlock_ = nullptr;
}

mvir::Operand MVIRGenerator::evaluateRValue(ExprNode& expr) {
        auto oldMode = evalMode_;
        evalMode_ = EvalMode::RValue;
        lastEvaluatedOperand_ = mvir::Operand{};
        expr.accept(*this);
        evalMode_ = oldMode;
        return lastEvaluatedOperand_;
    }

mvir::Operand MVIRGenerator::evaluateLValue(ExprNode& expr) {
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::LValue;
    expr.accept(*this);
    evalMode_ = oldMode;
    return lastEvaluatedOperand_;
}

// ── ASTVisitor: Statements ───────────────────────────────────────────────────

void MVIRGenerator::visit(ProgramNode& node) {
    for (auto& item : node.items) {
        item->accept(*this);
    }
}

void MVIRGenerator::visit(VarDeclNode& node) {
    // Variable Declaration:
    // 1. Allocate space on stack: %id = alloca type
    // 2. If initialized, evaluate RHS and store: store val, %id

    const Type* varType = typeChecker_.typeOf(node.symbolId);
    
    mvir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(
        std::make_unique<mvir::LocalInst>(ptr, varType)
    );
    
    // Save pointer location to mapping
    varAllocas_[node.symbolId] = ptr;

    if (!scopeStack_.empty()) {
        scopeStack_.back().push_back(ptr);
    }

    if (node.initializer) {
        mvir::Operand initVal = evaluateRValue(*node.initializer);
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::StoreInst>(varType, initVal, ptr)
        );
    }
}

void MVIRGenerator::visit(AssignExpr& node) {
    // Assignment:
    // 1. Evaluate RHS
    // 2. Load pointer from symbol mapping
    // 3. store val, %id

    mvir::Operand val = evaluateRValue(*node.value);
    mvir::Operand ptr = evaluateLValue(*node.lvalue);

    currentBlock_->instructions.push_back(
        std::make_unique<mvir::StoreInst>(node.lvalue->inferredType, val, ptr)
    );
    
    lastEvaluatedOperand_ = val;
}

void MVIRGenerator::visit(BlockStmtNode& node) {
    for (auto& stmt : node.body) {
        // Dead-code gate: nếu block đã có terminator (return/break/continue),
        // bỏ qua tất cả statement phía sau để tránh LLVM Builder crash.
        if (currentBlock_->terminator != nullptr) break;
        stmt->accept(*this);
    }
    if (currentBlock_->terminator == nullptr && node.tailExpr) {
        lastEvaluatedOperand_ = evaluateRValue(*node.tailExpr);
    }
}

void MVIRGenerator::visit(IfStmtNode& node) {
    mvir::Operand cond = evaluateRValue(*node.condition);

    mvir::LabelId thenLbl  = nextLabel("then");
    mvir::LabelId mergeLbl = nextLabel("merge");
    mvir::LabelId elseLbl  = node.elseBranch ? nextLabel("else") : mergeLbl;

    terminateBlock(std::make_unique<mvir::BranchTerm>(cond, thenLbl, elseLbl));

    // ── Then block ────────────────────────────────────────────────────────────
    startBlock(thenLbl);
    node.thenBranch->accept(*this);
    bool thenTerminated = currentBlock_->terminator != nullptr;
    if (!thenTerminated) terminateBlock(std::make_unique<mvir::JumpTerm>(mergeLbl));

    // ── Else block ────────────────────────────────────────────────────────────
    bool elseTerminated = false;
    if (node.elseBranch) {
        startBlock(elseLbl);
        node.elseBranch->accept(*this); // handles else-if recursively
        elseTerminated = currentBlock_->terminator != nullptr;
        if (!elseTerminated) terminateBlock(std::make_unique<mvir::JumpTerm>(mergeLbl));
    }

    // ── Merge block ───────────────────────────────────────────────────────────
    // Chỉ tạo merge block nếu ít nhất một nhánh cần nhảy vào đây.
    // Nếu cả then lẫn else đều return/break, merge block sẽ không có predecessor
    // và LLVM verifier sẽ báo lỗi → bỏ qua hoàn toàn.
    bool needsMerge = !thenTerminated || !elseTerminated || !node.elseBranch;
    if (needsMerge) startBlock(mergeLbl);
}

void MVIRGenerator::visit(WhileStmtNode& node) {
    mvir::LabelId condLbl = nextLabel("while_cond");
    mvir::LabelId bodyLbl = nextLabel("while_body");
    mvir::LabelId endLbl = nextLabel("while_end");

    terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

    // Condition block
    startBlock(condLbl);
    mvir::Operand cond = evaluateRValue(*node.condition);
    terminateBlock(std::make_unique<mvir::BranchTerm>(cond, bodyLbl, endLbl));

    // Body block
    startBlock(bodyLbl);
    loopTargets_.push_back({condLbl, endLbl});
    node.body->accept(*this);
    loopTargets_.pop_back();
    terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

    // End block
    startBlock(endLbl);
}

// ── ASTVisitor: Expressions ──────────────────────────────────────────────────

void MVIRGenerator::visit(LiteralExpr& node) {
    if (node.kind == LiteralKind::Integer) {
        lastEvaluatedOperand_ = mvir::Number{std::string(node.rawText)};
    } else if (node.kind == LiteralKind::Bool) {
        lastEvaluatedOperand_ = mvir::Boolean{std::string(node.rawText) == "true"};
    } else if (node.kind == LiteralKind::Str) {
        static int stringCounter = 1;
        std::string globalIdStr = "@str_" + std::to_string(stringCounter++);
        mvir::GlobalId gid{globalIdStr};

        mvir::GlobalDecl globalDecl;
        globalDecl.name = gid;
        globalDecl.type = node.inferredType;
        
        std::string rawStr = std::string(node.rawText);
        if (rawStr.length() >= 2 && rawStr.front() == '"' && rawStr.back() == '"') {
            rawStr = rawStr.substr(1, rawStr.length() - 2);
        }
        // Handle basic escapes (like \n, \t)
        std::string unescaped;
        for (size_t i = 0; i < rawStr.length(); ++i) {
            if (rawStr[i] == '\\' && i + 1 < rawStr.length()) {
                switch(rawStr[i+1]) {
                    case 'n': unescaped += '\n'; break;
                    case 't': unescaped += '\t'; break;
                    case 'r': unescaped += '\r'; break;
                    case '0': unescaped += '\0'; break;
                    case '\\': unescaped += '\\'; break;
                    case '"': unescaped += '"'; break;
                    default: unescaped += rawStr[i+1]; break;
                }
                i++;
            } else {
                unescaped += rawStr[i];
            }
        }
        
        globalDecl.stringLiteral = unescaped + '\0'; // Add null terminator
        module_->globalDecls.push_back(globalDecl);
        
        lastEvaluatedOperand_ = gid;
    } else {
        lastEvaluatedOperand_ = mvir::Number{"0"}; // Default stub for others
    }
}

void MVIRGenerator::visit(IdentifierExpr& node) {
    if (node.resolvedSymbol != kInvalidSymbolID) {
        const auto& sym = table_.getSymbol(node.resolvedSymbol);
        if (sym.kind == SymbolKind::EnumVariant) {
            size_t variantIdx = 0;
            const Type* enumTy = node.inferredType;
            if (auto* enumType = dynamic_cast<const EnumType*>(enumTy)) {
                const auto& enumSym = table_.getSymbol(enumType->enumSymbolId);
                if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                    auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                    for (size_t i = 0; i < enumDecl->variants.size(); ++i) {
                        if (enumDecl->variants[i]->symbolId == node.resolvedSymbol) {
                            variantIdx = i;
                            break;
                        }
                    }
                }
            }
            mvir::LocalId dest = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::VariantInst>(dest, enumTy, variantIdx, std::vector<mvir::Operand>{}));
            lastEvaluatedOperand_ = dest;
            return;
        }
    }

    if (!varAllocas_.count(node.resolvedSymbol)) {
        std::cerr << "Missing symbolId in varAllocas_: " << node.segments.back() << " (ID: " << node.resolvedSymbol << ")\n";
        assert(false && "Identifier symbolId not allocated");
    }
    mvir::LocalId ptr = varAllocas_[node.resolvedSymbol];

    if (evalMode_ == EvalMode::LValue) {
        // Return pointer directly
        lastEvaluatedOperand_ = ptr;
    } else {
        // Evaluate to value
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LoadInst>(dest, node.inferredType, ptr)
        );
        lastEvaluatedOperand_ = dest;
    }
}

void MVIRGenerator::visit(BinaryExpr& node) {
    mvir::Operand left = evaluateRValue(*node.left);
    mvir::Operand right = evaluateRValue(*node.right);

    mvir::AluOp op;
    if (node.op == BinaryOp::Add) op = mvir::AluOp::Add;
    else if (node.op == BinaryOp::Sub) op = mvir::AluOp::Sub;
    else if (node.op == BinaryOp::Mul) op = mvir::AluOp::Mul;
    else if (node.op == BinaryOp::Div) op = mvir::AluOp::Div;
    else if (node.op == BinaryOp::Eq) op = mvir::AluOp::Eq;
    else if (node.op == BinaryOp::Ne) op = mvir::AluOp::Ne;
    else if (node.op == BinaryOp::Lt) op = mvir::AluOp::Lt;
    else if (node.op == BinaryOp::Le) op = mvir::AluOp::Le;
    else if (node.op == BinaryOp::Gt) op = mvir::AluOp::Gt;
    else if (node.op == BinaryOp::Ge) op = mvir::AluOp::Ge;
    else {
        assert(false && "Unknown binary operator");
        op = mvir::AluOp::Add;
    }

    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(
        std::make_unique<mvir::AluInst>(dest, op, left, right)
    );

    lastEvaluatedOperand_ = dest;
}

// ── Dummy stubs for unsupported nodes ────────────────────────────────────────

void MVIRGenerator::visit(FunctionDeclNode& node) {
    // Do not generate code for generic template definitions.
    // Only specialized versions (where genericParams is empty) should be generated.
    if (!node.genericParams.empty()) return;

    resetFunctionState();
    
    const Type* funcType = typeChecker_.typeOf(node.symbolId);
    const Type* retType = nullptr;
    if (auto* fTy = dynamic_cast<const FunctionType*>(funcType)) {
        retType = fTy->returnType;
    }
    
    const auto& sym = table_.getSymbol(node.symbolId);
    std::string funcName = sym.mangledName.empty() ? std::string(node.name) : sym.mangledName;
    
    auto func = std::make_unique<mvir::Function>(mvir::GlobalId{"@" + funcName}, retType);
    func->isAsync = node.isAsync;
    currentFunction_ = func.get();
    
    for (auto& p : node.params) {
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        func->params.push_back(mvir::Param{pType, nextLocal()});
    }
    
    module_->functions.push_back(std::move(func));
    
    startBlock(nextLabel("entry"));
    
    for (size_t i = 0; i < node.params.size(); ++i) {
        auto& p = node.params[i];
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        mvir::LocalId ptr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, pType));
        varAllocas_[p->symbolId] = ptr;
        
        mvir::LocalId argId = currentFunction_->params[i].id;
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(pType, argId, ptr));
    }
    
    if (node.body) {
        node.body->accept(*this);
    }
    
    terminateBlock(std::make_unique<mvir::RetTerm>());
    currentFunction_ = nullptr;
    currentBlock_ = nullptr;
}
void MVIRGenerator::visit(ParamDeclNode&) {}
void MVIRGenerator::visit(StructDeclNode& node) {
    const Type* stType = typeChecker_.typeOf(node.symbolId);
    if (auto st = dynamic_cast<const StructType*>(stType)) {
        mvir::TypeDecl decl;
        decl.name = "%" + std::string(node.name);
        for (auto& field : node.fields) { decl.fields.push_back(typeChecker_.typeOf(field->symbolId)); }
        module_->typeDecls.push_back(decl);
    }
}
void MVIRGenerator::visit(StructFieldNode&) {}
void MVIRGenerator::visit(EnumDeclNode&) {}
void MVIRGenerator::visit(EnumVariantNode&) {}
void MVIRGenerator::visit(TraitDeclNode&) {}
void MVIRGenerator::visit(ImplDeclNode& node) {
    if (!node.genericParams.empty()) return; // Skip generic impls
    for (auto& method : node.methods) {
        method->accept(*this);
    }
}
void MVIRGenerator::visit(ModDeclNode& node) {
    for (auto& d : node.decls) {
        d->accept(*this);
    }
}
void MVIRGenerator::visit(UseDeclNode&) {}
void MVIRGenerator::visit(ExternDeclNode& node) {
    if (node.func) {
        const auto& sym = table_.getSymbol(node.func->symbolId);
        std::string funcName = sym.mangledName.empty() ? std::string(node.func->name) : sym.mangledName;
        
        mvir::ExternFunction ext;
        ext.name = mvir::GlobalId{"@" + funcName};
        const Type* funcType = typeChecker_.typeOf(node.func->symbolId);
        if (auto* fTy = dynamic_cast<const FunctionType*>(funcType)) {
            ext.paramTypes = fTy->paramTypes;
            ext.returnType = fTy->returnType;
            ext.isVariadic = fTy->isVariadic;
        } else {
            ext.returnType = typeChecker_.getContext().getPrimitive(BuiltinKind::Void);
            ext.isVariadic = node.func->isVariadic;
        }
        module_->externFunctions.push_back(std::move(ext));
    }
}

void MVIRGenerator::visit(TypeAliasDeclNode&) {}

void MVIRGenerator::visit(ExprStmtNode& node) {
    lastEvaluatedOperand_ = evaluateRValue(*node.expr);
}

void MVIRGenerator::visit(ReturnStmtNode& node) {
    if (node.value) {
        mvir::Operand val = evaluateRValue(*node.value);
        terminateBlock(std::make_unique<mvir::RetTerm>(val));
    } else {
        terminateBlock(std::make_unique<mvir::RetTerm>());
    }
}
void MVIRGenerator::visit(ForStmtNode& node) {
    if (node.kind == ForKind::CStyle) {
        if (node.init) {
            node.init->accept(*this);
        }

        mvir::LabelId condLbl = nextLabel("for_cond");
        mvir::LabelId bodyLbl = nextLabel("for_body");
        mvir::LabelId stepLbl = nextLabel("for_step");
        mvir::LabelId endLbl = nextLabel("for_end");

        terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

        // Condition block
        startBlock(condLbl);
        if (node.cond) {
            mvir::Operand cond = evaluateRValue(*node.cond);
            terminateBlock(std::make_unique<mvir::BranchTerm>(cond, bodyLbl, endLbl));
        } else {
            terminateBlock(std::make_unique<mvir::JumpTerm>(bodyLbl));
        }

        // Body block
        startBlock(bodyLbl);
        loopTargets_.push_back({stepLbl, endLbl});
        if (node.body) node.body->accept(*this);
        loopTargets_.pop_back();
        terminateBlock(std::make_unique<mvir::JumpTerm>(stepLbl));

        // Step block
        startBlock(stepLbl);
        if (node.step) {
            node.step->accept(*this);
        }
        terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

        // End block
        startBlock(endLbl);
    } else {
        // For-Each implementation
        mvir::Operand arrayOp = evaluateLValue(*node.iterable); // Needs pointer to array for GetPtrInst
        
        mvir::LocalId idxLoc = nextLocal();
        auto* i32Type = typeChecker_.getContext().getPrimitive(BuiltinKind::I32);
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LocalInst>(idxLoc, i32Type)
        );
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::StoreInst>(i32Type, mvir::Number{"0"}, idxLoc)
        );

        mvir::LabelId condLbl = nextLabel("foreach_cond");
        mvir::LabelId bodyLbl = nextLabel("foreach_body");
        mvir::LabelId stepLbl = nextLabel("foreach_step");
        mvir::LabelId endLbl  = nextLabel("foreach_end");

        terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

        // Cond
        startBlock(condLbl);
        mvir::LocalId currIdx = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LoadInst>(currIdx, i32Type, idxLoc)
        );
        
        mvir::Operand condResult = mvir::Number{"0"};
        auto* arrType = dynamic_cast<const ArrayType*>(node.iterable->inferredType);
        if (arrType) {
            mvir::LocalId cmpRes = nextLocal();
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::AluInst>(cmpRes, mvir::AluOp::Lt, currIdx, mvir::Number{std::to_string(arrType->length)})
            );
            condResult = cmpRes;
        }
        terminateBlock(std::make_unique<mvir::BranchTerm>(condResult, bodyLbl, endLbl));

        // Body
        startBlock(bodyLbl);
        mvir::LocalId elemAddr = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::IndexInst>(elemAddr, arrType, arrayOp, currIdx)
        );
        mvir::LocalId elemVal = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LoadInst>(elemVal, arrType->elementType, elemAddr)
        );
        
        if (node.bindingId != kInvalidSymbolID) {
            mvir::LocalId varAlloca = nextLocal();
            varAllocas_[node.bindingId] = varAlloca;
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::LocalInst>(varAlloca, arrType->elementType)
            );
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::StoreInst>(arrType->elementType, elemVal, varAlloca)
            );
        }

        loopTargets_.push_back({stepLbl, endLbl});
        if (node.body) node.body->accept(*this);
        loopTargets_.pop_back();
        terminateBlock(std::make_unique<mvir::JumpTerm>(stepLbl));

        // Step
        startBlock(stepLbl);
        mvir::LocalId nextIdx = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::AluInst>(nextIdx, mvir::AluOp::Add, currIdx, mvir::Number{"1"})
        );
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::StoreInst>(i32Type, nextIdx, idxLoc)
        );
        terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

        // End
        startBlock(endLbl);
    }
}
void MVIRGenerator::visit(BreakStmtNode&) {
    if (!loopTargets_.empty()) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(loopTargets_.back().endLbl));
    }
}
void MVIRGenerator::visit(ContinueStmtNode&) {
    if (!loopTargets_.empty()) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(loopTargets_.back().stepLbl));
    }
}
void MVIRGenerator::visit(UnsafeStmtNode& node) {
    if (node.body) node.body->accept(*this);
}
void MVIRGenerator::visit(ComptimeStmtNode&) {}

void MVIRGenerator::visit(UnaryExpr& node) {
    if (node.op == UnaryOp::Ref || node.op == UnaryOp::RefMut) {
        auto oldMode = evalMode_;
        evalMode_ = EvalMode::LValue;
        node.operand->accept(*this);
        evalMode_ = oldMode;

        mvir::Operand addr = lastEvaluatedOperand_;
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::BorrowInst>(
            dest, node.op == UnaryOp::RefMut, addr
        ));
        lastEvaluatedOperand_ = mvir::Operand(dest);
    } 
    else if (node.op == UnaryOp::Deref) {
        auto oldMode = evalMode_;
        evalMode_ = EvalMode::RValue;
        node.operand->accept(*this);
        evalMode_ = oldMode;

        mvir::Operand ptrVal = lastEvaluatedOperand_;

        if (evalMode_ == EvalMode::LValue) {
            lastEvaluatedOperand_ = ptrVal;
        } else {
            mvir::LocalId val = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, std::get<mvir::LocalId>(ptrVal)));
            lastEvaluatedOperand_ = mvir::Operand(val);
        }
    } else if (node.op == UnaryOp::Neg) {
        mvir::Operand val = evaluateRValue(*node.operand);
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::UnaryInst>(dest, mvir::UnaryOp::Negate, val));
        lastEvaluatedOperand_ = mvir::Operand(dest);
    } else if (node.op == UnaryOp::BitNot) {
        mvir::Operand val = evaluateRValue(*node.operand);
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::UnaryInst>(dest, mvir::UnaryOp::BitNot, val));
        lastEvaluatedOperand_ = mvir::Operand(dest);
    }
}

void MVIRGenerator::visit(CallExpr& node) {
    std::vector<mvir::Operand> args;
    for (auto& arg : node.args) {
        args.push_back(evaluateRValue(*arg.value));
    }
    
    mvir::Operand callee;
    
    if (node.isClosureCall) {
        mvir::Operand closureVal = evaluateRValue(*node.callee);
        const ClosureType* closTy = dynamic_cast<const ClosureType*>(node.callee->inferredType);
        
        mvir::LocalId closurePtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(closurePtr, closTy));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(closTy, closureVal, closurePtr));
        
        mvir::LocalId fptrLoc = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(fptrLoc, closTy, closurePtr, 0));
        
        mvir::LocalId funcPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(funcPtr, typeChecker_.getContext().getPointerType(closTy->signature, false), fptrLoc));
        
        callee = funcPtr;
        args.insert(args.begin(), closurePtr); // __env pointer
    } else {
        if (auto* ident = dynamic_cast<IdentifierExpr*>(node.callee.get())) {
            if (ident->resolvedSymbol != kInvalidSymbolID) {
                const auto& sym = table_.getSymbol(ident->resolvedSymbol);
                if (sym.kind == SymbolKind::EnumVariant) {
                    size_t variantIdx = 0;
                    const Type* enumTy = node.inferredType;
                    if (auto* enumType = dynamic_cast<const EnumType*>(enumTy)) {
                        const auto& enumSym = table_.getSymbol(enumType->enumSymbolId);
                        if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                            auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                            for (size_t i = 0; i < enumDecl->variants.size(); ++i) {
                                if (enumDecl->variants[i]->symbolId == ident->resolvedSymbol) {
                                    variantIdx = i;
                                    break;
                                }
                            }
                        }
                    }
                    mvir::LocalId dest = nextLocal();
                    currentBlock_->instructions.push_back(std::make_unique<mvir::VariantInst>(dest, enumTy, variantIdx, args));
                    lastEvaluatedOperand_ = dest;
                    return;
                }
            }
        }
    
        // Evaluate callee. Prefer resolvedFn if available!
        if (node.resolvedFn != kInvalidSymbolID) {
            SymbolID actualFn = node.resolvedFn;
            while (actualFn != kInvalidSymbolID) {
                const auto& sym = table_.getSymbol(actualFn);
                if (sym.kind == SymbolKind::Alias) {
                    actualFn = sym.aliasTo;
                } else {
                    break;
                }
            }
            const auto& sym = table_.getSymbol(actualFn);
            std::string calleeName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
            callee = mvir::GlobalId{"@" + calleeName};

            if (sym.isExternal && generatedExternals_.insert(actualFn).second) {
                mvir::ExternFunction ext;
                ext.name = std::get<mvir::GlobalId>(callee);
                if (auto* fTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(actualFn))) {
                    ext.paramTypes = fTy->paramTypes;
                    ext.returnType = fTy->returnType;
                    ext.isVariadic = fTy->isVariadic;
                }
                module_->externFunctions.push_back(ext);
            }
        } else if (auto* ident = dynamic_cast<IdentifierExpr*>(node.callee.get())) {
            if (ident->resolvedSymbol != kInvalidSymbolID) {
                const auto& sym = table_.getSymbol(ident->resolvedSymbol);
                std::string calleeName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
                callee = mvir::GlobalId{"@" + calleeName};

                if (sym.isExternal && generatedExternals_.insert(ident->resolvedSymbol).second) {
                    mvir::ExternFunction ext;
                    ext.name = std::get<mvir::GlobalId>(callee);
                    if (auto* fTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(ident->resolvedSymbol))) {
                        ext.paramTypes = fTy->paramTypes;
                        ext.returnType = fTy->returnType;
                        ext.isVariadic = fTy->isVariadic;
                    }
                    module_->externFunctions.push_back(ext);
                }
            } else if (!ident->segments.empty()) {
                callee = mvir::GlobalId{"@" + std::string(ident->segments.back())};
            }
        } else {
            callee = evaluateRValue(*node.callee);
        }
    }

    std::optional<mvir::LocalId> dest = std::nullopt;
    if (evalMode_ == EvalMode::RValue) {
        dest = nextLocal();
        lastEvaluatedOperand_ = *dest;
    }
    
    const FunctionType* fTy = nullptr;
    if (node.isClosureCall) {
        if (auto* cTy = dynamic_cast<const ClosureType*>(node.callee->inferredType)) {
            fTy = cTy->signature;
        }
    } else if (auto* funcTy = dynamic_cast<const FunctionType*>(node.callee->inferredType)) {
        fTy = funcTy;
    }

    currentBlock_->instructions.push_back(
        std::make_unique<mvir::CallInst>(dest, callee, args, fTy)
    );
}

void MVIRGenerator::visit(MethodCallExpr& node) {
    std::vector<mvir::Operand> args;
    args.push_back(evaluateRValue(*node.object)); // Receiver is the first argument
    for (auto& arg : node.args) {
        args.push_back(evaluateRValue(*arg.value));
    }
    
    const Type* objTy = node.object->inferredType;
    if (auto* refTy = dynamic_cast<const ReferenceType*>(objTy)) objTy = refTy->pointee;
    else if (auto* ptrTy = dynamic_cast<const PointerType*>(objTy)) objTy = ptrTy->pointee;

    std::optional<mvir::LocalId> dest = std::nullopt;
    if (evalMode_ == EvalMode::RValue) {
        dest = nextLocal();
        lastEvaluatedOperand_ = *dest;
    }

    if (auto* traitObjTy = dynamic_cast<const TraitObjectType*>(objTy)) {
        size_t methodIndex = 0;
        const auto& sym = table_.getSymbol(traitObjTy->traitId);
        if (sym.decl && sym.kind == SymbolKind::Trait) {
            auto* traitDecl = static_cast<const TraitDeclNode*>(sym.decl);
            for (size_t i = 0; i < traitDecl->methods.size(); ++i) {
                if (traitDecl->methods[i]->name == node.methodName) {
                    methodIndex = i;
                    break;
                }
            }
        }
        
        const Type* methodTy = typeChecker_.typeOf(node.resolvedFn);
        const FunctionType* fnTy = dynamic_cast<const FunctionType*>(methodTy);
        
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::VirtualCallInst>(dest, args[0], objTy, methodIndex, fnTy, args)
        );
        return;
    }

    // The callee must have been resolved by TypeResolver for static dispatch
    mvir::Operand callee;
    if (node.resolvedFn != kInvalidSymbolID) {
        const auto& sym = table_.getSymbol(node.resolvedFn);
        callee = mvir::GlobalId{"@" + sym.name.str()};
    } else {
        // Fallback for unresolved
        callee = mvir::GlobalId{"@" + std::string(node.methodName)};
    }
    
    currentBlock_->instructions.push_back(
        std::make_unique<mvir::CallInst>(dest, callee, args)
    );
}
void MVIRGenerator::visit(IndexExpr& node) {
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::LValue;
    node.base->accept(*this);
    evalMode_ = oldMode;
    
    mvir::Operand basePtr = lastEvaluatedOperand_;
    mvir::Operand indexOp = evaluateRValue(*node.index);
    
    mvir::LocalId fieldPtr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::IndexInst>(
        fieldPtr, node.inferredType, basePtr, indexOp
    ));

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(fieldPtr);
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, fieldPtr));
        lastEvaluatedOperand_ = mvir::Operand(val);
    }
}
void MVIRGenerator::visit(MemberExpr& node) {
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::LValue;
    node.object->accept(*this);
    evalMode_ = oldMode;

    mvir::Operand objPtr = lastEvaluatedOperand_; // this is the base pointer
    mvir::LocalId fieldPtr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(
        fieldPtr, node.object->inferredType, objPtr, node.resolvedFieldIndex
    ));

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(fieldPtr);
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, fieldPtr));
        lastEvaluatedOperand_ = mvir::Operand(val);
    }
}

void MVIRGenerator::visit(TupleIndexExpr& node) {
    // Tuples are lowered to anonymous structs — index maps directly to field index.
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::LValue;
    node.object->accept(*this);
    evalMode_ = oldMode;

    mvir::Operand objPtr = lastEvaluatedOperand_;
    mvir::LocalId fieldPtr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(
        fieldPtr, node.object->inferredType, objPtr, static_cast<size_t>(node.index)
    ));

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(fieldPtr);
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, fieldPtr));
        lastEvaluatedOperand_ = mvir::Operand(val);
    }
}

void MVIRGenerator::visit(CastExpr& node) {
    mvir::Operand val = evaluateRValue(*node.expr);
    mvir::LocalId dest = nextLocal();
    
    const Type* targetTy = node.inferredType;
    const Type* innerTy = targetTy;
    if (auto* refTy = dynamic_cast<const ReferenceType*>(innerTy)) innerTy = refTy->pointee;
    else if (auto* ptrTy = dynamic_cast<const PointerType*>(innerTy)) innerTy = ptrTy->pointee;
    
    if (auto* traitObjTy = dynamic_cast<const TraitObjectType*>(innerTy)) {
        std::vector<SymbolID> vtableMethods;
        const auto& sym = table_.getSymbol(traitObjTy->traitId);
        if (sym.decl && sym.kind == SymbolKind::Trait) {
            auto* traitDecl = static_cast<const TraitDeclNode*>(sym.decl);
            const Type* concreteTy = node.expr->inferredType;
            if (auto* refTy = dynamic_cast<const ReferenceType*>(concreteTy)) concreteTy = refTy->pointee;
            else if (auto* ptrTy = dynamic_cast<const PointerType*>(concreteTy)) concreteTy = ptrTy->pointee;
            
            for (const auto& method : traitDecl->methods) {
                TypeChecker::MethodInfo mInfo;
                if (typeChecker_.getMethodResolver().probe(concreteTy, std::string(method->name), mInfo)) {
                    vtableMethods.push_back(mInfo.methodId);
                } else {
                    vtableMethods.push_back(kInvalidSymbolID);
                }
            }
        }
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::MakeTraitObjectInst>(dest, val, node.expr->inferredType, targetTy, std::move(vtableMethods)));
        lastEvaluatedOperand_ = mvir::Operand(dest);
        return;
    }
    
    currentBlock_->instructions.push_back(std::make_unique<mvir::CastInst>(dest, val, node.inferredType));
    lastEvaluatedOperand_ = mvir::Operand(dest);
}

void MVIRGenerator::visit(UnsizeCastExpr& node) {
    mvir::Operand val = evaluateRValue(*node.expr);
    mvir::LocalId dest = nextLocal();
    
    std::vector<SymbolID> vtableMethods;
    const Type* targetTy = node.targetTypePtr;
    if (auto* refTy = dynamic_cast<const ReferenceType*>(targetTy)) targetTy = refTy->pointee;
    else if (auto* ptrTy = dynamic_cast<const PointerType*>(targetTy)) targetTy = ptrTy->pointee;
    
    if (auto* traitObjTy = dynamic_cast<const TraitObjectType*>(targetTy)) {
        const auto& sym = table_.getSymbol(traitObjTy->traitId);
        if (sym.decl && sym.kind == SymbolKind::Trait) {
            auto* traitDecl = static_cast<const TraitDeclNode*>(sym.decl);
            const Type* concreteTy = node.expr->inferredType;
            if (auto* refTy = dynamic_cast<const ReferenceType*>(concreteTy)) concreteTy = refTy->pointee;
            else if (auto* ptrTy = dynamic_cast<const PointerType*>(concreteTy)) concreteTy = ptrTy->pointee;
            
            for (const auto& method : traitDecl->methods) {
                TypeChecker::MethodInfo mInfo;
                if (typeChecker_.getMethodResolver().probe(concreteTy, std::string(method->name), mInfo)) {
                    vtableMethods.push_back(mInfo.methodId);
                } else {
                    vtableMethods.push_back(kInvalidSymbolID);
                }
            }
        }
    }

    currentBlock_->instructions.push_back(
        std::make_unique<mvir::MakeTraitObjectInst>(dest, val, node.expr->inferredType, node.targetTypePtr, std::move(vtableMethods)));
    lastEvaluatedOperand_ = mvir::Operand(dest);
}
void MVIRGenerator::visit(ArrayLiteralExpr& node) {
    auto arrTy = dynamic_cast<const ArrayType*>(node.inferredType);
    if (!arrTy) return;

    mvir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, arrTy));

    for (size_t i = 0; i < node.elements.size(); ++i) {
        mvir::Operand val = evaluateRValue(*node.elements[i]);
        mvir::LocalId fieldPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::IndexInst>(
            fieldPtr, arrTy->elementType, ptr, mvir::Operand(mvir::Number{std::to_string(i)})
        ));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(arrTy->elementType, val, fieldPtr));
    }

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = ptr;
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, arrTy, ptr));
        lastEvaluatedOperand_ = val;
    }
}
void MVIRGenerator::visit(TupleLiteralExpr& node) {
    auto tupTy = dynamic_cast<const TupleType*>(node.inferredType);
    if (!tupTy) return;

    mvir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, tupTy));

    for (size_t i = 0; i < node.elements.size(); ++i) {
        mvir::Operand val = evaluateRValue(*node.elements[i]);
        mvir::LocalId fieldPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(
            fieldPtr, tupTy, ptr, i
        ));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(node.elements[i]->inferredType, val, fieldPtr));
    }

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = ptr;
    } else {
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, tupTy, ptr));
        lastEvaluatedOperand_ = res;
    }
}
void MVIRGenerator::visit(StructInitExpr& node) {
    auto st = dynamic_cast<const StructType*>(node.inferredType);
    if (!st) return;

    mvir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, st));

    const auto& sym = table_.getSymbol(st->structSymbolId);
    if (sym.kind == SymbolKind::Struct && sym.decl) {
        auto* structDecl = static_cast<StructDeclNode*>(sym.decl);
        for (auto& field : node.fields) {
            mvir::Operand val = evaluateRValue(*field.value);
            size_t idx = 0;
            for (size_t i = 0; i < structDecl->fields.size(); ++i) {
                if (structDecl->fields[i]->name == field.name) {
                    idx = i;
                    break;
                }
            }
            mvir::LocalId fieldPtr = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(
                fieldPtr, st, ptr, idx
            ));
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(field.value->inferredType, val, fieldPtr));
        }
    }

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = ptr;
    } else {
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, st, ptr));
        lastEvaluatedOperand_ = res;
    }
}
void MVIRGenerator::visit(LambdaExpr& node) {
    auto* closureTy = dynamic_cast<const ClosureType*>(node.inferredType);
    if (!closureTy) return;
    
    auto sym = table_.getSymbol(node.generatedFuncId);
    std::string funcName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
    
    mvir::Function* outerFunction = currentFunction_;
    mvir::BasicBlock* outerBlock = currentBlock_;
    auto outerVarAllocas = varAllocas_;
    
    resetFunctionState();
    
    auto func = std::make_unique<mvir::Function>(mvir::GlobalId{"@" + funcName}, closureTy->signature->returnType);
    currentFunction_ = func.get();
    
    mvir::LocalId envArgId = nextLocal();
    func->params.push_back(mvir::Param{typeChecker_.getContext().getPointerType(closureTy, false), envArgId});
    
    for (size_t i = 0; i < node.params.size(); ++i) {
        auto& p = node.params[i];
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        func->params.push_back(mvir::Param{pType, nextLocal()});
    }
    
    module_->functions.push_back(std::move(func));
    
    startBlock(nextLabel("entry"));
    
    mvir::LocalId envPtr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(envPtr, typeChecker_.getContext().getPointerType(closureTy, false)));
    currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(typeChecker_.getContext().getPointerType(closureTy, false), envArgId, envPtr));
    
    for (size_t i = 0; i < closureTy->capturedSymbols.size(); ++i) {
        auto capSym = closureTy->capturedSymbols[i];
        const Type* capTy = typeChecker_.typeOf(capSym);
        
        mvir::LocalId capAlloca = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(capAlloca, capTy));
        varAllocas_[capSym] = capAlloca;
        
        mvir::LocalId fieldPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(fieldPtr, closureTy, envArgId, i + 1));
        
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, capTy, fieldPtr));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(capTy, val, capAlloca));
    }
    
    for (size_t i = 0; i < node.params.size(); ++i) {
        auto& p = node.params[i];
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        mvir::LocalId ptr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, pType));
        varAllocas_[p->symbolId] = ptr;
        
        mvir::LocalId argId = currentFunction_->params[i + 1].id;
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(pType, argId, ptr));
    }
    
    if (node.body) {
        node.body->accept(*this);
    }
    
    terminateBlock(std::make_unique<mvir::RetTerm>());
    
    currentFunction_ = outerFunction;
    currentBlock_ = outerBlock;
    varAllocas_ = outerVarAllocas;
    
    if (currentFunction_) {
        mvir::LocalId structPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(structPtr, closureTy));
        
        mvir::LocalId fptr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(fptr, closureTy, structPtr, 0));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(typeChecker_.getContext().getPointerType(closureTy->signature, false), mvir::GlobalId{"@" + funcName}, fptr));
        
        for (size_t i = 0; i < closureTy->capturedSymbols.size(); ++i) {
            auto capSym = closureTy->capturedSymbols[i];
            const Type* capTy = typeChecker_.typeOf(capSym);
            
            mvir::LocalId capAlloca = outerVarAllocas[capSym];
            mvir::LocalId capVal = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(capVal, capTy, capAlloca));
            
            mvir::LocalId fieldPtr = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::FieldInst>(fieldPtr, closureTy, structPtr, i + 1));
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(capTy, capVal, fieldPtr));
        }
        
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, closureTy, structPtr));
        lastEvaluatedOperand_ = res;
    }
}
void MVIRGenerator::visit(MatchExpr& node) {
    mvir::Operand subj = evaluateRValue(*node.subject);
    mvir::Operand matchSubject = subj;
    
    if (node.subject->inferredType && node.subject->inferredType->getKind() == TypeKind::Enum) {
        mvir::LocalId tagPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::TagInst>(tagPtr, subj));
        matchSubject = tagPtr;
    }
    
    mvir::LabelId endLbl = nextLabel("match_end");
    mvir::LabelId defaultLbl = endLbl;
    
    std::vector<std::pair<mvir::Number, mvir::LabelId>> cases;
    std::vector<std::pair<mvir::LabelId, MatchArmNode*>> armBlocks;
    
    for (auto& arm : node.arms) {
        mvir::LabelId armLbl = nextLabel("match_arm");
        if (auto* litPat = dynamic_cast<LiteralPatternNode*>(arm.pattern.get())) {
            if (litPat->lit->kind == LiteralKind::Integer) {
                cases.push_back({mvir::Number{std::string(litPat->lit->rawText)}, armLbl});
                armBlocks.push_back({armLbl, &arm});
            }
        } else if (auto* enumPat = dynamic_cast<EnumPatternNode*>(arm.pattern.get())) {
            if (enumPat->variantSymbolId != kInvalidSymbolID) {
                // Find variant index
                size_t variantIdx = 0;
                if (auto* enumType = dynamic_cast<const EnumType*>(node.subject->inferredType)) {
                    auto enumSym = table_.getSymbol(enumType->enumSymbolId);
                    if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                        auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                        for (size_t i = 0; i < enumDecl->variants.size(); ++i) {
                            if (enumDecl->variants[i]->symbolId == enumPat->variantSymbolId) {
                                variantIdx = i;
                                break;
                            }
                        }
                    }
                }
                cases.push_back({mvir::Number{std::to_string(variantIdx)}, armLbl});
                armBlocks.push_back({armLbl, &arm});
            }
        } else if (dynamic_cast<IdentifierPatternNode*>(arm.pattern.get()) || dynamic_cast<WildcardPatternNode*>(arm.pattern.get())) {
            defaultLbl = armLbl;
            armBlocks.push_back({armLbl, &arm});
        }
    }
    
    mvir::LocalId resultPtr = mvir::LocalId{""};
    if (node.inferredType && node.inferredType->getKind() != TypeKind::Unknown) {
        resultPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(resultPtr, node.inferredType));
    }

    terminateBlock(std::make_unique<mvir::SwitchTerm>(matchSubject, cases, defaultLbl));
    
    for (auto& armBlock : armBlocks) {
        startBlock(armBlock.first);
        
        // If it is an enum pattern with fields, extract them!
        if (auto* enumPat = dynamic_cast<EnumPatternNode*>(armBlock.second->pattern.get())) {
            size_t variantIdx = 0;
            if (auto* enumType = dynamic_cast<const EnumType*>(node.subject->inferredType)) {
                auto enumSym = table_.getSymbol(enumType->enumSymbolId);
                if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                    auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                    for (size_t i = 0; i < enumDecl->variants.size(); ++i) {
                        if (enumDecl->variants[i]->symbolId == enumPat->variantSymbolId) {
                            variantIdx = i;
                            break;
                        }
                    }
                }
            }
            
            std::vector<const Type*> payloadTypes;
            for (size_t i = 0; i < enumPat->fields.size(); ++i) {
                payloadTypes.push_back(enumPat->fields[i]->inferredType);
            }
            
            for (size_t i = 0; i < enumPat->fields.size(); ++i) {
                if (auto* identPat = dynamic_cast<IdentifierPatternNode*>(enumPat->fields[i].get())) {
                    mvir::LocalId fieldVal = nextLocal();
                    currentBlock_->instructions.push_back(std::make_unique<mvir::ExtractInst>(fieldVal, subj, payloadTypes, variantIdx, i));
                    
                    mvir::LocalId fieldPtr = nextLocal();
                    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(fieldPtr, identPat->inferredType));
                    currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(identPat->inferredType, fieldVal, fieldPtr));
                    
                    varAllocas_[identPat->symbolId] = fieldPtr;
                }
            }
        } else if (auto* identPat = dynamic_cast<IdentifierPatternNode*>(armBlock.second->pattern.get())) {
            mvir::LocalId varPtr = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(varPtr, identPat->inferredType));
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(identPat->inferredType, subj, varPtr));
            varAllocas_[identPat->symbolId] = varPtr;
        }
        
        if (armBlock.second->body) {
            armBlock.second->body->accept(*this);
            mvir::Operand armRes = lastEvaluatedOperand_;
            
            bool isEmptyRes = false;
            if (auto* loc = std::get_if<mvir::LocalId>(&armRes)) {
                if (loc->name.empty()) isEmptyRes = true;
            }
            if (!resultPtr.name.empty() && !isEmptyRes) {
                currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(node.inferredType, armRes, resultPtr));
            }
        }
        terminateBlock(std::make_unique<mvir::JumpTerm>(endLbl));
    }
    
    startBlock(endLbl);
    if (!resultPtr.name.empty()) {
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, node.inferredType, resultPtr));
        lastEvaluatedOperand_ = res;
    } else {
        lastEvaluatedOperand_ = mvir::Operand{};
    }
}
void MVIRGenerator::visit(AwaitExpr& node) {
    mvir::Operand futureVal = evaluateRValue(*node.expr);
    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::AwaitInst>(dest, futureVal, node.inferredType));
    lastEvaluatedOperand_ = mvir::Operand(dest);
}
void MVIRGenerator::visit(SizeofExpr& node) {
    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::SizeofInst>(dest, node.evaluatedTargetType));
    lastEvaluatedOperand_ = mvir::Operand(dest);
}
void MVIRGenerator::visit(AlignofExpr& node) {
    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::AlignofInst>(dest, node.evaluatedTargetType));
    lastEvaluatedOperand_ = mvir::Operand(dest);
}

} // namespace fl

