#include "mellis/Support/OSUtils.h"

// Platform-specific headers
#if defined(_WIN32)
#  ifndef WIN32_LEAN_AND_MEAN
#    define WIN32_LEAN_AND_MEAN
#  endif
#  ifndef NOMINMAX
#    define NOMINMAX
#  endif
#  include <windows.h>
#else
#  include <unistd.h>
#  include <limits.h>
#endif

#include <filesystem>
#include <iostream>

namespace fl {

std::string OSUtils::getExecutablePath() {
    std::error_code ec;
#if defined(_WIN32)
    wchar_t buf[4096] = {};
    DWORD len = GetModuleFileNameW(nullptr, buf, 4096);
    if (len > 0) {
        return std::filesystem::path(buf).string();
    }
    return "."; // Fallback
#else
    char buf[PATH_MAX];
    ssize_t len = ::readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    if (len != -1) {
        buf[len] = '\0';
        return std::string(buf);
    }
    return "."; // Fallback
#endif
}

std::string OSUtils::getParentDirectory(const std::string& path, int levels) {
    std::filesystem::path p(path);
    for (int i = 0; i < levels; ++i) {
        if (p.has_parent_path()) {
            p = p.parent_path();
        } else {
            break;
        }
    }
    return p.string();
}

bool OSUtils::fileExists(const std::string& path) {
    std::error_code ec;
    return std::filesystem::exists(path, ec);
}

} // namespace fl
