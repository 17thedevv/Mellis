#include "mellis/Core/SourceManager.h"
#include <fstream>
#include <filesystem>

namespace fl {

SourceManager::SourceManager(DiagnosticEngine& diag) : diag_(diag) {}

FileID SourceManager::loadFile(const std::string& filepath) {
    // Check if already loaded
    auto it = pathToId_.find(filepath);
    if (it != pathToId_.end()) {
        return it->second;
    }

    std::ifstream file(filepath, std::ios::binary | std::ios::ate);
    if (!file.is_open()) {
        diag_.error(SourceLocation::invalid(), "Khong the mo file '" + filepath + "'");
        return kInvalidFileID;
    }

    std::streamsize size = file.tellg();
    std::string content(size, '\0');
    file.seekg(0, std::ios::beg);
    if (!file.read(content.data(), size)) {
        diag_.error(SourceLocation::invalid(), "Loi doc file '" + filepath + "'");
        return kInvalidFileID;
    }

    FileID newId = static_cast<FileID>(files_.size());
    auto srcFile = std::make_unique<SourceFile>();
    srcFile->fileId = newId;
    srcFile->filepath = filepath;
    srcFile->content = std::move(content);

    pathToId_[filepath] = newId;
    files_.push_back(std::move(srcFile));

    return newId;
}

FileID SourceManager::addVirtualFile(const std::string& uri, const std::string& content) {
    auto it = pathToId_.find(uri);
    if (it != pathToId_.end()) {
        files_[it->second]->content = content;
        return it->second;
    }

    FileID newId = static_cast<FileID>(files_.size());
    auto srcFile = std::make_unique<SourceFile>();
    srcFile->fileId = newId;
    srcFile->filepath = uri;
    srcFile->content = content;

    pathToId_[uri] = newId;
    files_.push_back(std::move(srcFile));

    return newId;
}

std::string_view SourceManager::getSource(FileID fileId) const {
    if (fileId < files_.size()) {
        return files_[fileId]->content;
    }
    return "";
}

std::string SourceManager::getFilepath(FileID fileId) const {
    if (fileId < files_.size()) {
        return files_[fileId]->filepath;
    }
    return "";
}

std::string SourceManager::resolveModulePath(FileID baseFileId, std::string_view moduleName) const {
    if (baseFileId >= files_.size()) return "";
    
    std::filesystem::path basePath(files_[baseFileId]->filepath);
    std::filesystem::path dir = basePath.parent_path();
    
    std::string modNameStr(moduleName);
    
    // 1. Check dir/moduleName.fl
    std::filesystem::path path1 = dir / (modNameStr + ".ms");
    if (std::filesystem::exists(path1)) {
        return path1.string();
    }
    
    // 2. Check dir/moduleName/mod.fl
    std::filesystem::path path2 = dir / modNameStr / "mod.ms";
    if (std::filesystem::exists(path2)) {
        return path2.string();
    }
    
    return "";
}

} // namespace fl
