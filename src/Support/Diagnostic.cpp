// =============================================================================
// mellis/Support/Diagnostic.cpp
//
// DiagnosticEngine implementation.
//
// FORMATTING NOTE:
//   All output formatting is concentrated in flush(). This is intentional.
//   Future work (ANSI color, source-snippet caret, error codes, sorted output)
//   must be added only here — no formatting logic should leak into report().
// =============================================================================

#include "mellis/Support/Diagnostic.h"
#include "mellis/Support/DiagnosticConsumer.h"
#include "mellis/Core/SourceManager.h"
#include <iostream>
#include <string_view>

namespace fl {

// =============================================================================
// Helpers (file-local)
// =============================================================================

namespace {

/// Convert severity to its display label.
/// This is the only place severity→string mapping lives.
constexpr std::string_view severityLabel(DiagSeverity sev) {
    switch (sev) {
        case DiagSeverity::Note:    return "note";
        case DiagSeverity::Warning: return "warning";
        case DiagSeverity::Error:   return "error";
        case DiagSeverity::Fatal:   return "fatal error";
    }
    return "unknown";
}

} // anonymous namespace

// =============================================================================
// DiagnosticEngine — Reporting
// =============================================================================

Diagnostic& DiagnosticEngine::report(DiagSeverity sev, SourceLocation loc,
                               std::string msg, std::string code) {
    // Deduplicate identical errors
    for (auto& d : diagnostics_) {
        if (d.severity == sev && d.location.line == loc.line && 
            d.location.column == loc.column && d.message == msg) {
            return d; // Return existing duplicate to allow adding notes to it
        }
    }
    
    diagnostics_.push_back({sev, loc, std::move(code), std::move(msg), {}});

    switch (sev) {
        case DiagSeverity::Warning:
            ++warningCount_;
            break;
        case DiagSeverity::Error:
        case DiagSeverity::Fatal:
            ++errorCount_;
            break;
        case DiagSeverity::Note:
            break;
    }
    return diagnostics_.back();
}

void DiagnosticEngine::note(SourceLocation loc, std::string msg) {
    report(DiagSeverity::Note, loc, std::move(msg));
}

Diagnostic& DiagnosticEngine::warning(SourceLocation loc, std::string msg, std::string code) {
    return report(DiagSeverity::Warning, loc, std::move(msg), std::move(code));
}

Diagnostic& DiagnosticEngine::error(SourceLocation loc, std::string msg, std::string code) {
    return report(DiagSeverity::Error, loc, std::move(msg), std::move(code));
}

Diagnostic& DiagnosticEngine::fatal(SourceLocation loc, std::string msg, std::string code) {
    return report(DiagSeverity::Fatal, loc, std::move(msg), std::move(code));
}

[[noreturn]] void DiagnosticEngine::ice(SourceLocation loc, std::string msg) {
    std::cerr << "error: internal compiler error: " << msg << "\n";
    if (loc.line > 0) {
        std::cerr << " --> " << loc.file << ":" << loc.line << ":" << loc.column << "\n";
    }
    std::cerr << "note: this is a compiler bug, not an error in your program\n";
    std::exit(101);
}

// =============================================================================
// DiagnosticEngine — Output
// =============================================================================

/// Format and emit all buffered diagnostics to stderr.
///
/// Current format (MVP):
///   <file>:<line>:<col>: <severity>: <message>
///   <file>: <severity>: <message>          (when line/col not tracked)
///
/// This is the SINGLE formatting point. All future enhancements live here:
///   - ANSI color codes (wrap severityLabel with \033[...m)
///   - Source snippet + caret (^) underline
///   - Error codes ("error[E0042]")
///   - Sorted output (errors before notes)
///   - TODO(DiagnosticConsumer): dispatch to consumer->handleDiagnostic()
///     instead of writing directly to stderr.
void DiagnosticEngine::flush() const {
    if (consumers_.empty()) {
        // Fallback or warning if no consumer is registered
        return;
    }
    for (const Diagnostic& d : diagnostics_) {
        for (const auto& consumer : consumers_) {
            consumer->handleDiagnostic(d);
        }
    }
}

// =============================================================================
// DiagnosticEngine — Reset
// =============================================================================

void DiagnosticEngine::reset() {
    diagnostics_.clear();
    errorCount_   = 0;
    warningCount_ = 0;
}

} // namespace fl
