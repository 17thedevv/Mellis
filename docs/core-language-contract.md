# Mellis Core Language Contract v1.0 (Frozen)

## Purpose and Scope

This document serves as the authoritative source of truth for the `core.ms` semantic contracts within the Mellis compiler architecture. It establishes the strict boundaries between **Compiler Contracts** (items the compiler has explicit knowledge of) and **Standard APIs** (items defined purely in user-space). 

The goal of this document is to ensure that future language features (e.g., closures, async, iterators) are implemented systematically via the `LangItem` registry without introducing ad-hoc string comparisons or violating semantic decoupling.

---

## 1. Compiler Contracts (LangItems)

The following declarations are explicitly identified by the compiler using `#[lang("...")]` annotations.

### 1.1 `LangItem::Drop`
*   **Canonical Name**: `#[lang("drop")]`
*   **Target Kind**: `Trait`
*   **Source Declaration**: `core::Drop`
*   **Identification**: Identifies the compiler destruction protocol. A type implementing this trait opts into explicit destruction semantics.
*   **Consumers**: `Typechecker`, `Borrowck`, `MVIR Generator`
*   **Policy Notes**: `LangItem::Drop` does not itself enforce any behavioral restrictions. It merely provides the semantic identity of the Drop protocol.
*   **Required**: Yes

### 1.2 `LangItem::DropFn`
*   **Canonical Name**: `#[lang("drop_fn")]`
*   **Target Kind**: `Method`
*   **Source Declaration**: `core::Drop::drop`
*   **Identification**: Identifies the canonical destruction hook method within the `Drop` protocol.
*   **Consumers**: `Typechecker`
*   **Policy Notes**: The LangItem identifies the canonical destruction hook. Any restriction on manually invoking this hook (e.g., forbidding explicit `x.drop()` calls) is a separate semantic policy enforced by the Typechecker. The name of the method itself (e.g., `"drop"`) is immaterial; only the `#[lang("drop_fn")]` identity matters.
*   **Required**: Yes

### 1.3 `LangItem::Try`
*   **Canonical Name**: `#[lang("try")]`
*   **Target Kind**: `Trait`
*   **Source Declaration**: `core::Try`
*   **Identification**: Identifies the protocol required to use the `?` (try) operator.
*   **Consumers**: `Typechecker`
*   **Policy Notes**: Types must implement this protocol to be desugared via the `?` operator.
*   **Required**: Yes

### 1.4 `LangItem::TryBranch`
*   **Canonical Name**: `#[lang("try_branch")]`
*   **Target Kind**: `Method`
*   **Source Declaration**: `core::Try::branch`
*   **Identification**: Identifies the branching hook that splits a `Try` type into success or residual (error) paths.
*   **Consumers**: `Typechecker`
*   **Policy Notes**: Used to typecheck the branching step of the `?` operator. Any restriction on manually invoking this hook is a separate semantic policy.
*   **Required**: Yes

### 1.5 `LangItem::TryFromOutput`
*   **Canonical Name**: `#[lang("try_from_output")]`
*   **Target Kind**: `Method`
*   **Source Declaration**: `core::Try::from_output`
*   **Identification**: Identifies the hook for wrapping a success value back into the `Try` container type.
*   **Consumers**: None currently.
*   **Policy Notes**: Required by the `Try` language contract, but not currently consumed by any compiler phase.
*   **Required**: Yes (part of the language protocol contract)

### 1.6 `LangItem::FromResidual`
*   **Canonical Name**: `#[lang("from_residual")]`
*   **Target Kind**: `Trait`
*   **Source Declaration**: `core::FromResidual`
*   **Identification**: Identifies the protocol used to convert residual error types across function boundaries during early returns.
*   **Consumers**: `Typechecker`
*   **Policy Notes**: Required to determine if the error state of a callee's `?` operation can be propagated into the caller's return type.
*   **Required**: Yes

### 1.7 `LangItem::FromResidualFn`
*   **Canonical Name**: `#[lang("from_residual_fn")]`
*   **Target Kind**: `Method`
*   **Source Declaration**: `core::FromResidual::from_residual`
*   **Identification**: Identifies the hook used to execute the conversion of a residual type.
*   **Consumers**: `Typechecker`
*   **Policy Notes**: Used by the typechecker to validate the early return path of the `?` operator.
*   **Required**: Yes

### 1.8 `LangItem::ControlFlow`
*   **Canonical Name**: `#[lang("control_flow")]`
*   **Target Kind**: `Enum`
*   **Source Declaration**: `core::ControlFlow`
*   **Identification**: Identifies the canonical control flow enum used as the return type for `Try::branch`.
*   **Consumers**: `Typechecker`
*   **Policy Notes**: Defines the intermediate type expected between `Try::branch` and the `Continue`/`Break` lowering.
*   **Required**: Yes

