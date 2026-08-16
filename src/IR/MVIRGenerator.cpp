#include "mellis/IR/MVIRGenerator.h"
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/MiddleEnd/Semantic/DecisionTree.h"
#include "mellis/MiddleEnd/ComptimeEvaluator.h"
#include "mellis/Support/Diagnostic.h"
#include <cassert>
#include <iostream>

namespace fl {

MVIRGenerator::MVIRGenerator(SymbolTable& symTable, DiagnosticEngine& diag, TypeChecker& typeChecker, std::unordered_map<const Type*, ClosureStorageKind>& storageMap)
    : table_(symTable), diag_(diag), typeChecker_(typeChecker), layoutBuilder_(symTable), storageMap_(storageMap) {}

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

void MVIRGenerator::pushLocalInst(std::unique_ptr<mvir::LocalInst> inst) {
    if (currentFunction_ && !currentFunction_->blocks.empty()) {
        auto& entryBlock = currentFunction_->blocks.front();
        std::cerr << "[DEBUG pushLocalInst] Inserting LocalInst for " << inst->dest.name << " into entry block " << entryBlock->label.name << std::endl;
        
        // Find the canonical position: after all existing LocalInsts
        auto it = entryBlock->instructions.begin();
        while (it != entryBlock->instructions.end() && dynamic_cast<mvir::LocalInst*>(it->get())) {
            ++it;
        }
        
        entryBlock->instructions.insert(it, std::move(inst));
    } else if (currentBlock_) {
        std::cerr << "[DEBUG pushLocalInst] Fallback to currentBlock_ " << currentBlock_->label.name << std::endl;
        // Fallback if no function context (shouldn't happen in normal AST, but just in case)
        currentBlock_->instructions.push_back(std::move(inst));
    }
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
    varType = typeChecker_.getContext().unificationTable.deepResolve(varType, typeChecker_.getContext());
    std::cerr << "[DEBUG] VarDeclNode type kind: " << (varType ? (int)varType->getKind() : -1) << std::endl;
    if (auto* closureTy = dynamic_cast<const ClosureType*>(varType)) {
        if (storageMap_[closureTy] == ClosureStorageKind::Heap) {
            std::cerr << "[DEBUG] VarDeclNode is Heap!" << std::endl;
            varType = typeChecker_.getContext().getPointerType(closureTy, false);
        } else {
            std::cerr << "[DEBUG] VarDeclNode is NOT Heap!" << std::endl;
        }
    }

    if (!currentFunction_) {
        // Global Declaration
        mvir::GlobalDecl globalDecl;
        globalDecl.id = mvir::GlobalId{"@" + std::string(node.name)};
        globalDecl.type = varType;
        globalDecl.kind = node.isMutable ? mvir::GlobalKind::Static : mvir::GlobalKind::Const;
        globalDecl.visibility = node.visibility;
        
        if (node.initializer) {
            DiagnosticEngine diag;
            ComptimeEvaluator eval(diag);
            globalDecl.initializer = eval.evaluateExpr(node.initializer.get());
        }
        
        module_->globalDecls.push_back(globalDecl);
        return;
    }

    // S7.2: Embed semantic identity — symbolId for uniqueness, expansionId for hygiene
    mvir::LocalId ptr = nextLocal(node.symbolId, node.expansionID);
    pushLocalInst(std::make_unique<mvir::LocalInst>(ptr, varType));

    // Save pointer location to mapping
    varAllocas_[node.symbolId] = ptr;

    if (!scopeStack_.empty()) {
        scopeStack_.back().push_back({ptr, varType});
    }

    if (node.initializer) {
        mvir::Operand initVal = evaluateRValue(*node.initializer);
        const Type* assignType = typeChecker_.typeOf(node.symbolId);
        assignType = typeChecker_.getContext().unificationTable.deepResolve(assignType, typeChecker_.getContext());
        if (auto* closureTy = dynamic_cast<const ClosureType*>(assignType)) {
            if (storageMap_[closureTy] == ClosureStorageKind::Heap) {
                assignType = typeChecker_.getContext().getPointerType(closureTy, false);
            }
        }

        if (currentBlock_->terminator == nullptr) {
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::StoreInst>(assignType, initVal, ptr)
            );
        }
    }
}

void MVIRGenerator::visit(AssignExpr& node) {
    // Assignment:
    // 1. Evaluate RHS
    // 2. Load pointer from symbol mapping
    // 3. store val, %id

    mvir::Operand val = evaluateRValue(*node.value);
    mvir::Operand ptr = evaluateLValue(*node.lvalue);

    const Type* assignType = node.lvalue->inferredType;
    assignType = typeChecker_.getContext().unificationTable.deepResolve(assignType, typeChecker_.getContext());
    if (auto* closureTy = dynamic_cast<const ClosureType*>(assignType)) {
        if (storageMap_[closureTy] == ClosureStorageKind::Heap) {
            assignType = typeChecker_.getContext().getPointerType(closureTy, false);
        }
    }

    currentBlock_->instructions.push_back(
        std::make_unique<mvir::StoreInst>(assignType, val, ptr)
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
        globalDecl.id = gid;
        globalDecl.type = node.inferredType;
        globalDecl.kind = mvir::GlobalKind::Const;
        
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
        
        globalDecl.initializer = mvir::ConstantValue::makeString(unescaped + '\0'); // Add null terminator
        module_->globalDecls.push_back(globalDecl);
        
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(gid));
    } else {
        lastEvaluatedOperand_ = mvir::Number{"0"}; // Default stub for others
    }
}

