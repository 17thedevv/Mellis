# Mellis Compiler RFC: Memory Model & Allocation Semantic Contract (P0.1 Hardened)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P0.1 Hardened)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P0.1)
- **Scope**: Memory Model Semantics, Object Lifecycle State Machine, Pointer Provenance, ZST Semantics, Reallocation Matrix, Allocator Pairing
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Memory Semantics ("What does memory mean?")

---

## 1. Architectural Boundary: P0 vs P1

To ensure absolute architectural hygiene, the Mellis memory subsystem is bifurcated into two independent, non-overlapping concerns:
- **P0 (Memory Model Contract)**: Defines the *semantic meaning* of memory, pointer provenance, object lifecycles, states, and allocation invariants.
- **P1 (Layout Model)**: Defines the *spatial layout* of data: `size`, `align`, `stride`, padding, field offsets, overflow bounds, and type composition.

P0 does not depend on P1. P1 builds strictly upon the semantic invariants established in P0.

---

## 2. The 4-State Object & Storage Lifecycle State Machine

A memory location holding an object in Mellis transitions deterministically through a 4-state lifecycle machine:

```text
                       Alloc
              ABSENT ─────────► ALLOCATED / UNINITIALIZED
                ▲                         │
                │                         │ Initialize
        Dealloc │                         ▼
                │                    INITIALIZED
                │                         │
                │                         │ DropInPlace
                └─────────────────────────┘
```

### 2.1 State Definitions

| State | Storage Status | Object Status | Valid Operations |
|:---|:---|:---|:---|
| **`ABSENT`** | No storage allocated | No object exists | `Alloc` |
| **`ALLOCATED / UNINITIALIZED`** | Valid storage reserved | No initialized object | `Initialize`, `Dealloc` |
| **`INITIALIZED`** | Valid storage reserved | Active, typed object `T` | `Read`, `Write` (overwrite), `MoveOut`, `DropInPlace` |

### 2.2 Transition Invariants (LIFE-1 to LIFE-6)

- **LIFE-1 (No Implicit Drop on Dealloc)**:
  Calling `Dealloc` on an `INITIALIZED` place is strictly forbidden ($\times$). Deallocation never implicitly drops an object. The compiler must emit `DropInPlace` prior to `Dealloc`.
- **LIFE-2 (No Drop on Uninitialized Storage)**:
  Calling `DropInPlace` on `ALLOCATED / UNINITIALIZED` storage is strictly forbidden ($\times$). Executing a destructor on uninitialized bits violates language soundess.
- **LIFE-3 (Initialization Gate)**:
  `Initialize(ptr, val)` is valid only when target storage is `ALLOCATED / UNINITIALIZED`. It moves ownership of `val: T` into the storage and transitions the state to `INITIALIZED`.
- **LIFE-4 (Overwrite Semantics)**:
  Writing to an already `INITIALIZED` place (`Write(ptr, val)`) requires that the existing instance in that place be dropped first if `needs_drop(T)` is true, or its drop obligation must be explicitly transferred:
  $$\text{Write}(ptr, val) \iff \text{DropInPlace}(ptr, T) \ \longrightarrow \ \text{Initialize}(ptr, val)$$
- **LIFE-5 (Move-Out Transition)**:
  Moving an object out of a heap place (`MoveOut(ptr) -> T`) transfers ownership of `T` to the caller and transitions the storage back to `ALLOCATED / UNINITIALIZED`.
- **LIFE-6 (Clean Release Obligation)**:
  Releasing memory back to the heap requires traversing:
  $$\text{INITIALIZED} \xrightarrow{\text{DropInPlace}} \text{ALLOCATED / UNINITIALIZED} \xrightarrow{\text{Dealloc}} \text{ABSENT}$$

---

## 3. Dropability & `needs_drop(T)` Invariant

Not all types have drop obligations. The semantic model distinguishes types that require destruction from those that are trivially droppable.

### 3.1 Dropability Predicate: `needs_drop(T) -> bool`

$$\text{needs\_drop}(T) = \begin{cases} 
\text{false} & \text{if } T \text{ is a primitive scalar } (i8..i128, u8..u128, f32, f64, bool) \\
\text{false} & \text{if } T \text{ is a raw pointer } (*T, *rw\ T) \text{ or shared reference } (\&T) \\
\text{true}  & \text{if } T \text{ implements trait } Drop \\
\bigvee_i \text{needs\_drop}(F_i) & \text{if } T \text{ is a struct, enum, tuple, or array with fields/elements } F_i
\end{cases}$$

### 3.2 Drop Invariants (DROP-1 to DROP-3)

