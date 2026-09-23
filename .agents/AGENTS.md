# Luna Compiler Architecture Rules

1. Every phase owns exactly one responsibility.
2. Every phase communicates only through stable IR.
3. No phase may inspect internal data of another phase.
4. Information only flows forward. Backward dependencies are forbidden.
5. Every intermediate representation has a single owner.
   - AST      -> FrontEnd
   - Semantic -> MiddleEnd
   - MVIR     -> Optimizer
   - LLVM IR  -> Backend

6. Luna Artifact Identity Principle ("Luna trusts identity, never existence"):
   - **Existence is not validity**: The presence of an artifact (`.llib` or `.mlib`) does not imply it is valid for the current compilation context. The compiler must validate its identity before loading it. If invalid, the compiler rejects it (it does not silently recompile).
   - **Discovery vs. Validation**: Discovery merely locates a provider candidate. Validation determines if its identity matches the current context. Package management (`mpm`) handles rebuilding if invalid.
   - **Identity is a Tuple**: Artifact identity is not just a source hash. It encompasses Source Identity, Dependency Interface Identity, Compiler Identity, Target Identity, Feature Identity, and Artifact Kind.
   - **Source Identity ≠ Interface Identity**: Private implementation changes affect source identity but not interface identity (canonical public contract). The compiler must use a deterministic serialization of the public interface for the `interface_hash`.
   - **Dependency Interface Fingerprinting**: Artifacts must record the interface identities of their dependencies. Precise incremental invalidation relies on whether a dependency's interface has changed, not just its implementation.
   - **Compiler Boundary**: The compiler validates and either loads or rejects an artifact. It never auto-rebuilds stale artifacts. `mpm` wraps this process to resolve and rebuild when the compiler rejects a candidate.
   - **The compiler validates; the application orchestrates**: The Luna compiler is unaware of package-management policy. Artifact validation is a compiler capability; rebuilding, caching, acquisition, versioning, and invalidation policy belong to application-layer tooling.

7. Compiler vs Package Manager Boundary & Module Architecture:
   - **Provider ≠ Module namespace:** `import` selects a *provider*, `module` defines a *namespace*, and `::` accesses a *namespace*.
     - Examples of providers: `alloc.ms` (source) or `alloc.mlib` (artifact). Both are providers of the same logical module.
     - Provider Resolution: `import <alloc>` searches for provider "alloc". `import "foo"` searches for local project provider "foo".
     - Namespace: A provider file can define namespaces via `module std { export struct Vec<T> ... }`.
     - Symbol usage: Symbols are accessed via their namespace (`std::Vec<int>`), not their provider. `import` does NOT bring symbols into scope.
     - Multiple providers can contribute to the same public namespace (e.g., `alloc.ms` -> `std::`, `core.ms` -> `std::`).
   - **Compiler Boundary:** `luna` is fully standalone. It resolves `import <foo>` using search paths and understands module semantics. It does not know about projects, versions, or caching.
   - **Project Boundary (MPM):** `mpm` manages packages, dependency graphs, versions, incremental rebuilds, and artifacts. It determines *which* providers/artifacts to feed to the compiler.
   - **Artifact Boundary:** `.mlib` is a compiler-defined artifact containing Manifest, ImplTable, GenericMVIR, ObjectCode, etc. It is functionally identical to a `.ms` source file from the language's perspective.

8. Grammar Authority Rule (mellis-grammar):
   - **Single Source of Truth:** `mellis-grammar` skill is the canonical source of truth for source-level syntax.
   - **Enforcement:** Agents MUST read the `mellis-grammar` skill before modifying parser, AST, or semantic syntax.
   - **No Inference:** Never infer Luna syntax from Rust/C++ syntax (e.g., using `mut` instead of `rw`, `use` instead of `import`, `let` instead of `dec`).
   - **No Unapproved Additions:** Never introduce a new keyword without updating the grammar in `grammar.ebnf` and the skill.
   - **Test Updates:** Add or update parser tests when changing grammar.
   - **Workflow:** When coding a new feature: (1) Search grammar skill. (2) Search parser tests. (3) Search existing accepted examples. (4) Only then propose changes. If syntax exists, reuse. If it doesn't, propose a grammar change before implementation.

