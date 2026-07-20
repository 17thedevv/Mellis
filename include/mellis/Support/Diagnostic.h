// =============================================================================
// mellis/Support/Diagnostic.h
//
// Diagnostic Engine — central subsystem for compiler error reporting.
//
// Architecture (Clang / Rust style):
//   All compiler passes call diag_.error() / diag_.warning() etc.
//   Diagnostics are BUFFERED internally.
//   flush() is the ONLY place that formats and writes to stderr.
//   This means: colorization, source snippets, caret (^), sorting —
//   all future formatting changes touch ONLY flush().
//
// Design TODOs (do not implement yet):
//   TODO(CompilerContext):
//     DiagnosticEngine will eventually be owned by a shared CompilerContext
//     alongside SymbolTable, SourceManager, StringInterner, etc.
//     For now it is constructed by the driver and passed by reference.
//
//   TODO(DiagnosticConsumer):
//     A future DiagnosticConsumer interface will allow pluggable output targets
//     (stderr text, IDE error protocol, JSON, LSP publishDiagnostics).
//     Sketch of the reserved interface:
//       struct DiagnosticConsumer {
//           virtual ~DiagnosticConsumer() = default;
//           virtual void handleDiagnostic(const Diagnostic&) = 0;
//       };
//     flush() will iterate diagnostics_ and call consumer_->handleDiagnostic().
//     Not implemented yet — flush() writes directly to stderr in MVP.
// =============================================================================

#pragma once
#include "mellis/Core/SourceLocation.h"
#include <string>
#include <vector>
#include <memory>

namespace fl {
class SourceManager;
class DiagnosticConsumer;
} // namespace fl
#include <cstddef>

namespace fl {

// =============================================================================
// DiagSeverity
// =============================================================================

/// Severity levels, ordered from least to most severe.
/// Fatal indicates an unrecoverable error that stops compilation immediately.
/// (Help is reserved for future "help: ..." attachments, not added yet.)
enum class DiagSeverity {
    Note,       ///< Supplementary context attached to an error or warning.
    Warning,    ///< Non-fatal issue; compilation may continue.
    Error,      ///< Recoverable error; compiler continues to find more errors.
    Fatal       ///< Unrecoverable; compilation stops after this diagnostic.
};

// =============================================================================
// Diagnostic
// =============================================================================

/// A single recorded compiler diagnostic.
/// Immutable after construction — created by DiagnosticEngine::report().
struct Diagnostic {
    DiagSeverity   severity;
    SourceLocation location;
    std::string    message;
};

// =============================================================================
// DiagnosticEngine
// =============================================================================

/// Central diagnostic engine for a compilation unit.
///
/// Usage:
///   DiagnosticEngine diag;
///   diag.error(loc, "Bien 'x' chua duoc khai bao.");
///   if (diag.hasErrors()) { diag.flush(); return; }
///
/// Lifetime: one engine per compilation (driver owns it).
/// Passes receive a DiagnosticEngine& — never a pointer or copy.
class DiagnosticEngine {
public:
    DiagnosticEngine() = default;
    
    // Connect SourceManager for rich diagnostics (fetching source snippets)
    void setSourceManager(const SourceManager* sm) { sourceMgr_ = sm; }

    // ── Reporting API ─────────────────────────────────────────────────────────

    /// Generic report — prefer the typed helpers below.
    void report(DiagSeverity sev, SourceLocation loc, std::string msg);

    void note   (SourceLocation loc, std::string msg);
    void warning(SourceLocation loc, std::string msg);
    void error  (SourceLocation loc, std::string msg);

    /// Fatal: records the diagnostic, then marks the engine as fatally errored.
    /// Callers should check hasErrors() / return early after calling fatal().
    void fatal  (SourceLocation loc, std::string msg);

    // ── Query ─────────────────────────────────────────────────────────────────

    /// Number of Error + Fatal diagnostics recorded.
    size_t errorCount()   const { return errorCount_;   }

    /// Number of Warning diagnostics recorded.
    size_t warningCount() const { return warningCount_; }

    /// True if any Error or Fatal diagnostic has been recorded.
    bool   hasErrors()    const { return errorCount_ > 0; }

    // ── Output — single formatting point ──────────────────────────────────────
    //
    // ALL formatting logic lives here.
    // Future changes (ANSI color, source snippet, caret ^, "error[E0001]" codes,
    // multi-line notes) require editing only this function.
    //
    // Does NOT clear the buffer — safe to call multiple times (e.g., in tests).
    void flush() const;

    // ── Inspection (for tests) ────────────────────────────────────────────────

    /// Direct access to the recorded diagnostic list.
    /// Intended for unit tests that need to inspect individual diagnostics
    /// without parsing stderr output.
    const std::vector<Diagnostic>& allDiagnostics() const { return diagnostics_; }
    
    void addConsumer(std::shared_ptr<DiagnosticConsumer> consumer) {
        consumers_.push_back(consumer);
    }

    // ── Reset ─────────────────────────────────────────────────────────────────

    /// Clear all recorded diagnostics and reset counters.
    /// Useful in test harnesses that reuse a single engine across test cases.
    void reset();

private:
    std::vector<Diagnostic> diagnostics_;
    std::vector<std::shared_ptr<DiagnosticConsumer>> consumers_;
    const SourceManager* sourceMgr_ = nullptr;
    size_t errorCount_   = 0;
    size_t warningCount_ = 0;
};

} // namespace fl
