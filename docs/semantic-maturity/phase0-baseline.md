# SEM-MATURITY-01 Phase 0 — Semantic Coverage Audit & Conformance Baseline

> Phase 1A update (2026-09-18): after rebasing onto hygiene-complete `main`
> (`399b3666ef57db7be602db3aac721a655d63bf7c`), the unchanged Phase 0 corpus
> reproduced all eight original gaps. SEM-GAP-03 and SEM-GAP-06 are now closed;
> the original Phase 0 measurements remain identified as baseline values below.

## SEM-MATURITY-01 PHASE 0 PRECHECK

### Worktree baseline

- Branch: `sem-maturity-01-phase-0`
- Worktree root: `C:/Users/84387/Documents/Codex/2026-09-18/luna-sem-maturity-01-phase-0-2`
- Baseline commit: `88b9779e760ee08d76cb75aa510e55868590abb1`
- Baseline commit subject: `docs(std): Phase 4H final architecture acceptance (STD-ARCH-01)`
- Dedicated worktree state before Phase 0 edits: clean
- Concurrent `REPO-HYGIENE-01` checkout: `D:/fdlang`; observed dirty and not used for Phase 0 work

### Frozen syntax sources

- `.agents/skills/mellis-grammar/SKILL.md` — declared canonical syntax SSOT.
- `.agents/skills/mellis-grammar/grammar.ebnf` — formal canonical surface grammar.
- `.agents/skills/luna-semantic-compliance/SKILL.md` — canonical Luna spelling for mutation, references, receivers, generics, modules, and lifetime relations.
- `docs/normative-rules-p0-p1.md`, Rule K — freezes grammar authority, comma-delimited struct fields, postfix `.await`, loop forms, receiver equivalence, provider/module orthogonality, and `using ... as ...`.
- `docs/grammar.md` and `docs/LanguageReference.md` contain historical syntax and are not used as current syntax authority where they conflict with the sources above.

### Frozen semantic sources

- `docs/normative-rules-p0-p1.md` — frozen Rules A–K: Copy/Drop, partial moves, associated projections, coherence, async, const/comptime, raw pointers, dyn/unsized, modules, and syntax invariants.
- `docs/memory/borrowck-ownership-rfc.md` — P6 ownership, move, borrow, place-tree, reinitialization, drop, and CFG rules.
- `docs/borrowck/NLL_spec.md` and `docs/borrowck/call_effect_model.md` — NLL regions and call-effect model.
- `.agents/skills/luna-semantic-compliance/references/lle_v1_rfc.md` — frozen lifetime-elision rules over canonical `life_from` / `where outlives` contracts.
- `docs/Phase16_comptime_Plan.md` — frozen comptime capability boundary and zero-fallback rule.
- `docs/diagnostics/diagnostics-v1.md` — frozen diagnostic architecture and numeric registry.
- `docs/core-language-contract.md` and `.agents/skills/luna-lang-contracts/SKILL.md` — language-contract identity and controlled auto-loading.
- `.agents/skills/luna-stdlib-compiler-boundary/SKILL.md` — user-defined generic behavior must not depend on stdlib container special cases.
- `.agents/skills/luna-stdlib-architecture/SKILL.md` — frozen provider/module architecture.
- `docs/std-arch-01-final-acceptance.md` and `docs/std-arch-01-compatibility-matrix.md` — final STD-ARCH-01 freeze evidence and canonical `.ln` / `.llib` surfaces.

### Authority reconciliation

- The canonical grammar skill and `grammar.ebnf` supersede obsolete `mod`, `use`, `mut`, semicolon-field, prefix-`await`, and `@<...>` examples in historical documents.
- The frozen visibility contract is public-by-default for struct fields. `export`
  is redundant-public compatibility syntax, while `private` is the explicit
  private modifier. This correction supersedes the stale private-by-default
  wording in the semantic-compliance skill and the original Phase 0 report.
- STD-ARCH-01 final acceptance supersedes older physical `.ms` / `.mlib` examples while preserving the semantic distinction between providers and namespaces.
- Historical “complete” implementation claims are evidence only and are rechecked against current production source and executable tests.

### Current semantic test roots

- `mellis-rs/crates/luna-semantic/tests/`
- `mellis-rs/crates/luna-driver/tests/semantic_gate_tests.rs`
- `mellis-rs/crates/luna-driver/tests/ui/`
- `mellis-rs/crates/luna-driver/tests/ui_tests.rs` (currently selects only a narrow `try_*` subset)

### Current borrowck test roots

- `mellis-rs/crates/luna-borrowck/tests/`
- `mellis-rs/crates/luna-driver/tests/ui/borrow*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/move*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/rw_*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/mut_iterator_*.ln`
- `mellis-rs/crates/luna-driver/tests/p0b_drop_subplace_tests.rs`
- `mellis-rs/crates/luna-driver/tests/test_reassign_soundness.rs`

### Current lifetime test roots

- `mellis-rs/crates/luna-semantic/tests/lifetime_*_tests.rs`
- `mellis-rs/crates/luna-driver/tests/lifetime_def_site_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/lifetime_outlives_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/lifetime_relation_abi_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/lifetime_virtual_dispatch_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/vec_iterator_lifetime_acceptance_tests.rs`

### Current generic test roots

- `mellis-rs/crates/luna-semantic/tests/struct_generics_test.rs`
- `mellis-rs/crates/luna-semantic/tests/p0c_tests.rs`
- `mellis-rs/crates/luna-driver/tests/generic_trait_dispatch_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/compiler_gap_c_gap_04_trait_bound_generic_args_tests.rs`
- `mellis-rs/crates/luna-driver/tests/ui/test_generic*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/test_inference*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/try_cross_module_generic.ln`

### Current trait / impl test roots

- `mellis-rs/crates/luna-semantic/tests/p0c_tests.rs`
- `mellis-rs/crates/luna-semantic/tests/p1a_tests.rs`
- `mellis-rs/crates/luna-driver/tests/generic_trait_dispatch_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/ui/test_trait*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/conflicting_impl.ln`
- `mellis-rs/crates/luna-driver/tests/p3_tests.rs`

### Current provider / module test roots

- `mellis-rs/crates/luna-driver/tests/provider_module_contract_acceptance_tests.rs`
- `mellis-rs/crates/luna-driver/tests/module_merge_tests.rs`
- `mellis-rs/crates/luna-driver/tests/module_import_boundary_tests.rs`
- `mellis-rs/crates/luna-driver/tests/import_invariants_tests.rs`
- `mellis-rs/crates/luna-driver/tests/hierarchical_provider_tests.rs`
- `mellis-rs/crates/luna-driver/tests/test_resolver_precedence.rs`
- `mellis-rs/crates/luna-driver/tests/core_provider_baseline_acceptance_tests.rs`

### Current comptime / const test roots

- `mellis-rs/crates/luna-parser/tests/comptime_parser_tests.rs`
- `mellis-rs/crates/luna-driver/tests/adv_comptime_dyn_tests.rs`
- `mellis-rs/crates/luna-driver/tests/p2a_tests.rs`
- `mellis-rs/crates/luna-driver/tests/ui/comptime_*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/test_const.ln`
- `mellis-rs/crates/luna-driver/tests/ui_ignored/comptime_*.ln`

