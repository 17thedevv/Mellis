# FDLang Project Status & Handover

## 1. Project Overview
**FDLang** is a custom compiled language featuring a professional-grade compiler architecture heavily inspired by Rust and C++. The core philosophy is **Strict Separation of Concerns (SoC)** and clean, senior-level code.

## 2. Current Architecture & State
The frontend and middle-end are highly structured and currently passing **100% of tests**.

- **FrontEnd (`Lexer`, `Parser`)**: 
  - Supports struct, enum, trait, impl, match expressions, and standard control flow (`if`, `while`, `for`, `break`, `continue`).
- **Resolver (`Resolver.cpp`)**:
  - Exclusively handles Name Resolution and Path Semantics (e.g., `Color::Red`).
- **Type Checker (`TypeChecker.cpp`)**:
  - Implemented using a constraint-based approach (Hindley-Milner style).
  - Divided strictly into:
    1. **TypePrePass**: Collects type signatures and populates `MethodResolver`.
    2. **ConstraintGenerator**: Only *collects* constraints (does not resolve them).
    3. **UnificationEngine**: Solves constraints (Equality, MethodCall, FunctionCall).
    4. **MethodResolver**: Dedicated structure for resolving method calls based on the receiver type.
- **Match Analyzer (`MatchAnalyzer.cpp`)**:
  - A post-typecheck pass dedicated to Exhaustiveness Checking (ensuring all enum variants or booleans are handled in `match` expressions).
- **Control Flow Analyzer (`CFG.cpp`)**:
  - Builds a real Control Flow Graph (`CFG` and `BasicBlock`).
  - Validates missing return statements (for non-void functions).
  - Validates `break` and `continue` contexts.

## 3. Recently Completed (A1.2, A1.4, A2.2, A3)
- **Method Resolution**: Fully implemented via `MethodResolver` mapping receiver types to methods. Unified arguments (including `self`).
- **Named Arguments**: Handled in `UnificationEngine`. Supports mixed positional/named arguments and detects duplicates/invalid orders.
- **CFG Validation**: Added comprehensive reachability and control flow analysis.
- **Test Coverage**: 7 test suites (`ASTManualTest`, `LexerTest`, `ParserTest`, `ResolverTest`, `DiagnosticTest`, `TypeCheckerTest`, `CFGTest`) are all **GREEN (Passed)**.

## 4. Known Issues & Tech Debt
- **`FLIRGenerator.cpp`**: Currently commented out in `CMakeLists.txt`. The AST has evolved rapidly (e.g., `PrintStmtNode` removed, `LiteralExpr` changes), and the FLIR (FDLang Intermediate Representation) generator is currently out of sync and needs an overhaul.

## 5. Next Steps
When starting a new session, you can pick up from here:
1. **Fix/Sync FLIRGenerator (Middle-End/Back-End Transition)**: Update FLIR generation to match the current AST so the compiler can lower the AST into IR.
2. **Traits & Generics (A1.5 / Advanced)**: Implement trait bounds and generic instantiation.
3. **Code Generation (BackEnd)**: LLVM IR translation.

## How to resume in a new chat:
Simply copy this prompt to the AI in the new chat:
> "Read the `docs/PROJECT_STATUS.md` file to understand the current architecture and state of the compiler. All tests are currently passing. Let's start working on [insert next task]."
