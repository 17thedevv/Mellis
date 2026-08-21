// =============================================================================
// mellis/Support/ClangLinker.cpp
//
// Implementation of ClangLinker.
// =============================================================================

#include "mellis/Support/ClangLinker.h"
#include <cstdlib>
#include <iostream>

namespace fl {

ClangLinker::ClangLinker(DiagnosticEngine& diag, bool verbose) 
    : diag_(diag), verbose_(verbose) {}

bool ClangLinker::link(const std::string& objFile,
                       const std::string& outFile,
                       const std::vector<std::string>& libs) {
    // We use clang as the driver, explicitly requesting it to link with lld
    std::string command = "clang -fuse-ld=lld -o " + outFile + " " + objFile;
    
    for (const auto& lib : libs) {
        command += " " + lib;
    }

    if (!verbose_) {
        // Suppress clang and lld output
        command += " > NUL 2>&1";
    } else {
        // std::cout << "Running command: " << command << "\n";
        std::cout.flush();
    }

    int result = std::system(command.c_str());
    if (result != 0) {
        diag_.error(SourceLocation{}, "Trinh lien ket (LLVM/Clang) that bai khi tao " + outFile + 
                    ". Kiem tra xem ban da cai dat LLVM/Clang chua.");
        return false;
    }

    return true;
}

} // namespace fl
