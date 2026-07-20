#pragma once
#include "mellis/Support/Diagnostic.h"

namespace fl {

class DiagnosticConsumer {
public:
    virtual ~DiagnosticConsumer() = default;
    
    // Called by DiagnosticEngine to report a single diagnostic
    virtual void handleDiagnostic(const Diagnostic& diag) = 0;
};

} // namespace fl