void MVIRGenerator::visit(IdentifierExpr& node) {
    if (node.resolvedSymbol != kInvalidSymbolID) {
        const auto& sym = table_.getSymbol(node.resolvedSymbol);
        if (sym.kind == SymbolKind::Const || sym.kind == SymbolKind::Static) {
            mvir::GlobalId globalId{"@" + std::string(sym.name.str())};
            if (evalMode_ == EvalMode::RValue) {
                mvir::LocalId dest = nextLocal();
                currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(dest, node.inferredType, globalId));
                lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
            } else {
                lastEvaluatedOperand_ = mvir::Operand(mvir::Place(globalId));
            }
            return;
        }
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
        diag_.ice(node.loc, "Identifier symbolId not allocated");
    }
    mvir::LocalId ptr = varAllocas_[node.resolvedSymbol];

    if (borrowedCaptures_.count(node.resolvedSymbol)) {
        mvir::LocalId loadedPtr = nextLocal();
        const Type* refTy = typeChecker_.getContext().getPointerType(node.inferredType, false);
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(loadedPtr, refTy, ptr));
        ptr = loadedPtr;
    }

    if (evalMode_ == EvalMode::LValue) {
        // Return pointer directly
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(ptr));
    } else {
        // Evaluate to value
        mvir::LocalId dest = nextLocal();
        const Type* loadType = node.inferredType;
        loadType = typeChecker_.getContext().unificationTable.deepResolve(loadType, typeChecker_.getContext());
        if (auto* closureTy = dynamic_cast<const ClosureType*>(loadType)) {
            if (storageMap_[closureTy] == ClosureStorageKind::Heap) {
                loadType = typeChecker_.getContext().getPointerType(closureTy, false);
            }
        }
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::LoadInst>(dest, loadType, ptr)
        );
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
    }
}

