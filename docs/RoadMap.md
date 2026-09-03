Vision
    ↓
MVP Compiler
    ↓
Usable Language
    ↓
Production Compiler
    ↓
Toolchain
    ↓
Ecosystem

Phase 1 — MVP Compiler (Language Core v1.0)

██████████████████████████

[✓] Lexer
[✓] Parser
[✓] Resolver

[✓] Diagnostic Engine
[✓] Type Checker (Traits, Generics, Borrowing)

[✓] MVIR Specification & Validation
[✓] MVIR Generator

[✓] LLVM Backend
[✓] MLib Generation (Deterministic)
[✓] Executable Generation

Phase 1 Goal:
.ms -> AST -> Resolver -> Type Checker -> MVIR -> MLib / LLVM IR -> Executable
(Status: 100% Completed, Verified & Frozen)

---

Phase 2 — Usable Language (Ecosystem)

░░░░░░░░░░░░░░░░░░░░░░░░░░

[ ] Standard Library (lib/std)
[ ] Core runtime (Async Executor)
[x] Lambda / Closures implementation (Phase 12D ✅)
[ ] Advanced Unsafe validation

Phase 2 Goal:
Provide developers with basic tools (`String`, `Vec`, `File`) and event loop capabilities to write real-world system applications.

---

Phase 3 — Production Compiler (Language Features)

████████████████████████████████

[x] Lambda / Closures (Phase 12D ✅)
[x] dyn Trait Objects (Phase 13 ✅)
[x] Macros (Phase 14 🔄) ← DESIGN PHASE - See Phase14_Macros_Design.md
[ ] async/await (Phase 15)

Phase 3 Goal:
Full-featured language with dynamic dispatch, metaprogramming, and async I/O.

---
Phase 4 — Metaprogramming & Async

██████████████████████████

[x] dyn Trait (Phase 13 ✅)
[x] Macros (Phase 14 ✅) ← NOW PLANNED
[ ] async/await (Phase 15)
[ ] Comptime (Phase 16)

Phase 4 Goal:
Complete metaprogramming capabilities and async I/O support.

Phase 5 — Advanced Features

██████████████████████████

[ ] async/await (Phase 15)
[ ] Comptime (Phase 16)