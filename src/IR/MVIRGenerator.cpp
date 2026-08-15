#include "mellis/IR/MVIRGenerator.h"
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

mvir::LocalId MVIRGenerator::nextLocal(
    uint32_t symbolId, uint32_t expansionId) {
    mvir::LocalId id;
    id.name        = "%" + std::to_string(nextLocalId_++);
    id.symbolId    = symbolId;
    id.expansionId = expansionId;
    return id;
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


void MVIRGenerator::emitDropsForScopes(size_t targetDepth) {
    if (scopeStack_.size() <= targetDepth) return;
    for (size_t i = scopeStack_.size(); i > targetDepth; --i) {
        const auto& vars = scopeStack_[i - 1];
        for (auto it = vars.rbegin(); it != vars.rend(); ++it) {
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::DropInst>(it->id, it->type)
            );
        }
    }
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
    for (size_t i = 0; i < node.items.size(); ++i) {
        auto& item = node.items[i];
        std::cerr << "[DEBUG MVIRGenerator] visiting item[" << i << "] typeid=" << typeid(*item).name() << std::endl;
        item->accept(*this);
        std::cerr << "[DEBUG MVIRGenerator] done item[" << i << "]" << std::endl;
    }
}

void MVIRGenerator::visit(VarDeclNode& node) {
    // Variable Declaration:
    // 1. Allocate space on stack: %id = alloca type
    // 2. If initialized, evaluate RHS and store: store val, %id

    const Type* varType = typeChecker_.typeOf(node.symbolId);

    // S7.2: Embed semantic identity — symbolId for uniqueness, expansionId for hygiene
    mvir::LocalId ptr = nextLocal(node.symbolId, node.expansionID);
    currentBlock_->instructions.push_back(
        std::make_unique<mvir::LocalInst>(ptr, varType)
    );

    // Save pointer location to mapping
    varAllocas_[node.symbolId] = ptr;

    if (!scopeStack_.empty()) {
        scopeStack_.back().push_back({ptr, varType});
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
    
    lastEvaluatedOperand_ = ptr;
}

void MVIRGenerator::visit(BlockStmtNode& node) {
    scopeStack_.push_back({});
    
    for (auto& stmt : node.body) {
        if (currentBlock_->terminator != nullptr) break;
        stmt->accept(*this);
    }
    
    if (currentBlock_->terminator == nullptr) {
        if (node.tailExpr) {
            lastEvaluatedOperand_ = evaluateRValue(*node.tailExpr);
        }
        
        const auto& vars = scopeStack_.back();
        for (auto it = vars.rbegin(); it != vars.rend(); ++it) {
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::DropInst>(it->id, it->type)
            );
        }
    }
    
    scopeStack_.pop_back();
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
    loopTargets_.push_back({condLbl, endLbl, scopeStack_.size()});
    node.body->accept(*this);
    loopTargets_.pop_back();
    terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

    // End block
    startBlock(endLbl);
}

// ── ASTVisitor: Expressions ──────────────────────────────────────────────────

void MVIRGenerator::visit(LiteralExpr& node) {
    if (node.kind == LiteralKind::Integer || node.kind == LiteralKind::Float) {
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
        
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(gid));
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
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
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
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(ptr));
    } else {
        // Evaluate to value
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LoadInst>(dest, node.inferredType, ptr)
        );
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
    }
}

void MVIRGenerator::visit(BinaryExpr& node) {
    mvir::Operand left = evaluateRValue(*node.left);
    mvir::Operand right = evaluateRValue(*node.right);

    if (node.overloadedMethodId != kInvalidSymbolID) {
        std::optional<mvir::LocalId> dest = std::nullopt;
        if (evalMode_ == EvalMode::RValue) {
            dest = nextLocal();
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
        }
        
        const auto& sym = table_.getSymbol(node.overloadedMethodId);
        std::string calleeName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
        mvir::Operand callee(mvir::Place(mvir::GlobalId{"@" + calleeName}));
        std::vector<mvir::Operand> args = {left, right};
        
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::CallInst>(dest, callee, args)
        );
        return;
    }

    mvir::AluOp op;
    if (node.op == BinaryOp::Add) op = mvir::AluOp::Add;
    else if (node.op == BinaryOp::Sub) op = mvir::AluOp::Sub;
    else if (node.op == BinaryOp::Mul) op = mvir::AluOp::Mul;
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

    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}