### Current function-value / call tests

- `mellis-rs/crates/luna-driver/tests/compiler_gap_c_gap_05_indirect_call_tests.rs`
- `mellis-rs/crates/luna-driver/tests/compiler_gap_c_gap_06_function_pointer_mangling_tests.rs`
- `mellis-rs/crates/luna-driver/tests/ui/indirect_call.ln`
- `mellis-rs/tests/indirect_call.ln`
- Method and receiver calls are additionally exercised by generic-trait, stdlib, lifetime, and language fixtures.

### Current control-flow tests

- `mellis-rs/crates/luna-driver/tests/ui/test_match.ln`
- `mellis-rs/crates/luna-driver/tests/ui/exhaustive*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/test_loops.ln`
- `mellis-rs/crates/luna-driver/tests/ui/test_for.ln`
- `mellis-rs/crates/luna-driver/tests/ui/drop_branching.ln`
- `mellis-rs/crates/luna-driver/tests/ui/drop_loop_*.ln`
- `mellis-rs/crates/luna-driver/tests/ui/move_loop.ln`

### Known historical semantic gaps to re-verify

- Cross-provider associated-type projection is explicitly identified as a pre-existing source-level limitation in `docs/std-arch-01-final-acceptance.md`.
- Historical generic compiler gaps have dedicated `compiler_gap_c_gap_01` through `compiler_gap_c_gap_06` driver suites; their current status must be re-run rather than inferred from filenames.
- `mellis-rs/tests/luna/language/c_gap_conditionally_moved_loop_break.ln` and `c_gap_orphan_fundamental.ln` record historical source-level gap shapes.
- The older `final_semantic_freeze_audit.md` claims cross-provider associated-type injection is complete, which conflicts with the later STD-ARCH-01 observation; current source-only reproduction decides the Phase 0 result.

### Known unsupported semantics under the frozen v1 design

- Multi-trait `dyn` composition.
- Associated types on `dyn Trait`.
- Supertrait-based `dyn` composition.
- `Box<dyn Trait>` and `Box<[T]>`.
- General range lowering (`E_UNSUPPORTED_FEATURE` in current semantic-gate evidence).
- Comptime I/O, networking, extern calls, async/await, OS/thread effects, and runtime-state mutation.
- Runtime `TypeValue`; full runtime RTTI embedding remains deferred.
- Re-export syntax (`export import`, `export using`) and wildcard imports.
- File-style module declarations and provider-derived namespaces.

### Current diagnostic code registry

- Frozen registry authority: `docs/diagnostics/diagnostics-v1.md`, numeric ranges `E0001`–`E6003`.
- Current production source does **not** implement the frozen typed `DiagnosticCode` registry. `luna-common/src/diagnostic.rs` stores only `{ level, span, message }` and permits spanless ad-hoc strings.
- Numeric codes currently found on the audited production paths include `E1002`,
  `E2016`, and `E3005`; the four isolated cases for E1003 and E3001–E3003 do not
  emit their frozen identities.
- Current source also emits many symbolic string identifiers such as `E_COPY_DROP_CONFLICT`, `E_PARTIAL_MOVE_UNDER_DROP`, `E_USE_OF_MOVED_VALUE`, `E_BORROW_CONFLICT`-equivalent text, `E_ASSOC_TYPE_CYCLE`, and `E_UNRESOLVED_PROJECTION` instead of the frozen numeric identities.
- Phase 0 will not invent codes or retrofit the registry. Negative conformance cases will assert only identities already emitted by current source and will record missing/wrong code identity as a compiler-maturity gap.

## Semantic inventory and coverage matrix

The inventory below counts **119 frozen semantic rules**. Nine explicitly
unsupported v1 boundaries are listed separately and are not included in the
frozen-rule denominator. `PROVEN` requires executable evidence for a generic
user-defined shape where that distinction matters; a stdlib-only path is never
the sole proof.

### A. Core type system

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Primitive types | Frozen | semantic gate and backend suites | yes | PROVEN |
| Tuples | Frozen | parser/semantic/MVIR suites | yes | PROVEN |
| Arrays | Frozen | semantic and const-array suites | yes | PROVEN |
| Structs | Frozen | broad semantic/driver suites | yes | PROVEN |
| Type aliases | Frozen | canonical serialization and resolver suites | partial | PARTIAL |
| Shared reference `&T` | Frozen | borrowck/lifetime suites | yes | PROVEN |
| Mutable reference `&rw T` | Frozen | borrowck suites | yes, with call-boundary gap | PARTIAL |
| Function types | Frozen | C-GAP-05/06 suites | yes | PROVEN |
| Generic type applications | Frozen | struct-generics plus SEM-GENERIC-01 | yes | PROVEN |

### B. Generic semantics

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Generic structs | Frozen | `struct_generics_test`; SEM-GENERIC-01 | yes | PROVEN |
| Generic functions | Frozen | generic dispatch suites; SEM-GENERIC-01 | yes | PROVEN |
| Multiple generic parameters | Frozen | C-GAP-04; SEM-GENERIC-01 | yes | PROVEN |
| Declaration-scoped repeated generic names | Frozen | artifact identity tests; SEM-GENERIC-01 | yes | PROVEN |
| Nested generic applications | Frozen | SEM-GENERIC-01 | yes | PROVEN |
| Generic methods | Frozen | `p3_tests` | yes | PROVEN |
| Generic trait arguments | Frozen | C-GAP-04; SEM-TRAIT-01 | yes | PROVEN |
| Generic impls | Frozen | semantic P1A/P3 and source iterator fixtures | yes | PROVEN |
| `where` / bound constraints | Frozen | P1A, P3, generic dispatch | yes | PROVEN |

### C. Traits and impls

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Trait declaration | Frozen | semantic and driver suites | yes | PROVEN |
| Inherent methods | Frozen | user inherent-priority source probe | yes | PROVEN |
| Trait methods | Frozen | generic dispatch and P3 | yes | PROVEN |
| Inherent-over-trait priority | Frozen | `user_inherent_method_precedence.ln` | yes | PROVEN |
| Generic traits | Frozen | C-GAP-04; SEM-TRAIT-01 | yes | PROVEN |
| Generic trait impls | Frozen | P1A/P3 and source probes | yes | PROVEN |
| Parameterized trait arguments | Frozen | C-GAP-04; SEM-TRAIT-01 | yes | PROVEN |
| Nominal-head orphan/coherence rule | Frozen | P1A reference/pointer controls; SEM-COHERENCE-01 | yes | PROVEN |
| Receiver `self` | Frozen | P3 and ownership suites | yes | PROVEN |
| Receiver `&self` | Frozen | generic dispatch/lifetime suites | yes | PROVEN |
| Receiver `&rw self` | Frozen | iterator/method source cases | yes, simple calls only | PARTIAL |

### D. Associated constructs

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Associated type declaration | Frozen | semantic P0C; SEM-ASSOC-01 | yes | PROVEN |
| Associated type binding | Frozen | semantic P0C; SEM-ASSOC-01 | yes | PROVEN |
| Same-provider projection | Frozen | artifact control; SEM-ASSOC-01 | yes | PROVEN |
| Cross-provider public projection A → B → C | Frozen | SEM-ASSOC-02: A owns trait/type/impl, B exposes `T::Output`, C consumes | yes | PROVEN |
| Associated functions | Frozen | P3 no-receiver trait cases | partial | PARTIAL |

