#include "mellis/BackEnd/LLVMIRGenerator.h"
#include <llvm/IR/Intrinsics.h>
#include <llvm/IR/Constants.h>
#include <llvm/IR/Verifier.h>
#include <llvm/Support/raw_ostream.h>
#include <iostream>
#include <cassert>
#include "mellis/AST/DeclNode.h"

namespace fl {

LLVMIRGenerator::LLVMIRGenerator(llvm::LLVMContext& context, llvm::Module& module, const SymbolTable& symTable)
    : context_(context), module_(module), builder_(context), symTable_(symTable) {}

bool LLVMIRGenerator::generate(const mvir::Module* mvirModule) {
    globalValues_.clear(); localValues_.clear(); pointerTypes_.clear();
    structTypes_.clear(); blocks_.clear();

    // 1. Declare Struct types (opaque first)
    for (const auto& tDecl : mvirModule->typeDecls) {
        std::string structName = tDecl.name.substr(1);
        llvm::StructType* st = llvm::StructType::create(context_, structName);
        structTypes_[structName] = st;
    }
    // Set bodies for Struct types
    for (const auto& tDecl : mvirModule->typeDecls) {
        std::string structName = tDecl.name.substr(1);
        llvm::StructType* st = structTypes_[structName];
        std::vector<llvm::Type*> fields;
        for (const auto& fType : tDecl.fields) {
            fields.push_back(mapType(fType));
        }
        st->setBody(fields);
    }

    // Pass 1: Declare all functions
    for (const auto& eFunc : mvirModule->externFunctions) {
        std::string funcName = eFunc.name.name.substr(1); // strip '@'
        std::vector<llvm::Type*> paramTypes;
        for (const auto& pType : eFunc.paramTypes) {
            paramTypes.push_back(mapType(pType));
        }
        llvm::FunctionType* fType = llvm::FunctionType::get(mapType(eFunc.returnType), paramTypes, eFunc.isVariadic);
        llvm::Function::Create(fType, llvm::Function::ExternalLinkage, funcName, module_);
    }

    bool hasAsyncMain = false;
    for (const auto& func : mvirModule->functions) {
        std::string funcName = func->name.name.substr(1);
        if (funcName == "main" && func->isAsync) {
            funcName = "__fd_main";
            hasAsyncMain = true;
        }
        std::vector<llvm::Type*> paramTypes;
        for (const auto& param : func->params) {
            paramTypes.push_back(mapType(param.type));
        }
        llvm::FunctionType* fType = llvm::FunctionType::get(mapType(func->returnType), paramTypes, false);
        llvm::Function* f = llvm::Function::Create(fType, llvm::Function::ExternalLinkage, funcName, module_);
        globalValues_[func->name.name] = f;
    }

    // 1.5 Global string literals
    for (const auto& gDecl : mvirModule->globalDecls) {
        std::string name = gDecl.name.name.substr(1);
        if (!gDecl.stringLiteral.empty()) {
            llvm::Constant* strConst = llvm::ConstantDataArray::getString(context_, gDecl.stringLiteral, true);
            llvm::GlobalVariable* gv = new llvm::GlobalVariable(
                module_, strConst->getType(), true,
                llvm::GlobalValue::PrivateLinkage, strConst, name
            );
            globalValues_[gDecl.name.name] = gv;
        }
    }

    // Translate instructions
    std::cout << "[LLVMGen] Start generating..." << std::endl;
    for (const auto& func : mvirModule->functions) {
        createFunctionStructure(func.get());
        emitFunctionBody(func.get());
    }
    std::cout << "[LLVMGen] Functions built." << std::endl;

    if (hasAsyncMain) {
        // Generate a synchronous C main that calls block_on
        llvm::FunctionType* mainTy = llvm::FunctionType::get(llvm::Type::getInt32Ty(context_), {}, false);
        llvm::Function* mainFn = llvm::Function::Create(mainTy, llvm::Function::ExternalLinkage, "main", module_);
        llvm::BasicBlock* entryBB = llvm::BasicBlock::Create(context_, "entry", mainFn);
        builder_.SetInsertPoint(entryBB);
        
        llvm::Function* fdMain = module_.getFunction("__fd_main");
        llvm::Value* fut = builder_.CreateCall(fdMain);
        
        llvm::BasicBlock* checkBB = llvm::BasicBlock::Create(context_, "check", mainFn);
        llvm::BasicBlock* resumeBB = llvm::BasicBlock::Create(context_, "resume", mainFn);
        llvm::BasicBlock* doneBB = llvm::BasicBlock::Create(context_, "done", mainFn);
        
        builder_.CreateBr(checkBB);
        
        builder_.SetInsertPoint(checkBB);
        llvm::Function* coroDoneFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_done);
        llvm::Value* isDone = builder_.CreateCall(coroDoneFn, {fut});
        builder_.CreateCondBr(isDone, doneBB, resumeBB);
        
        builder_.SetInsertPoint(resumeBB);
        llvm::Function* coroResumeFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_resume);
        builder_.CreateCall(coroResumeFn, {fut});
        builder_.CreateBr(checkBB);
        
