#pragma once

#include "mellis/IR/MVIR.h"
#include "mellis/Support/Diagnostic.h"
#include <memory>
#include <string>
#include <vector>

namespace fl {

struct IRVerificationResult {
    bool ok;
    std::string error;
};

class SemanticSnapshot;

class IRVerifier {
public:
    explicit IRVerifier(DiagnosticEngine& diag, const SemanticSnapshot* snapshot = nullptr) 
        : diag_(diag), snapshot_(snapshot) {}

    IRVerificationResult verify(const mvir::Module& module);

private:
    DiagnosticEngine& diag_;
    const SemanticSnapshot* snapshot_;

    IRVerificationResult verifySemanticClosure(const mvir::Module& module);
    IRVerificationResult verifyTypeSemanticClosure(const Type* t, const std::string& context);
    IRVerificationResult verifyFunction(const mvir::Function& function);
    IRVerificationResult verifyEntryBlock(const mvir::Function& function);
    IRVerificationResult verifyTerminators(const mvir::Function& function);
    IRVerificationResult verifyReachability(const mvir::Function& function);
    IRVerificationResult verifyAllocaPlacement(const mvir::Function& function);
    IRVerificationResult verifyDominance(const mvir::Function& function);
    IRVerificationResult verifyBranchTargets(const mvir::Function& function);
    IRVerificationResult verifyTypeConsistency(const mvir::Function& function);
    IRVerificationResult verifyBorrowTargets(const mvir::Function& function);
    IRVerificationResult verifyDropTargets(const mvir::Function& function);
    IRVerificationResult verifyCallSignatures(const mvir::Module& module, const mvir::Function& function);
};

} // namespace fl
