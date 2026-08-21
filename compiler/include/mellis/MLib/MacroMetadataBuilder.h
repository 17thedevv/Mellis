#pragma once

#include "mellis/MLib/BinaryWriter.h"
#include <string>
#include <vector>

namespace fl {
namespace mlib {

class MacroMetadataBuilder {
public:
    void addMacro(const std::string& name, const std::string& rawSource);
    void serialize(BinaryWriter& writer) const;

private:
    struct MacroEntry {
        std::string name;
        std::string rawSource;
    };
    std::vector<MacroEntry> macros;
};

} // namespace mlib
} // namespace fl