### E. Ownership and moves

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Full move | Frozen | borrowck move suites | yes | PROVEN |
| Use-after-move rejection | Frozen | borrowck; SEM-OWN-01 | yes | PROVEN |
| Copy values remain usable | Frozen | Copy/Drop suites | yes | PROVEN |
| Non-Copy values move | Frozen | move/drop suites; SEM-OWN-01 | yes | PROVEN |
| Partial move without Drop | Frozen | subplace suites | yes | PROVEN |
| Partial move under Drop | Frozen | P0B; SEM-DROP-01 | yes | PROVEN |
| Move through branches | Frozen | P0B and CFG suites | yes | PROVEN |
| Move through match | Frozen | dormant UI fixtures only | no registered public-boundary proof | NOT TESTED |
| Move in loops | Frozen | borrowck/UI and language probes | partial | PARTIAL |
| Reassignment after move | Frozen | `test_reassign_soundness` | yes | PROVEN |

### F. Borrowing

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Shared borrow | Frozen | borrowck suites | yes | PROVEN |
| Mutable borrow | Frozen | borrowck suites | yes | PROVEN |
| Conflicting borrow rejection | Frozen | borrowck; SEM-BORROW-02 | yes | PROVEN |
| Multiple shared borrows | Frozen | borrowck; SEM-BORROW-01 | yes | PROVEN |
| Mutable after ended shared borrows | Frozen | SEM-BORROW-01 | yes | PROVEN |
| Shared after ended mutable borrow | Frozen | NLL suites | partial | PARTIAL |
| Non-escaping call receiver borrow ends at call boundary | Frozen | SEM-METHOD-01 plus escaping/overlap controls | yes | PROVEN |
| Borrow across branches | Frozen | NLL/CFG suites | partial | PARTIAL |
| Borrow across loops | Frozen | dormant UI fixtures only | no registered public-boundary proof | NOT TESTED |
| Borrow through aggregate fields | Frozen v1 whole-value domain | borrowck plus SEM-BORROW-01/02 | yes, conservative precision | PARTIAL |

### G. Lifetime semantics

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Valid returned borrow | Frozen | lifetime def-site suite | yes | PROVEN |
| Local borrow escape rejection | Frozen | lifetime def-site; SEM-LIFETIME-02 | yes | PROVEN |
| `life_from(a \| b)` provenance | Frozen | lifetime relation suite; SEM-LIFETIME-01 | yes | PROVEN |
| Receiver lifetime behavior | Frozen | virtual-dispatch lifetime suite | yes | PROVEN |
| Reference mutability in contracts | Frozen | lifetime and metadata suites | yes | PROVEN |
| Provider-exposed lifetime signature | Frozen | lifetime ABI suite | yes | PROVEN |
| `where outlives(...)` | Frozen | lifetime outlives suite | yes | PROVEN |

### H. Drop semantics

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Drop contract | Frozen | Copy/Drop and P0B suites | yes | PROVEN |
| Implicit drop | Frozen | drop/backend suites | yes | PROVEN |
| Moved value not dropped twice | Frozen | move/drop suites | yes | PROVEN |
| Partial move behavior | Frozen | P0B; SEM-DROP-01 | yes | PROVEN |
| Generic Drop | Frozen | C-GAP-01 and generic source probes | yes | PROVEN |
| Drop through aggregates | Frozen | P0B subplace suite | yes | PROVEN |
| Drop through control flow | Frozen | P0B branch/loop cases | yes | PROVEN |

### I. Control flow

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| `if` | Frozen | semantic/driver suites | yes | PROVEN |
| `match` | Frozen | match/exhaustiveness suites | yes | PROVEN |
| `loop` | Frozen | loop suites | yes | PROVEN |
| `while` | Frozen | loop/drop suites | yes | PROVEN |
| range `for` | Frozen | loop suites | yes | PROVEN |
| protocol-driven `for-in` | Frozen | SEM-ITER-01 plus owned Vec control | yes | PROVEN |
| `break` | Frozen | loop suites | yes | PROVEN |
| `continue` | Frozen | loop suites | yes | PROVEN |
| `return` | Frozen | pervasive | yes | PROVEN |
| Value-producing `if` / `match` | Frozen | semantic expression suites | yes | PROVEN |
| Void control-flow cases | Frozen | loop and function suites | yes | PROVEN |

### J. Iterator language contracts

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| User-defined `Iterator<T>` impl and direct `next` | Frozen | language source probes | yes | PROVEN |
| User-defined `IntoIterator<T, I>` and manual call | Frozen | language source probes | yes | PROVEN |
| `for-in` lowering through contracts | Frozen | SEM-ITER-01 | yes | PROVEN |
| Generic iterator implementation | Frozen | generic `CounterIter<T>` source proof | yes | PROVEN |
| Arbitrary user-defined iterable, independent of Vec | Frozen | SEM-ITER-01 | yes | PROVEN |

### K. Functions and calls

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Direct calls | Frozen | semantic gate/backend suites | yes | PROVEN |
| Generic calls | Frozen | generic dispatch; SEM-GENERIC-01 | yes | PROVEN |
| Function pointers | Frozen | C-GAP-05/06 | yes | PROVEN |
| Indirect calls | Frozen | C-GAP-05 | yes | PROVEN |
| Method calls generally | Frozen | trait/inherent suites | partial | PARTIAL |
| Unknown-method rejection | Frozen | SEM-DIAG-01; semantic gate regression | yes | PROVEN |
| Receiver mutability | Frozen | P3 plus SEM-METHOD-01 and two negative controls | yes | PROVEN |
| Extern calls where semantically relevant | Frozen | FFI/UI/backend cases | stdlib-heavy | PARTIAL |

### L. Modules and providers

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Root exports | Frozen | provider acceptance | yes | PROVEN |
| Explicit module | Frozen | provider acceptance | yes | PROVEN |
| Multiple modules | Frozen | module boundary suites | yes | PROVEN |
| Same local names in separate modules | Frozen | canonical identity tests | yes | PROVEN |
| Imports | Frozen | provider/module suites | yes | PROVEN |
| `using ... as ...` | Frozen | resolver/UI tests | yes | PROVEN |
| Multiple providers contributing to explicit module | Frozen | module merge/boundary suites | yes | PROVEN |
| Duplicate definitions | Frozen | provider acceptance; SEM-PROVIDER-01 | yes | PROVEN |
| Explicit-private field access rejection | Frozen public-default contract | SEM-VIS-01 | yes | PROVEN |
| Unresolved provider | Frozen | import invariant suites | yes | PROVEN |

### M. Const and comptime

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Const-evaluable expression | Frozen | P2A/advanced comptime; SEM-CONST-01 | yes | PROVEN |
| Const call | Frozen | advanced comptime | yes | PROVEN |
| Cycle rejection | Frozen | P2A | yes | PROVEN |
| Division-by-zero rejection | Frozen | SEM-CONST-02 | yes | PROVEN |
| Comptime mutation | Frozen | SEM-CONST-01 | yes | PROVEN |
| Comptime loops | Frozen | P2A | yes | PROVEN |
| I/O ban | Frozen | P2A/advanced negative cases | yes | PROVEN |
| Extern-call ban | Frozen | P2A/advanced negative cases | yes | PROVEN |
| Await ban | Frozen | advanced negative cases | yes | PROVEN |