        builder_.SetInsertPoint(doneBB);
        llvm::Function* coroPromiseFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_promise);
        llvm::Value* promisePtr = builder_.CreateCall(coroPromiseFn, {fut, builder_.getInt32(8), builder_.getInt1(false)});
        
        llvm::StructType* promiseTy = llvm::StructType::get(context_, {llvm::Type::getInt8Ty(context_), llvm::Type::getInt32Ty(context_)});
        llvm::Value* resPtr = builder_.CreateStructGEP(promiseTy, promisePtr, 1);
        llvm::Value* res = builder_.CreateLoad(llvm::Type::getInt32Ty(context_), resPtr);
        builder_.CreateRet(res);
    }

    // Inject runtime helpers
    llvm::FunctionType* doneFnTy = llvm::FunctionType::get(llvm::Type::getInt1Ty(context_), {llvm::PointerType::getUnqual(context_)}, false);
    llvm::Function* doneFn = llvm::Function::Create(doneFnTy, llvm::Function::ExternalLinkage, "mellis_coro_is_done", module_);
    llvm::BasicBlock* doneBB = llvm::BasicBlock::Create(context_, "entry", doneFn);
    builder_.SetInsertPoint(doneBB);
    llvm::Function* coroDoneIntrin = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_done);
    llvm::Value* isDone = builder_.CreateCall(coroDoneIntrin, {doneFn->getArg(0)});
    builder_.CreateRet(isDone);

    llvm::FunctionType* resumeFnTy = llvm::FunctionType::get(llvm::Type::getVoidTy(context_), {llvm::PointerType::getUnqual(context_)}, false);
    llvm::Function* resumeFn = llvm::Function::Create(resumeFnTy, llvm::Function::ExternalLinkage, "mellis_coro_resume", module_);
    llvm::BasicBlock* resumeBB = llvm::BasicBlock::Create(context_, "entry", resumeFn);
    builder_.SetInsertPoint(resumeBB);
    llvm::Function* coroResumeIntrin = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_resume);
    builder_.CreateCall(coroResumeIntrin, {resumeFn->getArg(0)});
    builder_.CreateRetVoid();

    std::cout << "[LLVMGen] Coroutines built." << std::endl;
    std::error_code EC;
    llvm::raw_fd_ostream os("llvm.ir", EC);
    if (!EC) {
        module_.print(os, nullptr);
        os.flush();
    }
    bool broken = llvm::verifyModule(module_, &llvm::errs());
    return !broken;
}

llvm::Type* LLVMIRGenerator::mapType(const Type* type) {
    if (!type) return llvm::Type::getVoidTy(context_);
    
    if (auto* prim = dynamic_cast<const PrimitiveType*>(type)) {
        switch (prim->builtinKind) {
            case BuiltinKind::I8:
            case BuiltinKind::U8:
                return llvm::Type::getInt8Ty(context_);
            case BuiltinKind::I16:
            case BuiltinKind::U16:
                return llvm::Type::getInt16Ty(context_);
            case BuiltinKind::I32:
            case BuiltinKind::U32:
            case BuiltinKind::Char:
                return llvm::Type::getInt32Ty(context_);
            case BuiltinKind::I64:
            case BuiltinKind::U64:
                return llvm::Type::getInt64Ty(context_);
            case BuiltinKind::F32: return llvm::Type::getFloatTy(context_);
            case BuiltinKind::F64: return llvm::Type::getDoubleTy(context_);
            case BuiltinKind::Bool: return llvm::Type::getInt1Ty(context_);
            case BuiltinKind::Str: return llvm::PointerType::getUnqual(context_);
            default: return llvm::Type::getVoidTy(context_);
        }
    }

    if (auto* ptr = dynamic_cast<const PointerType*>(type)) {
        if (dynamic_cast<const TraitObjectType*>(ptr->pointee)) {
            return llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::PointerType::getUnqual(context_) });
        }
        return llvm::PointerType::getUnqual(context_);
    }
    if (auto* ref = dynamic_cast<const ReferenceType*>(type)) {
        if (dynamic_cast<const TraitObjectType*>(ref->pointee)) {
            return llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::PointerType::getUnqual(context_) });
        }
        return llvm::PointerType::getUnqual(context_);
    }
    if (auto* st = dynamic_cast<const StructType*>(type)) {
        const auto& sym = symTable_.getSymbol(st->structSymbolId);
        std::string name = std::string(sym.name.str());
        if (structTypes_.count(name)) return structTypes_[name];
        
        // If not found in structTypes_, we can try to look it up or create opaque
        // For now, return an opaque struct if it wasn't pre-declared
        return llvm::StructType::create(context_, name);
    }
    if (auto* tup = dynamic_cast<const TupleType*>(type)) {
        std::vector<llvm::Type*> elements;
        for (auto* elemTy : tup->elements) {
            elements.push_back(mapType(elemTy));
        }
        return llvm::StructType::get(context_, elements, false);
    }
    if (auto* et = dynamic_cast<const EnumType*>(type)) {
        // Simple enum for now: { i32 tag, [4 x i64] payload }
        // This allows storing by value without malloc for MVP.
        return llvm::StructType::get(context_, { llvm::Type::getInt32Ty(context_), llvm::ArrayType::get(llvm::Type::getInt64Ty(context_), 4) }, false);
    }
    if (auto* arr = dynamic_cast<const ArrayType*>(type)) {
        return llvm::ArrayType::get(mapType(arr->elementType), arr->length);
    }
    if (auto* closTy = dynamic_cast<const ClosureType*>(type)) {
        std::vector<llvm::Type*> elements;
        for (auto* fTy : closTy->fieldTypes) {
            elements.push_back(mapType(fTy));
        }
        return llvm::StructType::get(context_, elements, false);
    }
    if (auto* futTy = dynamic_cast<const FutureType*>(type)) {
        return llvm::PointerType::getUnqual(context_); // Future is just a pointer to the Coroutine Frame (Handle)
    }
    return llvm::Type::getVoidTy(context_);
}

