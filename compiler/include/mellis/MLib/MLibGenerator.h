// =============================================================================
// mellis/MLib/MLibGenerator.h
//
// Generates the .mlib binary file containing object code and metadata.
// =============================================================================

#pragma once

#include "mellis/Support/Diagnostic.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/SemanticSnapshot.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include <llvm/IR/Module.h>
#include <string>

namespace fl {

class MLibGenerator {
public:
    MLibGenerator(DiagnosticEngine& diag, const SemanticSnapshot& snapshot, MacroRegistry& macroReg, std::string_view sourceCode);

    /// Generate a .mlib file from the LLVM module and SymbolTable metadata.
    ///
    /// @param llvmModule The generated LLVM module.
    /// @param outputPath The desired path to the final .mlib file.
    /// @return true if generation succeeded.
    bool generate(llvm::Module* llvmModule, const std::string& outputPath);

private:
    DiagnosticEngine& diag_;
    const SemanticSnapshot& snapshot_;
    MacroRegistry& macroReg_;
    std::string_view sourceCode_;
};

} // namespace fl
