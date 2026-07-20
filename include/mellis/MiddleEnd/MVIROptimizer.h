
#pragma once

#include "mellis/MiddleEnd/MVIR.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class MVIROptimizationPass {
public:
    virtual ~MVIROptimizationPass() = default;
    virtual std::string getName() const = 0;
    
    // Returns true if the pass modified the function.
    virtual bool runOnFunction(mvir::Function& func) = 0;
};

class ConstantFoldingPass : public MVIROptimizationPass {
public:
    std::string getName() const override { return "ConstantFolding"; }
    bool runOnFunction(mvir::Function& func) override;
};

class ControlFlowSimplificationPass : public MVIROptimizationPass {
public:
    std::string getName() const override { return "ControlFlowSimplification"; }
    bool runOnFunction(mvir::Function& func) override;
};

class UnreachableBlockEliminationPass : public MVIROptimizationPass {
public:
    std::string getName() const override { return "UnreachableBlockElimination"; }
    bool runOnFunction(mvir::Function& func) override;
};

class DeadCodeEliminationPass : public MVIROptimizationPass {
public:
    std::string getName() const override { return "DeadCodeElimination"; }
    bool runOnFunction(mvir::Function& func) override;
};

class DropInsertionPass : public MVIROptimizationPass {
public:
    std::string getName() const override { return "DropInsertion"; }
    bool runOnFunction(mvir::Function& func) override;
};

class MVIROptimizer {
    std::vector<std::unique_ptr<MVIROptimizationPass>> passes_;
    DiagnosticEngine& diag_;
public:
    MVIROptimizer(DiagnosticEngine& diag);
    
    // Returns true if any optimization was performed.
    bool optimize(mvir::Module& module);
};

} // namespace fl
