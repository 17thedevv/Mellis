
# Mellis Compiler Architecture Rules

1. Every phase owns exactly one responsibility.
2. Every phase communicates only through stable IR.
3. No phase may inspect internal data of another phase.
4. Information only flows forward. Backward dependencies are forbidden.
5. Every intermediate representation has a single owner.
   - AST      -> FrontEnd
   - Semantic -> MiddleEnd
   - MVIR     -> Optimizer
   - LLVM IR  -> Backend

6. Import Provider Resolution Algorithm:
   - Resolve logical module name.
   - Search for `.mlib`. If found, load it.
   - Otherwise, search for `.ms`. If found, compile to `.mlib` (in-memory or persisted) and load it.
   - Otherwise, emit unresolved-module diagnostic.
   - Rule: If `.mlib` exists, the compiler NEVER silently recompiles `.ms`. The artifact is the source of truth for downstream dependencies.

7. Compiler vs Package Manager Boundary & Module Architecture:
   - **Provider ≠ Module namespace:** `import` selects a *provider*, `module` defines a *namespace*, and `::` accesses a *namespace*.
     - Examples of providers: `alloc.ms` (source) or `alloc.mlib` (artifact). Both are providers of the same logical module.
     - Provider Resolution: `import <alloc>` searches for provider "alloc". `import "foo"` searches for local project provider "foo".
     - Namespace: A provider file can define namespaces via `module std { export struct Vec<T> ... }`.
     - Symbol usage: Symbols are accessed via their namespace (`std::Vec<int>`), not their provider. `import` does NOT bring symbols into scope.
     - Multiple providers can contribute to the same public namespace (e.g., `alloc.ms` -> `std::`, `core.ms` -> `std::`).
   - **Compiler Boundary:** `mellis` is fully standalone. It resolves `import <foo>` using search paths and understands module semantics. It does not know about projects, versions, or caching.
   - **Project Boundary (MPM):** `mpm` manages packages, dependency graphs, versions, incremental rebuilds, and artifacts. It determines *which* providers/artifacts to feed to the compiler.
   - **Artifact Boundary:** `.mlib` is a compiler-defined artifact containing Manifest, ImplTable, GenericMVIR, ObjectCode, etc. It is functionally identical to a `.ms` source file from the language's perspective.

8. Grammar Authority Rule (mellis-grammar):
   - **Single Source of Truth:** `mellis-grammar` skill is the canonical source of truth for source-level syntax.
   - **Enforcement:** Agents MUST read the `mellis-grammar` skill before modifying parser, AST, or semantic syntax.
   - **No Inference:** Never infer Mellis syntax from Rust/C++ syntax (e.g., using `mut` instead of `rw`, `use` instead of `import`, `let` instead of `dec`).
   - **No Unapproved Additions:** Never introduce a new keyword without updating the grammar in `grammar.ebnf` and the skill.
   - **Test Updates:** Add or update parser tests when changing grammar.
   - **Workflow:** When coding a new feature: (1) Search grammar skill. (2) Search parser tests. (3) Search existing accepted examples. (4) Only then propose changes. If syntax exists, reuse. If it doesn't, propose a grammar change before implementation.
