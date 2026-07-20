#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/BinaryReader.h"
#include "mellis/MLib/Serializer.h"
#include "mellis/MLib/Deserializer.h"
#include <iostream>
#include <cassert>
#include <cstring>

using namespace fl::mlib;

void testBasicWriterReader() {
    BinaryWriter writer;
    writer.writeU8(0xAA);
    writer.writeU16(0xBBCC);
    writer.writeU32(0xDDEEFF00);
    writer.writeU64(0x1122334455667788ULL);
    writer.writeFloat(3.14159f);
    writer.writeDouble(2.718281828);

    auto buf = writer.takeBuffer();
    BinaryReader reader(buf.data(), buf.size());

    assert(reader.readU8() == 0xAA);
    assert(reader.readU16() == 0xBBCC);
    assert(reader.readU32() == 0xDDEEFF00);
    assert(reader.readU64() == 0x1122334455667788ULL);
    
    // Check floats with a small epsilon
    float f = reader.readFloat();
    assert(std::abs(f - 3.14159f) < 0.0001f);
    
    double d = reader.readDouble();
    assert(std::abs(d - 2.718281828) < 0.0000001);

    std::cout << "testBasicWriterReader passed\n";
}

void testMLibHeader() {
    MLibHeader header;
    std::memcpy(header.magic, MLIB_MAGIC, 4);
    header.formatVersion = 1;
    header.compilerVersion = 1;
    std::strncpy(header.targetTriple, "x86_64-pc-windows-msvc", sizeof(header.targetTriple));
    for (int i = 0; i < 16; ++i) header.moduleUUID[i] = static_cast<uint8_t>(i);
    header.moduleHash = 0xABCDEF1234567890ULL;
    header.timestamp = 1629813291;
    header.flags = 0; // Default flags
    header.sectionCount = 2;
    header.sectionTableOffset = sizeof(MLibHeader);

    BinaryWriter writer;
    writer.writeStruct(header);

    SectionEntry s1;
    s1.sectionID = 0;
    s1.sectionType = static_cast<uint32_t>(SectionType::StringTable);
    s1.offset = 100;
    s1.size = 200;
    s1.version = 1;
    s1.compression = SectionCompression::None;
    std::memset(s1.reserved, 0, sizeof(s1.reserved));
    s1.hash = 0x9999999999999999ULL;

    writer.writeStruct(s1);

    auto buf = writer.takeBuffer();
    BinaryReader reader(buf.data(), buf.size());

    MLibHeader outHeader;
    reader.readStruct(outHeader);

    assert(std::memcmp(outHeader.magic, MLIB_MAGIC, 4) == 0);
    assert(outHeader.formatVersion == 1);
    assert(outHeader.compilerVersion == 1);
    assert(std::strcmp(outHeader.targetTriple, "x86_64-pc-windows-msvc") == 0);
    assert(outHeader.moduleHash == 0xABCDEF1234567890ULL);
    assert(outHeader.timestamp == 1629813291);
    assert(outHeader.sectionCount == 2);
    assert(outHeader.sectionTableOffset == sizeof(MLibHeader));

    for (int i = 0; i < 16; ++i) {
        assert(outHeader.moduleUUID[i] == static_cast<uint8_t>(i));
    }

    SectionEntry outS1;
    reader.readStruct(outS1);
    assert(outS1.sectionID == 0);
    assert(outS1.sectionType == static_cast<uint32_t>(SectionType::StringTable));
    assert(outS1.offset == 100);
    assert(outS1.size == 200);
    assert(outS1.version == 1);
    assert(outS1.compression == SectionCompression::None);
    assert(outS1.hash == 0x9999999999999999ULL);

    std::cout << "testMLibHeader passed\n";
}

void testSerializer() {
    BinaryWriter writer;
    Serializer ser(writer);

    ser.writeString("Hello Mellis!");
    uint8_t uuid[16] = {0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 
                        0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00};
    ser.writeUUID(uuid);

    auto buf = writer.takeBuffer();
    BinaryReader reader(buf.data(), buf.size());
    Deserializer des(reader);

    std::string str = des.readString();
    assert(str == "Hello Mellis!");

    uint8_t outUuid[16];
    des.readUUID(outUuid);
    for (int i = 0; i < 16; ++i) {
        assert(outUuid[i] == uuid[i]);
    }

    std::cout << "testSerializer passed\n";
}

int main() {
    testBasicWriterReader();
    testMLibHeader();
    testSerializer();

    std::cout << "All MLib tests passed!\n";
    return 0;
}
