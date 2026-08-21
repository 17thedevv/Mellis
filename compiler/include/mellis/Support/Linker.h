// =============================================================================
// mellis/Support/Linker.h
//
// Platform-independent linker abstraction.
// =============================================================================

#pragma once

#include <string>
#include <vector>

namespace fl {

class Linker {
public:
    virtual ~Linker() = default;

    /// Link an object file with libraries to produce an executable.
    ///
    /// @param objFile   Path to the input object file.
    /// @param outFile   Path to the output executable.
    /// @param libs      List of paths to libraries to link against.
    /// @return true if linking succeeded.
    virtual bool link(const std::string& objFile,
                      const std::string& outFile,
                      const std::vector<std::string>& libs) = 0;
};

} // namespace fl
