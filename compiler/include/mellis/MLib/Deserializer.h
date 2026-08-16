#ifndef MELLIS_MLIB_DESERIALIZER_H
#define MELLIS_MLIB_DESERIALIZER_H

#include "mellis/MLib/BinaryReader.h"
#include <string>
#include <vector>

namespace fl {
namespace mlib {

class Deserializer {
public:
    explicit Deserializer(BinaryReader& reader) : reader(reader) {}

    // Strings are read as: length (U32) followed by characters
    std::string readString();

    // Vector of primitive types or types that have custom deserializers
    template<typename T>
    std::vector<T> readVector(T (Deserializer::*readFunc)()) {
        uint32_t size = reader.readU32();
        std::vector<T> vec;
        vec.reserve(size);
        for (uint32_t i = 0; i < size; ++i) {
            vec.push_back((this->*readFunc)());
        }
        return vec;
    }

    void readUUID(uint8_t uuid[16]);

    // Access to underlying reader for primitives
    BinaryReader& getReader() { return reader; }

private:
    BinaryReader& reader;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_DESERIALIZER_H
