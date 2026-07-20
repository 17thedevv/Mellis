#ifndef MELLIS_MLIB_BINARYWRITER_H
#define MELLIS_MLIB_BINARYWRITER_H

#include <vector>
#include <cstdint>
#include <string>

namespace fl {
namespace mlib {

class BinaryWriter {
public:
    BinaryWriter() = default;
    
    // Write primitive types
    void writeU8(uint8_t value);
    void writeU16(uint16_t value);
    void writeU32(uint32_t value);
    void writeU64(uint64_t value);
    
    void writeI8(int8_t value);
    void writeI16(int16_t value);
    void writeI32(int32_t value);
    void writeI64(int64_t value);
    
    void writeFloat(float value);
    void writeDouble(double value);
    
    // Write raw bytes
    void writeBytes(const uint8_t* data, size_t size);
    
    // Write string (without prefix length, just the bytes)
    void writeStringRaw(const std::string& str);
    
    // Pad to a specific alignment
    void pad(size_t alignment);

    // Get the current offset
    size_t getOffset() const;
    
    // Seek to an offset and overwrite data
    void writeAt(size_t offset, const uint8_t* data, size_t size);

    // Write a struct directly (must be packed)
    template<typename T>
    void writeStruct(const T& value) {
        writeBytes(reinterpret_cast<const uint8_t*>(&value), sizeof(T));
    }
    
    template<typename T>
    void writeStructAt(size_t offset, const T& value) {
        writeAt(offset, reinterpret_cast<const uint8_t*>(&value), sizeof(T));
    }

    const std::vector<uint8_t>& getBuffer() const { return buffer; }
    std::vector<uint8_t> takeBuffer() { return std::move(buffer); }

private:
    std::vector<uint8_t> buffer;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_BINARYWRITER_H