llvm::Value* LLVMIRGenerator::mapOperand(const mvir::Operand& op) {
    if (std::holds_alternative<mvir::LocalId>(op)) {
        const auto& local = std::get<mvir::LocalId>(op);
        auto it = localValues_.find(local.name);
        if (it != localValues_.end()) return it->second;
        std::cerr << "LocalId not found in environment: " << local.name << std::endl;
        return nullptr;
    } else if (std::holds_alternative<mvir::GlobalId>(op)) {
        const auto& global = std::get<mvir::GlobalId>(op);
        auto it = globalValues_.find(global.name);
        if (it != globalValues_.end()) return it->second;
        
        std::string name = global.name.substr(1);
        llvm::Function* f = module_.getFunction(name);
        if (f) return f;
        
        std::cerr << "Global function/value not found: " << name << std::endl;
        return nullptr;
    } else if (std::holds_alternative<mvir::Number>(op)) {
        const auto& num = std::get<mvir::Number>(op);
        return llvm::ConstantInt::get(llvm::Type::getInt32Ty(context_), std::stoull(num.value, nullptr, 10), true);
    } else if (std::holds_alternative<mvir::Boolean>(op)) {
        const auto& b = std::get<mvir::Boolean>(op);
        return llvm::ConstantInt::get(llvm::Type::getInt1Ty(context_), b.value ? 1 : 0);
    }
    std::cerr << "Unknown operand type" << std::endl;
    return nullptr;
}



void LLVMIRGenerator::createFunctionStructure(const mvir::Function* func) {
    localValues_.clear();
    blocks_.clear();
    pointerTypes_.clear();
    
    llvm::Function* llvmFunc = llvm::cast<llvm::Function>(globalValues_[func->name.name]);
    
    size_t idx = 0;
    for (auto& arg : llvmFunc->args()) {
        localValues_[func->params[idx].id.name] = &arg;
        arg.setName(func->params[idx].id.name.substr(1));
        idx++;
    }

    for (const auto& block : func->blocks) {
        llvm::BasicBlock* bb = llvm::BasicBlock::Create(context_, block->label.name, llvmFunc);
        blocks_[block->label.name] = bb;
    }
}

