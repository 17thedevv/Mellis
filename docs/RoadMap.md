# Luna — Canonical Roadmap 2026

> Canonical, high-level roadmap. It describes **maturity layers**, not the
> historical order in which features were implemented. Legacy numbering such as
> "Phase 12D / 13 / 14 / 15" is retired from the canonical roadmap — it may still
> exist in historical docs and implementation milestones, but the canonical
> roadmap does not depend on it.

```
Vision
  ↓
Language Foundation
  ↓
Semantic Maturity
  ↓
Language & Stdlib Maturity
  ↓
Runtime Architecture
  ↓
Practical Software Capability
  ↓
Production Toolchain
  ↓
Real-World Validation
  ↓
Ecosystem
```

---

## Stage 1 — Language Foundation — COMPLETE & FROZEN ✅

Lexer · Parser · Resolver · Diagnostic Engine · Type System · Traits · Generics ·
Ownership / Borrowing · MVIR · LLVM Backend · Native executable generation ·
Artifact generation · `.ln` source model · `.llib` canonical artifact model.

Canonical pipeline:

```
.ln
 ↓
Parse
 ↓
Import / Artifact Resolution
 ↓
Macro Expansion
 ↓
Name Resolution
 ↓
Typecheck + Comptime
 ↓
Monomorphization
 ↓
Borrow / Region Analysis
 ↓
MVIR
 ↓
LLVM
 ↓
Native Executable
```

---

## Stage 2 — Semantic Maturity — FROZEN ✅

`SEM-MATURITY-01` · `REGION-01` · `REGION-02` · `SEM-STRESS-01`

Key distinctions established (not "a borrow checker exists", but a defined,
stress-tested semantic authority model):

- Place Identity ≠ Loan Liveness
- Value Provenance ≠ Type Syntax
- Region ≠ Storage Duration
- Region authority ≠ Borrow authority

Modeled concepts: Place · Provenance · Loan · Capability · Region · Lifetime
relation · strong / weak update · CFG lifetime realization · call-site lifetime
obligations · struct lifetime invariants · source/.llib semantic parity ·
100-program adversarial semantic corpus.

---

## Stage 3 — Existing Language Capability Maturity — FROZEN ✅

`LANG-FEATURE-MATURITY-01`

Audited:

- **Macros** — fragments · repetitions · hygiene · recursion limits · `.llib` transport
- **Comptime** — mutation · loops · recursion · structs · const evaluation · runtime parity · execution limits
- **Unsafe** — raw pointer dereference · pointer arithmetic · unsafe containment
- **FFI** — `repr(C)` · safe layout checks · safe-reference restrictions
- **Cross-feature staging** — Macro → Comptime · Generic → Comptime boundaries

Known language gaps:

- `LANGUAGE-GAP-01` — Borrowed enum payload projection
- `LANGUAGE-GAP-02` — Range expression syntax sugar
- `LANGUAGE-GAP-03` — Open-generic-dependent comptime under the current pre-monomorphization staging model

Macros/comptime are therefore no longer future roadmap items.

---

## Stage 4 — Standard Library v1 Foundation — FROZEN ✅

`STDLIB-MATURITY-01`

Current baseline:

```
lang/
    Drop, Option, Iterator, IntoIterator

core/
    Result, Try, Clone, Copy, Eq / Ord, Hash, Slice, Ptr, Mem,
    iterator adapters, iterator consumers

alloc/
    Box, Vec, String, HashMap, HashSet, iterator collect, allocator / raw table

io/
    basic stdout / stderr
```

Canonical sysroot baseline before runtime/practical expansion:

- 25 providers
- acyclic provider DAG
- source / `.llib` parity
- compiler has no stdlib special cases

---

## Stage 5 — Runtime Foundation — FROZEN ✅

`RUNTIME-RENAME-01`

Canonical identity:

- **Luna Runtime**
- ABI prefix: `__luna_`
- Library: `luna-runtime.lib`
- Runtime ABI: `v1`
- Legacy Mellis runtime symbols: `0`

Compiler / runtime / stdlib now share one unified identity.

---

## Stage 6 — Runtime Architecture Reassessment — NEXT 🔄