### N. Error and diagnostic semantics

| Semantic rule | Spec status | Existing evidence | User-defined proof | Result |
|---|---|---|---|---|
| Deterministic semantic rejection | Frozen | negative suites including SEM-DIAG-01 | yes | PROVEN |
| E1003 private-access identity | Frozen | SEM-DIAG-05 | yes | COMPILER GAP |
| E3001 use-after-move identity | Frozen | SEM-DIAG-02 | yes | COMPILER GAP |
| E3002 partial-move-under-Drop identity | Frozen | SEM-DIAG-03 | yes | COMPILER GAP |
| E3003 borrow-conflict identity | Frozen | SEM-DIAG-04 | yes | COMPILER GAP |
| No compiler panic in Phase 0 corpus | Frozen doctrine | active and gap corpus executions | yes | PROVEN |
| No backend invariant failure in Phase 0 corpus | Frozen doctrine | active and gap corpus executions | yes | PROVEN |
| No silent semantic fallback | Frozen doctrine | SEM-DIAG-01 plus MVIR E6001 invariant guard | yes | PROVEN |

### Explicit frozen-v1 unsupported boundaries

| Construct | Result |
|---|---|
| Multi-trait `dyn` composition | UNSUPPORTED BY FROZEN SPEC |
| Associated types on `dyn Trait` | UNSUPPORTED BY FROZEN SPEC |
| Supertrait-based `dyn` composition | UNSUPPORTED BY FROZEN SPEC |
| `Box<dyn Trait>` / `Box<[T]>` | UNSUPPORTED BY FROZEN SPEC |
| Full general range lowering beyond the frozen subset | UNSUPPORTED BY FROZEN SPEC |
| Runtime `TypeValue` / full RTTI embedding | UNSUPPORTED BY FROZEN SPEC |
| Re-export and wildcard-import syntax | UNSUPPORTED BY FROZEN SPEC |
| File-style module declarations / provider namespaces | UNSUPPORTED BY FROZEN SPEC |
| Comptime I/O, extern, await, OS/thread effects | UNSUPPORTED BY FROZEN SPEC |

### Category-level existing coverage audit

| Category | Audit result | Reason |
|---|---|---|
| A. Core type system | PARTIAL | Alias proof is not yet a generic user-defined public-boundary case. |
| B. Generic semantics | PROVEN | New generic fixtures close the user-defined proof gap. |
| C. Traits / impls | PARTIAL | Coherence is proven; `&rw self` coverage remains partial. |
| D. Associated constructs | PARTIAL | Projections are proven; associated-function coverage remains partial. |
| E. Ownership / moves | PARTIAL | Move-through-match lacks a registered public-boundary proof. |
| F. Borrowing | PARTIAL | Call-boundary loan termination is incorrect; loop proof is missing. |
| G. Lifetime semantics | PROVEN | Valid, invalid, union, receiver, outlives, and provider cases pass. |
| H. Drop semantics | PROVEN | P0B, Copy/Drop, and new source cases cover the frozen rules. |
| I. Control flow | PARTIAL | Protocol-driven `for-in` is rejected. |
| J. Iterator contracts | PARTIAL | Direct/manual protocol calls work; language lowering does not. |
| K. Functions / calls | PARTIAL | Unknown methods reject; mutable call boundaries remain incorrect. |
| L. Modules / providers | PROVEN | Public-default fields and explicit-private rejection match the corrected frozen contract. |
| M. Const / comptime | PROVEN | Positive execution and explicit negative rejection are covered. |
| N. Diagnostics | PARTIAL | Silent fallback is closed; the frozen numeric registry remains incomplete. |

## Conformance cases

Harness: `mellis-rs/crates/luna-cli/tests/semantic_maturity_phase0.rs`.
Fixtures: `mellis-rs/tests/semantic_maturity/`. All entrypoints are `.ln` and
the harness enters through the public `luna check` command.

| Case | Validity | Invariant | Layer | Outcome |
|---|---|---|---|---|
| SEM-GENERIC-01 | valid | Nested user-defined generics and declaration-local names | resolver/typecheck/mono/MVIR | PASS |
| SEM-TRAIT-01 | valid | Generic trait argument and user-defined impl dispatch | typecheck/mono | PASS |
| SEM-BORROW-01 | valid | Shared field loans end before mutable loan | borrowck/NLL | PASS |
| SEM-BORROW-02 | invalid | Shared/mutable field conflict rejects | borrowck | PASS (rejected) |
| SEM-LIFETIME-01 | valid | `life_from(a \| b)` accepts either input | lifetime/MVIR | PASS |
| SEM-LIFETIME-02 | invalid | Local reference escape rejects | lifetime | PASS (E3005) |
| SEM-OWN-01 | invalid | Use after moving generic owner rejects | move analysis | PASS (rejected) |
| SEM-DROP-01 | invalid | Partial move under Drop rejects | move/drop analysis | PASS (symbolic code) |
| SEM-ASSOC-01 | valid | Same-provider associated projection | typecheck/mono | PASS |
| SEM-ASSOC-02 | valid | A owns trait/type/impl; B publicly exposes `T::Output`; C consumes B | provider/typecheck/mono | PASS |
| SEM-PROVIDER-01 | invalid | Duplicate root export from two providers rejects | provider/resolver | PASS (E1002) |
| SEM-CONST-01 | valid | Pure comptime local mutation produces const | comptime | PASS |
| SEM-CONST-02 | invalid | Division by zero rejects without fallback | comptime | PASS (rejected) |
| SEM-ITER-01 | valid | Generic user-defined `for-in` through language contracts | typecheck/mono/MVIR | PASS (SEM-GAP-01 closed) |
| SEM-METHOD-01 | valid | Sequential non-escaping `&rw self` calls | effect inference/borrowck | PASS (SEM-GAP-02 closed) |
| SEM-DIAG-01 | invalid | Unknown user-defined method rejects | typecheck/MVIR | PASS (SEM-GAP-03 closed) |
| SEM-DIAG-02 | invalid | Use after move emits frozen E3001 | diagnostics | EXPECTED FAIL (SEM-GAP-04) |
| SEM-DIAG-03 | invalid | Partial move emits frozen E3002 | diagnostics | EXPECTED FAIL (SEM-GAP-07) |
| SEM-DIAG-04 | invalid | Borrow conflict emits frozen E3003 | diagnostics | EXPECTED FAIL (SEM-GAP-08) |
| SEM-VIS-01 | invalid | Explicit-private field rejects externally | parser/resolver | PASS (rejected) |
| SEM-DIAG-05 | invalid | Explicit-private access emits frozen E1003 | diagnostics | EXPECTED FAIL (SEM-GAP-09) |
| SEM-COHERENCE-01 | invalid | Reference-head orphan impl rejects | coherence | PASS (SEM-GAP-06 closed) |

Why existing tests were insufficient:

- `ui_tests.rs` registers only a narrow `try_*` subset; the large `ui/` tree is
  not executable evidence by mere presence.
