#include <iostream>
#include <fstream>
#include <string>
#include <cstdlib>
#include <cassert>
#include "fdlang/Core/SourceLocation.h"
#include "fdlang/FrontEnd/Lexer.h"
#include "fdlang/FrontEnd/Parser.h"
#include "fdlang/MiddleEnd/SymbolTable.h"
#include "fdlang/MiddleEnd/Resolver.h"
#include "fdlang/MiddleEnd/TypeChecker.h"
#include "fdlang/MiddleEnd/MatchAnalyzer.h"
#include "fdlang/MiddleEnd/FLIRGenerator.h"
#include "fdlang/MiddleEnd/BorrowChecker.h"
#include "fdlang/BackEnd/LLVMIRGenerator.h"
#include "fdlang/BackEnd/ExecutableGenerator.h"
#include "fdlang/Support/LLDLinker.h"
#include "fdlang/Support/Diagnostic.h"

using namespace fl;

std::string readFile(const std::string& filepath, DiagnosticEngine& diag) {
    std::ifstream file(filepath, std::ios::binary | std::ios::ate);
    if (!file.is_open()) {
        diag.error(SourceLocation::invalid(), "Khong the mo file '" + filepath + "'");
        return "";
    }
    std::streamsize size = file.tellg();
    std::string buffer(size, '\0');
    file.seekg(0, std::ios::beg);
    if (file.read(buffer.data(), size)) return buffer;
    return "";
}

int main(int argc, char* argv[]) {
    std::cout << "[FDLANG COMPILER - RESOLVER PHASE]" << std::endl;
    DiagnosticEngine diag;
    if (argc < 2) {
        std::cerr << "fdlang: error: Cach su dung: fdlang <file_path> [-v|--verbose]\n";
        return 1;
    }

    std::string filepath = argv[1];
    bool verbose = false;
    for (int i = 2; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "-v" || arg == "--verbose") {
            verbose = true;
        }
    }

    std::string sourceCode = readFile(filepath, diag);
    if (sourceCode.empty() && diag.errorCount() > 0) {
        diag.flush();
        return 65; // data format error
    }

    std::cout << "Compiling: " << filepath << std::endl;
    std::cout << "-----------------------------------" << std::endl;

    // ── Phase 1 & 2: Lexer and Parser ─────────────────────────────────────────
    std::cout << "[1] Phan tich cu phap (Parser)..." << std::endl;
    Lexer lexer(sourceCode);
    Parser parser(lexer, diag);
    auto ast = parser.parse();

    if (diag.errorCount() > 0 || !ast) {
        diag.error(SourceLocation::invalid(), "Parsing that bai.");
        diag.flush();
        return 65;
    }
    std::cout << "[2] Parse thanh cong! ("
              << ast->items.size() << " cau lenh)" << std::endl;

    // ── Phase 3: Resolver ─────────────────────────────────────────────────────
    std::cout << "[3] Giai quyet ten (Resolver)..." << std::endl;
    SymbolTable symbolTable;
    Resolver resolver(symbolTable, diag);
    bool resolveOk = resolver.resolve(ast.get());

    if (!resolveOk) {
        diag.error(SourceLocation::invalid(), "Resolver that bai.");
        diag.flush();
        return 65;
    }

    std::cout << "[4] Resolver thanh cong!" << std::endl;

    // ── Phase 4: TypeChecker ──────────────────────────────────────────────────
    std::cout << "[5] Kiem tra kieu du lieu (TypeChecker)..." << std::endl;
    TypeContext typeContext;
    TypeChecker typeChecker(symbolTable, diag, typeContext);
    bool tcOk = typeChecker.check(ast.get());

    if (!tcOk) {
        diag.error(SourceLocation::invalid(), "TypeChecker that bai.");
        diag.flush();
        return 65;
    }
    std::cout << "[6] Type Checker thanh cong!" << std::endl;

    // ── Phase 5: Match Analyzer ──────────────────────────────────────────────
    std::cout << "[6.1] Phan tich Pattern Matching (MatchAnalyzer)..." << std::endl;
    MatchAnalyzer matchAnalyzer(symbolTable, diag);
    bool matchOk = matchAnalyzer.analyze(ast.get());
    if (!matchOk) {
        diag.error(SourceLocation::invalid(), "MatchAnalyzer that bai.");
        diag.flush();
        return 65;
    }
    std::cout << "[6.2] MatchAnalyzer thanh cong!" << std::endl;

    // ── Phase 6: FLIR Generation ──────────────────────────────────────────────
    std::cout << "[7] Sinh FLIR (FLIRGenerator)..." << std::endl;
    FLIRGenerator flirGen(symbolTable, typeChecker);
    auto* prog = dynamic_cast<ProgramNode*>(ast.get());
    assert(prog);
    auto flirModule = flirGen.generate(*prog);
    if (!flirModule) {
        diag.error(SourceLocation::invalid(), "Khong the sinh FLIR.");
        diag.flush();
        return 65;
    }
    std::cout << "[8] Sinh FLIR thanh cong!" << std::endl;

    // ── Phase 7: Borrow Checker ──────────────────────────────────────────────
    std::cout << "[8.1] Kiem tra muon tham chieu (BorrowChecker)..." << std::endl;
    BorrowChecker borrowChecker(flirModule.get(), diag);
    bool borrowOk = borrowChecker.check();
    if (!borrowOk) {
        diag.error(SourceLocation::invalid(), "BorrowChecker that bai.");
        diag.flush();
        return 65;
    }
    std::cout << "[8.2] BorrowChecker thanh cong!" << std::endl;

    // ── Phase 8: LLVM IR Generation ──────────────────────────────────────────
    llvm::LLVMContext llvmContext;
    llvm::Module llvmModule(filepath, llvmContext);
    
    LLVMIRGenerator llvmGen(llvmContext, llvmModule, symbolTable);
    bool llvmOk = llvmGen.generate(flirModule.get());
    
    if (!llvmOk) {
        diag.error(SourceLocation::invalid(), "Loi trong qua trinh sinh LLVM IR.");
        diag.flush();
        return 65;
    }
    
    if (verbose) {
        std::cout << "\n=== LLVM IR ===\n";
        llvmModule.print(llvm::outs(), nullptr);
        llvm::outs().flush();
    }

    // ── Phase 9: LLD Linker / Executable Generation ──────────────────────────
    std::cout << "[9] Tao thuc thi (ExecutableGenerator)..." << std::endl;
    LLDLinker linker(diag);
    ExecutableGenerator exeGen(diag, linker);
    
    std::string outName = filepath;
    size_t lastDot = outName.find_last_of('.');
    if (lastDot != std::string::npos) {
        outName = outName.substr(0, lastDot);
    }
    outName += ".exe";

    bool exeOk = exeGen.generateExecutable(&llvmModule, outName);
    if (!exeOk) {
        diag.error(SourceLocation::invalid(), "Tao file thuc thi that bai.");
        diag.flush();
        return 65;
    }

    std::cout << "[10] Thanh cong! File dau ra: " << outName << std::endl;

    return 0;
}