9. Semantic Compliance Rule (luna-semantic-compliance):
   - **Mandatory Alignment:** Agents MUST follow `luna-semantic-compliance` before writing `.ln`/`.ms` code, Core, Alloc, Stdlib, ABI-facing APIs, compiler-generated code, or semantic tests.
   - **No Foreign Semantics:** Never substitute Rust, C++, or LLVM semantic models or syntax (e.g. `'a`, turbofish, `let`, `mut`).
   - **Semantic Contract Enforcement:** Ensure explicit lifetime relations (`life_from`, `where outlives`), proper `rw` mutability, and `.llib` ABI parity are preserved and actively utilized.

10. Ownership Abstraction Guard Rule (BOX-FROZEN):
   - **Generic Ownership Parity:** Any future ownership abstraction (e.g., `Rc<T>`, `Arc<T>`, custom smart pointers, allocator containers) must be expressible strictly through existing generic ownership, move, drop, borrow, lifetime, and memory primitives (`ptr`/`mem`/`HeapFree`).
   - **No Compiler Builtin Containers:** The compiler must never re-introduce specialized semantic types, dedicated borrowck branches, or distinguished MIR instructions for specific library ownership containers unless a new language-level semantic contract is explicitly justified and frozen.

11. Stdlib–Compiler Boundary Rule (luna-stdlib-compiler-boundary):
   - **Mandatory Separation:** The standard library is a consumer of the language, not an extension of the compiler's type system.
   - **No Type-Specific Branches or Opcodes:** The compiler MUST NOT acquire type-specific semantic knowledge, IR opcodes, or backend lowering branches for stdlib containers (`Box`, `Vec`, `String`, `HashMap`, `HashSet`, etc.).
   - **Capability-Gap Protocol:** Compiler fixes made during stdlib development must be generic language machinery fixes benefiting arbitrary user-defined types.
   - **Mandatory Reporting:** Walkthroughs modifying compiler code during stdlib development must follow the mandatory reporting format defined in `luna-stdlib-compiler-boundary`.

12. Testing Strategy Boundary Rule (luna-testing-strategy):
   - **Highest Public Abstraction Boundary:** Test at the highest abstraction boundary that can faithfully observe the claimed behavior.
   - **Layer Separation:**
     - Compiler Internals & Invariants (coherence, typechecker, borrowck, mono, MVIR, backend) -> Rust `cargo test`.
     - Language & Standard Library Behavior -> standalone `.ln` fixtures executed through `luna` CLI.
     - Source vs `.llib` Parity -> CLI E2E tests verifying identical public behavior.
     - Freeze Regression -> CLI harness orchestration.
   - **No Convenience Rust Stdlib Suites:** Do not create `crates/luna-driver/tests/stdlib_*` suites merely because internal Rust driver helpers are convenient. First determine whether the behavior can be expressed as a standalone `.ln` fixture executed through `luna`.
   - **Compiler Gap Protocol:** When a compiler gap is encountered during stdlib development, stop stdlib work, isolate the bug with a user-defined reproducer in a dedicated `C-GAP-*` test, and freeze the generic compiler fix before resuming stdlib work.

13. Stdlib Architecture Routing:
   - For any task involving splitting core.ln / alloc.ln, component-level stdlib providers, lang/ core/ alloc/ io/, logical std module composition, language-contract auto-loading, or stdlib provider/artifact migration.
   - Agents MUST read `luna-stdlib-architecture`, `luna-lang-contracts`, `luna-stdlib-migration`, `luna-stdlib-compiler-boundary`, and `luna-testing-strategy`.
   - Migration must start with a strict audit mapping declaration -> component -> dependency -> artifact before any implementation plan is drafted.
