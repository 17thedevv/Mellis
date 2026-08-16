#include "mellis/MLib/BinaryWriter.h"
#include <cstring>
#include <stdexcept>

namespace fl {
namespace mlib {

void BinaryWriter::writeU8(uint8_t value) {
    buffer.push_back(value);
}

void BinaryWriter::writeU16(uint16_t value) {
    // Little endian
    buffer.push_back(static_cast<uint8_t>(value & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 8) & 0xFF));
}

void BinaryWriter::writeU32(uint32_t value) {
    buffer.push_back(static_cast<uint8_t>(value & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 8) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 16) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 24) & 0xFF));
}

void BinaryWriter::writeU64(uint64_t value) {
    buffer.push_back(static_cast<uint8_t>(value & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 8) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 16) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 24) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 32) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 40) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 48) & 0xFF));
    buffer.push_back(static_cast<uint8_t>((value >> 56) & 0xFF));
}

void BinaryWriter::writeI8(int8_t value) {
    writeU8(static_cast<uint8_t>(value));
}

void BinaryWriter::writeI16(int16_t value) {
    writeU16(static_cast<uint16_t>(value));
}

void BinaryWriter::writeI32(int32_t value) {
    writeU32(static_cast<uint32_t>(value));
}

void BinaryWriter::writeI64(int64_t value) {
    writeU64(static_cast<uint64_t>(value));
}

void BinaryWriter::writeFloat(float value) {
    uint32_t bits;
    std::memcpy(&bits, &value, sizeof(float));
    writeU32(bits);
}

void BinaryWriter::writeDouble(double value) {
    uint64_t bits;
    std::memcpy(&bits, &value, sizeof(double));
    writeU64(bits);
}

void BinaryWriter::writeBytes(const uint8_t* data, size_t size) {
    buffer.insert(buffer.end(), data, data + size);
}

void BinaryWriter::writeStringRaw(const std::string& str) {
    buffer.insert(buffer.end(), str.begin(), str.end());
}

void BinaryWriter::writeString(const std::string& str) {
    writeU32(static_cast<uint32_t>(str.size()));
    buffer.insert(buffer.end(), str.begin(), str.end());
}

void BinaryWriter::pad(size_t alignment) {
    size_t remainder = buffer.size() % alignment;
    if (remainder != 0) {
        size_t padding = alignment - remainder;
        buffer.insert(buffer.end(), padding, 0);
    }
}

size_t BinaryWriter::getOffset() const {
    return buffer.size();
}

void BinaryWriter::writeAt(size_t offset, const uint8_t* data, size_t size) {
    if (offset + size > buffer.size()) {
        throw std::out_of_range("writeAt out of bounds");
    }
    std::memcpy(buffer.data() + offset, data, size);
}

} // namespace mlib
} // namespace fl