- Existing iterator regressions were dormant manual files or stdlib-container
  cases; neither proved arbitrary user-defined `for-in` lowering.
- Existing receiver tests did not isolate sequential non-escaping mutable calls.
- Existing negative tests frequently asserted message fragments or symbolic
  labels rather than the frozen numeric diagnostic identity.
- Historical associated-type reports described an externally owned impl but did
  not contain a minimal transitive public-signature reproduction. The corrected
  SEM-ASSOC-02 is source-only A → B → C: provider A owns the trait, concrete type,
  and associated binding; provider B publicly returns A's `T::Output` projection;
  provider C instantiates and consumes B's API. It passes at this commit.

### User-defined generic coverage

Eight cases exercise strictly generic user-defined declarations rather than
stdlib container behavior: SEM-GENERIC-01, SEM-TRAIT-01, SEM-BORROW-01,
SEM-BORROW-02, SEM-OWN-01, SEM-ASSOC-01, and SEM-ASSOC-02. SEM-ITER-01 adds an
eighth user-defined protocol case (`CounterIter<T>`). No maturity
claim in those classes relies solely on Vec, String, HashMap, HashSet, Option,
or Result.

### Frozen diagnostic-contract audit

| Code | Frozen contract | Current evidence | Baseline classification |
|---|---|---|---|
| E1002 | Duplicate definition | SEM-PROVIDER-01 emits E1002 | CONFORMANT |
| E1003 | Private access | SEM-VIS-01 rejects, but omits E1003 | COMPILER GAP (SEM-GAP-09) |
| E1004 | Unresolved provider | importer rejects with `Could not resolve module provider`; no isolated Phase 0 code assertion | PARTIAL / NOT ISOLATED |
| E2001 | Type mismatch | semantic-gate test rejects by message; no isolated Phase 0 code assertion | PARTIAL / NOT ISOLATED |
| E2016 | Lifetime violation | lifetime suites emit E2016 | CONFORMANT |
| E3001 | Use after move | SEM-OWN-01 rejects, but omits E3001 | COMPILER GAP (SEM-GAP-04) |
| E3002 | Partial move under Drop | emits `E_PARTIAL_MOVE_UNDER_DROP` | COMPILER GAP (SEM-GAP-07) |
| E3003 | Borrow conflict | SEM-BORROW-02 rejects, but omits E3003 | COMPILER GAP (SEM-GAP-08) |
| E3005 | Local borrow escape | SEM-LIFETIME-02 emits E3005 | CONFORMANT |
| E6001 | Backend invariant | registry contract exists; no user-source case should intentionally reach this invariant | NOT EXERCISED |

The diagnostic representation is a shared enabling deficiency: production
`Diagnostic` has no typed code field. The observed violations are nevertheless
**multiple independent conformance gaps**, because E1003, E3001, E3002, and
E3003 have distinct frozen meanings, triggering programs, emitters, and owning
subsystems. Fixing one emitter would not satisfy the others. The old broad
SEM-GAP-04 was therefore decomposed as follows: SEM-GAP-04 now means E3001;
E3002 moved to SEM-GAP-07; E3003 moved to SEM-GAP-08. SEM-GAP-09 records the
newly verified E1003 identity failure. SEM-GAP-05 is withdrawn without reusing
or shifting later stable IDs.

## Compiler gaps

### SEM-GAP-01 — User-defined `for-in` rejected by MVIR

Status:
  CLOSED — PHASE 1B

Severity:
  P1

Frozen semantic:
  `for value in iterable` lowers through the frozen `IntoIterator` and `Iterator`
  language contracts for arbitrary user-defined types.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_iter_01_user_defined_for_in.ln`

Expected:
  Source passes semantic analysis and lowers to contract calls.

Phase 0 actual:
  `luna check` rejects with `for-each over non-range iterables is not yet supported`.

Diagnostic:
  Uncoded ad-hoc MVIR error.

Source-only reproduction:
  YES

Affected subsystem:
  typechecker / monomorphization / MVIR

Root cause:
  PROVEN

Notes:
  The typechecker previously checked only the iterable expression; it did not
  select or record an `IntoIterator`/`Iterator` protocol plan. Monomorphization
  therefore had no synthetic `into_iter`/`next` instances, and MVIR handled a
  range AST directly while rejecting every other `ForEach`.

Phase 1B resolution:
  Typechecking now selects both language contracts by lang-item identity,
  records their impl substitutions and concrete Item/IntoIter/Option types,
  and rejects missing or ambiguous implementations before lowering.
  Monomorphization queues both synthetic calls. MVIR invokes those selected
  instances, branches on the Option::Some lang-item variant, and owns/drops the
  iterator temporary at loop exit. The implementation contains no Vec, Slice,
  provider, or concrete-container branch. The generic `CounterIter<T>` repro and
  an owned Vec control both build successfully.

### SEM-GAP-02 — Non-escaping mutable receiver loan survives call boundary

Status:
  CLOSED — PHASE 1B

Severity:
  P1

Frozen semantic:
  A non-escaping `&rw self` receiver loan ends after the call's last use, allowing
  a subsequent mutable call and later field access.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_method_01_sequential_mut_receivers.ln`

Expected:
  Two sequential `counter.advance()` calls and a later read are accepted.

Phase 0 actual:
  Borrowck reports that the second call is already borrowed as `&rw`, followed by
  an access-while-borrowed error.

Diagnostic:
  No frozen numeric code; two ad-hoc borrow messages.

Source-only reproduction:
  YES

Affected subsystem:
  borrowck / call-effect loan lifetime

Root cause:
  PROVEN

Notes:
  `compile` computed interprocedural effects with `SemanticContext`, while
  `check` used context-free inference. Context-free inference conservatively
  treated a primitive return value as borrow-carrying, so the receiver loan was
  propagated through the call result and remained live.

Phase 1B resolution:
  `check` now uses semantic-context-aware effect inference, matching `compile`.
  Semantic return types terminate provenance for values that cannot contain a
  reference. Load propagation remains conservative so real loans carried by
  aggregates, projections, and iterator-adapter chains are not shortened.
  Sequential non-escaping calls pass; a returned `&rw self` loan and an
  overlapping shared/mutable receiver loan remain rejected.

### SEM-GAP-03 — Unknown method silently lowers to zero

Status:
  CLOSED — PHASE 1A

Severity:
  P0

Frozen semantic:
  A call to an undeclared method is an explicit deterministic type error and must
  never reach lowering.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_diag_01_unknown_method.ln`

Expected:
  Type checking rejects `value.missing_method()`.

Phase 0 actual:
  `luna check` succeeds. `luna build` emits an executable whose MVIR replaces the
  call/return with `ret 0`; the executable exits 0.

Phase 0 diagnostic:
  none

Source-only reproduction:
  YES

Affected subsystem:
  typechecker / MVIR

Root cause:
  PROVEN

Notes:
  The method-call typechecker falls through to a fresh inference variable without
  a diagnostic when lookup fails. MVIR then returns `Operand::Number("0")` when it
  has no method symbol. This is the one confirmed silent-fallback case.

Phase 1A resolution:
  Failed lookup now emits `Method ... not found for type ...` and returns the
  semantic error type, so the driver gate stops before MVIR. MVIR also emits the
  existing E6001 backend-invariant identity if an unresolved method ever reaches
  lowering; its placeholder operand is never accepted or emitted by the driver.

### SEM-GAP-04 — Use-after-move rejection omits E3001

Status:
  OPEN

Severity:
  P2

Frozen semantic:
  Use after move rejects with the stable E3001 identity from
  `docs/diagnostics/diagnostics-v1.md`.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_own_01_use_after_move.ln`