// Dummy stubs for unsupported nodes

void MVIRGenerator::visit(FunctionDeclNode& node) {
    bool isGenericTemplate = false;
    for (const auto& gp : node.genericParams) {
        if (!gp.name.empty() && gp.name[0] != '\'') {
            isGenericTemplate = true;
            break;
        }
    }

    // Skip generating body for generic templates
    if (isGenericTemplate) return;

    resetFunctionState();
    
    const Type* funcType = typeChecker_.typeOf(node.symbolId);
    const Type* retType = nullptr;
    if (auto* fTy = dynamic_cast<const FunctionType*>(funcType)) {
        retType = fTy->returnType;
    }
    
    const auto& sym = table_.getSymbol(node.symbolId);
    std::string funcName = sym.mangledName.empty() ? std::string(node.name) : sym.mangledName;
    
    auto func = std::make_unique<mvir::Function>(mvir::GlobalId{"@" + funcName, node.symbolId}, retType);
    func->isAsync = node.isAsync;
    func->isGeneric = isGenericTemplate;
    currentFunction_ = func.get();
    
    for (auto& p : node.params) {
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        // S7.2: params carry symbolId + expansionId for hygiene tracking
        func->params.push_back(mvir::Param{pType, nextLocal(p->symbolId, p->expansionID)});
    }

    module_->functions.push_back(std::move(func));

    startBlock(nextLabel("entry"));

    scopeStack_.push_back({});

    for (size_t i = 0; i < node.params.size(); ++i) {
        auto& p = node.params[i];
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        mvir::LocalId ptr = nextLocal(p->symbolId, p->expansionID);
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, pType));
        varAllocas_[p->symbolId] = ptr;

        scopeStack_.back().push_back({ptr, pType});

        mvir::LocalId argId = currentFunction_->params[i].id;
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(pType, argId, ptr));
    }
    
    if (node.body) {
        node.body->accept(*this);
    }
    
    if (currentBlock_ && currentBlock_->terminator == nullptr) {
        emitDropsForScopes(0);
        if (!retType || retType->getKind() == TypeKind::Void) {
            terminateBlock(std::make_unique<mvir::RetTerm>());
        }
    }
    
    scopeStack_.pop_back();
    
    terminateBlock(std::make_unique<mvir::RetTerm>());
    currentFunction_ = nullptr;
    currentBlock_ = nullptr;
}
void MVIRGenerator::visit(ParamDeclNode&) {}