- **DROP-1 (Static Elision)**:
  If $\text{needs\_drop}(T) == \text{false}$, `DropInPlace(ptr, T)` is a static compile-time no-op. The compiler generates zero runtime instructions and transitions the state directly from `INITIALIZED` to `ALLOCATED / UNINITIALIZED`.
- **DROP-2 (Recursive Destruction)**:
  If $\text{needs\_drop}(T) == \text{true}$, `DropInPlace(ptr, T)` invokes `T::drop(&rw *ptr)` (if defined) and recursively invokes `DropInPlace` on all subfields in reverse declaration order.
- **DROP-3 (Panic Containment in Drop)**:
  If a destructor panics during `DropInPlace`, the runtime terminates via deterministic abort (`__mellis_panic`). Destructors must never unwind or leak incomplete drop state.

---

## 4. Zero-Sized Types (ZST) Semantic Contract

### 4.1 Separation of Semantic Contract from Runtime Limit

The ZST semantic contract is decoupled from any hardcoded physical page or alignment constants:

> **ZST Semantic Rule**: A ZST allocation request returns a canonical non-null pointer representation whose numerical address satisfies the requested alignment:
> $$ptr \ne \text{null} \quad \land \quad (ptr \pmod{align} == 0)$$

The runtime sentinel `g_mellis_zero_sentinel` (aligned to 4096 in `mellis-runtime.lib`) is an implementation artifact that satisfies all alignments $align \le 4096$. If a future target supports higher alignments ($align > 4096$), the semantic contract remains intact while the runtime implementation provides an adequately aligned address.

### 4.2 Address Identity vs. Element-Position Identity

A critical semantic invariant governs pointer arithmetic and slices on ZSTs:

$$\text{Address Identity} \ne \text{Element-Position Identity}$$

- **ZST Arithmetic Invariant**:
  For $p: *T$ where $\text{size\_of}::<T>() == 0$:
  $$\text{physical\_address}(p + n) = \text{physical\_address}(p) \quad (\forall n \in \text{isize})$$
  $$\text{logical\_index}(p + n) = \text{logical\_index}(p) + n$$
- **Compiler / Optimizer Barrier**:
  The compiler and optimizer must not treat $p + 0$ and $p + 1$ as identical semantic values. Even though their physical memory addresses coincide, they represent distinct logical element positions within a slice or array $[T; N]$. Array bounds checks, iterator loops, and length calculations evaluate logical element positions, never physical byte displacements.

---

## 5. Pointer Provenance & Range Bounds

### 5.1 Formal Provenance Model

Mellis formally distinguishes an allocation from a pointer referencing it:

```text
┌────────────────────────────────────────────────────────┐
│                      Allocation                        │
├───────────────────┬──────────────────┬─────────────────┤
│ id: AllocId       │ range: [b, b+sz) │ lifetime: State │
└───────────────────┴──────────────────┴─────────────────┘
                            ▲
                            │ provenance
┌───────────────────────────┴──────────┐
│                   Pointer            │
├───────────────────┬──────────────────┤
│ address: usize    │ prov: AllocId    │
└───────────────────┴──────────────────┘
```

- **`AllocId`**: Unique identifier generated when an allocation is created (stack alloca, global item, or heap allocation).
- **`Lifetime`**: The active lifespan of the allocation. Transitions to `Terminated` upon deallocation, stack frame exit, or reallocation.
- **`Provenance`**: The binding between a pointer value and its originating `AllocId`.

### 5.2 Dereferenceable Range vs. One-Past Pointer

A pointer with provenance `AllocId` bounded by $[base, base + size)$ satisfies:

1. **Dereferenceable Range**:
   A pointer $p$ is valid to dereference for type $T$ if and only if:
   $$base \le p \quad \land \quad p + \text{size\_of}::<T>() \le base + size \quad \land \quad (p \pmod{\text{align\_of}::<T>()} == 0)$$
2. **One-Past Pointer Permitted**:
   The address $p_{end} = base + size$ is a **valid, non-dereferenceable pointer**.
   - Creating, copying, comparing, and using $p_{end}$ in loop termination conditions (`while p != end`) is fully valid and retains `AllocId` provenance.
   - Attempting to dereference $p_{end}$ (reading or writing) is immediate Undefined Behavior.
3. **Out-of-Range Pointers**:
   Calculating $p < base$ or $p > base + size$ strips valid provenance and cannot be dereferenced even if subsequent arithmetic brings it back into range.

### 5.3 Integer to Pointer Casts

- `ptr as usize`: Extracts raw address; strips provenance.
- `addr as *rw T`: Produces an integer-derived pointer with **wildcard/untracked provenance**.
- **Enforcement**: Permitted strictly in `unsafe` blocks. The compiler assumes no aliasing or provenance guarantees for integer-derived pointers.

---

## 6. Formal Reallocation State Transition Matrix

