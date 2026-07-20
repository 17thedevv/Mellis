#include "mellis/MLib/Serializer.h"

namespace fl {
namespace mlib {

void Serializer::writeString(const std::string& str) {
    writer.writeU32(static_cast<uint32_t>(str.length()));
    writer.writeStringRaw(str);
}

void Serializer::writeUUID(const uint8_t uuid[16]) {
    writer.writeBytes(uuid, 16);
}

} // namespace mlib
} // namespace fl
