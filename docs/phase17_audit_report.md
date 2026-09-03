# Phase 17 Master Semantic Audit Report

This document continuously logs the semantic gaps found during the Master Semantic Audit across the 10-point checklist.

## 1. Grammar Coverage Matrix

| Construct | Grammar | AST | Resolver | Typecheck | MVIR | LLVM | Status | Notes |
|-----------|---------|-----|----------|-----------|------|------|--------|-------|
| `import` | ✅ | ✅ | ❌ | ➖ | ➖ | ➖ | **BROKEN** | Handled loosely in Driver (`loader.rs`), but completely missing from Semantic passes. |
| `using` | ✅ | ✅ | ❌ | ❌ | ➖ | ➖ | **BROKEN** | Parser creates `Decl::Using`, but `resolver.rs` does not resolve aliases. Dead code. |
| `type` (alias) | ✅ | ✅ | ✅ | ➖ | ➖ | ➖ | COMPLETE | Parses and resolves correctly. |
| `module` | ✅ | ❌ | ❌ | ➖ | ➖ | ➖ | **BROKEN** | `Decl::Module` exists in AST but is completely ignored by `resolver.rs` and `typechecker.rs`. Inner declarations are never evaluated. |
| `cast (type)` | ❌ | ➖ | ➖ | ➖ | ➖ | ➖ | **BROKEN** | `Expr::Cast` exists in AST, but `mellis-parser` does not parse `cast(type) expr` syntax at all. Keyword missing. |
| `match` | ✅ | ✅ | ✅ | ✅ | ❌ | ➖ | **BROKEN** | MVIR `generate_pat_match` only implements `Enum`, `Identifier`, `Wildcard`. `Pattern::Literal` (e.g. `10`) falls back to hardcoded `Operand::Boolean(false)`, breaking any non-enum matches. |
| `for` | ✅ | ❌ | ❌ | ➖ | ➖ | ➖ | **BROKEN** | `resolver.rs` ignores the `binding` field of `Stmt::For`. The iterator variable (e.g., `i` in `for i in 0..10`) is never inserted into the symbol table, causing "Symbol not found". |
| `unsafe` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | COMPLETE | Works fine, correctly toggles `is_unsafe_context` in typechecker. |
| `enum` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | COMPLETE | Works. Lowers to `Instruction::Variant` successfully. |

*(Matrix will be populated as Audit progresses)*

## 2. Type-System Strictness

- [x] **typeof**: FAILS due to unresolved AST identifiers.
- [x] **Never**: FAILS due to missing MVIR backend lowering.

## 5. Control-flow Robustness

- [x] **Divergence (`!`)**: FAILS. `panic() -> !` returns 0 in MVIR instead of `unreachable`. Branches merging with `Never` incorrectly emit jumps to the merge block.
- [x] **Short-circuiting (`&&`, `||`)**: FAILS. `true && false` does not short-circuit. Worse, it falls through a wildcard match and compiles to `add true, false` in MVIR!

## 10. Backend Semantic Parity

- [x] **Binary Operators**: FAILS. Any `BinaryOp` not explicitly mapped in `generator.rs` (e.g. `&&`, `||`, `..`, `..=`, `&`, `|`, `^`, `<<`, `>>`) hits a `_ => Instruction::Add` wildcard. This causes `1..10` to compile to `1 + 10 = 11`.

## Actionable Gaps Found So Far

1. **`Never` Divergence Bug**: `mellis-mvir` must understand `SemanticType::Never` and emit `Instruction::Unreachable` or prevent fall-through.
2. **`typeof` Resolution Bug**: `mellis-semantic/src/resolver.rs` must visit `Type::Typeof(expr)` and recursively resolve the `expr`.
3. **`using` Alias Bug**: `resolver.rs` must implement namespace aliasing for `Decl::Using`.
4. **`import` Gap**: `import` needs a formal semantic representation for MLib loading to ensure type signatures match.
5. **BinaryOp Wildcard & Short-circuiting**: `&&` and `||` must generate branching MVIR blocks for short-circuiting. The wildcard `_ => Add` in `generator.rs` must be removed and all operators explicitly implemented or reported as semantic errors.
6. **Destructuring Partial-Move Bug**: `Extract` lowers to accessing the parent value directly, but Borrow Checker treats this as querying a `PartialMoved` value. Borrow Checker must either track per-field access correctly or `Extract` should operate strictly on pointers.
7. **Async Void Allocation Panic**: `async_lowering.rs` generates allocations of type `SemanticTypeId(0)` (`Void`). The environment pointer needs a valid `Pointer` or `Struct` type.
8. **Closure Monomorphization Miss**: `mono.rs` completely ignores `Closure` types, so closure function bodies are never instantiated. Additionally, `||` syntax requires spaces to parse (`| |`) because the parser looks for `BitOr` rather than handling the `LogicalOr` token.
9. **Optimizer Trait Object DCE Bug**: `DeadCodeElimination` in `mellis-optimizer` drops variable usages from `Instruction::MakeTraitObject` and `CallVirt`, causing backend mapping crashes.
10. **Silent Return Type Mismatch**: `typechecker.rs` only typechecks `Stmt::Return` for `async` functions! Regular functions can return any type and silently bypass semantic checks.
11. **Modules Dropped**: `Decl::Module` is completely ignored by `resolver.rs` and `typechecker.rs`. Inner declarations vanish.
12. **Type Casting Missing**: `cast(type) expr` is in the grammar, `Expr::Cast` is in the AST, but the `mellis-parser` lacks any logic to parse it.
13. **Match Expressions Broken**: `generate_pat_match` in MVIR hardcodes `false` for everything except `Enum`, `Identifier`, and `Wildcard`. Matching on integer literals is currently impossible.
14. **For-Loop Iterator Unbindable**: `resolver.rs` ignores the `binding` field of `Stmt::For`, so loop variables are never declared, causing immediate compilation failure.