The behavior of `realloc(ptr, old_size, old_align, new_size) -> *rw u8` is formally specified across all permutation states:

| Source State (`old_size`) | Target State (`new_size`) | Allocator Outcome | Resulting Pointer & Allocation Action | Provenance Transition |
|:---|:---|:---|:---|:---|
| **non-ZST** ($> 0$) | **non-ZST** ($> 0$) | Success | Block resized (in-place or moved). $\min(\text{old}, \text{new})$ bytes preserved. | Old `AllocId` **revoked**; fresh `AllocId` assigned to returned pointer |
| **non-ZST** ($> 0$) | **ZST** ($== 0$) | Success | Old physical allocation deallocated; returns canonical ZST pointer. | Old `AllocId` **revoked**; returned pointer receives ZST provenance |
| **ZST** ($== 0$) | **non-ZST** ($> 0$) | Success | Fresh physical allocation of `new_size` bytes created. | Returned pointer receives fresh heap `AllocId` |
| **ZST** ($== 0$) | **ZST** ($== 0$) | Success | No physical allocation occurs; returns canonical ZST pointer. | Retains ZST provenance |
| **non-ZST** ($> 0$) | **non-ZST** ($> 0$) | Failure (OOM) | Reallocation fails. | **Old allocation remains valid, unmodified, and owned by caller under old `AllocId`** |
| **ZST** ($== 0$) | any | Failure | N/A | Trivial request; zero bytes never triggers OOM |

### 6.1 Strict Provenance Revocation on Realloc (Same-Address Invariant)

> **REAL-PROV Invariant**: Every successful reallocation that transitions a non-ZST allocation terminates the old allocation's lifetime and immediately revokes its `AllocId`.
> 
> Even if the underlying OS allocator resizes the memory block in-place such that:
> $$\text{new\_address} == \text{old\_address}$$
> the old pointer's provenance is **permanently dead**. Accessing memory through the old pointer variable is an illegal use-after-realloc violation. Only the returned pointer holds the fresh, valid `AllocId`.

---

## 7. Allocator Identity & Pairing Contract

In modern systems programming, allocations must be deallocated by the same authority that created them.

### 7.1 Allocator Domain: `AllocDomain`

Every allocation belongs to an explicit **Allocator Domain**:
$$\text{AllocDomain} \in \{ \text{GlobalHeap}, \text{Arena}(\text{ArenaId}), \text{Region}(\text{RegionId}), \text{Custom} \}$$

### 7.2 The 4-Way Pairing Contract (PAIR-1)

For any deallocation `Dealloc(ptr, size, align)` or reallocation `Realloc(ptr, old_size, old_align, new_size)` to be sound, all 4 parameters must match the original allocation:

1. **Identity**: `ptr` must point to the base address of an active allocation.
2. **Size Contract**: `size` must exactly match the size specified during allocation.
3. **Alignment Contract**: `align` must exactly match the alignment specified during allocation.
4. **Domain Match**: The deallocator must belong to the exact same `AllocDomain` that executed `Alloc`.

*Cross-domain deallocation (e.g. allocating via an Arena and deallocating via Global Heap) is immediate Undefined Behavior.*

---

## 8. Semantic Meaning of `*rw u8` in Runtime Allocator ABI

The Tier 1 runtime declaration is:
```rust
export extern fn __mellis_alloc(size: usize, align: usize) -> *rw u8;
export extern fn __mellis_dealloc(ptr: *rw u8, size: usize, align: usize);
export extern fn __mellis_realloc(ptr: *rw u8, old_size: usize, old_align: usize, new_size: usize) -> *rw u8;
```

### 8.1 Semantic Storage Invariant (ABI-RAW)

> **ABI-RAW Invariant**: The return type `*rw u8` from `__mellis_alloc` and `__mellis_realloc` denotes a **raw, untyped memory storage handle**.
> 
> It does **NOT** denote that a valid `u8` object has been created or initialized.
> Dereferencing or reading from the returned `*rw u8` before executing `Initialize` is strictly undefined behavior.

---

## 9. Conclusion & Transition to P1 Layout Model

This P0.1 document fully answers **"What does memory mean?"**:
- Object lifecycle transitions across 4 formal states.
- ZST semantics are separated from runtime bounds.
- ZST address identity is decoupled from element position.
- One-past pointers are formally permitted.
- `realloc` is governed by an exhaustive 6-case state transition matrix.
- Same-address realloc provenance revocation is locked.
- `needs_drop(T)` dictates static destruction elision.
- Allocator pairing requires domain equivalence.

With P0.1 hardened and frozen, the compiler foundation is ready for **P1: Layout Model Specification (`Layout`, size, align, stride, composition, overflow guards)**.
