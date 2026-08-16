// =============================================================================
// mellis/Core/SourceLocation.h
//
// Source location for diagnostic-quality error reporting.
//
// Design rationale (inspired by Clang's SourceLocation):
//   - Every declaration and identifier reference carries a SourceLocation.
//   - Storing { file, line, column, offset } allows:
//       * Fast error printing without re-scanning (line/col pre-computed)
//       * Range-based highlighting when we have a SourceRange (future)
//       * Multi-file support via FileID
//   - The byte offset is kept alongside line/col because:
//       * Lexer currently only tracks offset (backward compatible)
//       * Offset enables O(1) substring extraction from source buffer
//
// MVP status:
//   - FileID is always kMainFileID (single-file compiler)
//   - line and column are 0 (Lexer does not track them yet)
//   - offset carries the byte position from Token.byteOffset
//
// Future: Lexer upgrade to track line/column will fill all four fields
// with zero changes to downstream code.
// =============================================================================

#pragma once
#include "mellis/Core/Types.h"

namespace fl {

struct SourceLocation {
    FileID   file   = kMainFileID;
    uint32_t line   = 0;       // 1-indexed; 0 = not tracked (MVP)
    uint32_t column = 0;       // 1-indexed; 0 = not tracked (MVP)
    uint32_t offset = 0;       // byte offset into source buffer

    // ── Factory helpers ──────────────────────────────────────────────────────

    /// Construct from a known byte offset (MVP use-case).
    static SourceLocation fromOffset(uint32_t byteOffset,
                                     FileID file = kMainFileID) {
        return {file, 0, 0, byteOffset};
    }

    /// Construct a fully-specified location (future use-case).
    static SourceLocation fromLineCol(FileID file,
                                      uint32_t line,
                                      uint32_t col,
                                      uint32_t offset) {
        return {file, line, col, offset};
    }

    /// An explicitly invalid location (e.g., compiler-synthesized nodes).
    static SourceLocation invalid() {
        return {kMainFileID, 0, 0, std::numeric_limits<uint32_t>::max()};
    }

    bool isValid() const {
        return offset != std::numeric_limits<uint32_t>::max();
    }
};

} // namespace fl
