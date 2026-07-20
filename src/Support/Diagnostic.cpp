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

void DiagnosticEngine::report(DiagSeverity sev, SourceLocation loc,
                               std::string msg) {
    diagnostics_.push_back({sev, loc, std::move(msg)});

    switch (sev) {
        case DiagSeverity::Warning:
            ++warningCount_;
            break;
        case DiagSeverity::Error:
            ++errorCount_;
            break;
        case DiagSeverity::Fatal:
            ++errorCount_;
            break;
        case DiagSeverity::Note:
            break;
    }
}

void DiagnosticEngine::note(SourceLocation loc, std::string msg) {
    report(DiagSeverity::Note, loc, std::move(msg));
}

void DiagnosticEngine::warning(SourceLocation loc, std::string msg) {
    report(DiagSeverity::Warning, loc, std::move(msg));
}

void DiagnosticEngine::error(SourceLocation loc, std::string msg) {
    report(DiagSeverity::Error, loc, std::move(msg));
}

void DiagnosticEngine::fatal(SourceLocation loc, std::string msg) {
    report(DiagSeverity::Fatal, loc, std::move(msg));
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
    const char* RESET = "\033[0m";
    const char* BOLD = "\033[1m";
    const char* RED = "\033[31m";
    const char* YELLOW = "\033[33m";
    const char* BLUE = "\033[34m";
    const char* GREEN = "\033[32m";

    for (const Diagnostic& d : diagnostics_) {
        std::string_view label = severityLabel(d.severity);
        const char* color = (d.severity == DiagSeverity::Error || d.severity == DiagSeverity::Fatal) ? RED : 
                            (d.severity == DiagSeverity::Warning ? YELLOW : GREEN);
        
        const SourceLocation& loc = d.location;

        std::string filename = "mellis";
        if (sourceMgr_ && loc.file != SourceManager::kInvalidFileID) {
            filename = sourceMgr_->getFilepath(loc.file);
        }

        if (loc.line != 0) {
            // First line: error: message
            std::cerr << BOLD << color << label << ": " << RESET << BOLD << d.message << RESET << "\n";
            // Second line:  --> filename:line:col
            std::cerr << " " << BLUE << "--> " << RESET << filename << ":" << loc.line << ":" << loc.column << "\n";
            
            if (sourceMgr_ && loc.file != SourceManager::kInvalidFileID) {
                std::string_view sourceView = sourceMgr_->getSource(loc.file);
                // Find the start of the line
                size_t lineStart = 0;
                uint32_t currentLine = 1;
                while (currentLine < loc.line && lineStart < sourceView.size()) {
                    if (sourceView[lineStart] == '\n') currentLine++;
                    lineStart++;
                }
                
                size_t lineEnd = lineStart;
                while (lineEnd < sourceView.size() && sourceView[lineEnd] != '\n' && sourceView[lineEnd] != '\r') {
                    lineEnd++;
                }
                
                if (lineStart < sourceView.size()) {
                    std::string_view sourceLine = sourceView.substr(lineStart, lineEnd - lineStart);
                    
                    std::cerr << "  " << BLUE << "|\n" << RESET;
                    std::cerr << loc.line << " " << BLUE << "| " << RESET;
                    
                    // Replace tabs with 4 spaces to align caret perfectly
                    std::string expandedSource = "";
                    size_t visualCol = 0;
                    for (size_t i = 0; i < sourceLine.size(); ++i) {
                        if (sourceLine[i] == '\t') {
                            expandedSource += "    ";
                            if (i < loc.column - 1) visualCol += 4;
                        } else {
                            expandedSource += sourceLine[i];
                            if (i < loc.column - 1) visualCol += 1;
                        }
                    }
                    
                    std::cerr << expandedSource << "\n";
                    std::cerr << "  " << BLUE << "| " << RESET;
                    
                    for (size_t i = 0; i < visualCol; ++i) std::cerr << " ";
                    std::cerr << color << BOLD << "^" << RESET << "\n";
                    
                    std::cerr << "  " << BLUE << "|\n" << RESET;
                }
            }
        } else {
            // MVP: location not yet tracked by Lexer — omit line/col.
            std::cerr << filename << ": byte: " << loc.offset << " " << BOLD << color << label << ": " << RESET << d.message << '\n';
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