void MVIRGenerator::visit(StructDeclNode& node) {
    const Type* stType = typeChecker_.typeOf(node.symbolId);
    if (auto st = dynamic_cast<const StructType*>(stType)) {
        mvir::TypeDecl decl;
        decl.id = st->structSymbolId;
        decl.name = "%" + std::string(node.name);
        for (auto& field : node.fields) { decl.fields.push_back(typeChecker_.typeOf(field->symbolId)); }
        module_->typeDecls.push_back(decl);
    }
}
void MVIRGenerator::visit(StructFieldNode&) {}
void MVIRGenerator::visit(EnumDeclNode& node) {
    const Type* etType = typeChecker_.typeOf(node.symbolId);
    if (auto et = dynamic_cast<const EnumType*>(etType)) {
        mvir::TypeDecl decl;
        decl.id = et->enumSymbolId;
        decl.name = "%" + std::string(node.name);
        decl.isEnum = true;
        for (auto& variant : node.variants) {
            std::vector<const Type*> payload;
            for (auto& field : variant->fields) {
                payload.push_back(typeChecker_.typeOf(field->symbolId));
            }
            decl.variants.push_back(payload);
        }
        module_->typeDecls.push_back(decl);
    }
}
void MVIRGenerator::visit(EnumVariantNode&) {}
void MVIRGenerator::visit(TraitDeclNode&) {}
void MVIRGenerator::visit(ImplDeclNode& node) {
    std::cerr << "[DEBUG MVIRGenerator] visit ImplDeclNode with " << node.methods.size() << " methods, genericParams: " << node.genericParams.size() << std::endl;
    if (!node.genericParams.empty()) return; // Skip generic impls
    for (auto& method : node.methods) {
        std::cerr << "[DEBUG MVIRGenerator] calling method->accept for " << method->name << std::endl;
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
    std::cerr << "[DEBUG] MVIRGenerator: visiting ReturnStmtNode for function " << (currentFunction_ ? currentFunction_->name.name : "null") << "\n";
    if (node.value) {
        mvir::Operand val = evaluateRValue(*node.value);
        std::cerr << "[DEBUG] MVIRGenerator: returning operand " << mvir::toString(val) << "\n";
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
    } else if (dynamic_cast<const ArrayType*>(node.iterable->inferredType) || dynamic_cast<const SliceType*>(node.iterable->inferredType)) {
        // For-Each implementation
        mvir::Operand arrayOp = evaluateLValue(*node.iterable); // Needs pointer to array for GetPtrInst
        auto* arrType = dynamic_cast<const ArrayType*>(node.iterable->inferredType);
        auto* i32Type = typeChecker_.getContext().getPrimitive(BuiltinKind::I32);

        // ── Pre-allocate all LocalInsts in the current (entry) block ──────────
        // MVIR Verifier requires all LocalInst to be in the entry block.
        mvir::LocalId idxLoc = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LocalInst>(idxLoc, i32Type)
        );
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::StoreInst>(i32Type, mvir::Number{"0"}, idxLoc)
        );

        // Pre-alloc for idxLocal2 (holds a copy of current index for GEP)
        mvir::LocalId idxLocal2 = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(idxLocal2, i32Type));

        // Pre-alloc binding variable if present
        mvir::LocalId varAlloca;
        if (node.bindingId != kInvalidSymbolID && arrType) {
            varAlloca = nextLocal();
            varAllocas_[node.bindingId] = varAlloca;
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::LocalInst>(varAlloca, arrType->elementType)
            );
        }

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
        if (arrType) {
            mvir::LocalId cmpRes = nextLocal();
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::AluInst>(cmpRes, mvir::AluOp::Lt, currIdx, mvir::Number{std::to_string(arrType->length)})
            );
            condResult = mvir::Operand(mvir::Place(cmpRes));
        }
        terminateBlock(std::make_unique<mvir::BranchTerm>(condResult, bodyLbl, endLbl));

        // Body
        startBlock(bodyLbl);
        // Store current index into idxLocal2, then GEP to load element
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(i32Type, currIdx, mvir::Operand(mvir::Place(idxLocal2))));
        mvir::LocalId elemVal = nextLocal();
        auto* arrayBase = mvir::getPlaceIf(arrayOp);
        if (arrayBase && arrType) {
            mvir::Place place = *arrayBase;
            place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Index, 0, idxLocal2});
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(elemVal, arrType->elementType, mvir::Operand(place)));
        }
        
        if (node.bindingId != kInvalidSymbolID && arrType) {
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

    } else if (node.iterMethodId != kInvalidSymbolID) {
        throw std::runtime_error("compilerBug: Trait-based Iterator MVIR lowering not fully implemented yet");
    } else {
        throw std::runtime_error("compilerBug: ForEach iterable is not Array/Slice and has no iterMethodId");
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
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
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
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, ptrVal));
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(val));
        }
    } else if (node.op == UnaryOp::Neg) {
        mvir::Operand val = evaluateRValue(*node.operand);
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::UnaryInst>(dest, mvir::UnaryOp::Negate, val));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
    } else if (node.op == UnaryOp::BitNot || node.op == UnaryOp::Not) {
        mvir::Operand val = evaluateRValue(*node.operand);
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::UnaryInst>(dest, mvir::UnaryOp::BitNot, val));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
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
        
        mvir::Place place2(closurePtr);
        place2.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, 0, std::nullopt});
        
        mvir::LocalId funcPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(funcPtr, typeChecker_.getContext().getPointerType(closTy->signature, false), mvir::Operand(place2)));
        
        callee = mvir::Operand(mvir::Place(funcPtr));
        args.insert(args.begin(), mvir::Operand(mvir::Place(closurePtr)));
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
                    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
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
            callee = mvir::Operand(mvir::Place(mvir::GlobalId{"@" + calleeName}));

            if (sym.isExternal && generatedExternals_.insert(actualFn).second) {
                mvir::ExternFunction ext;
                ext.name = (*mvir::getGlobalIf(callee));
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
                callee = mvir::Operand(mvir::Place(mvir::GlobalId{"@" + calleeName}));

                if (sym.isExternal && generatedExternals_.insert(ident->resolvedSymbol).second) {
                    mvir::ExternFunction ext;
                    ext.name = (*mvir::getGlobalIf(callee));
                    if (auto* fTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(ident->resolvedSymbol))) {
                        ext.paramTypes = fTy->paramTypes;
                        ext.returnType = fTy->returnType;
                        ext.isVariadic = fTy->isVariadic;
                    }
                    module_->externFunctions.push_back(ext);
                }
            } else if (!ident->segments.empty()) {
                callee = mvir::Operand(mvir::Place(mvir::GlobalId{"@" + std::string(ident->segments.back())}));
            }
        } else {
            callee = evaluateRValue(*node.callee);
        }
    }

    std::optional<mvir::LocalId> dest = std::nullopt;
    if (evalMode_ == EvalMode::RValue) {
        dest = nextLocal();
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
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
    
    // Auto-reference the receiver if the method expects a reference
    bool needsAutoRef = false;
    bool isMutRef = false;
    if (node.resolvedFn != kInvalidSymbolID) {
        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(node.resolvedFn))) {
            if (!fnTy->paramTypes.empty()) {
                if (auto* refTy = dynamic_cast<const ReferenceType*>(fnTy->paramTypes[0])) {
                    if (node.object->inferredType && node.object->inferredType->getKind() != TypeKind::Reference) {
                        needsAutoRef = true;
                        isMutRef = refTy->isMutable;
                    }
                }
            }
        }
    }

    if (needsAutoRef) {
        auto oldMode = evalMode_;
        evalMode_ = EvalMode::LValue;
        node.object->accept(*this);
        evalMode_ = oldMode;

        mvir::Operand addr = lastEvaluatedOperand_;
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::BorrowInst>(
            dest, isMutRef, addr
        ));
        args.push_back(mvir::Operand(mvir::Place(dest)));
    } else {
        args.push_back(evaluateRValue(*node.object)); // Receiver is the first argument
    }

    for (auto& arg : node.args) {
        args.push_back(evaluateRValue(*arg.value));
    }
    
    const Type* objTy = node.object->inferredType;
    if (auto* refTy = dynamic_cast<const ReferenceType*>(objTy)) objTy = refTy->pointee;
    else if (auto* ptrTy = dynamic_cast<const PointerType*>(objTy)) objTy = ptrTy->pointee;

    std::optional<mvir::LocalId> dest = std::nullopt;
    if (evalMode_ == EvalMode::RValue) {
        dest = nextLocal();
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
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
        std::string calleeName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
        callee = mvir::Operand(mvir::Place(mvir::GlobalId{"@" + calleeName}));
    } else {
        // Fallback for unresolved
        callee = mvir::Operand(mvir::Place(mvir::GlobalId{"@" + std::string(node.methodName)}));
    }
    
    currentBlock_->instructions.push_back(
        std::make_unique<mvir::CallInst>(dest, callee, args)
    );
}
void MVIRGenerator::visit(IndexExpr& node) {
    auto oldMode = evalMode_;
    bool isRef = node.base->inferredType->getKind() == TypeKind::Reference || 
                 node.base->inferredType->getKind() == TypeKind::Pointer;
    evalMode_ = isRef ? EvalMode::RValue : EvalMode::LValue;
    node.base->accept(*this);
    evalMode_ = oldMode;
    
    mvir::Operand basePtr = lastEvaluatedOperand_;
    mvir::Operand indexOp = evaluateRValue(*node.index);
    
    // Build index projection
    mvir::LocalId idxAlloca = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(idxAlloca, node.index->inferredType));
    currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(node.index->inferredType, indexOp, mvir::Operand(mvir::Place(idxAlloca))));

    if (auto* base = mvir::getPlaceIf(basePtr)) {
        mvir::Place place(*base);
        place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Index, 0, idxAlloca});

        if (evalMode_ == EvalMode::LValue) {
            lastEvaluatedOperand_ = mvir::Operand(place);
        } else {
            mvir::LocalId val = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, mvir::Operand(place)));
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(val));
        }
    } else {
        lastEvaluatedOperand_ = mvir::Operand{};
    }
}
void MVIRGenerator::visit(MemberExpr& node) {
    auto oldMode = evalMode_;
    bool isRef = node.object->inferredType->getKind() == TypeKind::Reference || 
                 node.object->inferredType->getKind() == TypeKind::Pointer;
    evalMode_ = isRef ? EvalMode::RValue : EvalMode::LValue;
    node.object->accept(*this);
    evalMode_ = oldMode;

    mvir::Operand objPtr = lastEvaluatedOperand_; // this is the base pointer
    mvir::Place place(*mvir::getPlaceIf(objPtr));
    place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, node.resolvedFieldIndex, std::nullopt});

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(place);
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, mvir::Operand(place)));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(val));
    }
}

