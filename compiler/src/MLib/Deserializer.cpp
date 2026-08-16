#include "mellis/MLib/Deserializer.h"

namespace fl {
namespace mlib {

std::string Deserializer::readString() {
    uint32_t length = reader.readU32();
    return reader.readStringRaw(length);
}

void Deserializer::readUUID(uint8_t uuid[16]) {
    reader.readBytes(uuid, 16);
}

} // namespace mlib
} // namespace fl
