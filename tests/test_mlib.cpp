#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/BinaryReader.h"
#include "mellis/MLib/Serializer.h"
#include "mellis/MLib/Deserializer.h"
#include "mellis/MLib/StringTableBuilder.h"
#include "mellis/MLib/ManifestBuilder.h"
#include "mellis/MLib/MetadataBuilder.h"

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
    
    float f = reader.readFloat();
    assert(std::abs(f - 3.14159f) < 0.0001f);
    
    double d = reader.readDouble();
    assert(std::abs(d - 2.718281828) < 0.0000001);
}

void testStringTableDeduplication() {
    StringTableBuilder stringTable;
    
    uint32_t id1 = stringTable.addString("Hello");
    uint32_t id2 = stringTable.addString("World");
    uint32_t id3 = stringTable.addString("Hello"); // Should be deduplicated

    assert(id1 != id2);
    assert(id1 == id3);

    // Empty string is always 0
    uint32_t id4 = stringTable.addString("");
    assert(id4 == 0);

    BinaryWriter writer;
    stringTable.serialize(writer);

    auto buf = writer.takeBuffer();
    BinaryReader reader(buf.data(), buf.size());

    // We can manually read strings using the offset
    reader.seek(id1);
    std::string str1 = "";
    while (char c = static_cast<char>(reader.readU8())) {
        str1 += c;
    }
    assert(str1 == "Hello");
}

void testManifestAndDependency() {
    StringTableBuilder stringTable;
    ManifestBuilder manifest(stringTable);

    manifest.setPackageName("core_collections");
    manifest.setAuthor("Mellis Team");
    manifest.setVersion("1.0.0");
    manifest.setLicense("MIT");

    manifest.addFeature("simd");
    manifest.addFeature("async");

    uint8_t uuid[16] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16};
    manifest.addDependency(uuid, "0.5.0", 0x12345678ULL, ImportMode::Public, {"allocator"});

    BinaryWriter writer;
    manifest.serialize(writer);

    auto buf = writer.takeBuffer();
    BinaryReader reader(buf.data(), buf.size());

    ManifestHeader header;
    reader.readStruct(header);

    assert(header.featureCount == 2);
    assert(header.dependencyCount == 1);

    uint32_t feat1 = reader.readU32();
    uint32_t feat2 = reader.readU32();
    // feat1 and feat2 map to "simd" and "async"

    DependencyEntry dep;
    reader.readStruct(dep);
    assert(dep.moduleUUID[0] == 1);
    assert(dep.moduleHash == 0x12345678ULL);
    assert(dep.importMode == ImportMode::Public);
    assert(dep.featureCount == 1);
}

void testMetadataArena() {
    StringTableBuilder stringTable;
    MetadataBuilder meta(stringTable);

    uint32_t nsID = meta.addNamespace("std");
    uint32_t typeID = meta.addType("Vec", nsID, 24, 8);
    uint32_t traitID = meta.addTrait("Display", nsID);
    uint32_t funcID = meta.addFunction("push", nsID, 0); // Dummy signature
    uint32_t implID = meta.addImpl(traitID, typeID);

    BinaryWriter writer;
    meta.serializeMetadata(writer);

    auto buf = writer.takeBuffer();
    BinaryReader reader(buf.data(), buf.size());

    uint32_t nsCount = reader.readU32();
    assert(nsCount == 1);
    NamespaceEntry nsEntry;
    reader.readStruct(nsEntry);
    assert(nsEntry.parentNamespaceID == 0xFFFFFFFF);

    uint32_t typeCount = reader.readU32();
    assert(typeCount == 1);
    TypeEntry typeEntry;
    reader.readStruct(typeEntry);
    assert(typeEntry.size == 24);
    assert(typeEntry.alignment == 8);

    uint32_t traitCount = reader.readU32();
    assert(traitCount == 1);
    TraitEntry traitEntry;
    reader.readStruct(traitEntry);

    uint32_t funcCount = reader.readU32();
    assert(funcCount == 1);
    FunctionEntry funcEntry;
    reader.readStruct(funcEntry);
    assert(funcEntry.namespaceID == nsID);
}

int main() {
    testBasicWriterReader();
    std::cout << "testBasicWriterReader passed\n";

    testStringTableDeduplication();
    std::cout << "testStringTableDeduplication passed\n";

    testManifestAndDependency();
    std::cout << "testManifestAndDependency passed\n";

    testMetadataArena();
    std::cout << "testMetadataArena passed\n";

    std::cout << "All MLib Phase M2 tests passed!\n";
    return 0;
}