void MVIRGenerator::visit(TupleIndexExpr& node) {
    // Tuples are lowered to anonymous structs — index maps directly to field index.
    auto oldMode = evalMode_;
    evalMode_ = EvalMode::LValue;
    node.object->accept(*this);
    evalMode_ = oldMode;

    mvir::Operand objPtr = lastEvaluatedOperand_;
    mvir::Place place(*mvir::getPlaceIf(objPtr));
    place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, static_cast<size_t>(node.index), std::nullopt});
    
    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(place);
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, node.inferredType, mvir::Operand(place)));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(val));
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
        std::vector<std::string> vtableMethods;
        const auto& sym = table_.getSymbol(traitObjTy->traitId);
        if (sym.decl && sym.kind == SymbolKind::Trait) {
            auto* traitDecl = static_cast<const TraitDeclNode*>(sym.decl);
            const Type* concreteTy = node.expr->inferredType;
            if (auto* refTy = dynamic_cast<const ReferenceType*>(concreteTy)) concreteTy = refTy->pointee;
            else if (auto* ptrTy = dynamic_cast<const PointerType*>(concreteTy)) concreteTy = ptrTy->pointee;
            
            for (const auto& method : traitDecl->methods) {
                TypeChecker::MethodInfo mInfo;
                if (typeChecker_.resolveMethod(concreteTy, std::string(method->name), mInfo)) {
                    vtableMethods.push_back(std::string(table_.getSymbol(mInfo.methodId).name.str()));
                } else {
                    vtableMethods.push_back("");
                }
            }
        }
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::MakeTraitObjectInst>(dest, val, node.expr->inferredType, targetTy, std::move(vtableMethods)));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
        return;
    }
    
    currentBlock_->instructions.push_back(std::make_unique<mvir::CastInst>(dest, val, node.inferredType));
    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}