### 1.9 `LangItem::ControlFlowContinue`
*   **Canonical Name**: `#[lang("continue")]`
*   **Target Kind**: `EnumVariant`
*   **Source Declaration**: `core::ControlFlow::Continue`
*   **Identification**: Identifies the success path of a `TryBranch` operation.
*   **Consumers**: `MVIR Generator`
*   **Policy Notes**: Used exclusively to structure the MVIR pattern matching for the success branch of the `?` operator.
*   **Required**: Yes

### 1.10 `LangItem::ControlFlowBreak`
*   **Canonical Name**: `#[lang("break")]`
*   **Target Kind**: `EnumVariant`
*   **Source Declaration**: `core::ControlFlow::Break`
*   **Identification**: Identifies the failure/early-return path of a `TryBranch` operation.
*   **Consumers**: `MVIR Generator`
*   **Policy Notes**: Used exclusively to structure the MVIR pattern matching for the failure branch of the `?` operator.
*   **Required**: Yes

---

## 2. Standard Library APIs (Non-Contracts)

The following declarations in `core.ms` are **pure user-space concepts**. They do NOT have `#[lang]` annotations and the compiler has zero hardcoded knowledge of them.

*   `enum Result<T, E>`
*   `enum Infallible`

### Protocol Implementations
The behavior of the `?` operator on `Result` works strictly because `Result` implements the generic compiler protocols (`Try` and `FromResidual`), NOT because the compiler knows what a `Result` is.
*   `impl<T, E> FromResidual<Result<Infallible, E>> for Result<T, E>`
*   `impl<T, E> Try for Result<T, E>`

---

## 3. Required Separation: Identification vs. Policy

The Mellis compiler strictly enforces a separation between *Identification* and *Policy*.

*   **Identification (LangItem Registry):** The process of resolving canonical identities from the source code. Example: "Which `SymbolId` represents `LangItem::DropFn`?"
*   **Policy (Compiler Phases):** The rules applied to identified symbols. Example: "Is the user allowed to explicitly call the `SymbolId` that matches `LangItem::DropFn`?"

**Never describe a `LangItem` itself as "forbidding", "requiring", or "allowing" behavior.** The `LangItem` simply provides the canonical identity to the Typechecker or Borrowchecker, which then enforce the policy.

---

## 4. Cross-Module Identity Rules

A `LangItem` provides a stable identity that transcends module boundaries and file structures. The resolution invariant is strictly defined as follows:

1.  **`LangItem`**: The abstract semantic role (e.g., `LangItem::Drop`).
2.  **`CanonicalSymbolId`**: The stable canonical declaration identity across compiler module/provider boundaries, mapping the `LangItem` to a specific provider and declaration name.
3.  **`ProviderInterface`**: The mechanism that maps local declarations to their global canonical identities upon module compilation.
4.  **Local Declaration Identity**: The raw `SymbolId` within the current compilation scope.

**CRITICAL INVARIANT**: Raw provider-local `SymbolId`s must *never* be used as cross-module stable identities. When checking if a symbol matches a `LangItem`, the compiler must query `ctx.lang_items.get(LangItem)` to retrieve the locally mapped `SymbolId` for comparison.

---

## 5. Bootstrap Registration Flow

The `core` component is fundamentally tied to the compiler. The compiler expects it to be discovered and registered dynamically without hardcoded file paths:

1.  **Sysroot Resolution**: Locate `<sysroot>/libs/external/` via compiler configuration or standard search paths.
2.  **External Component Discovery**: Query the component discovery engine to find the required component `core`.
3.  **Core Bootstrap**: Load the discovered component into the compilation session via `bootstrap_core()`.
4.  **Core Resolution**: Parse and semantically resolve the component declaration units.
5.  **`#[lang]` Registration**: Scan attributes, binding declarations to `CanonicalSymbolId`s in the `LangItemRegistry`.
6.  **Validation**: Verify all mandatory `LangItem`s are registered; abort if any required item is missing.

*Note: Core bootstrap provides the LangItems to the compiler, but it does **not** implicitly inject the `core` namespace into the user's lexical scope. Users must still `import <core>` to name the symbols directly.*

---

## 6. Rename Independence Audit

As of this contract definition, **ZERO** semantic string dependencies remain in the compiler. No phase of the compiler checks strings such as `"drop"`, `"branch"`, or `"from_residual"` for semantic lowering. The only strings present are diagnostic text, documentation, and the canonical `#[lang("...")]` mappings. 

## 7. Evidence Requirements and Summary

**Summary:**
*   Language Contracts discovered: 10
*   Core APIs discovered: 2
*   Protocol implementations discovered: 2
*   Semantic string dependencies remaining: 0
*   Undocumented discrepancies: 0

**Tests proving invariants:**
*   `test_invariant_a_try_operator_independent_of_lexical_try` (Proves `?` operates independently of lexical string names).
*   `test_invariant_b_drop_independent_of_lexical_drop` (Proves destructor logic operates independently of lexical string names).
*   `test_invariant_e_lang_items_mapped_to_canonical_declarations` (Proves all LangItems bind to CanonicalSymbolIds).
*   `test_core_bootstrap_isolation.rs` suite (Proves core constraints and isolation).
