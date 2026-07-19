// =============================================================================
// mellis/BackEnd/LLVMIRGenerator.h
//
// LLVMIRGenerator — Lowering pass from MVIR to LLVM IR.
//
// Responsibilities:
//   - Consume a mvir::Module and produce a well-formed llvm::Module.
//   - Pure translation: maps types, operands, and instructions 1:1.
//
// Constraints:
//   - No optimizations, no dead code elimination.
//   - Semantic analysis is forbidden (no access to AST, Resolver, TypeChecker).
//   - Output must pass llvm::verifyModule.
// =============================================================================

#pragma once

#include "mellis/MiddleEnd/MVIR.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/IRBuilder.h>
#include <llvm/IR/Verifier.h>
#include <string>
#include <unordered_map>
#include <memory>

namespace fl {

class LLVMIRGenerator {
public:
    explicit LLVMIRGenerator(llvm::LLVMContext& context, llvm::Module& module, const SymbolTable& symTable);

    /// Generate LLVM IR from the given MVIR module.
    ///
    /// @return true if generation succeeded and passed verification.
    bool generate(const mvir::Module* mvirModule);

private:
    llvm::LLVMContext& context_;
    llvm::Module& module_;
    llvm::IRBuilder<> builder_;
    const SymbolTable& symTable_;

    // ── Environments ─────────────────────────────────────────────────────────
    
    // Maps MVIR LocalId ("%0") to the allocated LLVM Value.
    std::unordered_map<std::string, llvm::Value*> globalValues_;
    std::unordered_map<std::string, llvm::Value*> localValues_;
    
    // Maps MVIR LocalId to its allocated LLVM Type (needed for LLVM 15+ opaque pointers)
    std::unordered_map<std::string, llvm::Type*> pointerTypes_;
    
    // Maps Struct name ("Foo") to the LLVM StructType.
    std::unordered_map<std::string, llvm::StructType*> structTypes_;
    
    // Maps MVIR LabelId ("bb0") to the LLVM BasicBlock.
    std::unordered_map<std::string, llvm::BasicBlock*> blocks_;

    // ── Translation Helpers ──────────────────────────────────────────────────

    llvm::Type* mapType(const Type* type);
    llvm::Value* mapOperand(const mvir::Operand& op);

    void createFunctionStructure(const mvir::Function* func);
    
    // Pass 2: Emit instructions
    void emitFunctionBody(const mvir::Function* func);
    void emitInstruction(const mvir::Instruction* inst);
    void emitTerminator(const mvir::Terminator* term);
    
    // External declarations
    llvm::FunctionCallee getOrDeclarePrint();
};

} // namespace fl
