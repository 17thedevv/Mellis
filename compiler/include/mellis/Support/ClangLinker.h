// =============================================================================
// mellis/Support/ClangLinker.h
//
// Clang-based linker implementation. Uses clang as the compiler driver
// to automatically discover the Windows SDK and invoke lld-link.
// =============================================================================

#ifndef MELLIS_CLANG_LINKER_H
#define MELLIS_CLANG_LINKER_H

#include "mellis/Support/Linker.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class ClangLinker : public Linker {
public:
    explicit ClangLinker(DiagnosticEngine& diag, bool verbose = false);

    bool link(const std::string& objFile,
              const std::string& outFile,
              const std::vector<std::string>& libs) override;

private:
    DiagnosticEngine& diag_;
    bool verbose_;
};

} // namespace fl

#endif // MELLIS_CLANG_LINKER_H
