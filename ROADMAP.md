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
Language Feature Maturity
  ↓
Runtime Architecture
  ↓
Standard Library Completion & Practical Capability
  ↓
Real-World Validation
  ↓
Production Toolchain
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

## Stage 6 — Runtime Architecture Reassessment — FROZEN ✅

`RUNTIME-ARCH-01`

- ABI architecture:                  FROZEN ✅
- Memory ownership:                  FROZEN ✅ (OwnedBuffer(ptr, len, cap) & BorrowedBuffer)
- Panic/trap model:                  FROZEN ✅
- Process invocation model:          FROZEN ✅ (argv[0] separate from user args argv[1..])
- I/O ABI foundation:                FROZEN ✅ (embedded NUL defense, 64-bit size query)
- Status model:                      FROZEN ✅ (LunaStatus stable 32-bit enum)
- Windows implementation:            VERIFIED ✅ (19/19 native conformance tests)
- Linux implementation:              IMPLEMENTED (POSIX pthreads sync/threading)
- Linux native conformance:          PENDING CI

Principle enforced:
> If functionality can be implemented safely and efficiently as ordinary Luna
> code, it should not live in the runtime.

---

## Stage 7 — Standard Library Completion & Practical Capability — ACTIVE 🔄

`STDLIB-GAP-AUDIT-01` (Umbrella Milestone)

### Philosophy & Operating Principles
- **Stdlib Completeness Precedes Real-World Validation**: The standard library is completed according to a well-defined v1 scope and frozen before opening Stage 8.
- **Role of Acceptance Applications**: Practical tools like `lunawc` provide localized integration evidence for individual phases; they do not signal premature transition to real-world validation.
- **Strict Disposition Rule**: Every identified stdlib capability must resolve to an explicit disposition:
  - `SUPPORTED & VERIFIED`
  - `DEFERRED BY DESIGN`
  - `LANGUAGE-BLOCKED`
  - `OUT-OF-SCOPE FOR STDLIB v1`
  Zero `UNKNOWN` items remain at completion.

### Execution Phases
- **Phase 0 — Real-Software Capability Audit** — COMPLETE & FROZEN ✅ (17 probes)
- **Phase 1 — Practical I/O & Core Types Foundation** — COMPLETE & FROZEN ✅
  - `C-GAP-07` — generic `?` monomorphization (repaired & frozen)
  - `String` Eq / Hash (`HashMap<String, V>` and `HashSet<String>` verified)
  - `process::args()` (canonical provider, argv[0] excluded, Windows UTF-8 decoded)
  - Whole-file byte & text I/O (`io::read`, `io::read_text`, `io::write`, `io::write_text`)
  - Stdin line input (`io::read_line` with EOF & CRLF normalization)
  - Capstone CLI application (`lunawc` verified with file & stdin modes)
  - Zero compiler special cases for stdlib types
  - Full workspace regression clean (0 failures)
- **Phase 2 — Text / Parsing / Formatting** — COMPLETE & FROZEN ✅
  - `String::split(char)`, `split_str(str)`, `split_string(&String)`, `lines()`, `split_whitespace()`
  - String search & predicates: `find`, `find_string`, `find_char`, `contains`, `starts_with`, `ends_with`
  - Integer parsing family: `i8`..`i64`, `u8`..`u64`, `usize`, `isize`, and `bool`
  - Primitive → String formatting: `string_from_bool`, `string_from_char`, `string_from_u8`..`string_from_u64`, `string_from_usize`, `string_from_i8`..`string_from_i64`, `string_from_isize`
  - Text composition & trimming: `push_string`, `trim`, `trim_string`, `trim_start`, `trim_end`
  - `C-GAP-08` resolved: 64-bit integer literal preservation in LLVM codegen and zero-extension in integer casts
  - Target probe closures verified: `text_split.ln` (PASS), `text_find.ln` (PASS), `parse_integer.ln` (PASS), `format_values.ln` (PASS), `simple_config_parse.ln` (PASS)
  - Pure Luna standard library implementation under Rule 11 (zero compiler special cases for String)
  - Source vs `.llib` parity verified, sysroot persistent artifacts verified, full workspace regression clean (0 failures)
- **Phase 3 — Core Stdlib Surface & Depth Audit** — COMPLETE & FROZEN ✅
  - Comprehensive checklist audit against Rust std and C++ std across 13 core library domains
  - Canonical inventory of 26 sysroot providers
  - 20 findings classified: proven surface candidates, semantic feasibility probes, design-required protocols, and deferred domains
  - Four key architectural findings: primitive method resolution already present in typechecker, absence of trait default method lowering in v1, `C-GAP-09` float codegen defect, and `Rc<T>` soundness tied to thread safety
  - Canonical audit document frozen in `docs/spec/stdlib-core-depth-audit.md`
