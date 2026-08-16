#include "mellis/MLib/GenericMetadataBuilder.h"

namespace fl {
namespace mlib {

void GenericMetadataBuilder::addGeneric(GenericKind kind, const std::string& name, const std::string& rawSource) {
    generics.push_back({kind, name, rawSource});
}

void GenericMetadataBuilder::serialize(BinaryWriter& writer) const {
    writer.writeU32(static_cast<uint32_t>(generics.size()));
    for (const auto& entry : generics) {
        writer.writeU8(static_cast<uint8_t>(entry.kind));
        writer.writeString(entry.name);
        writer.writeString(entry.rawSource);
    }
}

} // namespace mlib
} // namespace fl