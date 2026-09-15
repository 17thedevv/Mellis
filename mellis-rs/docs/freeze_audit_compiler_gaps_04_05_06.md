# Compiler Gap Freeze Audit: C-GAP-04, C-GAP-05, C-GAP-06

**Date:** 2026-09-15  
**Scope:** Generic Compiler Capabilities Isolation (`Rule 11` & `luna-stdlib-compiler-boundary`)  
**Status:** FROZEN & VERIFIED  

---

## 1. Executive Summary

During initial preparation for Phase Stdlib-05.1 (Iterator Adapters Foundation), three latent compiler capability gaps were encountered. Per `Rule 11` and the `luna-stdlib-compiler-boundary` protocol, **work on Stdlib-05.1 was stopped immediately**, and the compiler capabilities were isolated from the standard library into three independent Compiler Gaps (`C-GAP-04`, `C-GAP-05`, `C-GAP-06`).

All three capabilities have been implemented purely as generic language machinery, verified with dedicated user-defined test suites independent of stdlib, free of any stdlib-specific branches, and verified with zero regression across the existing test suites.

---

## 2. Invariant & Architecture Compliance

| Metric | Status | Evidence |
|---|---|---|
| **Stdlib-Independent** | **PASS** | Test suites use only user-defined structs, traits, and types. No `<core>` or stdlib imports required. |
| **No Stdlib-Specific Branches** | **PASS** | Verified via grep: 0 occurrences of `Map`, `Filter`, `Enumerate`, `Take`, `Skip`, `Zip`, `iter_map`, etc. in compiler crates. |
| **No Semantic Fallback Corruption** | **PASS** | Strict elimination of `const_zero()` fallback in `CallIndirect`, `CallDirect`, and `CallVirt`. Void maps to unit struct `{}` and unexpected void returns error as invariant violations. |
| **Canonical Naming SSOT** | **PASS** | Function value references delegate to `CanonicalInstanceIdentity::symbol_name_with_tables` without ad-hoc `!= "main"` exceptions. |
| **Generic Instance Identity for Function Values** | **PASS** | Generic function values (`identity<i32>` vs `identity<i64>`) bind concrete `subst` into `CanonicalInstanceIdentity`, producing distinct symbols (`_MFN8identityEGlE` != `_MFN8identityEGxE`) and monomorphized instances. |

---

## 3. Detailed Gap Breakdown

### C-GAP-04: Parameterized Trait-Bound Generic Arguments Propagation
- **Root Cause:** When generic parameters were declared with parameterized trait bounds (`T: Trait<Arg1, Arg2>`), the parser captured `generic_args`, but `SemanticTables::trait_bounds` dropped them. Furthermore, method resolution on generic parameter receivers lacked substitution of the trait's generic parameters, blocking valid generic compositions with `E_CANNOT_MONOMORPHIZE`.
- **Compiler Implementation:**
  - `crates/luna-semantic/src/semantic_tables.rs`: Added `pub trait_args: Vec<SemanticTypeId>` to `TraitBound` and `pub trait_generic_params: HashMap<SymbolId, Vec<SymbolId>>` to `SemanticTables`.
  - `crates/luna-semantic/src/resolver.rs`: Populated `trait_generic_params` on `Decl::Trait`.
  - `crates/luna-semantic/src/typechecker.rs`: Lowered and recorded trait argument types in `populate_trait_bounds`. In `Expr::MethodCall` and `Expr::Member` on `GenericParam`, substituted trait generic parameters with `bound.trait_args`.
- **Acceptance Verification:**
  - File: `crates/luna-driver/tests/compiler_gap_c_gap_04_trait_bound_generic_args_tests.rs`
  - Tests:
    1. `test_c_gap_04_parameterized_trait_bound_distinct_args`: User-defined `Pipeline<In, Out, Tr: Transformer<In, Out>>` with `In != Out` (`i32 != i64`).
    2. `test_c_gap_04_generic_function_trait_bound_propagation`: User-defined `execute<A, B, C: Converter<A, B>>(c: &C, arg: A) -> B`.
  - Result: 2/2 tests PASSED.

---

### C-GAP-05: MVIR Indirect-Call Lowering in LLVM Backend
- **Root Cause:** `Instruction::CallIndirect { callee, args }` was specified in MVIR, but the LLVM backend had no lowering implementation and fell through to a stub `const_zero()`, corrupting function pointer execution.
- **Compiler Implementation:**
  - `crates/luna-backend/src/llvm_codegen.rs`: Implemented `Instruction::CallIndirect` using `self.builder.build_indirect_call(fn_ty, fn_ptr, &llvm_args)`.
  - **Strict Semantic Return Typing:**
    - If return type is `SemanticType::Void`: lowered to LLVM void function, and returns the canonical empty unit struct `self.context.struct_type(&[], false).const_zero()`.
    - If return type is non-void: queries `self.map_type(data.ty)?`. If LLVM returns void, returns `Err(BackendError::InvariantViolation)` instead of fallback `0`.
    - Applied identical invariant checks across `CallDirect` and `CallVirt`.
