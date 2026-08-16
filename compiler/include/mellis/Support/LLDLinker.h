// =============================================================================
// mellis/Support/LLDLinker.h
//
// Implementation of Linker using lld-link.
// =============================================================================

#pragma once

#include "mellis/Support/Linker.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class LLDLinker : public Linker {
public:
    explicit LLDLinker(DiagnosticEngine& diag, bool verbose = false);

    bool link(const std::string& objFile,
              const std::string& outFile,
              const std::vector<std::string>& libs) override;

private:
    DiagnosticEngine& diag_;
    bool verbose_;
};

} // namespace fl
