#include "mellis/MLib/BinaryReader.h"
#include <cstring>

namespace fl {
namespace mlib {

BinaryReader::BinaryReader(const uint8_t* data, size_t size) 
    : data(data), size(size), offset(0) {}

void BinaryReader::ensureCapacity(size_t required) {
    if (offset + required > size) {
        throw std::out_of_range("BinaryReader: Unexpected EOF");
    }
}

uint8_t BinaryReader::readU8() {
    ensureCapacity(1);
    return data[offset++];
}

uint16_t BinaryReader::readU16() {
    ensureCapacity(2);
    uint16_t val = static_cast<uint16_t>(data[offset]) |
                   (static_cast<uint16_t>(data[offset+1]) << 8);
    offset += 2;
    return val;
}

uint32_t BinaryReader::readU32() {
    ensureCapacity(4);
    uint32_t val = static_cast<uint32_t>(data[offset]) |
                   (static_cast<uint32_t>(data[offset+1]) << 8) |
                   (static_cast<uint32_t>(data[offset+2]) << 16) |
                   (static_cast<uint32_t>(data[offset+3]) << 24);
    offset += 4;
    return val;
}

uint64_t BinaryReader::readU64() {
    ensureCapacity(8);
    uint64_t val = static_cast<uint64_t>(data[offset]) |
                   (static_cast<uint64_t>(data[offset+1]) << 8) |
                   (static_cast<uint64_t>(data[offset+2]) << 16) |
                   (static_cast<uint64_t>(data[offset+3]) << 24) |
                   (static_cast<uint64_t>(data[offset+4]) << 32) |
                   (static_cast<uint64_t>(data[offset+5]) << 40) |
                   (static_cast<uint64_t>(data[offset+6]) << 48) |
                   (static_cast<uint64_t>(data[offset+7]) << 56);
    offset += 8;
    return val;
}

int8_t BinaryReader::readI8() {
    return static_cast<int8_t>(readU8());
}

int16_t BinaryReader::readI16() {
    return static_cast<int16_t>(readU16());
}

int32_t BinaryReader::readI32() {
    return static_cast<int32_t>(readU32());
}

int64_t BinaryReader::readI64() {
    return static_cast<int64_t>(readU64());
}

float BinaryReader::readFloat() {
    uint32_t bits = readU32();
    float val;
    std::memcpy(&val, &bits, sizeof(float));
    return val;
}

double BinaryReader::readDouble() {
    uint64_t bits = readU64();
    double val;
    std::memcpy(&val, &bits, sizeof(double));
    return val;
}

void BinaryReader::readBytes(uint8_t* out, size_t count) {
    ensureCapacity(count);
    std::memcpy(out, data + offset, count);
    offset += count;
}

void BinaryReader::skip(size_t count) {
    ensureCapacity(count);
    offset += count;
}

void BinaryReader::seek(size_t target_offset) {
    if (target_offset > size) {
        throw std::out_of_range("BinaryReader: seek out of bounds");
    }
    offset = target_offset;
}

std::string BinaryReader::readStringRaw(size_t length) {
    ensureCapacity(length);
    std::string str(reinterpret_cast<const char*>(data + offset), length);
    offset += length;
    return str;
}

std::string BinaryReader::readString() {
    uint32_t len = readU32();
    return readStringRaw(static_cast<size_t>(len));
}

const uint8_t* BinaryReader::currentPtr() const {
    return data + offset;
}

} // namespace mlib
} // namespace fl
