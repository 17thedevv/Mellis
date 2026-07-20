#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/BinaryReader.h"
#include "mellis/MLib/Serializer.h"
#include "mellis/MLib/Deserializer.h"
#include "mellis/MLib/StringTableBuilder.h"
#include "mellis/MLib/ManifestBuilder.h"
#include "mellis/MLib/MetadataBuilder.h"
#include "mellis/MLib/GenericSerializer.h"
#include "mellis/MLib/GenericDeserializer.h"
#include "mellis/MLib/ObjectCodeBuilder.h"
#include "mellis/MLib/ObjectCodeExtractor.h"
#include "mellis/MiddleEnd/MVIR.h"

#include <iostream>
#include <cassert>
#include <cstring>
#include <unordered_map>

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

void testGenericSectionIndex() {
    using namespace fl::mvir;
    
    // Create a mock Generic Function (max<T>)
    Function func;
    func.name = GlobalId{"max"};
    
    auto bb = std::make_unique<BasicBlock>(LabelId{"bb0"});
    
    bb->instructions.push_back(std::make_unique<AluInst>(
        LocalId{"%res"}, AluOp::Add, LocalId{"%a"}, LocalId{"%b"}
    ));
    
    bb->terminator = std::make_unique<RetTerm>(LocalId{"%res"});
    
    func.blocks.push_back(std::move(bb));
    
    GenericSerializer serializer;
    std::vector<uint32_t> genericIDs = {100};
    std::vector<uint32_t> constraintIDs = {200};
    
    // Serialize package
    auto pkgBytes = serializer.serializePackage(17, genericIDs, constraintIDs, 50, func);
    
    // Manually construct the whole section
    BinaryWriter sectionWriter;
    sectionWriter.writeU32(1); // Version 1
    sectionWriter.writeU32(1); // Function Count
    
    // Index Table
    GenericFunctionIndex idx;
    idx.functionID = 17;
    idx.compressedSize = pkgBytes.size();
    idx.uncompressedSize = pkgBytes.size();
    // Calculate offset: Version(4) + Count(4) + IndexEntry(48)
    idx.offset = 4 + 4 + sizeof(GenericFunctionIndex);
    idx.mvirHash = 0xDEADBEEF12345678ULL;
    idx.constraintHash = 0;
    idx.signatureHash = 0;
    
    sectionWriter.writeStruct(idx);
    
    // Payload
    sectionWriter.writeBytes(pkgBytes.data(), pkgBytes.size());
    
    auto sectionBytes = sectionWriter.takeBuffer();
    
    // Test Deserialization & Lazy Loading
    GenericDeserializer deserializer(sectionBytes.data(), sectionBytes.size());
    deserializer.parseIndexTable();
    
    // Try loading a non-existent function
    auto nullFunc = deserializer.loadFunction(99);
    assert(nullFunc == nullptr);
    
    // Load the correct function
    auto loadedFunc = deserializer.loadFunction(17);
    assert(loadedFunc != nullptr);
    assert(loadedFunc->blocks.size() == 1);
    assert(loadedFunc->blocks[0]->label.name == "bb0");
    assert(loadedFunc->blocks[0]->instructions.size() == 1);
    
    assert(loadedFunc->blocks[0]->instructions[0]->getOpcode() == Opcode::Alu);
    assert(loadedFunc->blocks[0]->terminator->getOpcode() == Opcode::Ret);
}

void testObjectCodeSection() {
    // Mock object blobs for 3 functions
    const uint8_t blob1[] = {0x48, 0x89, 0xC3, 0x90};        // func 1
    const uint8_t blob2[] = {0x55, 0x48, 0x8B, 0xEC, 0xC3};  // func 2
    const uint8_t blob3[] = {0xC3};                            // func 3

    ObjectCodeBuilder builder;
    builder.addFunction(10, blob1, sizeof(blob1));
    builder.addFunction(20, blob2, sizeof(blob2));
    builder.addFunction(30, blob3, sizeof(blob3));

    assert(builder.getFunctionCount() == 3);

    BinaryWriter writer;
    builder.serialize(writer);
    auto sectionBytes = writer.takeBuffer();

    ObjectCodeExtractor extractor(sectionBytes.data(), sectionBytes.size());
    extractor.parseIndexTable();

    assert(extractor.getFunctionCount() == 3);

    // Test: extracting non-existent function
    auto missing = extractor.extractFunction(99);
    assert(!missing.has_value());

    // Test: Lazy loading — extract only function 20
    auto func2 = extractor.extractFunction(20);
    assert(func2.has_value());
    assert(func2->size() == sizeof(blob2));
    assert(std::memcmp(func2->data(), blob2, sizeof(blob2)) == 0);

    // Test: function 10 and 30 untouched, extract correctly
    auto func1 = extractor.extractFunction(10);
    assert(func1.has_value());
    assert(std::memcmp(func1->data(), blob1, sizeof(blob1)) == 0);

    auto func3 = extractor.extractFunction(30);
    assert(func3.has_value());
    assert(func3->data()[0] == 0xC3);

    // Test: getHash is consistent
    auto hash20a = extractor.getHash(20);
    assert(hash20a.has_value());

    // Test: extractAll() returns concatenated blobs in order
    auto all = extractor.extractAll();
    assert(all.size() == sizeof(blob1) + sizeof(blob2) + sizeof(blob3));
    assert(std::memcmp(all.data(), blob1, sizeof(blob1)) == 0);
    assert(std::memcmp(all.data() + sizeof(blob1), blob2, sizeof(blob2)) == 0);
    assert(all.back() == 0xC3);
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

    testGenericSectionIndex();
    std::cout << "testGenericSectionIndex passed\n";

    testObjectCodeSection();
    std::cout << "testObjectCodeSection passed\n";

    std::cout << "All MLib Phase M2, M3 & M4 tests passed!\n";
    return 0;
}
