#include "mellis/BackEnd/LLVMIRGenerator.h"
#include <llvm/IR/Intrinsics.h>
#include <llvm/IR/Constants.h>
#include <llvm/IR/Verifier.h>
#include <llvm/Support/raw_ostream.h>
#include <iostream>
#include <cassert>

namespace fl {

LLVMIRGenerator::LLVMIRGenerator(llvm::LLVMContext& context, llvm::Module& module, TraitObjectLayoutBuilder& layoutBuilder,
                                 std::unordered_map<const Type*, ClosureStorageKind>& closureStorageMap)
    : context_(context), module_(module), builder_(context), layoutBuilder_(layoutBuilder), closureStorageMap_(closureStorageMap) {}

bool LLVMIRGenerator::generate(const mvir::Module* mvirModule) {
    mvirModule_ = mvirModule;
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
        if (tDecl.isEnum) {
            uint64_t maxPayloadSize = 0;
            unsigned maxPayloadAlign = 1;
            for (const auto& variant : tDecl.variants) {
                std::vector<llvm::Type*> fieldTypes;
                for (auto* fTy : variant) fieldTypes.push_back(mapType(fTy));
                if (fieldTypes.empty()) continue;
                llvm::StructType* tempStruct = llvm::StructType::get(context_, fieldTypes, false);
                uint64_t size = module_.getDataLayout().getTypeAllocSize(tempStruct);
                unsigned align = module_.getDataLayout().getABITypeAlign(tempStruct).value();
                if (size > maxPayloadSize) maxPayloadSize = size;
                if (align > maxPayloadAlign) maxPayloadAlign = align;
            }
            llvm::Type* payloadType = nullptr;
            if (maxPayloadSize == 0) {
                payloadType = llvm::StructType::get(context_); // empty struct
            } else {
                unsigned alignBits = maxPayloadAlign * 8;
                if (alignBits > 64) alignBits = 64; // Fallback for very large alignments
                llvm::Type* alignType = llvm::Type::getIntNTy(context_, alignBits);
                uint64_t numElements = (maxPayloadSize + maxPayloadAlign - 1) / maxPayloadAlign;
                payloadType = llvm::ArrayType::get(alignType, numElements);
            }
            st->setBody({llvm::Type::getInt32Ty(context_), payloadType});
        } else {
            std::vector<llvm::Type*> fields;
            for (const auto& fType : tDecl.fields) {
                fields.push_back(mapType(fType));
            }
            st->setBody(fields);
        }
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
        if (func->isGeneric) continue;

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

    // Pass 2: Define functions
    for (const auto& func : mvirModule->functions) {
        if (func->isGeneric) continue;
        
        // Create BasicBlocks first to allow forward references.
        createFunctionStructure(func.get());
        emitFunctionBody(func.get());
    }

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
    llvm::Function* doneFn = llvm::Function::Create(doneFnTy, llvm::Function::LinkOnceAnyLinkage, "mellis_coro_is_done", module_);
    llvm::BasicBlock* doneBB = llvm::BasicBlock::Create(context_, "entry", doneFn);
    builder_.SetInsertPoint(doneBB);
    llvm::Function* coroDoneIntrin = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_done);
    llvm::Value* isDone = builder_.CreateCall(coroDoneIntrin, {doneFn->getArg(0)});
    builder_.CreateRet(isDone);

    llvm::FunctionType* resumeFnTy = llvm::FunctionType::get(llvm::Type::getVoidTy(context_), {llvm::PointerType::getUnqual(context_)}, false);
    llvm::Function* resumeFn = llvm::Function::Create(resumeFnTy, llvm::Function::LinkOnceAnyLinkage, "mellis_coro_resume", module_);
    llvm::BasicBlock* resumeBB = llvm::BasicBlock::Create(context_, "entry", resumeFn);
    builder_.SetInsertPoint(resumeBB);
    llvm::Function* coroResumeIntrin = llvm::Intrinsic::getDeclaration(&module_, llvm::Intrinsic::coro_resume);
    builder_.CreateCall(coroResumeIntrin, {resumeFn->getArg(0)});
    builder_.CreateRetVoid();