void MVIRGenerator::visit(UnsizeCastExpr& node) {
    mvir::Operand val = evaluateRValue(*node.expr);
    mvir::LocalId dest = nextLocal();
    
    std::vector<std::string> vtableMethods;
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
                if (typeChecker_.resolveMethod(concreteTy, std::string(method->name), mInfo)) {
                    vtableMethods.push_back(std::string(table_.getSymbol(mInfo.methodId).name.str()));
                } else {
                    vtableMethods.push_back("");
                }
            }
        }
    }

    currentBlock_->instructions.push_back(
        std::make_unique<mvir::MakeTraitObjectInst>(dest, val, node.expr->inferredType, node.targetTypePtr, std::move(vtableMethods)));
    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}
void MVIRGenerator::visit(ArrayLiteralExpr& node) {
    auto arrTy = dynamic_cast<const ArrayType*>(node.inferredType);
    if (!arrTy) return;

    mvir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, arrTy));

    for (size_t i = 0; i < node.elements.size(); ++i) {
        mvir::Operand val = evaluateRValue(*node.elements[i]);
        mvir::LocalId idxPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(idxPtr, typeChecker_.getContext().getPrimitive(BuiltinKind::I32)));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(typeChecker_.getContext().getPrimitive(BuiltinKind::I32), mvir::Operand(mvir::Number{std::to_string(i)}), mvir::Operand(mvir::Place(idxPtr))));
        
        mvir::Place place(ptr);
        place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Index, 0, idxPtr});
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(arrTy->elementType, val, mvir::Operand(place)));
    }

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(ptr));
    } else {
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, arrTy, ptr));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(val));
    }
}
void MVIRGenerator::visit(TupleLiteralExpr& node) {
    auto tupTy = dynamic_cast<const TupleType*>(node.inferredType);
    if (!tupTy) return;

    mvir::LocalId ptr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(ptr, tupTy));

    for (size_t i = 0; i < node.elements.size(); ++i) {
        mvir::Operand val = evaluateRValue(*node.elements[i]);
        mvir::Place place(ptr);
        place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, i, std::nullopt});
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(node.elements[i]->inferredType, val, mvir::Operand(place)));
    }

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(ptr));
    } else {
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, tupTy, ptr));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(res));
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
            mvir::Place place(ptr);
            place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, idx, std::nullopt});
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(field.value->inferredType, val, mvir::Operand(place)));
        }
    }

    if (evalMode_ == EvalMode::LValue) {
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(ptr));
    } else {
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, st, ptr));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(res));
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
    
    auto func = std::make_unique<mvir::Function>(mvir::GlobalId{"@" + funcName, kInvalidSymbolID}, closureTy->signature->returnType);
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
        
        mvir::Place place(envArgId);
        place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, i + 1, std::nullopt});
        
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, capTy, mvir::Operand(place)));
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
        
        mvir::Place place1(structPtr);
        place1.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, 0, std::nullopt});
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(typeChecker_.getContext().getPointerType(closureTy->signature, false), mvir::GlobalId{"@" + funcName}, mvir::Operand(place1)));
        
        for (size_t i = 0; i < closureTy->capturedSymbols.size(); ++i) {
            auto capSym = closureTy->capturedSymbols[i];
            const Type* capTy = typeChecker_.typeOf(capSym);
            
            mvir::LocalId capAlloca = outerVarAllocas[capSym];
            mvir::LocalId capVal = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(capVal, capTy, capAlloca));
            
            mvir::Place place2(structPtr);
            place2.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, i + 1, std::nullopt});
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(capTy, capVal, mvir::Operand(place2)));
        }
        
        mvir::LocalId res = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, closureTy, structPtr));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(res));
    }
}
void MVIRGenerator::compilePattern(PatternNode* pattern, mvir::Operand subject, mvir::LabelId successLbl, mvir::LabelId failLbl) {
    if (!pattern) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(successLbl));
        return;
    }
    if (dynamic_cast<WildcardPatternNode*>(pattern)) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(successLbl));
    } else if (auto* id = dynamic_cast<IdentifierPatternNode*>(pattern)) {
        mvir::LocalId varPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(varPtr, id->inferredType));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(id->inferredType, subject, varPtr));
        varAllocas_[id->symbolId] = varPtr;
        terminateBlock(std::make_unique<mvir::JumpTerm>(successLbl));
    } else if (auto* lit = dynamic_cast<LiteralPatternNode*>(pattern)) {
        mvir::Operand litOp = mvir::Number{std::string(lit->lit->rawText)};
        if (lit->lit->kind == LiteralKind::Bool) litOp = mvir::Boolean{std::string(lit->lit->rawText) == "true"};
        
        mvir::LocalId cmpRes = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::AluInst>(cmpRes, mvir::AluOp::Eq, subject, litOp));
        terminateBlock(std::make_unique<mvir::BranchTerm>(mvir::Operand(mvir::Place(cmpRes)), successLbl, failLbl));
    } else if (auto* enumPat = dynamic_cast<EnumPatternNode*>(pattern)) {
        size_t variantIdx = 0;
        if (auto* enumType = dynamic_cast<const EnumType*>(enumPat->inferredType)) {
            auto enumSym = table_.getSymbol(enumType->enumSymbolId);
            if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                for (size_t i = 0; i < enumDecl->variants.size(); ++i) {
                    if (enumDecl->variants[i]->symbolId == enumPat->variantSymbolId) {
                        variantIdx = i; break;
                    }
                }
            }
        }
        
        mvir::LocalId tagPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::TagInst>(tagPtr, subject));
        mvir::Operand tagVal = mvir::Operand(mvir::Place(tagPtr));
        
        mvir::LabelId extractLbl = nextLabel("match_extract");
        
        mvir::LocalId cmpRes = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::AluInst>(cmpRes, mvir::AluOp::Eq, tagVal, mvir::Number{std::to_string(variantIdx)}));
        terminateBlock(std::make_unique<mvir::BranchTerm>(mvir::Operand(mvir::Place(cmpRes)), extractLbl, failLbl));
        
        startBlock(extractLbl);
        
        std::vector<const Type*> payloadTypes;
        std::vector<PatternNode*> subPatterns;
        std::vector<mvir::Operand> subSubjects;
        
        for (size_t i = 0; i < enumPat->fields.size(); ++i) {
            payloadTypes.push_back(enumPat->fields[i]->inferredType);
            subPatterns.push_back(enumPat->fields[i].get());
        }
        
        for (size_t i = 0; i < enumPat->fields.size(); ++i) {
            mvir::LocalId fieldVal = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::ExtractInst>(fieldVal, subject, payloadTypes, variantIdx, i));
            subSubjects.push_back(mvir::Operand(mvir::Place(fieldVal)));
        }
        
        compilePatternList(subPatterns, subSubjects, 0, successLbl, failLbl);
    } else if (auto* tup = dynamic_cast<TuplePatternNode*>(pattern)) {
        std::vector<PatternNode*> subPatterns;
        std::vector<mvir::Operand> subSubjects;
        // Not full tuple support, but structure is here
        for (size_t i = 0; i < tup->elements.size(); ++i) {
            subPatterns.push_back(tup->elements[i].get());
            // Should extract tuple fields
        }
        compilePatternList(subPatterns, subSubjects, 0, successLbl, failLbl);
    } else {
        terminateBlock(std::make_unique<mvir::JumpTerm>(failLbl));
    }
}

