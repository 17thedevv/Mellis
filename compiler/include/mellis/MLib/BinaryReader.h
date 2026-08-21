#ifndef MELLIS_MLIB_BINARYREADER_H
#define MELLIS_MLIB_BINARYREADER_H

#include <cstdint>
#include <string>
#include <vector>
#include <stdexcept>

namespace fl {
namespace mlib {

class BinaryReader {
public:
    // Initialize reader with a memory buffer
    BinaryReader(const uint8_t* data, size_t size);

    uint8_t readU8();
    uint16_t readU16();
    uint32_t readU32();
    uint64_t readU64();

    int8_t readI8();
    int16_t readI16();
    int32_t readI32();
    int64_t readI64();

    float readFloat();
    double readDouble();

    // Read fixed amount of bytes
    void readBytes(uint8_t* out, size_t size);

    // Skip ahead
    void skip(size_t size);

    // Skip to specific offset
    void seek(size_t offset);

    // Read a string (without prefix length, you must know length)
    std::string readStringRaw(size_t length);

    // Read a length-prefixed string (matches writeString)
    std::string readString();

    // Read a struct directly (must be packed)
    template<typename T>
    void readStruct(T& out) {
        readBytes(reinterpret_cast<uint8_t*>(&out), sizeof(T));
    }

    // Get current offset
    size_t getOffset() const { return offset; }
    
    // Get total size
    size_t getSize() const { return size; }
    
    // Check if EOF
    bool isEOF() const { return offset >= size; }

    // Get a pointer directly to the current data (useful for zero-copy)
    const uint8_t* currentPtr() const;

private:
    void ensureCapacity(size_t required);

    const uint8_t* data;
    size_t size;
    size_t offset;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_BINARYREADER_H