    // std::cout << "[LLVMGen] Coroutines built." << std::endl;
    std::error_code EC;
    llvm::raw_fd_ostream os_file("llvm.ir", EC);
    if (!EC) {
        module_.print(os_file, nullptr);
        os_file.flush();
    }
    std::string errStr;
    llvm::raw_string_ostream os(errStr);
    if (llvm::verifyModule(module_, &os)) {
        os.flush();
        std::cerr << "[LLVM Verify Error]\n" << errStr << "\n";
        return false;
    }
    return true;
}

llvm::Type* LLVMIRGenerator::mapType(const Type* type) {
    if (!type) return llvm::Type::getVoidTy(context_);
    
    if (auto* prim = dynamic_cast<const PrimitiveType*>(type)) {
        switch (prim->builtinKind) {
            case BuiltinKind::I4:
            case BuiltinKind::U4:
                return llvm::Type::getIntNTy(context_, 4);
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
            case BuiltinKind::I128:
            case BuiltinKind::U128:
                return llvm::Type::getInt128Ty(context_);
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
        } else if (dynamic_cast<const SliceType*>(ptr->pointee)) {
            return llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::Type::getInt64Ty(context_) });
        }
        return llvm::PointerType::getUnqual(context_);
    }
    if (auto* ref = dynamic_cast<const ReferenceType*>(type)) {
        if (dynamic_cast<const TraitObjectType*>(ref->pointee)) {
            return llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::PointerType::getUnqual(context_) });
        } else if (dynamic_cast<const SliceType*>(ref->pointee)) {
            return llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::Type::getInt64Ty(context_) });
        }
        return llvm::PointerType::getUnqual(context_);
    }
    if (auto* st = dynamic_cast<const StructType*>(type)) {
        std::string name = "";
        if (mvirModule_) {
            for (const auto& tDecl : mvirModule_->typeDecls) {
                if (tDecl.id == st->structSymbolId) {
                    name = tDecl.name.substr(1);
                    break;
                }
            }
        }
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
        if (mvirModule_) {
            for (const auto& tDecl : mvirModule_->typeDecls) {
                if (tDecl.id == et->enumSymbolId && tDecl.isEnum) {
                    std::string name = tDecl.name.substr(1);
                    if (structTypes_.count(name)) return structTypes_[name];
                    
                    uint64_t maxPayloadSize = 0;
                    unsigned maxPayloadAlign = 1;
                    
                    for (const auto& variant : tDecl.variants) {
                        std::vector<llvm::Type*> fieldTypes;
                        for (auto* fTy : variant) {
                            fieldTypes.push_back(mapType(fTy));
                        }
                        if (fieldTypes.empty()) continue;
                        llvm::StructType* tempStruct = llvm::StructType::get(context_, fieldTypes, false);
                        uint64_t size = module_.getDataLayout().getTypeAllocSize(tempStruct);
                        unsigned align = module_.getDataLayout().getABITypeAlign(tempStruct).value();
                        if (size > maxPayloadSize) maxPayloadSize = size;
                        if (align > maxPayloadAlign) maxPayloadAlign = align;
                    }
                    
                    llvm::Type* payloadType = nullptr;
                    if (maxPayloadSize == 0) {
                        payloadType = llvm::StructType::get(context_); // empty struct
                    } else {
                        unsigned alignBits = maxPayloadAlign * 8;
                        if (alignBits > 64) alignBits = 64; // Fallback for very large alignments
                        llvm::Type* alignType = llvm::Type::getIntNTy(context_, alignBits);
                        uint64_t numElements = (maxPayloadSize + maxPayloadAlign - 1) / maxPayloadAlign;
                        payloadType = llvm::ArrayType::get(alignType, numElements);
                    }
                    
                    llvm::StructType* enumStruct = llvm::StructType::create(context_, name);
                    enumStruct->setBody({llvm::Type::getInt32Ty(context_), payloadType}, false);
                    structTypes_[name] = enumStruct;
                    return enumStruct;
                }
            }
        }
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

llvm::Value* LLVMIRGenerator::mapOperand(const mvir::Operand& op, llvm::Type* expectedType) {
    if (const auto* place = mvir::getPlaceIf(op)) {
        llvm::Value* baseVal = nullptr;
        
        if (auto* loc = std::get_if<mvir::LocalId>(&place->base)) {
            auto it = localValues_.find(loc->name);
            if (it != localValues_.end()) baseVal = it->second;
            else {
                std::cerr << "LocalId not found in environment: " << loc->name << std::endl;
                return nullptr;
            }
        } else if (auto* glob = std::get_if<mvir::GlobalId>(&place->base)) {
            auto it = globalValues_.find(glob->name);
            if (it != globalValues_.end()) baseVal = it->second;
            else {
                std::string name = glob->name.substr(1);
                llvm::Function* f = module_.getFunction(name);
                if (f) baseVal = f;
                else {
                    std::cerr << "Global function/value not found: " << name << std::endl;
                    return nullptr;
                }
            }
        }

        if (!baseVal) return nullptr;
        
        llvm::Value* current = baseVal;
        llvm::Type* currentPointeeType = nullptr;
        // Seed the currentPointeeType from the base local's allocation type
        if (auto* loc = std::get_if<mvir::LocalId>(&place->base)) {
            auto it = pointerTypes_.find(loc->name);
            if (it != pointerTypes_.end()) currentPointeeType = it->second;
        }
        
        for (const auto& proj : place->projections) {
            if (proj.kind == mvir::ProjectionKind::Deref) {
                current = builder_.CreateLoad(llvm::PointerType::getUnqual(context_), current, "deref");
                // For FDLang, if currentPointeeType was a pointer, we don't know the pointee type.
                // But if currentPointeeType is already the StructType (set by custom pointerTypes_ tracking),
                // we preserve it so Field projection works!
                if (currentPointeeType && currentPointeeType->isPointerTy()) {
                    currentPointeeType = nullptr;
                }
            } else if (proj.kind == mvir::ProjectionKind::Field || proj.kind == mvir::ProjectionKind::TupleIndex) {
                // GEP into struct/tuple at field index.
                if (!currentPointeeType) {
                    std::cerr << "[LLVMIRGen] Field/Tuple projection: unknown aggregate type for base\n";
                    return nullptr;
                }
                const char* projName = (proj.kind == mvir::ProjectionKind::TupleIndex) ? "tidx" : "fld";
                current = builder_.CreateStructGEP(currentPointeeType, current,
                    static_cast<unsigned>(proj.fieldIndex), projName);
                // Advance currentPointeeType to the field's element type for chained projections
                if (auto* st = llvm::dyn_cast<llvm::StructType>(currentPointeeType)) {
                    if (proj.fieldIndex < st->getNumElements()) {
                        currentPointeeType = st->getElementType(static_cast<unsigned>(proj.fieldIndex));
                    } else {
                        currentPointeeType = nullptr;
                    }
                } else {
                    currentPointeeType = nullptr;
                }
            } else if (proj.kind == mvir::ProjectionKind::Index) {
                // GEP into array at dynamic index stored in indexLocal.
                if (!currentPointeeType) {
                    std::cerr << "[LLVMIRGen] Index projection: unknown array type for base\n";
                    return nullptr;
                }
                llvm::Value* idxVal = nullptr;
                if (proj.indexLocal.has_value()) {
                    auto it = localValues_.find(proj.indexLocal->name);
                    if (it != localValues_.end()) {
                        llvm::Type* i32Ty = llvm::Type::getInt32Ty(context_);
                        idxVal = builder_.CreateLoad(i32Ty, it->second, "idx");
                    }
                }
                if (!idxVal) idxVal = llvm::ConstantInt::get(llvm::Type::getInt32Ty(context_), 0);
                llvm::Value* zero = llvm::ConstantInt::get(llvm::Type::getInt32Ty(context_), 0);
                current = builder_.CreateGEP(currentPointeeType, current, {zero, idxVal}, "elem");
                // Advance to element type
                if (auto* arrTy = llvm::dyn_cast<llvm::ArrayType>(currentPointeeType)) {
                    currentPointeeType = arrTy->getElementType();
                } else {
                    currentPointeeType = nullptr;
                }
            }
        }
        
        return current;

    } else if (std::holds_alternative<mvir::Number>(op)) {
        const auto& num = std::get<mvir::Number>(op);
        if (expectedType && expectedType->isFloatTy()) {
            return llvm::ConstantFP::get(expectedType, llvm::StringRef(num.value));
        } else if (expectedType && expectedType->isDoubleTy()) {
            return llvm::ConstantFP::get(expectedType, llvm::StringRef(num.value));
        } else if (expectedType && expectedType->isIntegerTy()) {
            return llvm::ConstantInt::get(expectedType, std::stoull(num.value, nullptr, 10), true);
        } else {
            if (num.value.find('.') != std::string::npos) {
                return llvm::ConstantFP::get(builder_.getDoubleTy(), llvm::StringRef(num.value));
            }
            return llvm::ConstantInt::get(llvm::Type::getInt32Ty(context_), std::stoull(num.value, nullptr, 10), true);
        }
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
        
        if (auto* refTy = dynamic_cast<const ReferenceType*>(func->params[idx].type)) {
            pointerTypes_[func->params[idx].id.name] = mapType(refTy->pointee);
        } else if (auto* pTy = dynamic_cast<const PointerType*>(func->params[idx].type)) {
            pointerTypes_[func->params[idx].id.name] = mapType(pTy->pointee);
        }
        
        arg.setName(func->params[idx].id.name.substr(1));
        idx++;
    }

    for (const auto& block : func->blocks) {
        llvm::BasicBlock* bb = llvm::BasicBlock::Create(context_, block->label.name, llvmFunc);
        blocks_[block->label.name] = bb;
    }
}

// Helper to strip references and pointers for GEP
static const Type* peel(const Type* t) {
    while (true) {
        if (auto* r = dynamic_cast<const ReferenceType*>(t)) {
            t = r->pointee;
            continue;
        }
        if (auto* p = dynamic_cast<const PointerType*>(t)) {
            t = p->pointee;
            continue;
        }
        return t;
    }
}

void LLVMIRGenerator::emitFunctionBody(const mvir::Function* func) {
    // std::cout << "[LLVMGen] Generating body for: " << func->name.name << std::endl;
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
            throw std::runtime_error("compilerBug: Basic block '" + block->label.toString() + "' has no terminator");
        }
    }
    currentCoroHdl_ = nullptr;
}