void MVIRGenerator::compilePatternList(const std::vector<PatternNode*>& patterns, const std::vector<mvir::Operand>& subjects, size_t index, mvir::LabelId successLbl, mvir::LabelId failLbl) {
    if (index >= patterns.size() || index >= subjects.size()) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(successLbl));
        return;
    }
    
    mvir::LabelId nextFieldLbl = nextLabel("match_next_field");
    compilePattern(patterns[index], subjects[index], nextFieldLbl, failLbl);
    
    startBlock(nextFieldLbl);
    compilePatternList(patterns, subjects, index + 1, successLbl, failLbl);
}

void MVIRGenerator::visit(MatchExpr& node) {
    mvir::Operand subj = evaluateRValue(*node.subject);
    
    mvir::LabelId endLbl = nextLabel("match_end");
    
    mvir::LocalId resultPtr = mvir::LocalId{""};
    if (node.inferredType && node.inferredType->getKind() != TypeKind::Unknown && node.inferredType->getKind() != TypeKind::Void) {
        resultPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LocalInst>(resultPtr, node.inferredType));
    }

    std::vector<mvir::LabelId> armFailLabels;
    for (size_t i = 0; i < node.arms.size(); ++i) {
        armFailLabels.push_back(nextLabel("match_arm_fail_" + std::to_string(i)));
    }

    for (size_t i = 0; i < node.arms.size(); ++i) {
        auto& arm = node.arms[i];
        mvir::LabelId armSuccessLbl = nextLabel("match_arm_success_" + std::to_string(i));
        mvir::LabelId armFailLbl = (i + 1 < node.arms.size()) ? armFailLabels[i + 1] : endLbl; // If last arm fails, go to endLbl
        
        compilePattern(arm.pattern.get(), subj, armSuccessLbl, armFailLbl);
        
        startBlock(armSuccessLbl);
        if (arm.body) {
            arm.body->accept(*this);
            if (!resultPtr.name.empty()) {
                mvir::Operand armRes = lastEvaluatedOperand_;
                bool isEmptyRes = false;
                if (auto* loc = mvir::getLocalIf(armRes)) {
                    if (loc->name.empty()) isEmptyRes = true;
                }
                if (!isEmptyRes && currentBlock_->terminator == nullptr) {
                    currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(node.inferredType, armRes, resultPtr));
                }
            }
        }
        if (currentBlock_->terminator == nullptr) {
            terminateBlock(std::make_unique<mvir::JumpTerm>(endLbl));
        }
        
        startBlock(armFailLbl); // Start the failure block for this arm, which the next arm will compile into!
    }
    
    if (currentBlock_->terminator == nullptr) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(endLbl));
    }
    
    startBlock(endLbl);
    if (!resultPtr.name.empty()) {
        mvir::LocalId finalRes = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(finalRes, node.inferredType, resultPtr));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(finalRes));
    } else {
        lastEvaluatedOperand_ = mvir::Operand{};
    }
}
void MVIRGenerator::visit(TryExpr& node) {
    mvir::Operand subj = evaluateRValue(*node.expr);
    
    mvir::LocalId tagPtr = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::TagInst>(tagPtr, subj));
    
    mvir::LabelId okLbl = nextLabel("try_ok");
    mvir::LabelId errLbl = nextLabel("try_err");
    
    std::vector<std::pair<mvir::Number, mvir::LabelId>> cases = {
        {mvir::Number{"0"}, okLbl},
        {mvir::Number{"1"}, errLbl}
    };
    terminateBlock(std::make_unique<mvir::SwitchTerm>(mvir::Operand(mvir::Place(tagPtr)), cases, errLbl));
    
    // Err block - extract Err/None payload (if any) and return new variant of function's return type
    startBlock(errLbl);
    const Type* errType = nullptr;
    const EnumType* exprEnumTy = dynamic_cast<const EnumType*>(
        typeChecker_.getContext().unificationTable.deepResolve(node.expr->inferredType, typeChecker_.getContext()));
    if (exprEnumTy && exprEnumTy->genericArgs.size() >= 2) {
        errType = exprEnumTy->genericArgs[1];
    }
    
    if (errType) {
        mvir::LocalId errPayloadDest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::ExtractInst>(errPayloadDest, subj, std::vector<const Type*>{errType}, 1, 0));
        mvir::LocalId newErrDest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::VariantInst>(newErrDest, currentFunction_->returnType, 1, std::vector<mvir::Operand>{mvir::Operand(mvir::Place(errPayloadDest))}));
        terminateBlock(std::make_unique<mvir::RetTerm>(mvir::Operand(mvir::Place(newErrDest))));
    } else {
        mvir::LocalId newErrDest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::VariantInst>(newErrDest, currentFunction_->returnType, 1, std::vector<mvir::Operand>{}));
        terminateBlock(std::make_unique<mvir::RetTerm>(mvir::Operand(mvir::Place(newErrDest))));
    }
    
    // Ok block - extract Ok/Some payload and continue
    startBlock(okLbl);
    if (node.inferredType) {
        mvir::LocalId okPayloadDest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::ExtractInst>(okPayloadDest, subj, std::vector<const Type*>{node.inferredType}, 0, 0));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(okPayloadDest));
    } else {
        lastEvaluatedOperand_ = mvir::Operand{};
    }
}
void MVIRGenerator::visit(AwaitExpr& node) {
    mvir::Operand futureVal = evaluateRValue(*node.expr);
    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::AwaitInst>(dest, futureVal, node.inferredType));
    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}
void MVIRGenerator::visit(SizeofExpr& node) {
    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::SizeofInst>(dest, node.evaluatedTargetType));
    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}
void MVIRGenerator::visit(AlignofExpr& node) {
    mvir::LocalId dest = nextLocal();
    currentBlock_->instructions.push_back(std::make_unique<mvir::AlignofInst>(dest, node.evaluatedTargetType));
    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}

} // namespace fl

