// =============================================================================
// mellis/LSP/LSPServer.h
//
// Language Server Protocol entry point for Mellis.
// =============================================================================

#pragma once

namespace fl {
namespace lsp {

/// Khởi chạy vòng lặp LSP Server, đọc từ stdin và ghi ra stdout.
/// Trả về mã lỗi để thoát ứng dụng.
int runServer();

} // namespace lsp
} // namespace fl
