# Mellis Compiler RFC: Raw Pointer Semantics & Provenance Model (P2 Hardened)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P2 Hardened)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P2)
- **Scope**: Pointer Value Model, Provenance Tags, Allocation State, Arithmetic & Overflow Invariants, One-Past Semantics, Dereference Preconditions, Integer Conversions, Pointer Equality & Subtraction
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Raw Pointers & Provenance

---

## 1. The Core Provenance Architecture

In Mellis, provenance answers: **"Where did a pointer originate?"**, while allocation state answers: **"Does that origin currently authorize access?"**

### 1.1 Provenance is an Intrinsic Property of `PointerValue`

Provenance is **NOT** a property of the physical memory address:
- If provenance were tied to the numerical address, address reuse by the allocator (after `dealloc` or `realloc`) would revive old dead pointers.
- Therefore, provenance is an intrinsic semantic component of the **`PointerValue`**:

```text
┌────────────────────────────────────────────────────────┐
│                      PointerValue                      │
├───────────────────┬──────────────────┬─────────────────┤
│ address: usize    │ prov: Provenance │ metadata: Meta  │
└───────────────────┴──────────────────┴─────────────────┘
```

For Sized types (`*T`, `*rw T`), `metadata` is unit `()`. For Dynamically Sized Types (DSTs like `*[T]` or `*dyn Trait`), `metadata` stores `len: usize` or `vtable: *const VTable`. The provenance authorizing memory access belongs strictly to the `PointerValue`.

---

## 2. Pointer Value & Provenance Data Model

### 2.1 The `Provenance` Enum