void LLVMIRGenerator::emitFunctionBody(const mvir::Function* func) {
    std::cout << "[LLVMGen] Generating body for: " << func->name.name << std::endl;
    auto it = globalValues_.find(func->name.name);
    if (it == globalValues_.end()) {
        std::cerr << "[LLVMGen] Function not found in globalValues: " << func->name.name << std::endl;
        return;
    }
    llvm::Function* llvmFunc = llvm::cast<llvm::Function>(it->second);
    if (func->isAsync) {
        llvmFunc->setPresplitCoroutine();
    }
    bool isEntry = true;
    for (const auto& block : func->blocks) {
        llvm::BasicBlock* bb = blocks_[block->label.name];
        builder_.SetInsertPoint(bb);
        
        if (isEntry && func->isAsync) {
            llvm::Function* coroIdFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_id);
            llvm::Value* nullPtr = llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_));
            llvm::Value* coroId = builder_.CreateCall(coroIdFn, {
                llvm::ConstantInt::get(llvm::Type::getInt32Ty(context_), 0),
                nullPtr, nullPtr, nullPtr
            });

            llvm::Function* coroSizeFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_size, {llvm::Type::getInt32Ty(context_)});
            llvm::Value* coroSize = builder_.CreateCall(coroSizeFn);

            llvm::FunctionCallee allocFn = module_.getOrInsertFunction("malloc", llvm::PointerType::getUnqual(context_), llvm::Type::getInt32Ty(context_));
            llvm::Value* alloc = builder_.CreateCall(allocFn, {coroSize});

            llvm::Function* coroBeginFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_begin);
            currentCoroHdl_ = builder_.CreateCall(coroBeginFn, {coroId, alloc});
            
            // Promise Initialization
            llvm::Type* promiseInnerTy = llvm::Type::getVoidTy(context_);
            if (auto* futTy = dynamic_cast<const FutureType*>(func->returnType)) {
                promiseInnerTy = mapType(futTy->innerType);
            }
            llvm::StructType* promiseTy = llvm::StructType::get(context_, {llvm::Type::getInt8Ty(context_), promiseInnerTy});
            
            llvm::Function* coroPromiseFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_promise);
            llvm::Value* promisePtr = builder_.CreateCall(coroPromiseFn, {currentCoroHdl_, builder_.getInt32(8), builder_.getInt1(false)});
            
            // Set state = 0 (Pending)
            llvm::Value* statePtr = builder_.CreateStructGEP(promiseTy, promisePtr, 0);
            builder_.CreateStore(builder_.getInt8(0), statePtr);
        }
        isEntry = false;
        
        for (const auto& inst : block->instructions) {
            if (auto* awt = dynamic_cast<const mvir::AwaitInst*>(inst.get())) {
                llvm::Value* futHdl = mapOperand(awt->futureVal);
                
                llvm::Type* promiseInnerTy = mapType(awt->innerType);
                llvm::StructType* promiseTy = llvm::StructType::get(context_, {llvm::Type::getInt8Ty(context_), promiseInnerTy});
                
                llvm::Function* coroPromiseFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_promise);
                llvm::Value* promisePtr = builder_.CreateCall(coroPromiseFn, {futHdl, builder_.getInt32(8), builder_.getInt1(false)});
                
                llvm::BasicBlock* checkBB = llvm::BasicBlock::Create(context_, "await.check", builder_.GetInsertBlock()->getParent());
                llvm::BasicBlock* resumeInnerBB = llvm::BasicBlock::Create(context_, "await.resume_inner", builder_.GetInsertBlock()->getParent());
                llvm::BasicBlock* suspendBB = llvm::BasicBlock::Create(context_, "await.suspend", builder_.GetInsertBlock()->getParent());
                llvm::BasicBlock* readyBB = llvm::BasicBlock::Create(context_, "await.ready", builder_.GetInsertBlock()->getParent());
                
                builder_.CreateBr(checkBB);
                builder_.SetInsertPoint(checkBB);
                
                llvm::Value* statePtr = builder_.CreateStructGEP(promiseTy, promisePtr, 0);
                llvm::Value* stateVal = builder_.CreateLoad(llvm::Type::getInt8Ty(context_), statePtr);
                llvm::Value* isDone = builder_.CreateICmpEQ(stateVal, builder_.getInt8(1));
                builder_.CreateCondBr(isDone, readyBB, resumeInnerBB);
                
                // Resume inner future
                builder_.SetInsertPoint(resumeInnerBB);
                llvm::Function* coroResumeFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_resume);
                builder_.CreateCall(coroResumeFn, {futHdl});
                
                // After resuming, check state again
                llvm::Value* stateVal2 = builder_.CreateLoad(llvm::Type::getInt8Ty(context_), statePtr);
                llvm::Value* isDone2 = builder_.CreateICmpEQ(stateVal2, builder_.getInt8(1));
                builder_.CreateCondBr(isDone2, readyBB, suspendBB);
                
                // Suspend outer future
                builder_.SetInsertPoint(suspendBB);
                
                // Suspend the current coroutine
                llvm::Function* coroSaveFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_save);
                llvm::Value* saveRes = builder_.CreateCall(coroSaveFn, {currentCoroHdl_});
                llvm::Function* coroSuspendFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_suspend);
                llvm::Value* suspendRes = builder_.CreateCall(coroSuspendFn, {saveRes, builder_.getInt1(false)});
                
                // Switch on suspend result
                llvm::BasicBlock* cleanupBB = llvm::BasicBlock::Create(context_, "await.cleanup", builder_.GetInsertBlock()->getParent());
                llvm::SwitchInst* switchInst = builder_.CreateSwitch(suspendRes, suspendBB, 2);
                switchInst->addCase(builder_.getInt8(0), checkBB); // Resumed, check again
                switchInst->addCase(builder_.getInt8(1), cleanupBB); // Destroyed
                
                builder_.SetInsertPoint(cleanupBB);
                builder_.CreateUnreachable(); // We don't implement full cleanup logic yet
                
                // Ready state
                builder_.SetInsertPoint(readyBB);
                llvm::Value* resultPtr = builder_.CreateStructGEP(promiseTy, promisePtr, 1);
                llvm::Value* res = builder_.CreateLoad(promiseInnerTy, resultPtr);
                localValues_[awt->dest.name] = res;
                continue;
            }
            emitInstruction(inst.get());
        }
        
        if (block->terminator) {
            emitTerminator(block->terminator.get());
        } else {
            builder_.CreateRetVoid();
        }
    }
    currentCoroHdl_ = nullptr;
}

