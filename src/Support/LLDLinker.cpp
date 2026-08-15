// =============================================================================
// mellis/Support/LLDLinker.cpp
//
// Implementation of LLDLinker.
// =============================================================================

#include "mellis/Support/LLDLinker.h"
#include "mellis/Support/OSUtils.h"
#include <cstdlib>
#include <iostream>

namespace fl {

LLDLinker::LLDLinker(DiagnosticEngine& diag, bool verbose) 
    : diag_(diag), verbose_(verbose) {}

bool LLDLinker::link(const std::string& objFile,
                       const std::string& outFile,
                       const std::vector<std::string>& libs) {
    std::string exePath = OSUtils::getExecutablePath();
    std::string binDir = OSUtils::getParentDirectory(exePath, 0); // Directory containing mellis.exe
    
    std::string portableLinker = binDir + "\\lld-link.exe";
    std::string linkerPath = "lld-link"; // Fallback to PATH
    
    if (OSUtils::fileExists(portableLinker)) {
        linkerPath = "\"" + portableLinker + "\"";
    } else {
        // Fallback to absolute path on the user's specific setup for LLVM-DEV
        int checkResult = std::system("lld-link --version > NUL 2>&1");
        if (checkResult != 0) {
            linkerPath = "\"D:\\Programs\\LLVM-DEV\\bin\\lld-link.exe\"";
        }
    }

    std::string command = linkerPath + " /OUT:" + outFile + " " + objFile;
    
    for (const auto& lib : libs) {
        command += " " + lib;
    }

    // Default library linkage for Windows (libcmt provides C runtime)
    command += " /DEFAULTLIB:libcmt";
    command += " /DEFAULTLIB:libucrt";
    command += " /DEFAULTLIB:legacy_stdio_definitions";
    command += " /subsystem:console";

    if (!verbose_) {
        // std::cout << "Running command: " << command << "\n";
        std::cout.flush();
    } else {
        // std::cout << "Running command: " << command << "\n";
        std::cout.flush();
    }

    int result = std::system(command.c_str());
    if (result != 0) {
        diag_.error(SourceLocation{}, "Trinh lien ket (lld-link) that bai khi tao " + outFile + 
                    ". Kiem tra xem ban da cai dat LLVM chua.");
        return false;
    }

    return true;
}

} // namespace fl