void MVIRGenerator::visit(BinaryExpr& node) {
    mvir::Operand left = evaluateRValue(*node.left);
    mvir::Operand right = evaluateRValue(*node.right);

    if (node.opResolution.isTraitMethod) {
        std::optional<mvir::LocalId> dest = std::nullopt;
        if (evalMode_ == EvalMode::RValue) {
            dest = nextLocal();
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
        }
        
        const auto& sym = table_.getSymbol(node.opResolution.methodId);
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
        diag_.ice(node.loc, "Unknown binary operator");
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
        pushLocalInst(std::make_unique<mvir::LocalInst>(ptr, pType));
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
    bool isGenericTemplate = false;
    for (const auto& gp : node.genericParams) {
        if (!gp.name.empty() && gp.name[0] != '\'') {
            isGenericTemplate = true;
            break;
        }
    }
    if (isGenericTemplate) return;

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
    bool isGenericTemplate = false;
    for (const auto& gp : node.genericParams) {
        if (!gp.name.empty() && gp.name[0] != '\'') {
            isGenericTemplate = true;
            break;
        }
    }
    if (isGenericTemplate) return;

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
        pushLocalInst(std::make_unique<mvir::LocalInst>(idxLoc, i32Type));
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::StoreInst>(i32Type, mvir::Number{"0"}, idxLoc)
        );

        // Pre-alloc for idxLocal2 (holds a copy of current index for GEP)
        mvir::LocalId idxLocal2 = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(idxLocal2, i32Type));
        // Zero-initialize to satisfy InitializationAnalyzer (loop join-point flow)
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::StoreInst>(i32Type, mvir::Number{"0"}, mvir::Operand(mvir::Place(idxLocal2)))
        );

        // Pre-alloc binding variable if present
        mvir::LocalId varAlloca;
        if (node.bindingId != kInvalidSymbolID && arrType) {
            varAlloca = nextLocal();
            varAllocas_[node.bindingId] = varAlloca;
            pushLocalInst(std::make_unique<mvir::LocalInst>(varAlloca, arrType->elementType));
            // Zero-initialize to satisfy InitializationAnalyzer at loop join-points
            currentBlock_->instructions.push_back(
                std::make_unique<mvir::StoreInst>(arrType->elementType, mvir::Number{"0"}, mvir::Operand(mvir::Place(varAlloca)))
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

    } else if (node.nextMethodId != kInvalidSymbolID) {
        // ── Trait-based Iterator path ────────────────────────────────────────
        // Protocol:
        //   1. If iterMethodId is valid: call iter(iterable) → iterator
        //   2. Loop: call next(iterator_ptr) → Option<T>
        //   3. Read tag: None (tag=0) → break, Some(x) → unwrap + body

        const Type* iterableTy = node.iterable->inferredType;
        const Type* iteratorTy = iterableTy; // Default: iterable IS the iterator

        // Step 1: If there is a separate iter() call, invoke it to obtain an iterator
        mvir::LocalId iteratorAlloca = nextLocal();
        if (node.iterMethodId != kInvalidSymbolID) {
            const auto& iterSym = table_.getSymbol(node.iterMethodId);
            std::string iterName = iterSym.mangledName.empty() ? std::string(iterSym.name.str()) : iterSym.mangledName;
            mvir::Operand iterablePtr = evaluateLValue(*node.iterable);
            mvir::LocalId iterAddrRef = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::BorrowInst>(iterAddrRef, false, iterablePtr));

            // Infer iterator type from method return type
            if (auto* fnTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(node.iterMethodId))) {
                iteratorTy = fnTy->returnType;
            }
            pushLocalInst(std::make_unique<mvir::LocalInst>(iteratorAlloca, iteratorTy));

            mvir::LocalId iterResult = nextLocal();
            mvir::Operand callee{mvir::Place{mvir::GlobalId{"@" + iterName}}};
            currentBlock_->instructions.push_back(std::make_unique<mvir::CallInst>(
                std::optional<mvir::LocalId>{iterResult},
                callee,
                std::vector<mvir::Operand>{mvir::Operand(mvir::Place(iterAddrRef))}
            ));
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(
                iteratorTy, mvir::Operand(mvir::Place(iterResult)), mvir::Operand(mvir::Place(iteratorAlloca))
            ));
        } else {
            // The iterable is itself the iterator; just take its address
            pushLocalInst(std::make_unique<mvir::LocalInst>(iteratorAlloca, iteratorTy));
            mvir::Operand iterablePtr = evaluateLValue(*node.iterable);
            mvir::LocalId iterVal = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(iterVal, iteratorTy, iterablePtr));
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(iteratorTy, mvir::Operand(mvir::Place(iterVal)), mvir::Operand(mvir::Place(iteratorAlloca))));
        }

        // Determine element type from next() return type: Option<T> → T
        const Type* elementType = typeChecker_.getContext().getUnknown();
        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(node.nextMethodId))) {
            if (auto* optTy = dynamic_cast<const EnumType*>(fnTy->returnType)) {
                if (!optTy->genericArgs.empty()) {
                    elementType = optTy->genericArgs[0];
                }
            }
        }

        // Pre-alloc binding variable
        mvir::LocalId varAlloca;
        if (node.bindingId != kInvalidSymbolID) {
            varAlloca = nextLocal();
            varAllocas_[node.bindingId] = varAlloca;
            pushLocalInst(std::make_unique<mvir::LocalInst>(varAlloca, elementType));
        }

        // Pre-alloc Option<T> result for next()
        const Type* nextReturnTy = typeChecker_.getContext().getUnknown();
        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(node.nextMethodId))) {
            nextReturnTy = fnTy->returnType;
        }
        mvir::LocalId optionAlloca = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(optionAlloca, nextReturnTy));

        mvir::LabelId condLbl = nextLabel("foriter_cond");
        mvir::LabelId bodyLbl = nextLabel("foriter_body");
        mvir::LabelId stepLbl = nextLabel("foriter_step");
        mvir::LabelId endLbl  = nextLabel("foriter_end");

        terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

        // Condition: call next(&iterator) → Option<T>; read tag; branch on Some vs None
        startBlock(condLbl);
        {
            const auto& nextSym = table_.getSymbol(node.nextMethodId);
            std::string nextName = nextSym.mangledName.empty() ? std::string(nextSym.name.str()) : nextSym.mangledName;
            mvir::LocalId iterRef = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::BorrowInst>(iterRef, true, mvir::Operand(mvir::Place(iteratorAlloca))));
            mvir::LocalId optionResult = nextLocal();
            mvir::Operand callee{mvir::Place{mvir::GlobalId{"@" + nextName}}};
            currentBlock_->instructions.push_back(std::make_unique<mvir::CallInst>(
                std::optional<mvir::LocalId>{optionResult},
                callee,
                std::vector<mvir::Operand>{mvir::Operand(mvir::Place(iterRef))}
            ));
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(nextReturnTy, mvir::Operand(mvir::Place(optionResult)), mvir::Operand(mvir::Place(optionAlloca))));

            // Read the discriminant tag: 0 = None, 1 = Some
            mvir::LocalId tagLocal = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::TagInst>(tagLocal, mvir::Operand(mvir::Place(optionResult))));
            // Some = tag 1; branch to body if tag != 0
            mvir::LocalId isSome = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::AluInst>(isSome, mvir::AluOp::Ne, mvir::Operand(mvir::Place(tagLocal)), mvir::Number{"0"}));
            terminateBlock(std::make_unique<mvir::BranchTerm>(mvir::Operand(mvir::Place(isSome)), bodyLbl, endLbl));
        }

        // Body: unwrap Some(x), assign to binding, execute loop body
        startBlock(bodyLbl);
        {
            if (node.bindingId != kInvalidSymbolID) {
                // Extract payload from Option<T>: variant=1 (Some), field=0
                mvir::LocalId optionVal = nextLocal();
                currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(optionVal, nextReturnTy, mvir::Operand(mvir::Place(optionAlloca))));
                mvir::LocalId elemVal = nextLocal();
                std::vector<const Type*> payloadTypes{elementType};
                currentBlock_->instructions.push_back(std::make_unique<mvir::ExtractInst>(elemVal, mvir::Operand(mvir::Place(optionVal)), payloadTypes, 1, 0));
                currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(elementType, mvir::Operand(mvir::Place(elemVal)), mvir::Operand(mvir::Place(varAlloca))));
            }
        }
        loopTargets_.push_back({stepLbl, endLbl});
        if (node.body) node.body->accept(*this);
        loopTargets_.pop_back();
        terminateBlock(std::make_unique<mvir::JumpTerm>(stepLbl));

        // Step: just jump back to cond (next() is called in cond block)
        startBlock(stepLbl);
        terminateBlock(std::make_unique<mvir::JumpTerm>(condLbl));

        // End
        startBlock(endLbl);
    } // end else if (nextMethodId)
} // end visit(ForStmtNode)
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
    if (node.opResolution.isTraitMethod) {
        mvir::Operand op = evaluateRValue(*node.operand);
        
        std::optional<mvir::LocalId> dest = std::nullopt;
        if (evalMode_ == EvalMode::RValue) {
            dest = nextLocal();
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
        }
        
        const auto& sym = table_.getSymbol(node.opResolution.methodId);
        std::string calleeName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
        mvir::Operand callee(mvir::Place(mvir::GlobalId{"@" + calleeName}));
        std::vector<mvir::Operand> args = {op};
        
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::CallInst>(dest, callee, args)
        );
        return;
    }

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
        bool isHeap = storageMap_[closTy] == ClosureStorageKind::Heap;
        const Type* varType = isHeap ? static_cast<const Type*>(typeChecker_.getContext().getPointerType(closTy, false)) : static_cast<const Type*>(closTy);
        
        mvir::LocalId closurePtr = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(closurePtr, varType));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(varType, closureVal, closurePtr));
        
        mvir::Place place2(closurePtr);
        if (isHeap) {
            place2.projections.push_back(mvir::Projection{mvir::ProjectionKind::Deref, 0, std::nullopt});
        }
        place2.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, 0, std::nullopt});
        
        mvir::LocalId funcPtr = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(funcPtr, typeChecker_.getContext().getPointerType(closTy->signature, false), mvir::Operand(place2)));
        
        callee = mvir::Operand(mvir::Place(funcPtr));
        // The closure pointer argument should be dereferenced for heap closures?
        // Wait, the closure pointer argument is passed as a pointer to the struct!
        // If it's a Heap closure, the struct pointer is what is inside closurePtr!
        // Wait! In `LambdaExpr`, the lambda takes a pointer to the struct!
        // So we MUST load the pointer from closurePtr and pass THAT!
        if (isHeap) {
            mvir::LocalId loadedPtr = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(loadedPtr, static_cast<const Type*>(typeChecker_.getContext().getPointerType(closTy, false)), closurePtr));
            args.insert(args.begin(), mvir::Operand(mvir::Place(loadedPtr)));
        } else {
            args.insert(args.begin(), mvir::Operand(mvir::Place(closurePtr)));
        }
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