void LLVMIRGenerator::emitInstruction(const mvir::Instruction* inst) {
    if (auto* drop = dynamic_cast<const mvir::DropInst*>(inst)) {
        // Generate call to Type_drop
        std::string typeName = "unknown";
        if (auto* st = dynamic_cast<const StructType*>(drop->type)) {
            typeName = "Struct" + std::to_string(st->structSymbolId);
        } else if (auto* et = dynamic_cast<const EnumType*>(drop->type)) {
            typeName = "Enum" + std::to_string(et->enumSymbolId);
        }
        std::string dropFnName = typeName + "_drop";
        
        llvm::Function* dropFn = module_.getFunction(dropFnName);
        if (!dropFn) {
            llvm::Type* voidTy = llvm::Type::getVoidTy(context_);
            llvm::Type* ptrTy = llvm::PointerType::getUnqual(context_);
            llvm::FunctionType* fnTy = llvm::FunctionType::get(voidTy, {ptrTy}, false);
            dropFn = llvm::Function::Create(fnTy, llvm::Function::ExternalLinkage, dropFnName, &module_);
        }
        
        llvm::Value* val = mapOperand(drop->value);
        builder_.CreateCall(dropFn, {val});
    }
    else if (auto* local = dynamic_cast<const mvir::LocalInst*>(inst)) {
        llvm::Type* ty = mapType(local->type);
        llvm::Value* val = builder_.CreateAlloca(ty, nullptr, local->dest.name.substr(1));
        localValues_[local->dest.name] = val;
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(inst)) {
        llvm::Value* ptr = mapOperand(load->ptr);
        llvm::Type* pointeeTy = mapType(load->type);
        llvm::Value* val = builder_.CreateLoad(pointeeTy, ptr, load->dest.name.substr(1));
        localValues_[load->dest.name] = val;
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(inst)) {
        llvm::Value* val = mapOperand(store->value);
        llvm::Value* ptr = mapOperand(store->ptr);
        builder_.CreateStore(val, ptr);
    }
    else if (auto* idx = dynamic_cast<const mvir::IndexInst*>(inst)) {
        llvm::Value* ptr = mapOperand(idx->base);
        llvm::Type* pointeeTy = mapType(idx->type);
        llvm::Value* indexVal = mapOperand(idx->index);
        llvm::Value* res = builder_.CreateGEP(pointeeTy, ptr, indexVal, idx->dest.name.substr(1));
        localValues_[idx->dest.name] = res;
    }
    else if (auto* field = dynamic_cast<const mvir::FieldInst*>(inst)) {
        llvm::Value* ptr = mapOperand(field->base);
        llvm::Type* pointeeTy = mapType(field->type);
        llvm::Value* res = builder_.CreateStructGEP(pointeeTy, ptr, field->index, field->dest.name.substr(1));
        localValues_[field->dest.name] = res;
    }
    else if (auto* castinst = dynamic_cast<const mvir::CastInst*>(inst)) {
        llvm::Value* val = mapOperand(castinst->value);
        llvm::Type* destTy = mapType(castinst->targetType);
        llvm::Value* res = val;
        if (val->getType()->isIntegerTy() && destTy->isIntegerTy()) {
            res = builder_.CreateIntCast(val, destTy, true, castinst->dest.name.substr(1));
        } else if (val->getType()->isPointerTy() && destTy->isIntegerTy()) {
            res = builder_.CreatePtrToInt(val, destTy, castinst->dest.name.substr(1));
        } else if (val->getType()->isIntegerTy() && destTy->isPointerTy()) {
            res = builder_.CreateIntToPtr(val, destTy, castinst->dest.name.substr(1));
        } else {
            res = builder_.CreateBitCast(val, destTy, castinst->dest.name.substr(1));
        }
        localValues_[castinst->dest.name] = res;
    }
    else if (auto* sizeofinst = dynamic_cast<const mvir::SizeofInst*>(inst)) {
        llvm::Type* ty = mapType(sizeofinst->targetType);
        llvm::Value* res = llvm::ConstantExpr::getSizeOf(ty);
        localValues_[sizeofinst->dest.name] = res;
    }
    else if (auto* alignofinst = dynamic_cast<const mvir::AlignofInst*>(inst)) {
        llvm::Type* ty = mapType(alignofinst->targetType);
        llvm::Value* res = llvm::ConstantExpr::getAlignOf(ty);
        localValues_[alignofinst->dest.name] = res;
    }
    else if (auto* borrow = dynamic_cast<const mvir::BorrowInst*>(inst)) {
        llvm::Value* baseVal = mapOperand(borrow->base);
        localValues_[borrow->dest.name] = baseVal;
        
        if (std::holds_alternative<mvir::LocalId>(borrow->base)) {
            std::string baseName = std::get<mvir::LocalId>(borrow->base).name;
            if (pointerTypes_.count(baseName)) {
                pointerTypes_[borrow->dest.name] = pointerTypes_[baseName];
            }
        }
    }
    else if (auto* alu = dynamic_cast<const mvir::AluInst*>(inst)) {
        llvm::Value* left = mapOperand(alu->left);
        llvm::Value* right = mapOperand(alu->right);
        llvm::Value* res = nullptr;
        
        switch (alu->op) {
            case mvir::AluOp::Add: res = builder_.CreateAdd(left, right); break;
            case mvir::AluOp::Sub: res = builder_.CreateSub(left, right); break;
            case mvir::AluOp::Mul: res = builder_.CreateMul(left, right); break;
            case mvir::AluOp::Div: res = builder_.CreateSDiv(left, right); break;
            case mvir::AluOp::Eq:  res = builder_.CreateICmpEQ(left, right); break;
            case mvir::AluOp::Ne:  res = builder_.CreateICmpNE(left, right); break;
            case mvir::AluOp::Lt:  res = builder_.CreateICmpSLT(left, right); break;
            case mvir::AluOp::Le:  res = builder_.CreateICmpSLE(left, right); break;
            case mvir::AluOp::Gt:  res = builder_.CreateICmpSGT(left, right); break;
            case mvir::AluOp::Ge:  res = builder_.CreateICmpSGE(left, right); break;
        }
        res->setName(alu->dest.name.substr(1));
        localValues_[alu->dest.name] = res;
    }
    else if (auto* unary = dynamic_cast<const mvir::UnaryInst*>(inst)) {
        llvm::Value* val = mapOperand(unary->operand);
        llvm::Value* res = nullptr;
        switch (unary->op) {
            case mvir::UnaryOp::Negate: res = builder_.CreateNeg(val); break;
            case mvir::UnaryOp::BitNot: res = builder_.CreateNot(val); break;
        }
        res->setName(unary->dest.name.substr(1));
        localValues_[unary->dest.name] = res;
    }
    else if (auto* mk = dynamic_cast<const mvir::MakeTraitObjectInst*>(inst)) {
        llvm::Value* val = mapOperand(mk->value);
        llvm::StructType* fatPtrTy = llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::PointerType::getUnqual(context_) });
        
        llvm::ArrayType* vtableTy = llvm::ArrayType::get(llvm::PointerType::getUnqual(context_), mk->vtableMethods.size());
        
        std::vector<llvm::Constant*> methodPtrs;
        for (auto methodId : mk->vtableMethods) {
            if (methodId == kInvalidSymbolID) {
                methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_)));
                continue;
            }
            std::string methodName = symTable_.getSymbol(methodId).name.str();
            llvm::Function* func = module_.getFunction(methodName);
            if (func) {
                methodPtrs.push_back(func);
            } else {
                methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_)));
            }
        }
        llvm::Constant* vtableInit = llvm::ConstantArray::get(vtableTy, methodPtrs);
        
        std::string vtableName = "vtable." + mk->concreteType->toString() + "." + mk->targetType->toString();
        std::replace(vtableName.begin(), vtableName.end(), ' ', '_');
        std::replace(vtableName.begin(), vtableName.end(), '&', 'r');
        std::replace(vtableName.begin(), vtableName.end(), '*', 'p');
        
        llvm::GlobalVariable* vtableGlobal = module_.getNamedGlobal(vtableName);
        if (!vtableGlobal) {
            vtableGlobal = new llvm::GlobalVariable(module_, vtableTy, true, llvm::GlobalValue::PrivateLinkage, vtableInit, vtableName);
        }
        
        llvm::Value* fatPtr = llvm::UndefValue::get(fatPtrTy);
        fatPtr = builder_.CreateInsertValue(fatPtr, val, {0});
        fatPtr = builder_.CreateInsertValue(fatPtr, vtableGlobal, {1});
        
        localValues_[mk->dest.name] = fatPtr;
    }
    else if (auto* call = dynamic_cast<const mvir::CallInst*>(inst)) {
        llvm::FunctionCallee fcallee;
        if (std::holds_alternative<mvir::GlobalId>(call->func)) {
            fcallee = module_.getFunction(std::get<mvir::GlobalId>(call->func).name.substr(1));
            if (!fcallee) {
                std::cerr << "Function not found: " << std::get<mvir::GlobalId>(call->func).name << std::endl;
            }
        } else {
            llvm::Value* calleeVal = mapOperand(call->func);
            if (!call->funcType) {
                std::cerr << "Indirect call MUST have funcType" << std::endl;
            }
            std::vector<llvm::Type*> paramTys;
            for (auto* p : call->funcType->paramTypes) paramTys.push_back(mapType(p));
            if (call->args.size() > call->funcType->paramTypes.size()) {
                paramTys.insert(paramTys.begin(), llvm::PointerType::getUnqual(context_));
            }
            llvm::FunctionType* llvmFuncTy = llvm::FunctionType::get(mapType(call->funcType->returnType), paramTys, false);
            fcallee = llvm::FunctionCallee(llvmFuncTy, calleeVal);
        }
        
        std::vector<llvm::Value*> args;
        for (const auto& arg : call->args) {
            args.push_back(mapOperand(arg));
        }
        
        llvm::Value* res = builder_.CreateCall(fcallee, args);
        if (call->dest && !res->getType()->isVoidTy()) {
            res->setName(call->dest->name.substr(1));
            localValues_[call->dest->name] = res;
        }
    }
    else if (auto* vcall = dynamic_cast<const mvir::VirtualCallInst*>(inst)) {
        llvm::Value* fatPtr = mapOperand(vcall->receiver);
        
        llvm::Value* dataPtr = builder_.CreateExtractValue(fatPtr, {0});
        llvm::Value* vtablePtr = builder_.CreateExtractValue(fatPtr, {1});
        
        llvm::Type* fnPtrTy = llvm::PointerType::getUnqual(context_);
        llvm::Value* methodIndexVal = builder_.getInt32(vcall->methodIndex);
        llvm::Value* methodPtrPtr = builder_.CreateGEP(fnPtrTy, vtablePtr, methodIndexVal);
        llvm::Value* methodPtr = builder_.CreateLoad(fnPtrTy, methodPtrPtr);
        
        std::vector<llvm::Value*> args;
        args.push_back(dataPtr);
        for (size_t i = 1; i < vcall->args.size(); ++i) {
            args.push_back(mapOperand(vcall->args[i]));
        }
        
        llvm::FunctionType* fnTy = nullptr;
        if (vcall->methodType) {
            std::vector<llvm::Type*> paramTys;
            paramTys.push_back(llvm::PointerType::getUnqual(context_)); 
            for (size_t i = 1; i < vcall->methodType->paramTypes.size(); ++i) {
                paramTys.push_back(mapType(vcall->methodType->paramTypes[i]));
            }
            llvm::Type* retTy = mapType(vcall->methodType->returnType);
            fnTy = llvm::FunctionType::get(retTy, paramTys, false);
        }
        
        if (!fnTy) {
            std::cerr << "Failed to build function type for virtual call" << std::endl;
        }
        llvm::Value* res = builder_.CreateCall(fnTy, methodPtr, args);
        if (vcall->dest && !res->getType()->isVoidTy()) {
            res->setName(vcall->dest->name.substr(1));
            localValues_[vcall->dest->name] = res;
        }
    }
    else if (auto* variant = dynamic_cast<const mvir::VariantInst*>(inst)) {
        llvm::Type* enumLLVMTy = mapType(variant->enumType);
        llvm::Value* destAlloc = builder_.CreateAlloca(enumLLVMTy);
        
        llvm::Value* tagPtr = builder_.CreateStructGEP(enumLLVMTy, destAlloc, 0);
        builder_.CreateStore(builder_.getInt32(variant->variantIndex), tagPtr);
        
        if (!variant->args.empty()) {
            std::vector<llvm::Type*> fieldTypes;
            for (const auto& arg : variant->args) {
                fieldTypes.push_back(mapOperand(arg)->getType());
            }
            llvm::StructType* payloadTy = llvm::StructType::get(context_, fieldTypes, false);
            
            llvm::Value* payloadArrPtr = builder_.CreateStructGEP(enumLLVMTy, destAlloc, 1);
            llvm::Value* payloadPtr = builder_.CreateBitCast(payloadArrPtr, llvm::PointerType::getUnqual(context_));
            
            for (size_t i = 0; i < variant->args.size(); ++i) {
                llvm::Value* argVal = mapOperand(variant->args[i]);
                llvm::Value* fieldPtr = builder_.CreateStructGEP(payloadTy, payloadPtr, i);
                builder_.CreateStore(argVal, fieldPtr);
            }
        }
        
        llvm::Value* res = builder_.CreateLoad(enumLLVMTy, destAlloc, variant->dest.name.substr(1));
        localValues_[variant->dest.name] = res;
    }
    else if (auto* tagInst = dynamic_cast<const mvir::TagInst*>(inst)) {
        llvm::Value* baseVal = mapOperand(tagInst->base);
        llvm::Value* res = builder_.CreateExtractValue(baseVal, 0, tagInst->dest.name.substr(1));
        localValues_[tagInst->dest.name] = res;
    }
      else if (auto* extractInst = dynamic_cast<const mvir::ExtractInst*>(inst)) {
          llvm::Value* baseVal = mapOperand(extractInst->base);
          llvm::Type* enumLLVMTy = baseVal->getType();
          
          llvm::Value* tempAlloc = builder_.CreateAlloca(enumLLVMTy);
          builder_.CreateStore(baseVal, tempAlloc);
          
          std::vector<llvm::Type*> fieldTypes;
          for (const auto* pType : extractInst->payloadTypes) {
              fieldTypes.push_back(mapType(pType));
          }
          llvm::StructType* payloadTy = llvm::StructType::get(context_, fieldTypes, false);
          
          llvm::Value* payloadArrPtr = builder_.CreateStructGEP(enumLLVMTy, tempAlloc, 1);
          llvm::Value* payloadPtr = builder_.CreateBitCast(payloadArrPtr, llvm::PointerType::getUnqual(context_));
          
          llvm::Value* fieldPtr = builder_.CreateStructGEP(payloadTy, payloadPtr, extractInst->fieldIndex);
          llvm::Value* res = builder_.CreateLoad(fieldTypes[extractInst->fieldIndex], fieldPtr, extractInst->dest.name.substr(1));
          localValues_[extractInst->dest.name] = res;
      }
}

