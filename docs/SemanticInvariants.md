# FDLang Semantic Invariants

This document outlines the core semantic invariants that the Mellis compiler enforces during the Middle-End phases.

## 1. Borrow Checking & CFG Joins
- **Exclusive Mutation**: A value cannot be mutated if it is currently borrowed by any active reference (immutable or mutable).
- **CFG State Merging**: At Control Flow Graph (CFG) join points, a reference variable may alias multiple places depending on the executed path. The borrow checker maintains a multi-path alias set (`std::vector<Place>`) to ensure that modifying *any* potentially aliased place correctly flags a borrow violation.

## 2. Initialization Tracking
- **Definite Initialization Before Use**: A variable must be definitively initialized on all possible execution paths before its value can be read or moved.
- **Weak Updates at CFG Joins**: If a variable is initialized in one branch but uninitialized in another, it is considered **uninitialized** at the join point.

## 3. Slice Representation
- **Fat Pointers**: Slices are natively represented as fat pointers `{ ptr: *T, len: uint_64 }`.
- **Length Extraction**: Slice length is extracted uniformly as `uint_64` to simplify backend lowering, array bounds checking, and pointer arithmetic.

## 4. Array and Slice Bounds Checking
- **Defined Behavior**: Out-of-bounds accesses on arrays and slices do not produce Undefined Behavior (UB). Instead, they are explicitly checked at runtime using the `@__mellis_bounds_fail` intrinsic, guaranteeing a controlled panic or defined error state.

## 5. Exhaustive Pattern Matching
- **Match Coverage**: All possible variants of an `enum` must be covered in a `match` expression.
- **Decision Tree Construction**: The semantic analyzer uses a decision tree and pattern matrix to formally prove exhaustiveness and detect unreachable patterns before MVIR lowering.
