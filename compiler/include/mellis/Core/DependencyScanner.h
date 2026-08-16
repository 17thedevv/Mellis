#ifndef MELLIS_CORE_DEPENDENCY_SCANNER_H
#define MELLIS_CORE_DEPENDENCY_SCANNER_H

#include <string_view>
#include <vector>
#include <string>

namespace fl {

class DependencyScanner {
public:
    // Scans the source code using the Lexer and extracts all external `use` paths.
    // Early exits when it encounters structural tokens (e.g. fn, struct, impl).
    static std::vector<std::string> scanDependencies(std::string_view source);
};

} // namespace fl

#endif // MELLIS_CORE_DEPENDENCY_SCANNER_H
