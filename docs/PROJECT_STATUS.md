# FDLang Project Status & Handover

## 1. Project Overview
**FDLang** is a custom compiled language featuring a professional-grade compiler architecture heavily inspired by Rust and C++. The core philosophy is **Strict Separation of Concerns (SoC)** and clean, senior-level code.

## 2. Current Architecture & State
Mellis v1.0 — Semantic Foundation Frozen / Core Hardening in Progress
The frontend, middle-end, and `.mlib` serialization systems are highly structured. The Language Core v1.0 is currently passing **100% of tests**, and we are working to reach 100% semantic completion.

- **FrontEnd (`Lexer`, `Parser`)**: 
  - Supports struct, enum, trait, impl, match expressions, and standard control flow.
- **Resolver (`Resolver.cpp`)**:
  - Exclusively handles Name Resolution and Path Semantics.
- **Type Checker (`TypeChecker.cpp`)**:
  - Constraint-based approach (Hindley-Milner style) separated into PrePass, ConstraintGenerator, and UnificationEngine.
- **Match Analyzer (`MatchAnalyzer.cpp`)**:
  - Exhaustiveness Checking.
- **Control Flow Analyzer (`CFG.cpp`)**:
  - Builds a real Control Flow Graph (`CFG` and `BasicBlock`) to validate return/break/continue paths.

## 3. Recently Completed (Phase 1-7)
- **MVIRGenerator & LLVM Backend**: Fully implemented and compiling to native executables (`.exe`).
- **Traits & Generics (Monomorphization)**: Fully implemented.
- **VTable & Dynamic Dispatch (`dyn Trait`)**: Fully operational.
- **Sprint 7 Final (Production-grade Infrastructure)**:
  - Deep Visibility Checks (Preventing export leakages).
  - Structural Semantic Hashing (FNV-1a).
  - Canonical Serialization & Deterministic UUIDs for `.mlib` reproducible builds.
  - Semantic Closure validation (MVIR vs AST separation).
  - Proper Runtime ABI layer separation for `stdlib`.
- **Sprint 8 (Destructuring & Comptime)**:
  - Comptime Evaluation unblocked.
  - Tuple and Struct destructuring implemented with proper RValue allocation to LLVM `getelementptr`.

---

## 4. Mellis v1.0 Public Release Gate (LOCKED SCOPE)

To prevent endless scope creep, we have firmly established the boundaries for **Mellis v1.0 Public Release**. We will **NOT** wait for complete async executors, networking stacks, package managers, or language servers before public release.

*(See the `ROADMAP.md` file in the root directory for full details on each phase).*

### MUST-HAVE for v1.0 Public Release:

- [ ] **Compiler Core Soundness** (Error handling `?`, Lifetime validation, Escape Analysis, Patterns, Intrinsic framework)
- [ ] **Ownership / Borrow / Lifetime**
- [ ] **Patterns / Destructuring**
- [ ] **Generics / Traits**
- [ ] **MLib & Cross-module tests**
- [ ] **Intrinsic framework**
- [ ] **Hosted Runtime** (Windows, Linux CLI process execution)
- [ ] **`core` & `alloc` libs**
- [ ] **`std::process`, `std::env`, `std::io`, `std::fs`, `std::time`**
- [ ] **Determinism & DX** (Docs, Diagnostics)
- [ ] **Release bundle**

---

## 5. Deliberately EXCLUDED from v1.0 (Post-v1.0 Roadmap)
The following features are strategically excluded to ensure we actually ship v1.0:
❌ Full Async Executor / HTTP / Networking / Sockets
❌ TLS / Database / GUI bindings
❌ Advanced Data Structures (HashMap, BTreeMap, LinkedList)
❌ Package Manager / LSP / Debugger
❌ macOS/Embedded/Freestanding complete coverage

## How to resume in a new chat:
Simply copy this prompt to the AI in the new chat:
> "Read the `ROADMAP.md` file to understand the locked scope for the Mellis v1.0 Public Release Gate. Check the checklist in Phase 8. Let's start working on the next missing item."
