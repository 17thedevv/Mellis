#include "mellis/MLib/MacroMetadataBuilder.h"

namespace fl {
namespace mlib {

void MacroMetadataBuilder::addMacro(const std::string& name, const std::string& rawSource) {
    macros.push_back({name, rawSource});
}

void MacroMetadataBuilder::serialize(BinaryWriter& writer) const {
    writer.writeU32(static_cast<uint32_t>(macros.size()));
    for (const auto& entry : macros) {
        writer.writeString(entry.name);
        writer.writeString(entry.rawSource);
    }
}

} // namespace mlib
} // namespace fl