void LLVMIRGenerator::emitInstruction(const mvir::Instruction* inst) {
    if (!inst) return;
    std::cerr << "[DEBUG LLVMIRGenerator] emitting instruction type: " << typeid(*inst).name() << std::endl;

    if (auto* drop = dynamic_cast<const mvir::DropInst*>(inst)) {
        if (!drop->type) return;

        bool isHeapClosure = false;
        const ClosureType* closTy = nullptr;
        if (auto* ptrTy = dynamic_cast<const PointerType*>(drop->type)) {
            if ((closTy = dynamic_cast<const ClosureType*>(ptrTy->pointee))) {
                auto it = closureStorageMap_.find(closTy);
                if (it != closureStorageMap_.end() && it->second == ClosureStorageKind::Heap) {
                    isHeapClosure = true;
                }
            }
        }
        
        if (!drop->type->needsDrop() && !isHeapClosure) {
            return;
        }

        if (isHeapClosure) {
            llvm::Value* closurePtr = mapOperand(drop->value);
            llvm::Value* heapPtr = builder_.CreateLoad(builder_.getPtrTy(), closurePtr, "heap_clos.drop.load");

            llvm::Type* llvmClosTy = mapType(closTy);
            for (size_t i = 0; i < closTy->captures.size(); ++i) {
                if (closTy->captures[i].envType && closTy->captures[i].envType->needsDrop()) {
                    llvm::Value* fieldPtr = builder_.CreateStructGEP(llvmClosTy, heapPtr, i + 1);
                    
                    const Type* capTy = closTy->captures[i].envType;
                    std::string capTypeName = "unknown";
                    if (auto* st = dynamic_cast<const StructType*>(capTy)) {
                        capTypeName = "Struct" + std::to_string(st->structSymbolId);
                    } else if (auto* et = dynamic_cast<const EnumType*>(capTy)) {
                        capTypeName = "Enum" + std::to_string(et->enumSymbolId);
                    }
                    std::string capDropFnName = capTypeName + "_drop";
                    llvm::Function* capDropFn = module_.getFunction(capDropFnName);
                    if (!capDropFn) {
                        for (auto& f : module_) {
                            if (f.getName().starts_with("drop_") && f.arg_size() == 1) {
                                capDropFn = &f;
                                break;
                            }
                        }
                    }
                    if (!capDropFn) {
                        llvm::Type* voidTy = llvm::Type::getVoidTy(context_);
                        llvm::Type* ptrTy = llvm::PointerType::getUnqual(context_);
                        llvm::FunctionType* fnTy = llvm::FunctionType::get(voidTy, {ptrTy}, false);
                        capDropFn = llvm::Function::Create(fnTy, llvm::Function::ExternalLinkage, capDropFnName, &module_);
                    }
                    builder_.CreateCall(capDropFn, {fieldPtr});
                }
            }
            return;
        }

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
            for (auto& f : module_) {
                if (f.getName().starts_with("drop_") && f.arg_size() == 1) {
                    dropFn = &f;
                    break;
                }
            }
        }
        if (!dropFn) {
            llvm::Type* voidTy = llvm::Type::getVoidTy(context_);
            llvm::Type* ptrTy = llvm::PointerType::getUnqual(context_);
            llvm::FunctionType* fnTy = llvm::FunctionType::get(voidTy, {ptrTy}, false);
            dropFn = llvm::Function::Create(fnTy, llvm::Function::ExternalLinkage, dropFnName, &module_);
        }
        
        llvm::Value* val = mapOperand(drop->value);
        builder_.CreateCall(dropFn, {val});
    }    
    else if (auto* heapAlloc = dynamic_cast<const mvir::HeapAllocInst*>(inst)) {
        llvm::Type* ty = mapType(heapAlloc->type);
        if (!ty->isVoidTy()) {
            // Get size of type
            uint64_t size = module_.getDataLayout().getTypeAllocSize(ty);
            
            llvm::Function* mallocFn = module_.getFunction("malloc");
            if (!mallocFn) {
                llvm::FunctionType* mallocTy = llvm::FunctionType::get(builder_.getPtrTy(), {builder_.getInt64Ty()}, false);
                mallocFn = llvm::Function::Create(mallocTy, llvm::Function::ExternalLinkage, "malloc", &module_);
            }
            
            llvm::Value* sizeVal = llvm::ConstantInt::get(builder_.getInt64Ty(), size);
            llvm::Value* ptr = builder_.CreateCall(mallocFn, {sizeVal}, heapAlloc->dest.name.substr(1) + "_malloc");
            
            if (localValues_.count(heapAlloc->dest.name)) {
                builder_.CreateStore(ptr, localValues_[heapAlloc->dest.name]);
            } else {
                localValues_[heapAlloc->dest.name] = ptr;
                pointerTypes_[heapAlloc->dest.name] = ty;
            }
        }
    }
    else if (auto* heapFree = dynamic_cast<const mvir::HeapFreeInst*>(inst)) {
        llvm::Value* val = mapOperand(heapFree->ptr);
        llvm::Value* heapPtr = builder_.CreateLoad(builder_.getPtrTy(), val, "heap_free.load");
        llvm::Function* freeFn = module_.getFunction("free");
        if (!freeFn) {
            llvm::FunctionType* freeTy = llvm::FunctionType::get(llvm::Type::getVoidTy(context_), {builder_.getPtrTy()}, false);
            freeFn = llvm::Function::Create(freeTy, llvm::Function::ExternalLinkage, "free", &module_);
        }
        builder_.CreateCall(freeFn, {heapPtr});
    }
    else if (auto* local = dynamic_cast<const mvir::LocalInst*>(inst)) {
        llvm::Type* ty = mapType(local->type);
        if (!ty->isVoidTy()) {
            llvm::Value* val = builder_.CreateAlloca(ty, nullptr, local->dest.name.substr(1));
            localValues_[local->dest.name] = val;
            
            llvm::Type* pointeeTy = ty;
            if (auto* pTy = dynamic_cast<const PointerType*>(local->type)) {
                pointeeTy = mapType(pTy->pointee);
            } else if (auto* rTy = dynamic_cast<const ReferenceType*>(local->type)) {
                pointeeTy = mapType(rTy->pointee);
            }
            pointerTypes_[local->dest.name] = pointeeTy;
        }
    }
    else if (auto* load = dynamic_cast<const mvir::LoadInst*>(inst)) {
        llvm::Type* pointeeTy = mapType(load->type);
        if (!pointeeTy->isVoidTy()) {
            llvm::Value* ptr = mapOperand(load->ptr);
            if (!ptr) std::cerr << "[FATAL] mapOperand returned nullptr for LoadInst ptr!" << std::endl;
            llvm::Value* val = builder_.CreateLoad(pointeeTy, ptr, load->dest.name.substr(1));
            localValues_[load->dest.name] = val;
            
            if (auto* refTy = dynamic_cast<const ReferenceType*>(load->type)) {
                pointerTypes_[load->dest.name] = mapType(refTy->pointee);
            } else if (auto* pTy = dynamic_cast<const PointerType*>(load->type)) {
                pointerTypes_[load->dest.name] = mapType(pTy->pointee);
            }

            std::cerr << "[DEBUG] Added LoadInst dest to localValues: " << load->dest.name << " (type: " << val->getType()->getTypeID() << ")" << std::endl;
        } else {
            std::cerr << "[DEBUG] LoadInst pointeeTy is Void, NOT added: " << load->dest.name << std::endl;
        }
    }
    else if (auto* store = dynamic_cast<const mvir::StoreInst*>(inst)) {
        llvm::Type* expectedType = mapType(store->type);
        if (!expectedType->isVoidTy()) {
            llvm::Value* val = mapOperand(store->value, expectedType);
            llvm::Value* ptr = mapOperand(store->ptr);
            builder_.CreateStore(val, ptr);
        }
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
        
        if (auto* loc = mvir::getLocalIf(borrow->base)) {
            std::string baseName = loc->name;
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
        
        size_t totalSlots = 3 + mk->vtableMethods.size();
        llvm::ArrayType* vtableTy = llvm::ArrayType::get(llvm::PointerType::getUnqual(context_), totalSlots);
        
        std::vector<llvm::Constant*> methodPtrs;
        methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_))); // 0: Size
        methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_))); // 1: Align
        methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_))); // 2: Drop

        for (auto& methodName : mk->vtableMethods) {
            if (methodName.empty()) {
                methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_)));
                continue;
            }
            llvm::Function* func = module_.getFunction(methodName);
            if (func) {
                methodPtrs.push_back(func);
            } else {
                std::cerr << "[FATAL LLVMIR] Cannot find VTABLE method: '" << methodName << "'" << std::endl;
                methodPtrs.push_back(llvm::ConstantPointerNull::get(llvm::PointerType::getUnqual(context_)));
            }
        }
        llvm::Constant* vtableInit = llvm::ConstantArray::get(vtableTy, methodPtrs);
        
        std::string vtableName = mk->vtableMangledName;
        
        llvm::GlobalVariable* vtableGlobal = module_.getNamedGlobal(vtableName);
        if (!vtableGlobal) {
            vtableGlobal = new llvm::GlobalVariable(module_, vtableTy, true, llvm::GlobalValue::PrivateLinkage, vtableInit, vtableName);
        }
        
        llvm::Value* fatPtr = builder_.CreateInsertValue(llvm::UndefValue::get(fatPtrTy), val, 0);
        fatPtr = builder_.CreateInsertValue(fatPtr, vtableGlobal, 1);
        
        localValues_[mk->dest.name] = fatPtr;
    }
    else if (auto* ms = dynamic_cast<const mvir::MakeSliceInst*>(inst)) {
        llvm::Value* ptrVal = mapOperand(ms->basePtr);
        llvm::Value* lenVal = mapOperand(ms->length);

        // lenVal is i64 or i32 in MVIR, ensure it's i64 for the fat pointer struct
        if (lenVal->getType()->isIntegerTy(32)) {
            lenVal = builder_.CreateZExt(lenVal, llvm::Type::getInt64Ty(context_));
        }

        llvm::StructType* fatPtrTy = llvm::StructType::get(context_, { llvm::PointerType::getUnqual(context_), llvm::Type::getInt64Ty(context_) });
        llvm::Value* fatPtr = builder_.CreateInsertValue(llvm::UndefValue::get(fatPtrTy), ptrVal, 0);
        fatPtr = builder_.CreateInsertValue(fatPtr, lenVal, 1);

        localValues_[ms->dest.name] = fatPtr;
    }
    else if (auto* call = dynamic_cast<const mvir::CallInst*>(inst)) {
        llvm::FunctionCallee fcallee;
        if (mvir::getGlobalIf(call->func)) {
            fcallee = module_.getFunction((*mvir::getGlobalIf(call->func)).name.substr(1));
            if (!fcallee) {
                std::cerr << "Function not found: " << (*mvir::getGlobalIf(call->func)).name << std::endl;
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
        llvm::FunctionType* fnTy = fcallee.getFunctionType();
        std::vector<llvm::Value*> args;
        for (size_t i = 0; i < call->args.size(); ++i) {
            llvm::Type* expectedType = nullptr;
            if (fnTy && i < fnTy->getNumParams()) {
                expectedType = fnTy->getParamType(i);
            }
            llvm::Value* val = mapOperand(call->args[i], expectedType);
            // ABI Rule: float must be promoted to double when passed to variadic function (...)
            if (fnTy && fnTy->isVarArg() && i >= fnTy->getNumParams()) {
                if (val->getType()->isFloatTy()) {
                    val = builder_.CreateFPExt(val, builder_.getDoubleTy());
                }
            }
            args.push_back(val);
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
            llvm::Type* expectedType = nullptr;
            if (vcall->methodType && i < vcall->methodType->paramTypes.size()) {
                expectedType = mapType(vcall->methodType->paramTypes[i]);
            }
            args.push_back(mapOperand(vcall->args[i], expectedType));
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
    } else {
        std::cerr << "[DEBUG] Instruction NOT handled by any if-else block!" << std::endl;
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
    else if (dynamic_cast<const mvir::UnreachableTerm*>(term)) {
        builder_.CreateUnreachable();
    }
}

}

