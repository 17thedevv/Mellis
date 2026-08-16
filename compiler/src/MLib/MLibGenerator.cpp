// =============================================================================
// mellis/MLib/MLibGenerator.cpp
// =============================================================================

#include "mellis/MLib/MLibGenerator.h"
#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/BinaryWriter.h"
#include "mellis/MLib/StringTableBuilder.h"
#include "mellis/MLib/ManifestBuilder.h"
#include "mellis/MLib/MetadataBuilder.h"
#include "mellis/MLib/MacroMetadataBuilder.h"
#include "mellis/MLib/GenericMetadataBuilder.h"
#include "mellis/MLib/ObjectCodeBuilder.h"
#include "mellis/MLib/MLibGenerator.h"
#include "mellis/MLib/MLibFormat.h"
#include "mellis/MLib/SemanticFingerprint.h"
#include "mellis/AST/DeclNode.h"

#include <llvm/MC/TargetRegistry.h>
#include <llvm/Support/TargetSelect.h>
#include <llvm/Target/TargetMachine.h>
#include <llvm/Target/TargetOptions.h>
#include <llvm/IR/LegacyPassManager.h>
#include <llvm/Support/raw_ostream.h>
#include <llvm/ADT/SmallVector.h>
#include <llvm/TargetParser/Host.h>
#include <llvm/Passes/PassBuilder.h>
#include <llvm/Support/FileSystem.h>
#include <llvm/Bitcode/BitcodeWriter.h>

#include <fstream>
#include <iostream>
#include <algorithm>