Provenance represents the origin authorization of a pointer value:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// Canonical null pointer (address 0). Authorizes zero memory access.
    Null,
    /// Originates from a specific Mellis allocation with identity AllocId.
    Alloc(AllocId),
    /// Wildcard provenance synthesized from integer-to-pointer casts in unsafe code.
    Wildcard,
}
```

*Note: `Terminated` is NOT a provenance tag. A pointer retains its origin identity `Alloc(AllocId)` throughout its existence. Temporal authorization is determined by querying the `AllocationState` of that `AllocId`.*

### 2.2 The `Allocation` Entity and Lifecycle State

Every allocation (heap buffer, stack variable, or static item) possesses a tracked entity:

```rust
pub struct Allocation {
    pub id: AllocId,
    pub base: usize,
    pub size: usize,
    pub align: usize,
    pub state: AllocationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocationState {
    Active,
    Terminated,
}
```

- When `alloc` is executed: A new `Allocation` is created with state `Active`.
- When `dealloc` is executed: `AllocationState` transitions to `Terminated`.
- When `realloc` succeeds: The old `AllocationState` transitions to `Terminated`, and a fresh `Allocation` is created with a new `AllocId` and state `Active`.

---

## 3. Pointer Arithmetic & Arithmetic Overflow Invariants

Pointer arithmetic in Mellis is typed and scaled by the element's stride:

$$\text{address}(p + n) = \text{address}(p) + (n \times \text{stride}(T))$$
$$\text{address}(p - n) = \text{address}(p) - (n \times \text{stride}(T))$$

### 3.1 Arithmetic Invariants (ARITH-1 to ARITH-4)

- **ARITH-1 (No Silent Address Wraparound)**:
  Pointer arithmetic must never wrap around the numerical address space:
  $$\text{address}(p) \pm (n \times \text{stride}(T)) \quad \text{must not overflow } \text{usize::MAX}$$
  Any computation that would wrap the address space is an arithmetic error / undefined behavior.
- **ARITH-2 (Provenance Invariance)**:
  Pointer arithmetic preserves the exact `Provenance` of the source pointer:
  $$\text{prov}(p \pm n) = \text{prov}(p)$$
- **ARITH-3 (Out-of-Allocation Pointer Formation)**:
  Computing an address that lies outside the allocation $[base, base + size]$ is arithmetically constructible as long as it does not wrap `usize`, but the resulting pointer cannot be dereferenced.
- **ARITH-4 (ZST Logical Position Semantics)**:
  For Zero-Sized Types ($size(T) == 0$, $stride(T) == 0$):
  $$\text{address}(p \pm n) = \text{address}(p)$$
  The physical address is unchanged. The logical element position is an **operation-level semantic property** evaluated in iterators, slices, and bounds checks; it is **NOT** an additional physical field stored inside `PointerValue`.

---

## 4. One-Past Semantics

Slice iteration and loop termination (`while p != end`) fundamentally rely on one-past pointers:

```text
┌──────────────────────────────────────────────┬──────────┐
│ Active Allocation Bytes [base, base + size)  │ one_past │
│ dereferenceable if aligned and within bounds │ base+sz  │
└──────────────────────────────────────────────┴──────────┘
```

### 4.1 One-Past Invariants (PAST-1 to PAST-3)

- **PAST-1 (Valid Pointer Value)**:
  The pointer with address $p_{end} = base + size$ and provenance `Alloc(id)` is a **valid pointer value**.
- **PAST-2 (Never Dereferenceable)**:
  For any type with $size(T) > 0$, the one-past pointer is **never dereferenceable**, because:
  $$\text{address}(p_{end}) + \text{size}(T) = base + size + \text{size}(T) > base + size$$
  which strictly violates spatial bounds.
- **PAST-3 (Permitted Operations)**:
  One-past pointers may be:
  1. Tested for equality: `p == end`, `p != end`.
  2. Compared relationally: `p < end`, `p <= end`.
  3. Subtracted: `end - start` (yielding the element count $n$).
  4. Stepped backwards: `end - 1` (yielding a dereferenceable pointer).

---

## 5. Dereference Validity Soundness Contract

Dereferencing a raw pointer (`*p` or `*rw p`) is an `unsafe` operation. It is sound if and only if **ALL five conditions** hold:

$$\text{Dereference}(p, T) \iff \text{PROV} \land \text{SPATIAL} \land \text{ALIGN} \land \text{TEMPORAL} \land \text{INIT}$$

### 5.1 The Five Preconditions

1. **`PROV` (Valid Provenance)**:
   The pointer must possess either:
   - Tracked provenance: $p.\text{prov} == \text{Alloc}(id)$.
   - Wildcard provenance: $p.\text{prov} == \text{Wildcard}$ within an `unsafe` block.
   *(Null provenance $p.\text{prov} == \text{Null}$ is always rejected).*
2. **`SPATIAL` (Spatial Bounds)**:
   The entire byte extent must fit within the allocation:
   $$base \le \text{address}(p) \quad \land \quad \text{address}(p) + \text{size}(T) \le base + size$$
   *(For Wildcard pointers, the programmer asserts physical address validity).*
3. **`ALIGN` (Alignment)**:
   The address must satisfy the type's alignment requirement:
   $$\text{address}(p) \pmod{\text{align}(T)} == 0$$
4. **`TEMPORAL` (Allocation Alive)**:
   - For tracked pointers ($p.\text{prov} == \text{Alloc}(id)$):
     $$\text{AllocationState}(id) == \text{Active}$$
     If `AllocationState` is `Terminated`, dereferencing is an immediate **Use-After-Free** or **Use-After-Realloc** violation.
   - For Wildcard pointers: Temporal validity is asserted by the programmer in `unsafe`.
5. **`INIT` (Initialization State for Reads)**:
   For read operations (`*p`), the target memory must be in the `INITIALIZED` state (from P0.1). Reading uninitialized memory is Undefined Behavior.

---

## 6. Pointer $\leftrightarrow$ Integer Conversions

### 6.1 Pointer to Integer: `p as usize`
- **Semantics**: Extracts the purely numerical `address: usize`.
- **Provenance**: Provenance is stripped.
- **Safety**: Safe operation (allowed in safe code). Inspecting an address violates no invariants.

### 6.2 Integer to Pointer: `addr as *rw T`
- **Semantics**: Synthesizes a pointer with numerical address `addr` and `Provenance::Wildcard`.
- **Safety**: **Strictly Unsafe**. Permitted only in an `unsafe` block.
- **Contract**: The compiler does not track allocation bounds or lifetimes for `Wildcard`. The programmer assumes responsibility that `addr` is valid, mapped, and aligned.

### 6.3 Roundtrip Loss of Provenance
When a tracked pointer is cast through an integer:
$$p \xrightarrow{\text{as usize}} \text{addr} \xrightarrow{\text{as *T}} p'$$
The resulting pointer $p'$ has `Provenance::Wildcard`, **NOT** the original `Alloc(id)`. Roundtripping through integers deliberately severs compiler provenance tracking.

---

## 7. Pointer Equality & Pointer Subtraction

### 7.1 Pointer Equality: `p1 == p2`

Pointer equality compares **semantic pointer identity**, not merely the numerical address:

$$\text{p1 == p2} \iff \text{address}(p1) == \text{address}(p2) \ \land \ \text{prov}(p1) == \text{prov}(p2) \ \land \ \text{meta}(p1) == \text{meta}(p2)$$

#### Implications:
- If pointer $A$ points to `Alloc(1)` at address `0x1000`, and pointer $B$ points to `Alloc(2)` at address `0x1000` (due to memory reuse after free/realloc), then:
  $$p_A == p_B \implies \text{false}$$
  They belong to different allocations and represent distinct semantic identities.
- For Wildcard pointers: Since their provenance is untracked, two Wildcard pointers compare by numerical address and metadata.

### 7.2 Pointer Subtraction: `p2 - p1`

Pointer subtraction computes the logical element distance between two pointers. It is **defined if and only if ALL of the following hold**:

1. **Same Provenance**: Both pointers must hold `Provenance::Alloc(id)` for the **identical** `AllocId`:
   $$\text{prov}(p1) == \text{prov}(p2) == \text{Alloc}(id)$$
   *Subtracting pointers originating from different allocations (e.g. `Alloc(1)` and `Alloc(2)`) is undefined behavior.*
2. **Same Allocation Range**: Both addresses must lie within the same valid range $[base, base + size]$.
3. **Compatible Stride**: $\text{stride}(T) > 0$.
4. **Exact Multiple**: The address difference must be an exact multiple of the element stride:
   $$(\text{address}(p2) - \text{address}(p1)) \pmod{\text{stride}(T)} == 0$$
5. **Representability**: The resulting distance $(\text{address}(p2) - \text{address}(p1)) / \text{stride}(T)$ must fit within a signed `isize`.

$$\text{distance}(p1, p2) = \frac{\text{address}(p2) - \text{address}(p1)}{\text{stride}(T)}$$

---

## 8. Lifetime Invalidation & Realloc Interaction

### 8.1 Reallocation Invalidation (Same-Address Invariant)

When `realloc(p, old_size, old_align, new_size)` succeeds and returns `new_p`:
1. The old allocation $\text{AllocId}_{old}$ is permanently marked `AllocationState::Terminated`.
2. A new allocation $\text{AllocId}_{new}$ is created with state `AllocationState::Active`.
3. Even if the operating system allocator resizes in-place such that:
   $$\text{address}(new\_p) == \text{address}(p)$$
   the old pointer $p$ still points to $\text{Alloc}(\text{AllocId}_{old})$, which is now `Terminated`.
4. Any attempt to dereference $p$ fails the `TEMPORAL` precondition, preventing **Use-After-Realloc**. Only `new_p` holds the valid `Alloc(\text{AllocId}_{new})`.

### 8.2 Stack Frame Pop
When a function returns:
- All stack allocations belonging to that frame transition to `AllocationState::Terminated`.
- Any raw pointer derived from a local variable becomes dangling with `Terminated` allocation state.

---

## 9. Summary & Transition to P3 Allocator Contract

With this P2 Hardened Specification:
1. `Provenance` is cleanly decoupled from `AllocationState`.
2. `round_up` and pointer arithmetic invariants forbid address space wraparound.
3. One-past pointers are formalized as valid non-dereferenceable pointers.
4. ZST logical index is established as an operation-level semantic property.
5. Wildcard provenance and the 5-condition dereference contract are harmonized.
6. Pointer equality and pointer subtraction are mathematically specified.
7. Reallocation same-address invalidation is locked.

The foundation is complete for **P3: Allocator Contract (`GlobalAlloc` / `Allocator` interface)**, where the allocator solely creates and terminates `Allocation` entities without carrying provenance bookkeeping logic itself.
