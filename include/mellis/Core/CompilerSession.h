#pragma once
#include <string>
#include <vector>
#include <memory>
#include "mellis/Support/Diagnostic.h"
#include "mellis/Core/SourceManager.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/TypeChecker.h"

namespace fl {

class ProgramNode;

class CompilerSession {
public:
    CompilerSession();
    ~CompilerSession();

    /// Biên dịch file đầu vào thành Executable.
    /// Trả về true nếu thành công, false nếu có lỗi.
    bool compile(const std::string& filepath, bool verbose = false, int optLevel = 0);

    /// Set the library search paths for external .mlib modules.
    /// Call before compile().
    void setLibraryPaths(std::vector<std::string> paths) {
        libraryPaths_ = std::move(paths);
    }

    DiagnosticEngine& getDiagnostics() { return diag_; }

private:
    // Fills libraryPaths_ with default locations relative to the compiler exe.
    void initDefaultLibraryPaths();

    DiagnosticEngine diag_;
    SourceManager sourceManager_;
    SymbolTable symbolTable_;
    TypeContext typeContext_;
    std::vector<std::string> libraryPaths_;

};

} // namespace fl
