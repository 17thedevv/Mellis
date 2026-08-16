#pragma once
#include <string>
#include <string_view>
#include <vector>
#include <unordered_map>
#include <memory>
#include "mellis/Core/Types.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

struct SourceFile {
    FileID fileId;
    std::string filepath;
    std::string content;
};

class SourceManager {
public:
    SourceManager(DiagnosticEngine& diag);
    ~SourceManager() = default;

    /// Loads a file and returns its FileID. If already loaded, returns the existing FileID.
    /// If the file cannot be loaded, reports a diagnostic error and returns kInvalidFileID.
    FileID loadFile(const std::string& filepath);

    /// Adds or updates a virtual file (e.g., from an LSP client) and returns its FileID.
    FileID addVirtualFile(const std::string& uri, const std::string& content);

    /// Gets the source content for a given FileID as a stable string_view.
    std::string_view getSource(FileID fileId) const;

    /// Gets the filepath for a given FileID.
    std::string getFilepath(FileID fileId) const;

    /// Resolves a module name from a base file (e.g., "foo" relative to "src/main.ms").
    /// Searches for "foo.ms" or "foo/mod.ms" in the base file's directory.
    /// Returns the resolved absolute or relative path, or empty string if not found.
    std::string resolveModulePath(FileID baseFileId, std::string_view moduleName) const;

    static constexpr FileID kInvalidFileID = std::numeric_limits<FileID>::max();

private:
    DiagnosticEngine& diag_;
    std::vector<std::unique_ptr<SourceFile>> files_;
    std::unordered_map<std::string, FileID> pathToId_;
};

} // namespace fl