- **Acceptance Verification:**
  - File: `crates/luna-driver/tests/compiler_gap_c_gap_05_indirect_call_tests.rs`
  - Tests:
    1. `test_c_gap_05_indirect_call_value_return`: Function pointer invocation returning integer.
    2. `test_c_gap_05_indirect_call_void_return`: Function pointer invocation returning void with side effects (no fallback).
    3. `test_c_gap_05_struct_field_indirect_call`: Invoking function pointer stored in struct field.
    4. `test_c_gap_05_direct_and_virtual_call_semantic_integrity`: Verifying non-void returns in `CallDirect` and `CallVirt` never degrade to 0.
  - Result: 4/4 tests PASSED.

---

### C-GAP-06: Function-Value Canonical Mangling & Generic Instantiation
- **Root Cause:**
  1. When an identifier represented a function value (e.g. `let f = my_fn;`), the generator emitted un-mangled local symbol name `"my_fn"`, while definition was emitted with mangled canonical identity (`_MFN5my_fnE`).
  2. Generic function values (`identity<i32>`, `identity<i64>`) previously lacked concrete substitution attachment in `CanonicalInstanceIdentity`, risking lowering generic function values as non-generic `subst = []` and missing monomorphization worklist registration.
- **Compiler Implementation:**
  - `crates/luna-parser/src/expr.rs`: Enhanced `is_value_generic_args` to recognize generic arguments on function values ending at punctuation (`Semi`, `Comma`, `RParen`, `RBracket`) when containing type tokens.
  - `crates/luna-semantic/src/typechecker.rs`: Inserted `subst` into `expr_substs` for explicit generic arguments on function value identifiers.
  - `crates/luna-semantic/src/mono.rs`: Added `Expr::Identifier` handling in `MonoCollector::visit_expr` to discover generic function pointer usages and register concrete `MonoInstance` into worklist and `current_mono_calls`.
  - `crates/luna-mvir/src/generator.rs`: Connected `Expr::Identifier` to `mono_calls.get(expr_id)` to attach concrete substitutions to `CanonicalInstanceIdentity { kind: Decl(decl_id), subst }`.
  - **Clean Root ABI Delegation:** Removed ad-hoc `&& sym_name != "main"` check, delegating root entry point naming to `CanonicalInstanceIdentity::symbol_name_with_tables`.
- **Acceptance Verification:**
  - File: `crates/luna-driver/tests/compiler_gap_c_gap_06_function_pointer_mangling_tests.rs`
  - Tests:
    1. `test_c_gap_06_canonical_function_pointer_value`: Passing non-generic top-level function pointer to higher-order caller.
    2. `test_c_gap_06_distinct_function_pointers`: Returning and invoking distinct functions with identical signatures through conditional branch.
    3. `test_c_gap_06_a_generic_function_pointer_value`: Lowering `identity<i32>` function value to monomorphized canonical symbol `_MFN8identityEGlE` and executing.
    4. `test_c_gap_06_b_distinct_generic_substitutions`: Same generic function with different substitutions (`identity<i32>` vs `identity<i64>`) producing distinct canonical symbols (`_MFN8identityEGlE` != `_MFN8identityEGxE`) with independent runtime execution.
  - Result: 4/4 tests PASSED.

---

## 4. Full Regression Verification Matrix

| Suite | Tests | Result |
|---|---|---|
| `compiler_gap_c_gap_04_trait_bound_generic_args_tests` | 2 | **PASSED** (100%) |
| `compiler_gap_c_gap_05_indirect_call_tests` | 4 | **PASSED** (100%) |
| `compiler_gap_c_gap_06_function_pointer_mangling_tests` | 4 | **PASSED** (100%) |
| `compiler_gap_c_gap_01_drop_mono_tests` | 7 | **PASSED** (100%) |
| `compiler_gap_c_gap_02_mangling_tests` | 7 | **PASSED** (100%) |
| `compiler_gap_c_gap_03_provider_linking_tests` | 7 | **PASSED** (100%) |
| `stdlib_collection_iterators_acceptance_tests` (04f) | 8 | **PASSED** (100%) |
| `stdlib_hashmap_acceptance_tests` (04d) | 13 | **PASSED** (100%) |
| `stdlib_hashset_acceptance_tests` (04e) | 11 | **PASSED** (100%) |
| Workspace unit tests (`cargo test --lib`) | 42 | **PASSED** (100%) |
| **Total Tests Verified** | **105** | **100% GREEN** |