void LLVMIRGenerator::emitTerminator(const mvir::Terminator* term) {
    if (auto* jump = dynamic_cast<const mvir::JumpTerm*>(term)) {
        llvm::BasicBlock* target = blocks_[jump->target.name];
        builder_.CreateBr(target);
    }
    else if (auto* branch = dynamic_cast<const mvir::BranchTerm*>(term)) {
        llvm::Value* cond = mapOperand(branch->condition);
        llvm::BasicBlock* trueBB = blocks_[branch->trueTarget.name];
        llvm::BasicBlock* falseBB = blocks_[branch->falseTarget.name];
        builder_.CreateCondBr(cond, trueBB, falseBB);
    }
    else if (auto* sw = dynamic_cast<const mvir::SwitchTerm*>(term)) {
        llvm::Value* cond = mapOperand(sw->condition);
        llvm::BasicBlock* defaultBB = blocks_[sw->defaultTarget.name];
        llvm::SwitchInst* switchInst = builder_.CreateSwitch(cond, defaultBB, sw->cases.size());
        for (const auto& c : sw->cases) {
            llvm::ConstantInt* caseVal = llvm::ConstantInt::get(llvm::Type::getInt32Ty(context_), std::stoull(c.first.value, nullptr, 10), true);
            llvm::BasicBlock* caseBB = blocks_[c.second.name];
            switchInst->addCase(caseVal, caseBB);
        }
    }
    else if (auto* ret = dynamic_cast<const mvir::RetTerm*>(term)) {
        if (ret->value) {
            llvm::Value* val = mapOperand(*(ret->value));
            if (currentCoroHdl_) {
                llvm::Function* coroPromiseFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_promise);
                llvm::Value* promisePtr = builder_.CreateCall(coroPromiseFn, {currentCoroHdl_, builder_.getInt32(8), builder_.getInt1(false)});
                
                llvm::Type* promiseInnerTy = val->getType();
                llvm::StructType* promiseTy = llvm::StructType::get(context_, {llvm::Type::getInt8Ty(context_), promiseInnerTy});
                
                // Write state = 1 (Done)
                llvm::Value* statePtr = builder_.CreateStructGEP(promiseTy, promisePtr, 0);
                builder_.CreateStore(builder_.getInt8(1), statePtr);
                
                // Write Result
                llvm::Value* resPtr = builder_.CreateStructGEP(promiseTy, promisePtr, 1);
                builder_.CreateStore(val, resPtr);
                
                llvm::Function* coroEndFn = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_end);
                builder_.CreateCall(coroEndFn, {currentCoroHdl_, builder_.getInt1(0), llvm::ConstantTokenNone::get(context_)});
                
                builder_.CreateRet(currentCoroHdl_);
            } else {
                builder_.CreateRet(val);
            }
        } else {
            builder_.CreateRetVoid();
        }
    }
}

}