Expected:
  Rejection identified as E3001.

Actual:
  Correct rejection category, but the message is only `Use of moved value`.

Diagnostic:
  missing identity (expected existing E3001)

Source-only reproduction:
  YES

Affected subsystem:
  borrowck move analysis / common diagnostics

Root cause:
  PROVEN

Notes:
  The missing code originates in the use-after-move emitter. Production
  `Diagnostic` also lacks a typed code field, but that shared structural weakness
  does not collapse this independently testable E3001 contract into other gaps.

### Withdrawn ID: SEM-GAP-05

The original SEM-GAP-05 is withdrawn, not renumbered. It incorrectly expected
unmodified fields to be private. Under the corrected frozen contract fields are
public by default; SEM-VIS-01 proves that an explicit `private` field is rejected.
The missing E1003 identity is separately tracked as SEM-GAP-09.

### SEM-GAP-06 — Reference-head orphan impl is accepted

Status:
  CLOSED — PHASE 1A

Severity:
  P0

Frozen semantic:
  Coherence checks the target's nominal head only; nested local types do not make
  a foreign head local, and frozen Rule D says fundamental wrappers do not alter
  the base rule.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_coherence_01_reference_head_orphan.ln`

Expected:
  `impl Iterator<LocalItem> for &LocalItem` is rejected as an orphan impl.

Phase 0 actual:
  `luna check` succeeds with no diagnostic.

Phase 0 diagnostic:
  none

Source-only reproduction:
  YES

Affected subsystem:
  semantic coherence

Root cause:
  PROVEN

Notes:
  `SemanticContext::nominal_head` recursively unwraps references and pointers,
  discovering the nested local type and treating the head as local.

Phase 1A resolution:
  `nominal_head` no longer unwraps `&T` or `*T`. Reference-head and pointer-head
  foreign-trait impls now reject with E_ORPHAN_IMPL, while a local trait remains
  legal on the same heads. The three stdlib impls that violated the frozen rule
  (`IntoIterator for &Vec`, `&HashMap`, and `&HashSet`) were removed; their
  explicit `.iter()` APIs and owned `IntoIterator` impls remain available.

### SEM-GAP-07 — Partial move under Drop emits a symbolic code instead of E3002

Status:
  OPEN

Severity:
  P2

Frozen semantic:
  Partial move under Drop rejects with the stable E3002 identity.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_drop_01_partial_move_under_drop.ln`

Expected:
  Rejection identified as E3002.

Actual:
  Correct rejection category, but the message contains
  `E_PARTIAL_MOVE_UNDER_DROP`.

Diagnostic:
  wrong identity (`E_PARTIAL_MOVE_UNDER_DROP`, expected existing E3002)

Source-only reproduction:
  YES

Affected subsystem:
  borrowck move analysis / common diagnostics

Root cause:
  PROVEN

Notes:
  This is a distinct emitter and frozen contract from E3001 even though both are
  affected by the common untyped diagnostic representation.

### SEM-GAP-08 — Borrow-conflict rejection omits E3003

Status:
  OPEN

Severity:
  P2

