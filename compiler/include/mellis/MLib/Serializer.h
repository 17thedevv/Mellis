#ifndef MELLIS_MLIB_SERIALIZER_H
#define MELLIS_MLIB_SERIALIZER_H

#include "mellis/MLib/BinaryWriter.h"
#include <string>
#include <vector>

namespace fl {
namespace mlib {

class Serializer {
public:
    explicit Serializer(BinaryWriter& writer) : writer(writer) {}

    // Strings are written as: length (U32) followed by characters
    void writeString(const std::string& str);

    // Vector of primitive types or types that have custom serializers
    template<typename T>
    void writeVector(const std::vector<T>& vec, void (Serializer::*writeFunc)(const T&)) {
        writer.writeU32(static_cast<uint32_t>(vec.size()));
        for (const auto& item : vec) {
            (this->*writeFunc)(item);
        }
    }

    void writeUUID(const uint8_t uuid[16]);

    // Access to underlying writer for primitives
    BinaryWriter& getWriter() { return writer; }

private:
    BinaryWriter& writer;
};

} // namespace mlib
} // namespace fl

#endif // MELLIS_MLIB_SERIALIZER_H