namespace fl {

MLibGenerator::MLibGenerator(DiagnosticEngine& diag, const SemanticSnapshot& snapshot, MacroRegistry& macroReg, std::string_view sourceCode)
    : diag_(diag), snapshot_(snapshot), macroReg_(macroReg), sourceCode_(sourceCode) {}

bool MLibGenerator::generate(llvm::Module* llvmModule, const std::string& outputPath) {
    using namespace fl::mlib;

    // 1. Generate Object Code (.obj) in memory
    llvm::InitializeNativeTarget();
    llvm::InitializeNativeTargetAsmParser();
    llvm::InitializeNativeTargetAsmPrinter();

    auto targetTriple = llvm::sys::getDefaultTargetTriple();
    llvmModule->setTargetTriple(targetTriple);

    std::string error;
    auto target = llvm::TargetRegistry::lookupTarget(targetTriple, error);
    if (!target) {
        diag_.error(SourceLocation{}, "MLibGenerator: Không thể tìm thấy Target: " + error);
        return false;
    }

    auto cpu = "generic";
    auto features = "";
    llvm::TargetOptions opt;
    auto rm = std::optional<llvm::Reloc::Model>();
    auto targetMachine = target->createTargetMachine(targetTriple, cpu, features, opt, rm);

    llvmModule->setDataLayout(targetMachine->createDataLayout());

    // Run Optimization
    llvm::LoopAnalysisManager LAM;
    llvm::FunctionAnalysisManager FAM;
    llvm::CGSCCAnalysisManager CGAM;
    llvm::ModuleAnalysisManager MAM;
    llvm::PassBuilder PB(targetMachine);
    PB.registerModuleAnalyses(MAM);
    PB.registerCGSCCAnalyses(CGAM);
    PB.registerFunctionAnalyses(FAM);
    PB.registerLoopAnalyses(LAM);
    PB.crossRegisterProxies(LAM, FAM, CGAM, MAM);
    llvm::ModulePassManager MPM = PB.buildPerModuleDefaultPipeline(llvm::OptimizationLevel::O1);
    MPM.run(*llvmModule, MAM);

    // Emit object code to buffer
    llvm::SmallVector<char, 0> objBuffer;
    llvm::raw_svector_ostream objStream(objBuffer);
    llvm::legacy::PassManager pass;
    if (targetMachine->addPassesToEmitFile(pass, objStream, nullptr, llvm::CodeGenFileType::ObjectFile)) {
        diag_.error(SourceLocation{}, "TargetMachine không hỗ trợ phát sinh object file.");
        return false;
    }
    pass.run(*llvmModule);

    // 2. Build MLib Sections
    
    // a. String Table
    StringTableBuilder strings;
    
    // Get module name from filename
    std::string moduleName = outputPath;
    size_t lastSlash = moduleName.find_last_of("/\\");
    if (lastSlash != std::string::npos) moduleName = moduleName.substr(lastSlash + 1);
    size_t lastDot = moduleName.find_last_of('.');
    if (lastDot != std::string::npos) moduleName = moduleName.substr(0, lastDot);
    
    // Type/Function Metadata
    MetadataBuilder metadataBuilder(strings);
    metadataBuilder.buildFromSnapshot(snapshot_);

    // Manifest
    ManifestBuilder manifest(strings);
    manifest.setPackageName(moduleName);
    manifest.setVersion("0.1.0");

    // Finalize strings AFTER all builders have registered their strings
    strings.finalize();

    BinaryWriter manifestWriter;
    manifest.serialize(manifestWriter);
    auto manifestBytes = manifestWriter.takeBuffer();

    // ExportTable (Empty for now)
    BinaryWriter exportWriter;
    exportWriter.writeU32(1); // version
    exportWriter.writeU32(0); // count
    auto exportBytes = exportWriter.takeBuffer();

    // TypeMetadata (Empty for now)
    BinaryWriter typeWriter;
    typeWriter.writeU32(1); // version
    typeWriter.writeU32(0); // count
    auto typeBytes = typeWriter.takeBuffer();

    // TraitMetadata (Empty for now)
    BinaryWriter traitWriter;
    traitWriter.writeU32(1); // version
    traitWriter.writeU32(0); // count
    auto traitBytes = traitWriter.takeBuffer();
    
    // StringTable serialization
    BinaryWriter strWriter;
    strings.serialize(strWriter);
    auto strBytes = strWriter.takeBuffer();

    // Macro metadata (Empty for now)
    MacroMetadataBuilder macroBuilder;
    BinaryWriter macroWriter;
    macroBuilder.serialize(macroWriter);
    auto macroBytes = macroWriter.takeBuffer();

    BinaryWriter metadataWriter;
    metadataBuilder.serializeMetadata(metadataWriter);
    auto metadataBytes = metadataWriter.takeBuffer();

    BinaryWriter implWriter;
    metadataBuilder.serializeImpls(implWriter);
    auto implBytes = implWriter.takeBuffer();

    // Generic metadata
    GenericMetadataBuilder genericBuilder;
    
    std::vector<const Symbol*> symbols;
    for (uint32_t i = 0; i < snapshot_.getSymbolTable().symbolCount(); ++i) {
        symbols.push_back(&snapshot_.getSymbolTable().getSymbol(i));
    }
    std::sort(symbols.begin(), symbols.end(), [](const Symbol* a, const Symbol* b) {
        return a->name.view() < b->name.view();
    });

    for (const Symbol* symPtr : symbols) {
        const auto& sym = *symPtr;
        if (sym.isExternal || !sym.decl) continue;

        bool isGeneric = false;
        fl::mlib::GenericKind gkind;
        if (auto* fd = dynamic_cast<const FunctionDeclNode*>(sym.decl)) {
            if (!fd->genericParams.empty()) { isGeneric = true; gkind = fl::mlib::GenericKind::Function; }
        } else if (auto* sd = dynamic_cast<const StructDeclNode*>(sym.decl)) {
            if (!sd->genericParams.empty()) { isGeneric = true; gkind = fl::mlib::GenericKind::Struct; }
        } else if (auto* ed = dynamic_cast<const EnumDeclNode*>(sym.decl)) {
            if (!ed->genericParams.empty()) { isGeneric = true; gkind = fl::mlib::GenericKind::Enum; }
        } else if (auto* td = dynamic_cast<const TraitDeclNode*>(sym.decl)) {
            // ALWAYS serialize traits as source so their methods are available!
            isGeneric = true; gkind = fl::mlib::GenericKind::Trait;
        } else if (auto* ta = dynamic_cast<const TypeAliasDeclNode*>(sym.decl)) {
            if (!ta->genericParams.empty()) { isGeneric = true; gkind = fl::mlib::GenericKind::TypeAlias; }
        }

        if (isGeneric) {
            uint32_t start = sym.decl->loc.offset;
            uint32_t end = sym.decl->endLoc.offset;
            std::cout << "[MLibGen] Found generic symbol: " << sym.name.view() << " start=" << start << " end=" << end << "\n";
            if (end > start && end <= sourceCode_.size()) {
                std::string rawSource(sourceCode_.substr(start, end - start));
                if (sym.visibility == Visibility::Public) {
                    rawSource = "export " + rawSource;
                }
                std::cout << "[MLibGen] Serializing source:\n" << rawSource << "\n";
                genericBuilder.addGeneric(gkind, std::string(sym.name.view()), rawSource);
            }
        }
    }
    BinaryWriter genericWriter;
    genericBuilder.serialize(genericWriter);
    auto genericBytes = genericWriter.takeBuffer();

    // Object Code
    ObjectCodeBuilder objBuilder;
    objBuilder.addFunction(0, reinterpret_cast<const uint8_t*>(objBuffer.data()), objBuffer.size());
    BinaryWriter objWriter;
    objBuilder.serialize(objWriter);
    auto finalObjBytes = objWriter.takeBuffer();

    // ── Assemble the .mlib binary ─────────────────────────────────────────────
    const uint32_t NUM_SECTIONS = 8;
    const uint64_t headerSize   = sizeof(MLibHeader);
    const uint64_t tableSize    = NUM_SECTIONS * sizeof(SectionEntry);
    
    uint64_t offset = headerSize + tableSize;
    
    auto getNextOffset = [&offset](size_t size) {
        uint64_t start = offset;
        offset += size;
        return start;
    };

    uint64_t manifestOffset = getNextOffset(manifestBytes.size());
    uint64_t strOffset      = getNextOffset(strBytes.size());
    uint64_t exportOffset   = getNextOffset(exportBytes.size());
    uint64_t typeOffset     = getNextOffset(metadataBytes.size());
    uint64_t traitOffset    = getNextOffset(implBytes.size());
    uint64_t macroOffset    = getNextOffset(macroBytes.size());
    uint64_t genericOffset  = getNextOffset(genericBytes.size());
    uint64_t objOffset      = getNextOffset(finalObjBytes.size());

    MLibHeader hdr{};
    std::memcpy(hdr.magic, MLIB_MAGIC, 4);
    hdr.formatVersion   = 1;
    hdr.compilerVersion = 1;
    hdr.mvirVersion     = 1;
    
    // UUID (Deterministic based on semantic fingerprint)
    std::string fingerprint = SemanticFingerprint::compute(snapshot_);
    std::memset(hdr.moduleUUID, 0, 16);
    std::memcpy(hdr.moduleUUID, fingerprint.data(), std::min(size_t(16), fingerprint.size()));
    
    // Timestamp (Deterministic/Zero for reproducible builds)
    hdr.timestamp = 0; 
    
    hdr.sectionCount       = NUM_SECTIONS;
    hdr.sectionTableOffset = headerSize;

    auto makeSection = [](uint32_t id, SectionType type, uint64_t off, uint64_t sz) {
        SectionEntry e{};
        e.sectionID   = id;
        e.sectionType = static_cast<uint32_t>(type);
        e.offset      = off;
        e.size        = sz;
        e.version     = 1;
        return e;
    };

    SectionEntry table[NUM_SECTIONS] = {
        makeSection(1, SectionType::Manifest,      manifestOffset, manifestBytes.size()),
        makeSection(2, SectionType::StringTable,   strOffset,      strBytes.size()),
        makeSection(3, SectionType::ExportTable,   exportOffset,   exportBytes.size()),
        makeSection(4, SectionType::TypeMetadata,  typeOffset,     metadataBytes.size()),
        makeSection(5, SectionType::TraitMetadata, traitOffset,    implBytes.size()),
        makeSection(6, SectionType::MacroMetadata, macroOffset,    macroBytes.size()),
        makeSection(7, SectionType::GenericMetadata, genericOffset, genericBytes.size()),
        makeSection(8, SectionType::ObjectCode,    objOffset,      finalObjBytes.size()),
    };

    std::ofstream out(outputPath, std::ios::binary);
    if (!out) {
        diag_.error(SourceLocation{}, "Khong the ghi file .mlib: " + outputPath);
        return false;
    }

    out.write(reinterpret_cast<const char*>(&hdr), headerSize);
    out.write(reinterpret_cast<const char*>(table), tableSize);
    out.write(reinterpret_cast<const char*>(manifestBytes.data()), manifestBytes.size());
    out.write(reinterpret_cast<const char*>(strBytes.data()), strBytes.size());
    out.write(reinterpret_cast<const char*>(exportBytes.data()), exportBytes.size());
    out.write(reinterpret_cast<const char*>(metadataBytes.data()), metadataBytes.size());
    out.write(reinterpret_cast<const char*>(implBytes.data()), implBytes.size());
    out.write(reinterpret_cast<const char*>(macroBytes.data()), macroBytes.size());
    out.write(reinterpret_cast<const char*>(genericBytes.data()), genericBytes.size());
    out.write(reinterpret_cast<const char*>(finalObjBytes.data()), finalObjBytes.size());

    return true;
}

} // namespace fl
