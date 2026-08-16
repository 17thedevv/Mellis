#include "mellis/Core/CompilerSession.h"
#include "mellis/MiddleEnd/SemanticSnapshot.h"
#include "mellis/MLib/MLibGenerator.h"
#include "mellis/Support/OSUtils.h"
#include <iostream>
#include <cassert>
#include <filesystem>
#include "mellis/Core/SourceLocation.h"
#include "mellis/FrontEnd/Lexer.h"
#include "mellis/FrontEnd/Parser.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/FrontEnd/MacroCollector.h"
#include "mellis/FrontEnd/ImportResolver.h"
#include "mellis/FrontEnd/MacroResolver.h"
#include "mellis/FrontEnd/MacroValidator.h"
#include "mellis/FrontEnd/MacroExpander.h"
#include "mellis/FrontEnd/LifetimeElider.h"
#include "mellis/MLib/ModuleLoader.h"
#include "mellis/MLib/MLibMetadataCache.h"
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/MiddleEnd/EscapeAnalyzer.h"
#include "mellis/MiddleEnd/MatchAnalyzer.h"
#include "mellis/MiddleEnd/ComptimeEvaluator.h"
#include "mellis/IR/MVIRGenerator.h"
#include "mellis/IR/IRVerifier.h"
#include "mellis/MiddleEnd/SemanticSnapshot.h"
#include "mellis/MiddleEnd/MonomorphizationEngine.h"
#include "mellis/Optimizer/MVIROptimizer.h"
#include "mellis/MiddleEnd/Semantic/SemanticAnalyzer.h"
#include "mellis/BackEnd/LLVMIRGenerator.h"
#include "mellis/BackEnd/ExecutableGenerator.h"
#include "mellis/Support/LLDLinker.h"
#include "mellis/Support/ConsoleDiagnosticConsumer.h"
#include "mellis/Support/OSUtils.h"
#include "mellis/AST/ProgramNode.h"