mvir::Operand MVIRGenerator::evaluateAutoRefReceiver(ExprNode& object, SymbolID methodId) {
    bool needsAutoRef = false;
    bool isMutRef = false;
    if (methodId != kInvalidSymbolID) {
        if (auto* fnTy = dynamic_cast<const FunctionType*>(typeChecker_.typeOf(methodId))) {
            if (!fnTy->paramTypes.empty()) {
                if (auto* refTy = dynamic_cast<const ReferenceType*>(fnTy->paramTypes[0])) {
                    if (object.inferredType && object.inferredType->getKind() != TypeKind::Reference) {
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
        object.accept(*this);
        evalMode_ = oldMode;

        mvir::Operand addr = lastEvaluatedOperand_;
        mvir::LocalId dest = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::BorrowInst>(
            dest, isMutRef, addr
        ));
        return mvir::Operand(mvir::Place(dest));
    } else {
        return evaluateRValue(object);
    }
}

void MVIRGenerator::visit(MethodCallExpr& node) {
    if (node.intrinsic != IntrinsicKind::None) {
        mvir::Operand base = evaluateRValue(*node.object);
        mvir::Operand offset = evaluateRValue(*node.args[0].value);
        
        std::optional<mvir::LocalId> dest = std::nullopt;
        if (evalMode_ == EvalMode::RValue) {
            dest = nextLocal();
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
        } else {
            // Should not happen for arithmetic
            dest = nextLocal();
        }
        
        if (node.intrinsic == IntrinsicKind::PtrSub) {
            mvir::LocalId negOffset = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::UnaryInst>(negOffset, mvir::UnaryOp::Negate, offset));
            offset = mvir::Operand(mvir::Place(negOffset));
        }
        
        const Type* objTy = typeChecker_.getContext().unificationTable.deepResolve(node.object->inferredType, typeChecker_.getContext());
        const Type* elemTy = objTy->unwrapAlias();
        if (auto* ptrTy = dynamic_cast<const PointerType*>(elemTy)) {
            elemTy = ptrTy->pointee;
        } else if (auto* refTy = dynamic_cast<const ReferenceType*>(elemTy)) {
            elemTy = refTy->pointee;
        }
        
        currentBlock_->instructions.push_back(std::make_unique<mvir::PtrOffsetInst>(*dest, base, offset, elemTy));
        return;
    }

    std::vector<mvir::Operand> args;
    args.push_back(evaluateAutoRefReceiver(*node.object, node.resolvedFn));

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
        const auto& layout = layoutBuilder_.getOrCreateLayout(traitObjTy);
        auto it = layout.methodSlots.find(node.resolvedFn);
        if (it != layout.methodSlots.end()) {
            methodIndex = it->second;
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
    if (node.opResolution.isTraitMethod) {
        mvir::Operand base = evaluateAutoRefReceiver(*node.base, node.opResolution.methodId);
        mvir::Operand indexOp = evaluateRValue(*node.index);
        
        std::optional<mvir::LocalId> dest = std::nullopt;
        if (evalMode_ == EvalMode::RValue) {
            dest = nextLocal();
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(*dest));
        }
        
        const auto& sym = table_.getSymbol(node.opResolution.methodId);
        std::string calleeName = sym.mangledName.empty() ? std::string(sym.name.str()) : sym.mangledName;
        mvir::Operand callee(mvir::Place(mvir::GlobalId{"@" + calleeName}));
        std::vector<mvir::Operand> args = {base, indexOp};
        
        mvir::LocalId retLocal = nextLocal();
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::CallInst>(retLocal, callee, args)
        );
        
        // trait method returns &T or &mut T. If we are in RValue mode, we must dereference it!
        // wait! FDLang Index method returns a reference? Yes, usually Index returns &Output and IndexMut returns &mut Output.
        // If we evaluate in RValue mode, we should Load it.
        if (evalMode_ == EvalMode::RValue) {
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(
                *dest, node.inferredType, mvir::Operand(mvir::Place(retLocal))
            ));
        } else {
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(retLocal));
        }
        return;
    }

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
    pushLocalInst(std::make_unique<mvir::LocalInst>(idxAlloca, node.index->inferredType));
    currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(node.index->inferredType, indexOp, mvir::Operand(mvir::Place(idxAlloca))));

    // Bounds check
    const Type* realBaseTy = typeChecker_.getContext().unificationTable.deepResolve(node.base->inferredType, typeChecker_.getContext());
    if (auto* refTy = dynamic_cast<const ReferenceType*>(realBaseTy)) {
        realBaseTy = typeChecker_.getContext().unificationTable.deepResolve(refTy->pointee, typeChecker_.getContext());
    }
    
    mvir::Operand lenOp;
    bool hasLen = false;
    if (auto* arrTy = dynamic_cast<const ArrayType*>(realBaseTy)) {
        lenOp = mvir::Number{std::to_string(arrTy->length)};
        hasLen = true;
    } else if (dynamic_cast<const SliceType*>(realBaseTy)) {
        mvir::LocalId lenLocal = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::TupleExtractInst>(
            lenLocal, basePtr, 1, typeChecker_.getContext().getPrimitive(BuiltinKind::U64)));
        lenOp = mvir::Operand(mvir::Place(lenLocal));
        hasLen = true;
    }
    
    if (hasLen) {
        mvir::LocalId cmpRes = nextLocal();
        // Check if index >= length
        currentBlock_->instructions.push_back(std::make_unique<mvir::AluInst>(cmpRes, mvir::AluOp::Ge, indexOp, lenOp));
        
        mvir::LabelId failLbl = nextLabel("bounds_fail");
        mvir::LabelId okLbl = nextLabel("bounds_ok");
        
        terminateBlock(std::make_unique<mvir::BranchTerm>(mvir::Operand(mvir::Place(cmpRes)), failLbl, okLbl));
        
        startBlock(failLbl);
        // Call __mellis_bounds_fail (we don't pass args yet, just call a generic external fail function)
        std::vector<mvir::Operand> failArgs = { indexOp, lenOp };
        currentBlock_->instructions.push_back(std::make_unique<mvir::CallInst>(
            std::nullopt, 
            mvir::Operand(mvir::Place(mvir::GlobalId{"@__mellis_bounds_fail"})), 
            failArgs, nullptr
        ));
        terminateBlock(std::make_unique<mvir::UnreachableTerm>());
        
        startBlock(okLbl);
    }

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
    place.projections.push_back(mvir::Projection{mvir::ProjectionKind::TupleIndex, static_cast<size_t>(node.index), std::nullopt});
    
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
        const Type* concreteTy = node.expr->inferredType;
        if (auto* refTy = dynamic_cast<const ReferenceType*>(concreteTy)) concreteTy = refTy->pointee;
        else if (auto* ptrTy = dynamic_cast<const PointerType*>(concreteTy)) concreteTy = ptrTy->pointee;

        std::string vtableMangled = layoutBuilder_.getVTableMangledName(concreteTy, traitObjTy);
        const auto& layout = layoutBuilder_.getOrCreateLayout(traitObjTy);
        std::vector<std::string> vtableMethods(layout.slotOrder.size(), "");
        for (size_t i = 0; i < layout.slotOrder.size(); ++i) {
            SymbolID traitMethodId = layout.slotOrder[i];
            const auto& traitMethodSym = table_.getSymbol(traitMethodId);
            std::string traitMethodName = std::string(traitMethodSym.name.str());
            MethodInfo mInfo;
            if (typeChecker_.getMethodResolver().probe(concreteTy, traitMethodName, mInfo, typeChecker_.getTraitSolver(), typeChecker_.getContext(), table_, typeChecker_.getTypeTable())) {
                SymbolID targetMethodId = mInfo.id;
                if (mInfo.implNode) {
                    if (auto* implNode = dynamic_cast<const ImplDeclNode*>(mInfo.implNode)) {
                        for (auto& m : implNode->methods) {
                            if (std::string(m->name) == traitMethodName) {
                                targetMethodId = m->symbolId;
                                break;
                            }
                        }
                    }
                }
                const auto& mSym = table_.getSymbol(targetMethodId);
                vtableMethods[i] = mSym.mangledName.empty() ? std::string(mSym.name.str()) : mSym.mangledName;
            } else {
                std::cerr << "[FATAL MVIR Cast] PROBE FAILED for method " << traitMethodName << " on type " << concreteTy->toString() << std::endl;
            }
        }
        
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::MakeTraitObjectInst>(dest, val, node.expr->inferredType, targetTy, std::move(vtableMangled), std::move(vtableMethods)));
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
        const Type* concreteTy = node.expr->inferredType;
        if (auto* refTy = dynamic_cast<const ReferenceType*>(concreteTy)) concreteTy = refTy->pointee;
        else if (auto* ptrTy = dynamic_cast<const PointerType*>(concreteTy)) concreteTy = ptrTy->pointee;
        
        std::string vtableMangled = layoutBuilder_.getVTableMangledName(concreteTy, traitObjTy);
        const auto& layout = layoutBuilder_.getOrCreateLayout(traitObjTy);
        std::vector<std::string> vtableMethods(layout.slotOrder.size(), "");
        for (size_t i = 0; i < layout.slotOrder.size(); ++i) {
            SymbolID traitMethodId = layout.slotOrder[i];
            const auto& traitMethodSym = table_.getSymbol(traitMethodId);
            std::string traitMethodName = std::string(traitMethodSym.name.str());
            MethodInfo mInfo;
            if (typeChecker_.getMethodResolver().probe(node.expr->inferredType, traitMethodName, mInfo, typeChecker_.getTraitSolver(), typeChecker_.getContext(), table_, typeChecker_.getTypeTable())) {
                SymbolID targetMethodId = mInfo.id;
                if (mInfo.implNode) {
                    if (auto* implNode = dynamic_cast<const ImplDeclNode*>(mInfo.implNode)) {
                        std::cerr << "[DEBUG] ImplDecl found! methods count: " << implNode->methods.size() << std::endl;
                        for (auto& m : implNode->methods) {
                            std::cerr << "   [DEBUG] Impl method: '" << m->name << "', matching with '" << traitMethodName << "'" << std::endl;
                            if (std::string(m->name) == traitMethodName) {
                                targetMethodId = m->symbolId;
                                break;
                            }
                        }
                    } else {
                        std::cerr << "[DEBUG] mInfo.implNode is NOT ImplDeclNode! It is at " << mInfo.implNode << std::endl;
                    }
                } else {
                    std::cerr << "[DEBUG] mInfo.implNode is NULL!" << std::endl;
                }
                const auto& mSym = table_.getSymbol(targetMethodId);
                vtableMethods[i] = mSym.mangledName.empty() ? std::string(mSym.name.str()) : mSym.mangledName;
            } else {
                std::cerr << "[FATAL MVIR UnsizeCast] PROBE FAILED for method " << traitMethodName << " on type " << node.expr->inferredType->toString() << std::endl;
            }
        }
        
        currentBlock_->instructions.push_back(
            std::make_unique<mvir::MakeTraitObjectInst>(dest, val, node.expr->inferredType, node.targetTypePtr, std::move(vtableMangled), std::move(vtableMethods)));
        lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
        return;
        const Type* srcTy = node.expr->inferredType;
        /* TRIGGER REBUILD */ ;
        if (auto* refTy = dynamic_cast<const ReferenceType*>(srcTy)) srcTy = refTy->pointee;
        else if (auto* ptrTy = dynamic_cast<const PointerType*>(srcTy)) srcTy = ptrTy->pointee;

        if (auto* arrTy = dynamic_cast<const ArrayType*>(srcTy)) {
            mvir::Operand lenOp = mvir::Number{std::to_string(arrTy->length)};
            currentBlock_->instructions.push_back(std::make_unique<mvir::MakeSliceInst>(dest, val, lenOp));
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
            return;
        }
    }

    // Fallback if anything goes wrong
    currentBlock_->instructions.push_back(std::make_unique<mvir::CastInst>(dest, val, node.targetTypePtr));
    lastEvaluatedOperand_ = mvir::Operand(mvir::Place(dest));
}
void MVIRGenerator::visit(ArrayLiteralExpr& node) {
    auto arrTy = dynamic_cast<const ArrayType*>(node.inferredType);
    if (!arrTy) return;

    mvir::LocalId ptr = nextLocal();
    pushLocalInst(std::make_unique<mvir::LocalInst>(ptr, arrTy));

    for (size_t i = 0; i < node.elements.size(); ++i) {
        mvir::Operand val = evaluateRValue(*node.elements[i]);
        mvir::LocalId idxPtr = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(idxPtr, typeChecker_.getContext().getPrimitive(BuiltinKind::I32)));
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
    pushLocalInst(std::make_unique<mvir::LocalInst>(ptr, tupTy));

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
    pushLocalInst(std::make_unique<mvir::LocalInst>(ptr, st));

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
    auto outerBlock = currentBlock_;
    auto outerVarAllocas = varAllocas_;
    auto outerBorrowedCaptures = borrowedCaptures_;
    auto outerScopeStack = scopeStack_;
    size_t outerLocalId = nextLocalId_;
    size_t outerLabelId = nextLabelId_;
    
    resetFunctionState();
    scopeStack_.clear();
    
    auto func = std::make_unique<mvir::Function>(mvir::GlobalId{"@" + funcName, node.generatedFuncId}, closureTy->signature->returnType);
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
    
    scopeStack_.push_back({});
    
    mvir::LocalId envPtr = nextLocal();
    const Type* envPtrType = typeChecker_.getContext().getPointerType(closureTy, false);
    pushLocalInst(std::make_unique<mvir::LocalInst>(envPtr, envPtrType));
    currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(envPtrType, envArgId, envPtr));
    scopeStack_.back().push_back({envPtr, envPtrType}); // Restored: lambda owns the environment for FnOnce semantics.
    
    for (size_t i = 0; i < closureTy->captures.size(); ++i) {
        auto capSym = closureTy->captures[i].symbolId;
        const Type* capTy = closureTy->captures[i].envType;
        
        if (closureTy->captures[i].mode == CaptureMode::Borrow || closureTy->captures[i].mode == CaptureMode::BorrowMut) {
            borrowedCaptures_.insert(capSym);
        }
        
        mvir::LocalId capAlloca = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(capAlloca, capTy));
        varAllocas_[capSym] = capAlloca;
        
        mvir::Place place(envArgId);
        place.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, i + 1, std::nullopt});
        
        mvir::LocalId val = nextLocal();
        currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(val, capTy, mvir::Operand(place)));
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(capTy, val, capAlloca));
        
        // Do NOT push capAlloca to scopeStack, to prevent double-drop of captures in the lambda body.
    }
    
    for (size_t i = 0; i < node.params.size(); ++i) {
        auto& p = node.params[i];
        const Type* pType = typeChecker_.typeOf(p->symbolId);
        mvir::LocalId ptr = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(ptr, pType));
        varAllocas_[p->symbolId] = ptr;
        
        mvir::LocalId argId = currentFunction_->params[i + 1].id;
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(pType, argId, ptr));
        
        scopeStack_.back().push_back({ptr, pType});
    }
    
    if (node.body) {
        node.body->accept(*this);
    }
    
    if (currentBlock_ && currentBlock_->terminator == nullptr) {
        std::cerr << "[DEBUG MVIRGenerator] ClosureExpr block needs terminator! emitting drops for size=" << scopeStack_.size() << std::endl;
        emitDropsForScopes(0);
        if (closureTy->signature->returnType && closureTy->signature->returnType->getKind() != TypeKind::Void) {
            terminateBlock(std::make_unique<mvir::RetTerm>(lastEvaluatedOperand_));
        } else {
            terminateBlock(std::make_unique<mvir::RetTerm>());
        }
    } else {
        std::cerr << "[DEBUG MVIRGenerator] ClosureExpr block already has terminator?!" << std::endl;
    }
    
    scopeStack_.pop_back();
    
    currentFunction_ = outerFunction;
    currentBlock_ = outerBlock;
    varAllocas_ = outerVarAllocas;
    borrowedCaptures_ = outerBorrowedCaptures;
    scopeStack_ = outerScopeStack;
    nextLocalId_ = outerLocalId;
    nextLabelId_ = outerLabelId;
    
    if (currentFunction_) {
        mvir::LocalId structPtr = nextLocal();
        bool isHeap = storageMap_[closureTy] == ClosureStorageKind::Heap;
        if (isHeap) {
            pushLocalInst(std::make_unique<mvir::LocalInst>(structPtr, typeChecker_.getContext().getPointerType(closureTy, false)));
            currentBlock_->instructions.push_back(std::make_unique<mvir::HeapAllocInst>(structPtr, closureTy));
        } else {
            pushLocalInst(std::make_unique<mvir::LocalInst>(structPtr, closureTy));
        }
        
        mvir::Place place1(structPtr);
        if (isHeap) {
            place1.projections.push_back(mvir::Projection{mvir::ProjectionKind::Deref, 0, std::nullopt});
        }
        place1.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, 0, std::nullopt});
        currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(typeChecker_.getContext().getPointerType(closureTy->signature, false), mvir::GlobalId{"@" + funcName}, mvir::Operand(place1)));
        
        for (size_t i = 0; i < closureTy->captures.size(); ++i) {
            auto capSym = closureTy->captures[i].symbolId;
            const Type* capTy = closureTy->captures[i].envType;
            
            mvir::LocalId capAlloca = outerVarAllocas[capSym];
            mvir::Operand capValOp;

            if (closureTy->captures[i].mode == CaptureMode::Borrow || closureTy->captures[i].mode == CaptureMode::BorrowMut) {
                if (outerBorrowedCaptures.count(capSym)) {
                    mvir::LocalId capVal = nextLocal();
                    currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(capVal, capTy, capAlloca));
                    capValOp = mvir::Operand(mvir::Place(capVal));
                } else {
                    capValOp = mvir::Operand(mvir::Place(capAlloca));
                }
            } else {
                mvir::LocalId capVal = nextLocal();
                if (outerBorrowedCaptures.count(capSym)) {
                    mvir::LocalId loadedPtr = nextLocal();
                    const Type* refTy = typeChecker_.getContext().getPointerType(capTy, false);
                    currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(loadedPtr, refTy, capAlloca));
                    currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(capVal, capTy, loadedPtr));
                } else {
                    currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(capVal, capTy, capAlloca));
                }
                capValOp = mvir::Operand(mvir::Place(capVal));
            }
            
            mvir::Place place2(structPtr);
            if (isHeap) {
                place2.projections.push_back(mvir::Projection{mvir::ProjectionKind::Deref, 0, std::nullopt});
            }
            place2.projections.push_back(mvir::Projection{mvir::ProjectionKind::Field, i + 1, std::nullopt});
            currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(capTy, capValOp, mvir::Operand(place2)));
        }
        
        if (isHeap) {
            // Heap closures are handled as pointers, so we evaluate the variable holding the pointer
            mvir::LocalId res = nextLocal();
            const Type* ptrTy = typeChecker_.getContext().getPointerType(closureTy, false);
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, ptrTy, structPtr));
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(res));
        } else {
            mvir::LocalId res = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(res, closureTy, structPtr));
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(res));
        }
    }
}
void MVIRGenerator::compileDecisionTree(DecisionNode* node, std::unordered_map<std::string, mvir::Operand>& places, const std::vector<mvir::LabelId>& armLabels, mvir::LabelId fallbackLbl) {
    if (!node) {
        terminateBlock(std::make_unique<mvir::JumpTerm>(fallbackLbl));
        return;
    }
    
    switch (node->kind) {
        case DecisionKind::Success: {
            auto* sNode = static_cast<SuccessNode*>(node);
            for (auto& bind : sNode->bindings) {
                mvir::LocalId varPtr = nextLocal();
                // We need the type. But we can't easily get it here. 
                // Wait! bindings maps SymbolID -> string. We have typeChecker_.typeOf(symbolId)
                const Type* varTy = typeChecker_.typeOf(bind.first);
                pushLocalInst(std::make_unique<mvir::LocalInst>(varPtr, varTy));
                currentBlock_->instructions.push_back(std::make_unique<mvir::StoreInst>(varTy, places[bind.second], varPtr));
                varAllocas_[bind.first] = varPtr;
            }
            terminateBlock(std::make_unique<mvir::JumpTerm>(armLabels[sNode->armIndex]));
            break;
        }
        case DecisionKind::Failure: {
            terminateBlock(std::make_unique<mvir::JumpTerm>(fallbackLbl));
            break;
        }
        case DecisionKind::SwitchTag: {
            auto* sNode = static_cast<SwitchTagNode*>(node);
            mvir::Operand subjectOp = places[sNode->placeStr];
            
            mvir::LocalId tagPtr = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::TagInst>(tagPtr, subjectOp));
            mvir::Operand tagVal = mvir::Operand(mvir::Place(tagPtr));
            
            // Build switch cascade
            // For now, since MVIR doesn't have SwitchTerm, we build a chain of Branches
            mvir::LabelId currentLbl = currentBlock_->label;
            
            for (size_t i = 0; i < sNode->cases.size(); ++i) {
                auto& cas = sNode->cases[i];
                // we need to know the index of the variant. The cas.first is the variant SymbolID.
                // we can map it back to index... Wait, TagInst returns the index! We don't have the index here easily unless ExtractNode or Variant gives it.
                // In MVIRGenerator previously:
                // We looked up EnumDeclNode to get variantIdx from variantSymbolId.
                size_t variantIdx = 0;
                auto enumSym = table_.getSymbol(cas.first);
                if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                    auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                    for (size_t v = 0; v < enumDecl->variants.size(); ++v) {
                        if (enumDecl->variants[v]->symbolId == cas.first) { variantIdx = v; break; }
                    }
                }
                
                mvir::LocalId cmpRes = nextLocal();
                currentBlock_->instructions.push_back(std::make_unique<mvir::AluInst>(cmpRes, mvir::AluOp::Eq, tagVal, mvir::Number{std::to_string(variantIdx)}));
                
                mvir::LabelId matchLbl = nextLabel("match_tag_" + std::to_string(variantIdx));
                mvir::LabelId nextTestLbl = nextLabel("match_tag_next");
                
                terminateBlock(std::make_unique<mvir::BranchTerm>(mvir::Operand(mvir::Place(cmpRes)), matchLbl, nextTestLbl));
                
                startBlock(matchLbl);
                compileDecisionTree(cas.second.get(), places, armLabels, fallbackLbl);
                
                startBlock(nextTestLbl);
            }
            compileDecisionTree(sNode->fallback.get(), places, armLabels, fallbackLbl);
            break;
        }
        case DecisionKind::SwitchLit: {
            auto* sNode = static_cast<SwitchLitNode*>(node);
            compileDecisionTree(sNode->fallback.get(), places, armLabels, fallbackLbl);
            break;
        }
        case DecisionKind::Extract: {
            auto* eNode = static_cast<ExtractNode*>(node);
            mvir::Operand subjectOp = places[eNode->placeStr];
            
            size_t variantIdx = 0;
            if (eNode->variantId != kInvalidSymbolID) {
                auto enumSym = table_.getSymbol(eNode->variantId);
                if (enumSym.kind == SymbolKind::Enum && enumSym.decl) {
                    auto* enumDecl = static_cast<EnumDeclNode*>(enumSym.decl);
                    for (size_t v = 0; v < enumDecl->variants.size(); ++v) {
                        if (enumDecl->variants[v]->symbolId == eNode->variantId) { variantIdx = v; break; }
                    }
                    
                    std::vector<const Type*> payloadTypes;
                    for (auto& f : enumDecl->variants[variantIdx]->fields) {
                        payloadTypes.push_back(typeChecker_.typeOf(f->symbolId));
                    }
                    
                    for (size_t i = 0; i < eNode->bindNames.size(); ++i) {
                        mvir::LocalId fieldVal = nextLocal();
                        currentBlock_->instructions.push_back(std::make_unique<mvir::ExtractInst>(fieldVal, subjectOp, payloadTypes, variantIdx, i));
                        places[eNode->bindNames[i]] = mvir::Operand(mvir::Place(fieldVal));
                    }
                }
            } else {
                // Tuple extract
            }
            
            compileDecisionTree(eNode->next.get(), places, armLabels, fallbackLbl);
            break;
        }
    }
}