- **Phase 4 — Core Gap Closure** — ACTIVE 🔄
  - **Phase 4A — Semantic Feasibility & Core Protocol Design** — COMPLETE & FROZEN ✅
    - All empirical feasibility & adversarial probes executed and verified (`tests/stdlib_gap_probes/`, `stdlib_phase4a_feasibility_tests.rs`, 11 passed, 0 failed)
    - Probe 1 (`CORE-GAP-03`): By-value `Entry` API proved blocked by lack of borrow-carrying aggregate lifetime parameterization (`E3005`); registered as `LANGUAGE-GAP-05`. Direct fallback methods (`get_or_insert`, `insert_if_absent`) verified and caller-consumption/drop-once ownership contracts frozen.
    - Probe 2 (`CORE-GAP-08`): `split_at_mut` dynamic disjointness proved sound via bounds-checked pointer math (`SAFE-API-VIA-AUDITED-UNSAFE`); formal mathematical proof obligations (precondition, disjointness, lifetime bounds, exclusivity) frozen.
    - Probe 3 (`CORE-GAP-05`): Carrier iterator lifetime retention (`Peekable` caching `Option<&T>`) proved sound across 3 adversarial cases: source drop rejected (`REJECTED_AS_EXPECTED`), carrier move accepted (`ACCEPTED`, exit code 0), and nested carrier source mutation rejected (`REJECTED_AS_EXPECTED`).
    - Probe 4 (`CORE-GAP-04`): `HashSet` set algebra: owned union/intersection verified; lazy borrowed views deferred (`LANGUAGE-GAP-04`).
    - Probe 5 (`CORE-GAP-09`): Slice copy aliasing probed (`SLICE-COPY-ALIAS`); confirmed borrowck prevents overlapping mutable/shared slices of same allocation in safe code, establishing `memcpy` soundness for safe `copy_from_slice`.
    - Probe 6 (`CORE-GAP-16A`): Nested virtual dispatch path (`Concrete Display -> Display::fmt -> &rw dyn Writer -> Writer::write_utf8`) verified in-place; sound UTF-8 contract frozen (`Writer::write_utf8` returns `Err(FmtError::InvalidUtf8)` on non-UTF-8 input, enforced by `impl Writer for String`); dependency rule frozen (`alloc` depends on `core`; `core` does not depend on `alloc`); `StaticBufWriter<N>` dropped.
    - Probe 7 (`CORE-GAP-12`): Trait coherence verified across 4 cases; confirmed compiler soundly rejects overlapping concrete impls alongside generic reflexive blanket impls (rejected by trait coherence); dot-dispatch for blanket generic impls deferred.
    - `C-GAP-09` float compiler pipeline audit completed with multi-layer closure gate across typechecker, comptime, optimizer, and LLVM backend; normative float-cast contract (truncation toward zero for representable finite; comptime evaluation diagnostic; deterministic runtime numeric-conversion trap) and exact mathematical float remainder `%` ($x - y \cdot \operatorname{trunc}(x/y)$, NaN on div by $\pm 0$ or $\infty$; semantic characterization) frozen; `CORE-GAP-18` float math strategy formulated (direct platform C FFI, zero runtime C-ABI additions).
    - Foundational protocols designed & frozen: Conversion (`Convert<T>` / `TryConvert<T>`), Default (`trait Default`), Formatting (`core/fmt` writer sink with `FmtError::InvalidUtf8`), and Minimal v1 Error Interoperability (`trait Error` decoupled from `?`).
    - `Rc<T>` concurrency evaluation: deferred to Phase 7 due to absence of static `Send`/`Sync` thread-confinement constraints.
    - Definitive transition matrix and normative contracts frozen in `docs/spec/stdlib-core-protocols-design.md` with Phase 3 canonical IDs and full capability scopes strictly preserved.
  - **Phase 4B — Proven Core Surface Closure** — ACTIVE / NEXT 🔄
    - `Vec` depth (`swap_remove`, `retain`, `dedup`, `extend`, `resize` with contracts)
    - `Vec` `Clone` and `Eq` implementations
    - `HashMap` direct entry methods (`get_or_insert`, `insert_if_absent`) with frozen ownership contracts
    - `HashSet` owned set algebra (`union`, `intersection`, `difference`)
    - Iterator adapters (`CORE-GAP-05`): `Chain`, `Peekable`, `FilterMap`, `TakeWhile`
    - Iterator consumers (`position`, `nth`, `last`, `sum`) & integer range coverage (including `usize`/`isize`)
    - Slice depth (`CORE-GAP-08`): `get`, `get_mut`, `split_at` (`IMPLEMENT-4B`) & `split_at_mut` (`AUDITED-UNSAFE-4B`)
    - Bulk slice operations (`CORE-GAP-09`): `copy_from_slice<T: Copy>`, `fill<T: Clone>`
    - Generic sorting (`CORE-GAP-10`): unstable in-place sort with bounded complexity ($O(n \log n)$) preserving memory safety on inconsistent comparators
    - `Option` & `Result` combinators (`unwrap_or_else`, `or_else`, `flatten`, `transpose`)
    - Primitive integer utilities (`CORE-GAP-14`): `min`, `max`, `clamp`, `checked_*`, `saturating_*`, `abs`, `checked_abs`
    - Memory utilities (`CORE-GAP-15`): `mem::swap`, `mem::replace` (`IMPLEMENT-4B`); `mem::take` in Phase 4C after `Default`
  - **Phase 4C — New Foundational Protocols & Float Math**:
    - `C-GAP-09` multi-layer float compiler completion
    - Float math utilities (`core/math`) via direct platform C FFI
    - Approved conversion protocol (`core/convert`)
    - Approved default protocol (`core/default`)
    - Approved formatting & error model (`core/fmt`, `core/error`)
    - `mem::take` (dependent on `Default`)