namespace fl {

CompilerSession::CompilerSession() : sourceManager_(diag_) {
    diag_.setSourceManager(&sourceManager_);
    diag_.addConsumer(std::make_shared<ConsoleDiagnosticConsumer>(&sourceManager_));
    initDefaultLibraryPaths();
}
CompilerSession::~CompilerSession() = default;

// Populate libraryPaths_ with well-known locations.
// Search order (first match wins at load time):
//   1. <compiler_exe_dir>/lib/      — installed alongside the binary
//   2. <compiler_exe_dir>/../lib/   — common install layout
//   3. ./lib/                       — current working directory (dev convenience)
// Users can prepend extra paths via setLibraryPaths().
void CompilerSession::initDefaultLibraryPaths() {
    namespace fs = std::filesystem;

    // Use current_path() as a portable baseline.
    // When invoked as `mellis.exe` from a shell, CWD is typically the project root
    // or the directory the user is in. We search lib/ relative to CWD and relative
    // to a sibling path for installed layouts.
    //
    // Candidate paths in priority order:
    //   1. ./lib/           — cạnh CWD (dev: run từ build/Debug/)
    //   2. ../lib/          — một tầng lên (dev: run từ trong subdir)
    std::error_code ec;
    fs::path cwd = fs::current_path(ec);

    // Get executable path
    std::string exePathStr = OSUtils::getExecutablePath();
    fs::path exeDir = fs::path(exePathStr).parent_path();

    std::vector<fs::path> candidates;
    if (!ec) {
        candidates.push_back(cwd);                // ./ (Current working directory)
        candidates.push_back(cwd / "lib");        // ./lib/ (Project local)
        candidates.push_back(cwd / "../lib");     // ../lib/
    }
    
    if (!exePathStr.empty()) {
        candidates.push_back(exeDir / "lib");             // <exe_dir>/lib/
        candidates.push_back(exeDir / "../lib");          // <exe_dir>/../lib/
    }

    for (auto& p : candidates) {
        std::error_code ec2;
        auto canonical = fs::canonical(p, ec2);
        if (!ec2 && fs::is_directory(canonical, ec2)) {
            // Avoid duplicates
            std::string s = canonical.string();
            bool found = false;
            for (auto& existing : libraryPaths_) {
                if (existing == s) { found = true; break; }
            }
            if (!found) libraryPaths_.push_back(s);
        }
    }
}
bool CompilerSession::compile(const std::string& filepath, bool verbose, int optLevel, bool emitLib) {
    FileID mainFileId = sourceManager_.loadFile(filepath);
    if (mainFileId == SourceManager::kInvalidFileID) {
        diag_.flush();
        return false;
    }

    // Thêm thư mục chứa file main vào danh sách tìm kiếm thư viện
    std::error_code ec;
    auto mainFileDir = std::filesystem::path(filepath).parent_path();
    if (mainFileDir.empty()) {
        mainFileDir = ".";
    }
    auto canonicalMainDir = std::filesystem::canonical(mainFileDir, ec);
    if (!ec) {
        std::string s = canonicalMainDir.string();
        bool found = false;
        for (auto& existing : libraryPaths_) {
            if (existing == s) { found = true; break; }
        }
        if (!found) libraryPaths_.push_back(s);
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
    ModuleLoader moduleLoader(symbolTable_, diag_, &macroRegistry, libraryPaths_);
    ImportResolver importResolver(diag_, symbolTable_, moduleLoader);
    MacroResolver macroResolver(macroRegistry, diag_);
    if (ast) {
        auto* prog = dynamic_cast<ProgramNode*>(ast.get());
        if (prog) {
            importResolver.resolve(*prog);
            
            auto generics = moduleLoader.takeInjectedGenerics();
            for (auto& g : generics) {
                prog->items.push_back(std::move(g));
            }
            macroResolver.resolve(*prog);
        }
    }
    
    // Save loaded mlib paths for linking phase
    loadedMLibs_ = moduleLoader.getLoadedPaths();

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
    
    // ── Phase 1.5: Lifetime Elision ─────────────────────────────
    if (verbose) std::cout << "[1.5] Lifetime Elision..." << std::endl;
    LifetimeElider elider(diag_);
    if (ast) {
        elider.elide(*ast);
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
    std::cerr << "[DEBUG] Resolver finished!" << std::endl;

    // ── Phase 4: TypeChecker ──────────────────────────────────────────────────
    if (verbose) std::cout << "[5] Kiem tra kieu du lieu (TypeChecker)..." << std::endl;
    std::cerr << "[DEBUG] Creating TypeChecker..." << std::endl;
    TypeChecker typeChecker(symbolTable_, diag_, typeContext_);
    std::cerr << "[DEBUG] Creating MonomorphizationEngine..." << std::endl;
    MonomorphizationEngine monoEngine(symbolTable_, resolver, typeChecker, diag_);
    typeChecker.setMonomorphizationEngine(&monoEngine);
    // Attach MLibMetadataCache so TypeChecker can resolve external symbol types.
    std::cerr << "[DEBUG] Creating MLibMetadataCache..." << std::endl;
    MLibMetadataCache metadataCache(typeContext_);
    typeChecker.setMetadataCache(&metadataCache);

    // Register Source-based generic Impl blocks loaded from .mlib
    for (const auto& pair : moduleLoader.getInjectedGenericImpls()) {
        SymbolID targetStructId = pair.first;
        ImplDeclNode* implNode = pair.second;
        monoEngine.registerGenericImpl(targetStructId, implNode);
    }

    std::cerr << "[DEBUG] Calling TypeChecker::check..." << std::endl;
    bool tcOk = typeChecker.check(ast.get());
    std::cerr << "[PHASE] TypeChecker done, tcOk=" << tcOk << std::endl;

    if (!tcOk) {
        diag_.error(SourceLocation::invalid(), "Compilation aborted due to TypeChecker errors.");
        diag_.flush();
        return false;
    }
    if (verbose) std::cout << "[6] Type Checker thanh cong!" << std::endl;
    std::cerr << "[PHASE] Injecting specialized ASTs..." << std::endl;

    auto* prog = dynamic_cast<ProgramNode*>(ast.get());
    if (!prog) diag_.ice(SourceLocation::invalid(), "Root AST node is not a ProgramNode");

    // -- Phase 4.5: Inject Specialized ASTs --
    if (verbose) std::cout << "[6.05] Tiem cac ham/struct/enum chuyen biet hoa vao AST..." << std::endl;
    while (true) {
        auto specializedASTs = monoEngine.takeSpecializedASTs();
        if (specializedASTs.empty()) break;
        for (auto& ast : specializedASTs) {
            prog->items.push_back(std::move(ast));
        }
    }

    std::cerr << "[PHASE] Specialized ASTs injected" << std::endl;

    // ── Phase 4.75: Comptime Evaluation ──────────────────────────────────────
    if (verbose) std::cout << "[6.08] Evaluate Comptime blocks..." << std::endl;
    ComptimeEvaluator comptimeEval(diag_);
    bool comptimeOk = comptimeEval.evaluateComptimeBlocks(ast.get());
    std::cerr << "[PHASE] ComptimeEval done, ok=" << comptimeOk << std::endl;
    if (!comptimeOk) {
        diag_.error(SourceLocation::invalid(), "Comptime evaluation failed.");
        diag_.flush();
        return false;
    }
    if (verbose) std::cout << "[6.09] Comptime Evaluation thanh cong!" << std::endl;

    // ── Phase 5: Match Analyzer ──────────────────────────────────────────────
    if (verbose) std::cout << "[6.1] Phan tich Pattern Matching (MatchAnalyzer)..." << std::endl;
    MatchAnalyzer matchAnalyzer(symbolTable_, typeChecker, diag_);
    bool matchOk = matchAnalyzer.analyze(ast.get());
    std::cerr << "[PHASE] MatchAnalyzer done, ok=" << matchOk << std::endl;
    if (!matchOk) {
        diag_.error(SourceLocation::invalid(), "Non-exhaustive match or invalid pattern. Aborting compilation.");
        diag_.flush();
        return false;
    }
    if (verbose) std::cout << "[6.2] MatchAnalyzer thanh cong!" << std::endl;

    // ── Phase 5.5: Escape Analysis ──────────────────────────────────────────
    if (verbose) std::cout << "[6.3] Phan tich Escape (EscapeAnalyzer)..." << std::endl;
    EscapeAnalyzer escapeAnalyzer(typeContext_, symbolTable_, diag_, closureStorageMap_);
    escapeAnalyzer.analyze(ast.get());
    if (diag_.hasErrors()) {
        diag_.flush();
        return false;
    }
    
    // ── Phase 6: MVIR Generation ──────────────────────────────────────────────
    if (verbose) std::cout << "[7] Sinh MVIR (MVIRGenerator)..." << std::endl;
    std::cerr << "[PHASE] MVIRGenerator starting" << std::endl;
    MVIRGenerator mvirGen(symbolTable_, diag_, typeChecker, closureStorageMap_);
    auto mvirModule = mvirGen.generate(*prog);
    std::cerr << "[PHASE] MVIRGenerator done" << std::endl;
    if (!mvirModule) {
        diag_.error(SourceLocation::invalid(), "Khong the sinh MVIR.");
        diag_.flush();
        return false;
    }

    if (verbose) std::cout << "[7b] Verifying MVIR invariants..." << std::endl;
    SemanticSnapshot snapshot(typeChecker.getTypeTable(), typeChecker.getContext(), symbolTable_);
    IRVerifier irVerifier(diag_, &snapshot);
    auto verifResult = irVerifier.verify(*mvirModule);
    if (!verifResult.ok) {
        std::cout << "\n=== MVIR (Verification Failed) ===\n";
        std::cout << mvirModule->toString() << "\n";
        diag_.error(SourceLocation::invalid(), "MVIR Verification failed: " + verifResult.error);
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

    // ── Phase 7: Semantic Analysis ───────────────────────────────────────────
    if (verbose) std::cout << "[8.1] Phan tich Ngu nghia (SemanticAnalyzer)..." << std::endl;
    SemanticAnalyzer semanticAnalyzer(mvirModule.get(), diag_, symbolTable_, closureStorageMap_);
    bool semanticOk = semanticAnalyzer.analyze();
    if (!semanticOk) {
        diag_.error(SourceLocation::invalid(), "SemanticAnalyzer that bai.");
        diag_.flush();
        return false;
    }
    if (verbose) std::cout << "[8.2] SemanticAnalyzer thanh cong!" << std::endl;

    // ── Phase 8: LLVM IR Generation ──────────────────────────────────────────
    llvm::LLVMContext llvmContext;
    llvm::Module llvmModule(filepath, llvmContext);
    TraitObjectLayoutBuilder layoutBuilder(symbolTable_);
    LLVMIRGenerator llvmGen(llvmContext, llvmModule, layoutBuilder, closureStorageMap_);
    bool llvmOk = llvmGen.generate(mvirModule.get());
    
    std::cout << "\n=== FORCE DUMP LLVM IR ===\n";
    llvmModule.print(llvm::outs(), nullptr);
    llvm::outs().flush();

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

    if (emitLib) {
        if (verbose) std::cout << "[9] Tao file thu vien (MLibGenerator)..." << std::endl;
        
        std::string outName = filepath;
        size_t lastDot = outName.find_last_of('.');
        if (lastDot != std::string::npos) {
            outName = outName.substr(0, lastDot);
        }
        outName += ".mlib";

        fl::SemanticSnapshot snapshot(typeChecker.getTypeTable(), typeContext_, symbolTable_);
        fl::MLibGenerator mlibGen(diag_, snapshot, macroRegistry, sourceCode);
        bool mlibOk = mlibGen.generate(&llvmModule, outName);
        if (!mlibOk) {
            diag_.error(SourceLocation::invalid(), "Tao file thu vien that bai.");
            diag_.flush();
            return false;
        }

        if (verbose) std::cout << "[10] Thanh cong! File dau ra: " << outName << std::endl;
        return true;
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

    bool exeOk = exeGen.generateExecutable(&llvmModule, outName, loadedMLibs_);
    if (!exeOk) {
        diag_.error(SourceLocation::invalid(), "Tao file thuc thi that bai.");
        diag_.flush();
        return false;
    }

    if (verbose) std::cout << "[10] Thanh cong! File dau ra: " << outName << std::endl;

    return true;
}

} // namespace fl