`RUNTIME-ARCH-01`

This is the real next milestone. Do not rush features in. Audit the runtime
against the principle:

> If functionality can be implemented safely and efficiently as ordinary Luna
> code, it should not live in the runtime.

Scope:

- Phase 0 — Ground-truth runtime audit
- Phase 1 — Stable ABI vs internal runtime boundary
- Phase 2 — Memory / allocator ownership model
- Phase 3 — Panic / trap model
- Phase 4 — Startup / process architecture
- Phase 5 — I/O ABI foundation
- Phase 6 — Platform abstraction
- Phase 7 — Thread / sync / coroutine relevance audit
- Phase 8 — Formal Luna Runtime ABI v1 spec
- Phase 9 — Measurement-driven optimization
- Phase 10 — Runtime conformance + freeze

This replaces the old, vague line "Core runtime (Async Executor)". Runtime ≠ an
async executor; an executor is at most a possible subsystem, not something the
core runtime is guaranteed to own.

---

## Stage 7 — Practical Software Capability

`STDLIB-GAP-AUDIT-01`

- Phase 0 — Real-software capability inventory — COMPLETE ✅ (17 probes)
- Phase 1:
  - `C-GAP-07` — generic `?` monomorphization
  - `String` Eq / Hash
  - `process::args()`
  - whole-file byte I/O
  - UTF-8 text I/O
  - stdin line input
  - a real CLI application
- Phase 2 — Text / Parsing / Formatting
- Phase 3 — Process / Environment / Path
- Phase 4 — Richer filesystem / streaming I/O — only if usage proves necessary

---

## Stage 8 — Real-World Validation

`REAL-WORLD-01`

Stop testing individual language features; write software. Examples: CLI
parser/tool · word counter · source scanner · config parser · file transformer ·
small build utility · mini interpreter/parser · maybe simple HTTP tooling later.

Questions to answer:

- Does Luna actually feel usable?
- Where does boilerplate explode?
- Which language gaps block real code?
- Which stdlib gaps matter?
- Where are compile times bad?
- Where do diagnostics fail?
- Where does the runtime abstraction leak?

Only then decide — **based on evidence** — about `LANGUAGE-GAP-01/02/03`, async,
new syntax, or new stdlib abstractions.

---

## Stage 9 — Production Compiler & Toolchain

Only once real software runs well does it become production-oriented.

Scope: incremental compilation · build graph · package manager · dependency
resolution · artifact cache · compiler reproducibility · toolchain
installation/update · cross compilation · target triples · debug info ·
release/debug profiles · LSP · formatter · documentation generator · test runner ·
benchmark runner · package publishing · ABI/version compatibility.

Possible shape:

```
luna
    compiler
luna build
luna run
luna test
luna fmt
luna doc
luna pkg
```

Naming / tool architecture come after an audit — do not copy Cargo.

---

## Stage 10 — Advanced Language Evolution

Evidence-based, not a hardcoded list. Comptime already exists. Future
candidates: borrowed enum payload projection · range expression syntax ·
post-mono / staged comptime · richer pattern matching · async/await maturity ·
async runtime/executor model · concurrency primitives · reflection /
metaprogramming extensions · better error-propagation ergonomics · advanced
generic facilities.

A feature is promoted only through:

```
real-world use case
    ↓
library solution insufficient
    ↓
language-level need proven
```

---

## Stage 11 — Ecosystem

Long-term: package registry · community packages · documentation ecosystem ·
editor plugins · debugger integration · CI templates · FFI packages ·
frameworks · libraries · cross-platform distributions · versioning policy ·
stability guarantees.

---

## Progress snapshot

```
LANGUAGE FOUNDATION              DONE ✅
SEMANTIC MATURITY                DONE ✅
LANGUAGE FEATURE MATURITY        DONE ✅
STDLIB v1 FOUNDATION             DONE ✅
RUNTIME IDENTITY                 DONE ✅

RUNTIME ARCHITECTURE             NEXT 🔄
PRACTICAL STDLIB                 AFTER
REAL-WORLD SOFTWARE              AFTER
PRODUCTION TOOLCHAIN             LATER
ECOSYSTEM                        LONG-TERM
```
