# Call-Effect Model Specification v0.1

## 1. Introduction & Motivation

In the Mellis architecture, `Instruction::Call` is not just a branch instruction; it represents an opaque boundary of memory and ownership effects. The current simplistic approach of treating `Call` as merely "reading" its arguments is fundamentally insufficient. Without knowing whether a callee consumes, mutates, escapes, or returns a reference tied to its arguments, both Non-Lexical Lifetimes (NLL) and Foreign Function Interface (FFI) integrations are impossible to get right.

This document formalizes the **Call-Effect Model**, which abstracts the behavior of any callable entity (Mellis function, C-ABI extern function, generic instantiation) into a standalone summary (`CallEffectSummary`). 

Crucially, **effects are analysis artifacts, not MVIR instructions**. MVIR retains pure program semantics, while BorrowCK and NLL rely exclusively on the `CallEffectSummary` to reason about lifetimes and aliasing without looking at the callee's body.

---

## 2. The Orthogonal Effect Dimensions

A function call's impact on memory is modeled across four orthogonal dimensions. Each dimension forms a lattice, allowing the compiler to perform fixed-point inference (for cyclic call graphs) or fallback to conservative worst-cases (for FFI).

### 2.1. Access Effect Lattice
Describes how the callee reads or writes the memory pointed to by an argument.
*   **Unknown** (Top - worst case, assumed ReadWrite)
*   **ReadWrite** (Requires exclusive access if mutable)
*   **Write** (Requires exclusive access)
*   **Read** (Requires shared access)
*   **None** (Bottom - no access, e.g., passing a pointer just to cast it)

### 2.2. Escape Effect Lattice
Describes whether a pointer/reference outlives the duration of the call.
*   **Unknown** (Top - worst case, assumed MayEscape)
*   **MayEscape**: The callee might store the pointer in a global variable, another argument, or retain it indefinitely (common in C-ABI).
*   **CallOnly**: The pointer is strictly used during the execution of the call and is forgotten before `return`.
*   **NoEscape** (Bottom - mathematically identical to CallOnly for basic inference, but can be explicitly annotated to guarantee zero retention).

### 2.3. Ownership Effect Lattice
Describes the consumption or borrowing semantics of the argument.
*   **Unknown** (Top)
*   **Consume** (Move): The callee takes full ownership; the caller cannot use this value again.
*   **BorrowMut**: Exclusive, mutable borrow.
*   **BorrowShared**: Shared, read-only borrow.
*   **Copy** (Bottom): Bitwise copy, no ownership transfer.

### 2.4. Return Dependency
Describes how the lifetime of the returned value ties to the lifetimes of the arguments.
*   **Independent**: The return value does not borrow from any arguments (e.g., returning an owned `i32` or a newly allocated string).
*   **BorrowsFrom(ArgIndex)**: The return value contains a reference tied to the lifetime of `ArgIndex`.
*   **MayEscapeFrom(ArgIndex)**: The return value might contain an escaped reference from `ArgIndex` (conservative FFI fallback).

---

## 3. Representation (IR / Data Structures)

The effect summary for a function is represented by the following conceptual structures in `mellis-borrowck`:

```rust
pub struct CallEffectSummary {
    pub args: Vec<ArgEffect>,
    pub ret: ReturnEffect,
    pub is_opaque: bool, // True for FFI or unanalyzable bodies
}

pub struct ArgEffect {
    pub access: AccessKind,
    pub ownership: OwnershipKind,
    pub escape: EscapeKind,
}

pub enum AccessKind { Read, Write, ReadWrite, None, Unknown }
pub enum OwnershipKind { Copy, BorrowShared, BorrowMut, Consume, Unknown }
pub enum EscapeKind { NoEscape, CallOnly, MayEscape, Unknown }

pub enum ReturnEffect {
    Independent,
    BorrowsFrom(Vec<usize>), // Indices of arguments it borrows from
    Unknown,
}
```

---

## 4. Sources of Effects

The compiler acquires a `CallEffectSummary` from one of three sources:

1.  **Inferred (Safe Mellis Code):**
    The compiler performs a bottom-up, intra-procedural dataflow analysis on the MVIR body of a Mellis function. It computes the exact bounds of Access, Ownership, and Return Dependencies. If a function calls itself (recursion), a fixed-point algorithm on the lattices resolves the final effects.
2.  **Declared (Signatures & Intrinsics):**
    Certain built-in intrinsics or explicitly annotated functions (future syntax: `effects { arg0: ReadWrite, noescape }`) bypass inference. The compiler blindly trusts the declared summary.
3.  **Conservative (Opaque / FFI boundaries):**
    When crossing into `extern "C"`, the compiler cannot see the body. Unless explicitly annotated, the compiler assumes the absolute worst-case scenario (the "Top" of the lattice) for every pointer argument: `ReadWrite`, `BorrowMut` (if mutable), and `MayEscape`.

---

## 5. Standard Examples (The 15 Cases)

The following examples map source-level signatures to their resulting `CallEffectSummary`.

### Category A: Standard Safe Mellis (Inferred)

**1. Pure Read**
```rust
fn read_val(x: &T) { print(*x); }
```
*   `arg0`: `Access: Read`, `Ownership: BorrowShared`, `Escape: CallOnly`
*   `ret`: `Independent`

**2. In-place Mutation**
```rust
fn inc(x: &mut i32) { *x += 1; }
```
*   `arg0`: `Access: ReadWrite`, `Ownership: BorrowMut`, `Escape: CallOnly`
*   `ret`: `Independent`

