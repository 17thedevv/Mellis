// =============================================================================
// mellis/BackEnd/ExecutableGenerator.cpp
// =============================================================================

#include "mellis/BackEnd/ExecutableGenerator.h"
#include "mellis/MLib/ObjectCodeExtractor.h"
#include <llvm/MC/TargetRegistry.h>
#include <llvm/Support/TargetSelect.h>
#include <llvm/Target/TargetMachine.h>
#include <llvm/Target/TargetOptions.h>
#include <llvm/IR/LegacyPassManager.h>
#include <llvm/Support/FileSystem.h>
#include <llvm/Support/raw_ostream.h>
#include <llvm/TargetParser/Host.h>
#include <llvm/Support/CodeGen.h>
#include <llvm/Passes/PassBuilder.h>
#include <llvm/Passes/StandardInstrumentations.h>
#include <system_error>
#include <iostream>
#include <filesystem>
#include <fstream>

namespace fl {

// Helper: Extract Object Code from a single .mlib file
static std::optional<std::string> extractMLibObject(const std::string& mlibPath,
                                                    const std::string& objDir,
                                                    DiagnosticEngine& diag) {
    namespace fs = std::filesystem;
    std::ifstream file(mlibPath, std::ios::binary | std::ios::ate);
    if (!file) {
        diag.error(SourceLocation{}, "Khong the mo file .mlib: " + mlibPath);
        return std::nullopt;
    }
    std::streamsize size = file.tellg();
    file.seekg(0, std::ios::beg);
    std::vector<uint8_t> buffer(size);
    if (!file.read(reinterpret_cast<char*>(buffer.data()), size)) {
        diag.error(SourceLocation{}, "Loi doc file .mlib: " + mlibPath);
        return std::nullopt;
    }

    if (size < sizeof(mlib::MLibHeader)) return std::nullopt;
    const auto* header = reinterpret_cast<const mlib::MLibHeader*>(buffer.data());
    if (header->magic[0] != 'M' || header->magic[1] != 'L' || header->magic[2] != 'I' || header->magic[3] != 'B') {
        return std::nullopt;
    }

    uint64_t currentOffset = header->directoryOffset;
    for (uint32_t i = 0; i < header->sectionCount; ++i) {
        if (currentOffset + sizeof(mlib::SectionHeader) > buffer.size()) break;
        const auto* secHeader = reinterpret_cast<const mlib::SectionHeader*>(buffer.data() + currentOffset);
        if (secHeader->sectionType == static_cast<uint32_t>(mlib::SectionType::ObjectCode)) {
            if (secHeader->offset + secHeader->size <= buffer.size()) {
                mlib::ObjectCodeExtractor extractor(buffer.data() + secHeader->offset, secHeader->size);
                extractor.parseIndexTable();
                auto objBytes = extractor.extractAll();

                fs::path mPath(mlibPath);
                std::string objFileName = mPath.stem().string() + ".o";
                fs::path outPath = fs::path(objDir) / objFileName;

                std::ofstream out(outPath, std::ios::binary);
                if (out.write(reinterpret_cast<const char*>(objBytes.data()), objBytes.size())) {
                    return outPath.string();
                }
            }
        }
        currentOffset += sizeof(mlib::SectionHeader);
    }
    return std::nullopt;
}

ExecutableGenerator::ExecutableGenerator(DiagnosticEngine& diag, Linker& linker)
    : diag_(diag), linker_(linker) {}

bool ExecutableGenerator::generateExecutable(llvm::Module* llvmModule,
                                             const std::string& outputPath,
                                             const std::vector<std::string>& extraMLibs) {

    // 1. Initialize Native Target
    llvm::InitializeNativeTarget();
    llvm::InitializeNativeTargetAsmParser();
    llvm::InitializeNativeTargetAsmPrinter();

    auto targetTriple = llvm::sys::getDefaultTargetTriple();
    llvmModule->setTargetTriple(targetTriple);

    std::string error;
    auto target = llvm::TargetRegistry::lookupTarget(targetTriple, error);

    if (!target) {
        diag_.error(SourceLocation{}, "Khong the tim thay Target cho LLVM: " + error);
        return false;
    }

    auto cpu = "generic";
    auto features = "";

    llvm::TargetOptions opt;
    auto rm = std::optional<llvm::Reloc::Model>();
    auto targetMachine = target->createTargetMachine(targetTriple, cpu, features, opt, rm);

    llvmModule->setDataLayout(targetMachine->createDataLayout());

    // 1.5 Run Optimizations (including Coroutines)
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

    std::cout << "[ExeGen] Building PassManager..." << std::endl;
    llvm::ModulePassManager MPM = PB.buildPerModuleDefaultPipeline(llvm::OptimizationLevel::O3);
    std::cout << "[ExeGen] Running Optimization Passes..." << std::endl;
    MPM.run(*llvmModule, MAM);
    std::cout << "[ExeGen] Optimizations complete." << std::endl;
    llvmModule->print(llvm::outs(), nullptr);
    llvm::outs().flush();

    // 2. Emit Object File
    std::string objPath = outputPath + ".obj";
    std::error_code ec;
    llvm::raw_fd_ostream dest(objPath, ec, llvm::sys::fs::OF_None);

    if (ec) {
        diag_.error(SourceLocation{}, "Khong the mo file de ghi: " + ec.message());
        return false;
    }

    llvm::legacy::PassManager pass;
    auto fileType = llvm::CodeGenFileType::ObjectFile;

    std::cout << "[ExeGen] Adding passes to emit file..." << std::endl;
    if (targetMachine->addPassesToEmitFile(pass, dest, nullptr, fileType)) {
        diag_.error(SourceLocation{}, "TargetMachine khong the emit kieu file nay.");
        return false;
    }

    std::cout << "[ExeGen] Running CodeGen Passes..." << std::endl;
    pass.run(*llvmModule);
    dest.flush();
    std::cout << "[ExeGen] Object file emitted." << std::endl;

    // 3. Link Object File into Executable
    std::vector<std::string> libs = { "build\\Release\\mellis_rt.lib" };
    
    // Extract MLib Object Codes to .fd_obj
    if (!extraMLibs.empty()) {
        std::filesystem::path objDir = std::filesystem::current_path() / ".fd_obj";
        if (!std::filesystem::exists(objDir)) {
            std::filesystem::create_directory(objDir);
        }
        for (const auto& mlibPath : extraMLibs) {
            auto extPath = extractMLibObject(mlibPath, objDir.string(), diag_);
            if (extPath) {
                libs.push_back(*extPath);
                std::cout << "[ExeGen] Extracted object code from " << mlibPath << " -> " << *extPath << std::endl;
            }
        }
    }

    return linker_.link(objPath, outputPath, libs);
}

} // namespace fl