Frozen semantic:
  A borrow aliasing conflict rejects with the stable E3003 identity.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_borrow_02_conflicting_field_borrow.ln`

Expected:
  Rejection identified as E3003.

Actual:
  Correct rejection category, but the message is an uncoded `Cannot borrow ...`
  diagnostic.

Diagnostic:
  missing identity (expected existing E3003)

Source-only reproduction:
  YES

Affected subsystem:
  borrowck borrow analysis / common diagnostics

Root cause:
  PROVEN

Notes:
  The E3003 contract is independently asserted because this diagnostic comes
  from borrow analysis, not either move-analysis emitter.

### SEM-GAP-09 — Explicit-private access rejection omits E1003

Status:
  OPEN

Severity:
  P2

Frozen semantic:
  Struct fields are public by default; an explicitly `private` field is
  inaccessible across the provider boundary and rejection carries E1003.

Minimal Luna reproduction:
  `mellis-rs/tests/semantic_maturity/sem_vis_01_explicit_private_field/`

Expected:
  External `secret.value` access rejects with E1003.

Actual:
  Access is correctly rejected as private, but the diagnostic has no numeric
  identity.

Diagnostic:
  missing identity (expected existing E1003)

Source-only reproduction:
  YES

Affected subsystem:
  resolver/typechecker field visibility / common diagnostics

Root cause:
  PROVEN

Notes:
  This is not a field-visibility semantic bug. SEM-GAP-05 was withdrawn because
  its private-by-default premise was wrong; SEM-GAP-09 covers only the frozen
  diagnostic identity.

### Severity summary

- Closed P0: SEM-GAP-03, SEM-GAP-06
- Open P0: none
- Closed P1: SEM-GAP-01, SEM-GAP-02
- Open P1: none
- Open P2: SEM-GAP-04, SEM-GAP-07, SEM-GAP-08, SEM-GAP-09
- Open P3: none

### Crashes and silent fallbacks

- Compiler panics caused by Phase 0 Luna programs: **0**.
- Backend invariant failures caused by Phase 0 Luna programs: **0**.
- Phase 0 baseline silent fallback cases: **1** (SEM-GAP-03).
- Current silent fallback cases in the corpus: **0**.
- The whole-workspace test harness panic is a separate baseline fixture issue:
  required generated component `.llib` files are absent from this clean checkout.

### Known associated-type result

- Same-provider: **PASS** (SEM-ASSOC-01).
- Cross-provider A → B → C: **PASS** (SEM-ASSOC-02: A owns the trait,
  concrete type, and associated binding; B exposes `T::Output` in a public
  generic API; C consumes it).
- Source-only: **YES**.
- Artifact involved: **NO**.
- Historical limitation reproduced: **NO** on baseline
  `88b9779e760ee08d76cb75aa510e55868590abb1`.

## Diagnostics observed

| Negative case | Actual identity/output | Conformance |
|---|---|---|
| SEM-BORROW-02 conflict | no code; `Cannot borrow ... as &rw because ... borrowed as &` | rejection correct, registry incomplete |
| SEM-LIFETIME-02 local escape | E3005 | correct |
| SEM-OWN-01 use after move | no code; `Use of moved value` | rejection correct, registry incomplete |
| SEM-DROP-01 partial move | `E_PARTIAL_MOVE_UNDER_DROP` | wrong code; frozen E3002 |
| SEM-PROVIDER-01 duplicate | E1002 | correct |
| SEM-CONST-02 division by zero | no code; comptime evaluation error | rejection correct, registry incomplete |
| SEM-DIAG-01 unknown method | uncoded `Method ... not found for type ...` | semantic rejection correct; GAP-03 closed |
| SEM-VIS-01 explicit-private access | no code; `Field ... is private` | semantic rejection correct; E1003 gap |
| SEM-COHERENCE-01 orphan impl | E_ORPHAN_IMPL | correct; GAP-06 closed |

No new diagnostic code was invented.

## Baseline metrics

- Frozen semantic rules inventoried: **119**
- Rules PROVEN: **93**
- Rules PARTIAL: **16**
- Rules NOT TESTED: **2**
- Rules in COMPILER GAP state: **8**
- Unique compiler gaps: **8**
- P0: **2**
- P1: **2**
- P2: **4**
- P3: **0**
- Compiler panics: **0**
- Silent fallback cases: **1**
- User-defined generic coverage count: **7 cases**
- New conformance tests: **22**
- Unique valid Luna programs: **9**
- Unique invalid Luna programs: **9**

### Current metrics after Phase 1A

- Frozen semantic rules inventoried: **119**
- Rules PROVEN: **97**
- Rules PARTIAL: **14**
- Rules NOT TESTED: **2**
- Rules in COMPILER GAP state: **6**
- Unique open compiler gaps: **6**
- Closed compiler gaps: **2**
- Open P0: **0**
- Open P1: **2**
- Open P2: **4**
- Open P3: **0**
- Compiler panics in the conformance corpus: **0**
- Silent fallback cases in the conformance corpus: **0**
- User-defined generic coverage count: **7 cases**
- Phase 0 conformance tests: **22**
- Active conformance tests: **16**
- Ignored open-gap reproductions: **6**
- Phase 1A Rust regression tests added: **4**

### Current metrics after Phase 1B

- Frozen semantic rules inventoried: **119**
- Rules PROVEN: **103**
- Rules PARTIAL: **10**
- Rules NOT TESTED: **2**
- Rules in COMPILER GAP state: **4**
- Unique open compiler gaps: **4**
- Closed compiler gaps: **4**
- Open P0: **0**
- Open P1: **0**
- Open P2: **4**
- Open P3: **0**
- Compiler panics in the conformance corpus: **0**
- Silent fallback cases in the conformance corpus: **0**
- User-defined generic coverage count: **8 cases**
- Phase 0 conformance tests: **22**
- Active conformance tests: **18**
- Ignored open-gap reproductions: **4**
- Phase 1B source control fixtures added: **3**

The Rust harness remains at 22 tests. Phase 1B converts the existing GAP-01 and
GAP-02 reproductions from ignored expected failures into active passing tests.
It adds three source scenarios inside those tests: owned Vec `for-in`, an
escaping mutable-receiver borrow, and shared/mutable receiver overlap. The
current source corpus therefore exercises 21 scenarios (10 valid + 11 invalid),
while four invalid scenarios retain a second ignored assertion for their exact
Phase 1C diagnostic identity.

The test/program totals count different things. The corpus contains 18 unique
program scenarios (9 valid + 9 invalid). Four invalid programs each have a
second ignored assertion dedicated to exact diagnostic identity: E1003, E3001,
E3002, and E3003. Thus **18 + 4 = 22 tests**. In the original baseline, only the
E3002 identity assertion was duplicated, which explains its previously
unreconciled **19th test**.

## Test execution baseline

Commands use `C:/Users/84387/.cargo/bin/cargo.exe`; test processes prepend
`D:/Programs/LLVM-DEV/bin` to `PATH` for `LLVM-C.dll`.

| Command | Result |
|---|---|
| `cargo test --workspace -- --list` | 812 tests discovered; 118 test binaries; 26 zero-test binaries |
| `cargo test -p luna-cli --test semantic_maturity_phase0 -- --test-threads=1` | 14 passed, 0 failed, 8 ignored |
| `cargo test -p luna-cli --test semantic_maturity_phase0 -- --ignored --test-threads=1` | 0 passed, 8 failed as the eight documented gap expectations |
| `cargo test -p luna-semantic -- --test-threads=1` | 106 passed, 0 failed |
| `cargo test -p luna-borrowck -- --test-threads=1` | 25 passed, 0 failed |
| 18 named relevant `luna-driver` binaries (listed below) | 152 passed, 0 failed; 2 binaries discovered zero tests |
| `cargo test --workspace -- --test-threads=1` | stopped at baseline artifact test: 157 passed, 1 failed, 6 ignored before stop |
| `cargo test --workspace` | same baseline stop: 157 passed, 1 failed, 6 ignored before stop |

The two workspace rows are retained from the original Phase 0 baseline and were
not rerun during this correction pass. Per scope, no generated `.llib` was built
or repaired.

The workspace failure is classified as a **BASELINE ENVIRONMENT /
GENERATED-ARTIFACT FAILURE**, not a semantic compiler gap. It is
`core_provider_baseline_acceptance_tests::component_artifacts_are_present_and_readable`.
It panics because generated component artifacts such as
`libs/external/lang/into_iterator.llib` / `libs/external/core/try.llib` are absent
from the clean baseline checkout. It predates and is independent of the new
source conformance corpus; Phase 0 did not generate, repair, or modify sysroot
artifacts.

### Phase 1A verification after the hygiene rebase

The pre-implementation rerun on hygiene-complete `main` was unchanged:
**14 passed / 8 ignored**, and the ignored command reproduced **8/8** Phase 0
gaps. After closing SEM-GAP-03 and SEM-GAP-06:

| Command | Result |
|---|---|
| `cargo test -p luna-cli --test semantic_maturity_phase0 -- --test-threads=1` | 16 passed, 0 failed, 6 ignored |
| `cargo test -p luna-cli --test semantic_maturity_phase0 -- --ignored --test-threads=1` | 0 passed, 6 failed as the six remaining gap expectations; 16 filtered out |
| `cargo test -p luna-semantic -- --test-threads=1` | 109 passed, 0 failed |
| `cargo test -p luna-borrowck -- --test-threads=1` | 25 passed, 0 failed |
| 16 current named relevant `luna-driver` binaries | 153 passed, 0 failed |
| four affected stdlib iterator/collection binaries | 40 passed, 0 failed |
| `cargo test --workspace -- --test-threads=1` | preceding binaries passed; failing binary: 6 passed, 1 generated-artifact failure |
| `cargo test --workspace` | preceding binaries passed; failing binary: 6 passed, 1 generated-artifact failure |

The hygiene cleanup removed the zero-test targets `module_merge_tests` and
`hierarchical_provider_tests`; the Phase 1A targeted command therefore uses the
16 remaining named binaries without changing semantic expectations. Both
workspace commands were rerun with one build job, incremental compilation off,
and debuginfo disabled after the first compile attempt exhausted the Windows
build volume. Their semantic result is the same known missing-`.llib` baseline
failure; no artifact was generated or repaired.

### Phase 1B verification

| Command / group | Result |
|---|---|
| `cargo test -p luna-cli --test semantic_maturity_phase0 -- --test-threads=1` | 18 passed, 0 failed, 4 ignored |
| `cargo test -p luna-cli --test semantic_maturity_phase0 -- --ignored --test-threads=1` | 0 passed, 4 failed as the four remaining diagnostic gaps; 18 filtered out |
| `luna build --lib` for generic `CounterIter<T>` and owned Vec fixtures | 2 built, 0 failed; protocol calls lowered through backend |
| `cargo test -p luna-semantic -- --test-threads=1` | 109 passed, 0 failed |
| `cargo test -p luna-borrowck -- --test-threads=1` | 25 passed, 0 failed |
| nine targeted semantic/generic/lifetime/receiver driver binaries | 87 passed, 0 failed |
| iterator adapters plus terminal/collect/slice/Vec suites | 69 passed, 0 failed |
| collection iterator suite | 2 passed, 6 generated-artifact fingerprint failures |
| `cargo test --workspace -- --test-threads=1` | preceding binaries passed; `core_provider_baseline_acceptance_tests`: 6 passed, 1 generated-artifact failure |
| `cargo test --workspace` | same baseline stop: 6 passed, 1 generated-artifact failure in the failing binary |

The four ignored tests are exactly SEM-GAP-04, SEM-GAP-07, SEM-GAP-08, and
SEM-GAP-09. Their actual outputs remain, respectively, uncoded use-after-move,
symbolic `E_PARTIAL_MOVE_UNDER_DROP`, uncoded borrow conflict, and uncoded
private-field rejection.

The collection suite failures are also classified as **BASELINE ENVIRONMENT /
GENERATED-ARTIFACT FAILURE**. Six HashMap/HashSet cases strict-reject a stale
`hashmap.llib` whose source fingerprint no longer matches. No ad-hoc artifact
copying or regeneration was performed. The source-only GAP-01 fixture, its
owned-Vec control, and all 69 unaffected iterator/container cases pass.

Exact targeted driver command used for the correction pass:

```text
cargo test -p luna-driver \
  --test semantic_gate_tests \
  --test generic_trait_dispatch_acceptance_tests \
  --test compiler_gap_c_gap_04_trait_bound_generic_args_tests \
  --test lifetime_def_site_acceptance_tests \
  --test lifetime_outlives_acceptance_tests \
  --test lifetime_relation_abi_acceptance_tests \
  --test lifetime_virtual_dispatch_acceptance_tests \
  --test p0b_drop_subplace_tests \
  --test p3_tests \
  --test provider_module_contract_acceptance_tests \
  --test module_merge_tests \
  --test module_import_boundary_tests \
  --test import_invariants_tests \
  --test hierarchical_provider_tests \
  --test compiler_gap_c_gap_05_indirect_call_tests \
  --test compiler_gap_c_gap_06_function_pointer_mangling_tests \
  --test adv_comptime_dyn_tests \
  --test test_reassign_soundness \
  -- --test-threads=1