- **Phase 5 — Filesystem / Path / Process Environment** — PLANNED
  - Path manipulation (`path_join.ln`)
  - Environment variables (`env_read.ln`), current working directory, process execution
  - Filesystem metadata, file existence, file copy (`file_copy.ln`), directory operations
- **Phase 6 — File Handles / Streaming I/O / Directory Operations** — PLANNED
  - `File` struct, `open`, `close` / `Drop`, `read`, `write`, `seek`, `flush`, `append`
  - OS handle ownership audit & `Drop` semantics
  - Streaming I/O and directory traversal
- **Phase 7 — Time / Thread / Sync / Platform Utilities** — PLANNED
  - High-resolution monotonic clock (`Instant`, `Duration`, `sleep`)
  - `Mutex<T>`, `Condvar`, and native threading stdlib wrappers
  - Atomic primitives (`AtomicBool`, `AtomicUsize`)
- **Phase 8 — Cross-Provider Integration & Sysroot Parity Audit** — PLANNED
- **Phase 9 — Adversarial Stdlib Stress & Final v1 Freeze** — PLANNED

---

## Stage 8 — Real-World Validation (After Stdlib v1 Freeze)

`REAL-WORLD-01`

Stage 8 opens only when `STDLIB-GAP-AUDIT-01` is `COMPLETE & FROZEN ✅` with 0 unresolved gaps within v1 scope.

The focus shifts completely:
- Not: *"What APIs are missing to write an app?"* (already resolved in Stage 7).
- But: *"With a standard library known to be functionally complete, when building larger software, which aspects of Luna's language and compiler design cause real architectural friction?"*

Examples: CLI tools · language source scanners · configuration engines · file transformers · build utilities · mini parsers/interpreters.

Questions to answer based on evidence:
- Where does boilerplate explode?
- Which language gaps block real code (`LANGUAGE-GAP-01/02/03`)?
- Where are compile times or diagnostics inadequate?
- Where does the runtime or memory model leak?

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
RUNTIME ARCHITECTURE             DONE ✅

STAGE 7: STDLIB COMPLETION       ACTIVE 🔄
  Phase 0  Capability Audit      DONE ✅
  Phase 1  Practical I/O & Core  DONE ✅
  Phase 2  Text / Parsing / Fmt  DONE ✅
  Phase 3  Core Depth Audit      DONE ✅
  Phase 4  Core Gap Closure      ACTIVE 🔄
  Phase 5  Filesystem & Path     PLANNED
  Phase 6  Streaming I/O & File  PLANNED
  Phase 7  Time, Thread & Sync   PLANNED
  Final    Stdlib v1 Freeze      PLANNED

STAGE 8: REAL-WORLD SOFTWARE     AFTER STDLIB FREEZE
STAGE 9: PRODUCTION TOOLCHAIN    LATER
STAGE 10: ADVANCED EVOLUTION     EVIDENCE-BASED
STAGE 11: ECOSYSTEM              LONG-TERM
```

