#pragma once
#include "mellis/Support/DiagnosticConsumer.h"
#include "mellis/Core/SourceManager.h"
#include <iostream>
#include <string_view>

namespace fl {

class ConsoleDiagnosticConsumer : public DiagnosticConsumer {
    const SourceManager* sourceMgr_;
public:
    explicit ConsoleDiagnosticConsumer(const SourceManager* sm = nullptr) : sourceMgr_(sm) {}

    void setSourceManager(const SourceManager* sm) { sourceMgr_ = sm; }

    void handleDiagnostic(const Diagnostic& d) override {
        const char* RESET = "\033[0m";
        const char* BOLD = "\033[1m";
        const char* RED = "\033[31m";
        const char* YELLOW = "\033[33m";
        const char* BLUE = "\033[34m";
        const char* GREEN = "\033[32m";

        std::string_view label;
        switch (d.severity) {
            case DiagSeverity::Fatal:   label = "fatal error"; break;
            case DiagSeverity::Error:   label = "error"; break;
            case DiagSeverity::Warning: label = "warning"; break;
            case DiagSeverity::Note:    label = "note"; break;
        }

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
                    
                    size_t printableCol = 1;
                    for (char c : sourceLine) {
                        if (c == '\t') std::cerr << "    ";
                        else std::cerr << c;
                    }
                    std::cerr << "\n";
                    
                    std::cerr << "  " << BLUE << "| " << RESET;
                    for (int i = 1; i < loc.column; ++i) {
                        if (i - 1 < sourceLine.size() && sourceLine[i - 1] == '\t') std::cerr << "    ";
                        else std::cerr << " ";
                    }
                    std::cerr << BOLD << color << "^" << RESET << "\n";
                }
            }
        } else {
            std::cerr << filename << ": " << BOLD << color << label << ": " << RESET << BOLD << d.message << RESET << "\n";
        }
    }
};

} // namespace fl