```

Initial infrastructure observations, not semantic evidence:

- `cargo` was not on `PATH`; the explicit rustup cargo path was used.
- The first test-list attempt without LLVM runtime path failed with
  `STATUS_DLL_NOT_FOUND`; adding the installed LLVM development `bin` directory
  resolved it.

## Phase 1 proposal

1. **Completed in Phase 1A:** reject unresolved method calls in the typechecker
   and retain an E6001 MVIR invariant guard; no zero fallback is accepted.
2. **Completed in Phase 1A:** enforce nominal-head coherence for references and
   pointers, with negative and local-trait control regressions.
3. **Completed in Phase 1B:** implement generic user-defined `for-in` lowering through language
   contracts across typecheck, monomorphization, and MVIR, without Vec or provider special cases.
4. **Completed in Phase 1B:** terminate non-escaping receiver loans from semantic return types
   while preserving conservative aggregate/adapter loan propagation.
5. **Phase 1C:** implement the typed frozen diagnostic registry, then migrate the independent
   E1003/E3001/E3002/E3003 emitters without treating one fix as proof of the
   other contracts (P2).
6. Add the two missing public-boundary proofs (move through match and borrow
   across loops), then upgrade remaining PARTIAL rows by subsystem.

## Proposed maturity gate

- No P0 semantic bugs in the source conformance corpus.
- No compiler panics or backend invariant failures for frozen semantic cases.
- No silent fallback values or skipped lowering.
- Unsupported states reject explicitly with a frozen diagnostic code.
- No provider- or stdlib-type-specific semantic paths.
- User-defined generic proof covers every major semantic class.
- Adding ordinary stdlib APIs rarely requires Rust compiler changes.
- Relevant semantic suites and the isolated conformance corpus are green; any
  workspace infrastructure prerequisite is explicit and reproducible.

## Scope and architecture compliance

The following entries describe the completed Phase 0 baseline/correction pass:

- Production compiler source modified: **NO**.
- Stdlib semantic source modified: **NO**.
- Runtime ABI modified: **NO**.
- Provider semantics modified: **NO**.
- Artifact semantics modified: **NO**.
- New syntax added: **NO**.
- New semantic feature added: **NO**.
- Hygiene work duplicated: **NO**.
- Phase 1 implementation started: **NO**.

Only a new audit document, one isolated CLI conformance harness, and isolated
source fixtures were added.

Phase 1A scope:

- Production compiler source modified: **YES**, limited to missing-method
  rejection, the MVIR invariant guard, and nominal-head locality.
- Stdlib semantic source modified: **YES**, limited to removal of three orphan
  borrowed-`IntoIterator` impls made invalid by the frozen rule.
- Runtime ABI modified: **NO**.
- Artifact semantics modified: **NO**.
- New syntax added: **NO**.
- New semantic feature added: **NO**; two frozen rejection rules are enforced.
- Provider-specific exception added: **NO**.
- Hygiene work duplicated: **NO**.
- Phase 1B/1C implementation started: **NO**.

Phase 1B scope:

- Production compiler source modified: **YES**, limited to semantic protocol
  selection/recording, synthetic iterator monomorphization/lowering, iterator
  temporary drop scopes, and receiver call-effect provenance.
- Stdlib semantic source modified: **NO**.
- Runtime ABI modified: **NO**.
- Artifact schema or semantics modified: **NO**.
- New syntax added: **NO**.
- New language semantic added: **NO**; two frozen valid-program rules are now
  implemented.
- Container- or provider-specific compiler branch added: **NO**.
- Stdlib API changed: **NO**.
- Hygiene work duplicated: **NO**.
- Phase 1C implementation started: **NO**.

## Correction-pass verdict

The corrected semantic authority, matrix counts, case/program totals, gap IDs,
and executable results reconcile. No Phase 1 implementation was started.

**SEM-MATURITY-01 PHASE 0 BASELINE CORRECTED**

## Phase 1A verdict

The hygiene rebase preserved the full eight-gap baseline. SEM-GAP-03 and
SEM-GAP-06 are closed without renumbering any gap ID; SEM-GAP-05 remains
withdrawn and is not reused. The source corpus has no open P0, no panic, and no
silent fallback. The only workspace failure is the unchanged generated-artifact
prerequisite documented above.

**SEM-MATURITY-01 PHASE 1A SEMANTIC SAFETY CLOSURE COMPLETE**

## Phase 1B verdict

SEM-GAP-01 and SEM-GAP-02 are closed by their original source-only
reproductions. The generic user-defined iterable and owned Vec control pass
through backend lowering. Sequential non-escaping mutable receiver calls pass,
while escaping and overlapping receiver-loan controls still reject. The Phase 0
corpus is 18 passed / 4 ignored, with no panic or silent fallback introduced.
Workspace and six collection-iterator failures remain isolated generated-LLIB
baseline environment failures.

**SEM-MATURITY-01 PHASE 1B VALID-PROGRAM COMPLETENESS COMPLETE**
