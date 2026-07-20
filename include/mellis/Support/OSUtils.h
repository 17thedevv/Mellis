#pragma once
#include <string>

namespace fl {

// OSUtils provides portable wrappers for OS-specific tasks, encapsulating
// platform-dependent headers (like windows.h) away from compiler headers.
class OSUtils {
public:
    // Returns the absolute path to the running executable.
    static std::string getExecutablePath();

    // Returns the parent directory of a given path.
    // Levels determines how many directories to step up (1 = parent).
    static std::string getParentDirectory(const std::string& path, int levels = 1);

    // Checks if a file exists at the given path.
    static bool fileExists(const std::string& path);
};

} // namespace fl