**3. Move / Consume**
```rust
fn drop_val(x: T) { /* drops x */ }
```
*   `arg0`: `Access: None`, `Ownership: Consume`, `Escape: NoEscape`
*   `ret`: `Independent`

**4. Lifetime Pass-through (Identity)**
```rust
fn get_ref<'a>(x: &'a T) -> &'a T { x }
```
*   `arg0`: `Access: Read`, `Ownership: BorrowShared`, `Escape: CallOnly`
*   `ret`: `BorrowsFrom([0])` -> Caller knows return value extends `arg0`'s loan.

**5. Multiple Inputs, Single Output**
```rust
fn choose<'a>(cond: bool, x: &'a T, y: &'a T) -> &'a T
```
*   `arg1, arg2`: `Access: Read`, `Ownership: BorrowShared`, `Escape: CallOnly`
*   `ret`: `BorrowsFrom([1, 2])`

**6. Mutable Split (Disjoint borrowing)**
```rust
fn split<'a>(x: &'a mut [T]) -> (&'a mut [T], &'a mut [T])
```
*   `arg0`: `Access: ReadWrite`, `Ownership: BorrowMut`, `Escape: CallOnly`
*   `ret`: `BorrowsFrom([0])`

**7. Struct Field Projection**
```rust
fn get_field<'a>(s: &'a mut Struct) -> &'a mut Field
```
*   `arg0`: `Access: ReadWrite`, `Ownership: BorrowMut`, `Escape: CallOnly`
*   `ret`: `BorrowsFrom([0])`

### Category B: C-ABI & FFI (Conservative / Declared)

**8. Opaque FFI Mutation (Worst-case)**
```rust
extern "C" fn process(p: *mut T);
```
*   `arg0`: `Access: ReadWrite`, `Ownership: BorrowMut`, `Escape: MayEscape`
*   *BorrowCK Action:* Caller must assume `p` is mutated and stored globally by C. The lifetime of `p` is essentially "leaked" to the C-world, requiring `unsafe` to ever trust exclusive access again.

**9. Opaque FFI Read-only**
```rust
extern "C" fn print_c(p: *const T);
```
*   `arg0`: `Access: Read`, `Ownership: BorrowShared`, `Escape: MayEscape`
*   *BorrowCK Action:* C might store the read-only pointer. Shared borrows are cheap, so this just extends the shared NLL indefinitely unless proven otherwise.

**10. FFI Callback Registration (Explicit Escape)**
```rust
extern "C" fn register_handler(p: *const Callback);
```
*   `arg0`: `Access: Unknown`, `Ownership: BorrowShared`, `Escape: MayEscape`

**11. FFI with Explicit Annotation (Future Feature)**
```rust
// User guarantees C does not store the pointer
extern "C" fn fast_compute(p: *mut T) effects { arg0: CallOnly };
```
*   `arg0`: `Access: ReadWrite`, `Ownership: BorrowMut`, `Escape: CallOnly`
*   *BorrowCK Action:* Behaves exactly like a safe Mellis `&mut T`.

### Category C: Complex Graph / Alias Interactions

**12. Reassignment of References (MayEscape intra-procedurally)**
```rust
static mut GLOBAL_REF: Option<&T> = None;
fn leak_ref(x: &'static T) { GLOBAL_REF = Some(x); }
```
*   `arg0`: `Access: Read`, `Ownership: BorrowShared`, `Escape: MayEscape`
*   *Reason:* The inference engine sees `x` being written into a static memory location, lifting its escape lattice to `MayEscape`.

**13. Reference via Struct (Nested Borrows)**
```rust
struct Wrapper<'a> { inner: &'a T }
fn unwrap<'a>(w: Wrapper<'a>) -> &'a T { w.inner }
```
*   `arg0`: `Access: Read`, `Ownership: Consume` (moves the wrapper), `Escape: CallOnly`
*   `ret`: `BorrowsFrom([0])` (Tracks through the consumed wrapper to the inner lifetime).

**14. Overlapping Mutable Aliasing (Rejected by Caller)**
```rust
fn double_mut(x: &mut T, y: &mut T)
```
*   `arg0, arg1`: `Access: ReadWrite`, `Ownership: BorrowMut`
*   *BorrowCK Action:* If caller does `double_mut(&mut a, &mut a)`, BorrowCK rejects it *before* the call because it requires overlapping `BorrowMut` tokens.

**15. Aliasing FFI Pointers**
```rust
extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize);
```
*   `dest`: `Access: Write`, `Ownership: BorrowMut`, `Escape: MayEscape`
*   `src`: `Access: Read`, `Ownership: BorrowShared`, `Escape: MayEscape`
*   *BorrowCK Action:* Ensures `dest` and `src` do not overlap natively (if passed as safe references that are cast to pointers).

---

## 6. Integration with NLL (Next Phase)

Once this model is merged, the Borrow Checker's `transfer_instruction` for `Instruction::Call` will be rewritten to:
1. Lookup the `CallEffectSummary` for the callee.
2. For each `ArgEffect`, apply the `AccessKind` (Read/Write) to the current `active_loans`.
3. If `EscapeKind == MayEscape`, the loan's liveness is forcibly extended to infinity (or until the unsafe boundary is dropped).
4. For `ReturnEffect::BorrowsFrom`, mint a new SSA token that inherits the liveness dependencies of the specified arguments.