void MVIRGenerator::visit(MatchExpr& node) {
    mvir::Operand subj = evaluateRValue(*node.subject);
    
    mvir::LabelId endLbl = nextLabel("match_end");
    
    mvir::LocalId resultPtr = mvir::LocalId{""};
    if (node.inferredType && node.inferredType->getKind() != TypeKind::Unknown && node.inferredType->getKind() != TypeKind::Void) {
        resultPtr = nextLocal();
        pushLocalInst(std::make_unique<mvir::LocalInst>(resultPtr, node.inferredType));
    }

    std::unordered_map<std::string, mvir::Operand> places;
    places["subject"] = subj;

    std::vector<mvir::LabelId> armLabels;
    for (size_t i = 0; i < node.arms.size(); ++i) {
        armLabels.push_back(nextLabel("match_arm_" + std::to_string(i)));
    }
    mvir::LabelId unreachableLbl = nextLabel("match_unreachable");
    
    compileDecisionTree(node.decisionTree.get(), places, armLabels, unreachableLbl);

    bool hasJumpsToEnd = false;

    for (size_t i = 0; i < node.arms.size(); ++i) {
        auto& arm = node.arms[i];
        startBlock(armLabels[i]);
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
            hasJumpsToEnd = true;
        }
    }
    
    startBlock(unreachableLbl);
    terminateBlock(std::make_unique<mvir::UnreachableTerm>());
    
    if (hasJumpsToEnd) {
        startBlock(endLbl);
        if (!resultPtr.name.empty()) {
            mvir::LocalId finalRes = nextLocal();
            currentBlock_->instructions.push_back(std::make_unique<mvir::LoadInst>(finalRes, node.inferredType, resultPtr));
            lastEvaluatedOperand_ = mvir::Operand(mvir::Place(finalRes));
        } else {
            lastEvaluatedOperand_ = mvir::Operand{};
        }
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

