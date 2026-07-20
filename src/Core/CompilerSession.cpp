#include "mellis/Core/CompilerSession.h"
#include <iostream>
#include <cassert>
#include "mellis/Core/SourceLocation.h"
#include "mellis/FrontEnd/Lexer.h"
#include "mellis/FrontEnd/Parser.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/FrontEnd/MacroCollector.h"
#include "mellis/FrontEnd/ImportResolver.h"
#include "mellis/FrontEnd/MacroResolver.h"
#include "mellis/FrontEnd/MacroValidator.h"
#include "mellis/FrontEnd/MacroExpander.h"
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/MiddleEnd/MatchAnalyzer.h"
#include "mellis/MiddleEnd/MVIRGenerator.h"
#include "mellis/MiddleEnd/MonomorphizationEngine.h"
#include "mellis/MiddleEnd/MVIROptimizer.h"
#include "mellis/MiddleEnd/BorrowChecker.h"
#include "mellis/BackEnd/LLVMIRGenerator.h"
#include "mellis/BackEnd/ExecutableGenerator.h"
#include "mellis/Support/LLDLinker.h"
#include "mellis/AST/ProgramNode.h"

namespace fl {

CompilerSession::CompilerSession() : sourceManager_(diag_) { diag_.setSourceManager(&sourceManager_); }
CompilerSession::~CompilerSession() = default;

bool CompilerSession::compile(const std::string& filepath, bool verbose, int optLevel) {
    FileID mainFileId = sourceManager_.loadFile(filepath);
    if (mainFileId == SourceManager::kInvalidFileID) {
        diag_.flush();
        return false;
    }

    if (verbose) {
        std::cout << "Compiling: " << filepath << std::endl;
        std::cout << "-----------------------------------" << std::endl;
    }

    // ── Phase 1 & 2: Lexer and Parser ─────────────────────────────────────────
    if (verbose) std::cout << "[1] Phan tich cu phap (Parser)..." << std::endl;
    std::string_view sourceCode = sourceManager_.getSource(mainFileId);
    Lexer lexer(sourceCode);
    Parser parser(lexer, diag_, &sourceManager_, mainFileId);
    auto ast = parser.parse();

    if (diag_.errorCount() > 0 || !ast) {
        diag_.error(SourceLocation::invalid(), "Parsing that bai.");
        diag_.flush();
        return false;
    }
    if (verbose) {
        std::cout << "[2] Parse thanh cong! ("
                  << ast->items.size() << " cau lenh top-level)" << std::endl;
    }

    // ── Phase 1.2: Macro Registry & Collector ────────────────────
    if (verbose) std::cout << "[1.2] Thu thap Macro..." << std::endl;
    MacroRegistry macroRegistry(diag_);
    MacroCollector macroCollector(macroRegistry, diag_);
    if (ast) {
        macroCollector.collect(*ast);
    }
    
    // ── Phase 1.3: Import & Macro Resolution ─────────────────────
    if (verbose) std::cout << "[1.3] Phan giai Import & Macro..." << std::endl;
    ImportResolver importResolver(diag_);
    MacroResolver macroResolver(macroRegistry, diag_);
    if (ast) {
        auto* prog = dynamic_cast<ProgramNode*>(ast.get());
        if (prog) {
            importResolver.resolve(*prog);
            macroResolver.resolve(*prog);
        }
    }
    
    // ── Phase 1.45: Macro Validation ─────────────────────────────
    if (verbose) std::cout << "[1.45] Kiem tra Macro..." << std::endl;
    MacroValidator macroValidator(macroRegistry, diag_);
    if (ast) {
        macroValidator.validate(*ast);
    }
    
    // ── Phase 1.4: Macro Expansion ──────────────────────────────
    if (verbose) std::cout << "[1.4] Khai trien Macro..." << std::endl;
    MacroExpander macroExpander(macroRegistry, diag_);
    if (ast) {
        ast = std::unique_ptr<ProgramNode>(static_cast<ProgramNode*>(macroExpander.transformNode(std::move(ast)).release()));
    }

    // ── Phase 3: Phân giải (Resolver) ────────────────────────────────
    if (verbose) std::cout << "[2] Phan giai ky hieu (Resolver)..." << std::endl;
    Resolver resolver(symbolTable_, diag_);
    bool resolveOk = resolver.resolve(ast.get());

    if (!resolveOk) {
        diag_.error(SourceLocation::invalid(), "Resolver that bai, nhung tiep tuc de test MVIR...");
        diag_.flush();
        // return false;
    }

    if (verbose) std::cout << "[4] Resolver thanh cong!" << std::endl;

    // ── Phase 4: TypeChecker ──────────────────────────────────────────────────
    if (verbose) std::cout << "[5] Kiem tra kieu du lieu (TypeChecker)..." << std::endl;
    TypeChecker typeChecker(symbolTable_, diag_, typeContext_);
    MonomorphizationEngine monoEngine(symbolTable_, resolver, typeChecker, diag_);
    typeChecker.setMonomorphizationEngine(&monoEngine);
    bool tcOk = typeChecker.check(ast.get());

    if (!tcOk) {
        diag_.error(SourceLocation::invalid(), "TypeChecker that bai, nhung tiep tuc de test MVIR...");
        diag_.flush();
        // return false;
    }
    if (verbose) std::cout << "[6] Type Checker thanh cong!" << std::endl;

    auto* prog = dynamic_cast<ProgramNode*>(ast.get());
    assert(prog);

    // -- Phase 4.5: Inject Specialized ASTs --
    if (verbose) std::cout << "[6.05] Tiem cac ham/struct/enum chuyen biet hoa vao AST..." << std::endl;
    while (true) {
        auto specializedASTs = monoEngine.takeSpecializedASTs();
        if (specializedASTs.empty()) break;
        for (auto& ast : specializedASTs) {
            prog->items.push_back(std::move(ast));
        }
    }

    // ── Phase 5: Match Analyzer ──────────────────────────────────────────────
    if (verbose) std::cout << "[6.1] Phan tich Pattern Matching (MatchAnalyzer)..." << std::endl;
    MatchAnalyzer matchAnalyzer(symbolTable_, diag_);
    bool matchOk = matchAnalyzer.analyze(ast.get());
    if (!matchOk) {
        diag_.error(SourceLocation::invalid(), "MatchAnalyzer that bai, nhung tiep tuc de test MVIR...");
        diag_.flush();
        // return false;
    }
    if (verbose) std::cout << "[6.2] MatchAnalyzer thanh cong!" << std::endl;

    // ── Phase 6: MVIR Generation ──────────────────────────────────────────────
    if (verbose) std::cout << "[7] Sinh MVIR (MVIRGenerator)..." << std::endl;
    MVIRGenerator mvirGen(symbolTable_, typeChecker);
    auto mvirModule = mvirGen.generate(*prog);
    if (!mvirModule) {
        diag_.error(SourceLocation::invalid(), "Khong the sinh MVIR.");
        diag_.flush();
        return false;
    }
    if (verbose) {
        std::cout << "[8] Sinh MVIR thanh cong!\n";
        std::cout << "\n=== MVIR ===\n";
        std::cout << mvirModule->toString() << "\n";
    }

    // ── Phase 6.5: MVIR Optimization ──────────────────────────────────────────
    if (optLevel >= 1) {
        if (verbose) std::cout << "[8.05] Toi uu hoa MVIR (MVIROptimizer -O1)..." << std::endl;
        MVIROptimizer mvirOpt(diag_);
        bool optimized = mvirOpt.optimize(*mvirModule);
        if (verbose) {
            if (optimized) {
                std::cout << "[8.06] MVIR sau khi toi uu:\n";
                std::cout << mvirModule->toString() << "\n";
            } else {
                std::cout << "[8.06] Khong co toi uu nao duoc thuc hien.\n";
            }
        }
    }

    // ── Phase 7: Borrow Checker ──────────────────────────────────────────────
    if (verbose) std::cout << "[8.1] Kiem tra muon tham chieu (BorrowChecker)..." << std::endl;
    BorrowChecker borrowChecker(mvirModule.get(), diag_);
    bool borrowOk = borrowChecker.check();
    if (!borrowOk) {
        diag_.error(SourceLocation::invalid(), "BorrowChecker that bai.");
        diag_.flush();
        return false;
    }
    if (verbose) std::cout << "[8.2] BorrowChecker thanh cong!" << std::endl;

    // ── Phase 8: LLVM IR Generation ──────────────────────────────────────────
    llvm::LLVMContext llvmContext;
    llvm::Module llvmModule(filepath, llvmContext);
    
    LLVMIRGenerator llvmGen(llvmContext, llvmModule, symbolTable_);
    bool llvmOk = llvmGen.generate(mvirModule.get());
    // llvmModule.print(llvm::errs(), nullptr);
    
    if (!llvmOk) {
        diag_.error(SourceLocation::invalid(), "Loi trong qua trinh sinh LLVM IR.");
        diag_.flush();
        return false;
    }
    
    if (verbose) {
        std::cout << "\n=== LLVM IR ===\n";
        llvmModule.print(llvm::outs(), nullptr);
        llvm::outs().flush();
    }

    // ── Phase 9: LLD Linker / Executable Generation ──────────────────────────
    if (verbose) std::cout << "[9] Tao thuc thi (ExecutableGenerator)..." << std::endl;
    LLDLinker linker(diag_);
    ExecutableGenerator exeGen(diag_, linker);
    
    std::string outName = filepath;
    size_t lastDot = outName.find_last_of('.');
    if (lastDot != std::string::npos) {
        outName = outName.substr(0, lastDot);
    }
    outName += ".exe";

    bool exeOk = exeGen.generateExecutable(&llvmModule, outName);
    if (!exeOk) {
        diag_.error(SourceLocation::invalid(), "Tao file thuc thi that bai.");
        diag_.flush();
        return false;
    }

    if (verbose) std::cout << "[10] Thanh cong! File dau ra: " << outName << std::endl;

    return true;
}

} // namespace fl
