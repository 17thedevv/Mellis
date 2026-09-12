# Mellis Compiler RFC: Allocator Contract & Interface Specification (P3 Hardened)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P3 Hardened)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P3)
- **Scope**: Storage Acquisition & Release, `Allocator` Trait, Preconditions/Postconditions, OOM Policies, 6-State Reallocation Matrix, AllocatorInstanceId & Domains, GlobalAlloc Contract
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Allocator Contracts ("How do we obtain and release storage?")

---

## 1. Architectural Role & Boundary

Following the freeze of **P0 (Memory Model)**, **P1 (Layout Model)**, and **P2 (Pointer & Provenance)**, the role of the Allocator subsystem is strictly delineated:

> **The Allocator Responsibility Boundary**:
> The Allocator is solely responsible for acquiring, resizing, and releasing physical memory storage blocks.
>
> The Allocator does **NOT**:
> - Construct, inspect, move, or drop typed objects $T$ (P0 / P6).
> - Compute or define type layouts, field offsets, or stride (P1).
> - Track pointer provenance tags or allocation lifetimes (P2).

### 1.1 Layering Architecture

```text
┌─────────────────────────────────────┐
│ P1 Layout Model                     │
│ "What storage shape is required?"   │
└──────────────────┬──────────────────┘
                   ▼
┌─────────────────────────────────────┐
│ P3 Allocator Contract               │
│ "How is raw storage obtained?"      │
│                                     │
│ AllocatorInstanceId                 │
│ AllocatorDomain                     │
│ allocate / deallocate / reallocate  │
└──────────────────┬──────────────────┘
                   ▼
┌─────────────────────────────────────┐
│ P0 / P2 Memory Model                │
│ AllocId + Provenance + Lifetime     │
└─────────────────────────────────────┘
```

The resulting allocation entity tracked by the compiler memory model is:

```rust
pub struct Allocation {
    pub id: AllocId,
    pub allocator_instance_id: AllocatorInstanceId,
    pub domain: AllocatorDomain,
    pub base: usize,
    pub size: usize,
    pub align: usize,
    pub state: AllocationState,
}
```

---

## 2. Core Allocator Interface (`Allocator` Trait)

Mellis defines an extensible, strongly-typed contract for memory allocation.

### 2.1 The `Allocator` Trait Definition

```rust
pub trait Allocator {
    /// Returns the unique instance identifier of this allocator.
    fn instance_id(&self) -> AllocatorInstanceId;

    /// Returns the domain category of this allocator.
    fn domain(&self) -> AllocatorDomain;

    /// Acquires a block of raw uninitialized storage matching `layout`.
    ///
    /// # Safety & Preconditions
    /// - `layout` must be a valid `Layout` (LAY-1, LAY-3).
    /// - For ZSTs (`layout.size == 0`), the allocator must return a valid,
    ///   non-null, properly aligned pointer without invoking the physical OS heap.
    unsafe fn allocate(&self, layout: Layout) -> Result<*rw u8, AllocError>;

    /// Releases a previously allocated storage block back to this allocator.
    ///
    /// # Safety & Preconditions
    /// - `ptr` must have been returned by an allocation or reallocation call
    ///   on THIS exact allocator instance (`self.instance_id() == allocation.allocator_instance_id`).
    /// - `layout` must match the exact layout passed when the block was allocated
    ///   (or resulting from the last reallocation).
    /// - Any typed object residing in this storage must have already been dropped (LIFE-1).
    unsafe fn deallocate(&self, ptr: *rw u8, layout: Layout);

    /// Resizes an existing raw storage block while preserving alignment.
    ///
    /// # Safety & Preconditions
    /// - `ptr` and `old_layout` must represent a currently active allocation
    ///   belonging to THIS exact allocator instance.
    /// - Alignment of `old_layout` is preserved for `new_size`.
    unsafe fn reallocate(
        &self,
        ptr: *rw u8,
        old_layout: Layout,
        new_size: usize,
    ) -> Result<*rw u8, AllocError>;
}
```

### 2.2 Error Ownership: `AllocError` vs `LayoutError`

Mellis maintains a clean boundary between semantic layout errors and allocator capability errors:

- **`LayoutError` (P1)**: The layout itself is mathematically or semantically invalid (e.g. non-power-of-2 alignment, arithmetic overflow, exceeds `MAX_OBJECT_SIZE`).
- **`AllocError` (P3)**: The layout is semantically valid, but this specific allocator instance or platform target cannot satisfy the request:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllocError {
    /// Insufficient physical/virtual memory available to satisfy the request.
    OutOfMemory,
    /// The layout is semantically valid, but exceeds this specific allocator's
    /// capability (e.g. requested alignment exceeds this allocator's maximum threshold).
    UnsupportedLayout,
}
```

---

## 3. Allocation Contract & Invariants

### 3.1 Preconditions (ALLOC-PRE)
When invoking `allocator.allocate(layout)`:
1. `layout.align` must be a non-zero power of 2.
2. `layout.size <= MAX_OBJECT_SIZE`.
3. `layout.align <= TARGET_MAX_SUPPORTED_ALIGN`.

### 3.2 Postconditions on Success (ALLOC-POST-OK)
If `allocate` returns `Ok(ptr)`:
1. **Non-Null**: $ptr \ne \text{null}$.
2. **Alignment**: $ptr \pmod{layout.align} == 0$.
3. **Contiguity & Exclusivity**: The byte range $[ptr, ptr + layout.size)$ is contiguous and exclusively reserved for the caller.
4. **Uninitialized Storage**: The initial contents of the memory block are completely uninitialized.
5. **ZST Guarantee**: If $layout.size == 0$, $ptr$ is a valid, aligned ZST representation (e.g. `g_mellis_zero_sentinel`), and no OS/physical memory was allocated.

### 3.3 The OOM Policy Separation
The `Allocator` trait operates purely on fallible results: `Result<*rw u8, AllocError>`.
Infallibility is a **caller/container policy**, not an internal allocator trait mode:
- **Fallible Callers** (e.g. `try_reserve`): Inspect `Err(AllocError::OutOfMemory)` and recover gracefully without state corruption.
- **Infallible Callers** (e.g. standard `Box::new`, default `Vec::push`): Convert `Err(AllocError::OutOfMemory)` into a deterministic panic/abort via `__mellis_panic_code(MELLIS_ERR_ALLOC_FAILURE)`.

---

## 4. Deallocation Contract & Pairing Invariants

Deallocation returns raw storage back to the owning allocator instance.

### 4.1 AllocatorInstanceId vs. AllocatorDomain

To ensure complete soundness, Mellis distinguishes between domain classification and exact instance identity:

- **`AllocatorDomain`**: The broad category/tier of allocator (`Global`, `Arena(u64)`, `Region(u64)`, `Custom(u64)`). Answers: *"What realm of allocator is this?"*
- **`AllocatorInstanceId`**: A globally unique 64-bit numerical identifier identifying the exact allocator object. Answers: *"Which exact allocator instance owns this allocation?"*

### 4.2 Preconditions (DEALLOC-PRE)
When invoking `allocator.deallocate(ptr, layout)`:
1. **Identity**: `ptr` must be the base address of an active allocation.
2. **Instance Match**: `allocator.instance_id() == allocation.allocator_instance_id`. Calling `deallocate` on a different allocator instance—even of the same type—is strictly Undefined Behavior.
3. **Size Match**: `layout.size` must equal the exact size under which `ptr` was allocated or last reallocated.
4. **Alignment Match**: `layout.align` must equal the exact alignment under which `ptr` was allocated.
5. **Object Destructed**: Any typed object $T$ residing in $[ptr, ptr + layout.size)$ must have already been dropped via `DropInPlace` (or moved out). Deallocation never implicitly drops an object.
6. **No Double-Deallocate**: `ptr` must not have been previously deallocated.

### 4.3 Postconditions (DEALLOC-POST)
1. The memory range $[ptr, ptr + layout.size)$ is surrendered back to the allocator.
2. For ZSTs ($layout.size == 0$), `deallocate` is a guaranteed semantic no-op.
3. The compiler's provenance layer marks the associated `AllocId` as `AllocationState::Terminated`.

---

## 5. Reallocation Contract & Matrix

### 5.1 Decoupling Raw-Byte Preservation from Object Relocation

> **Raw-Storage Reallocation Invariant**:
> `Allocator::reallocate` is strictly a raw-storage operation. It preserves the byte contents of the overlapping raw storage range $[0, \min(\text{old\_size}, \text{new\_size}))$ byte-for-byte.
>
> `Allocator::reallocate` does **NOT** begin, end, move, copy, or drop the lifetime of typed objects. Typed-object relocation, drop obligations, and move semantics are governed exclusively by the language ownership model (P0 / P6).

### 5.2 The 6-State Reallocation Matrix

| Source State (`old_layout.size`) | Target State (`new_size`) | Allocator Outcome | Resulting Storage Action | Preservation Invariant |
|:---|:---|:---|:---|:---|
| **non-ZST** ($> 0$) | **non-ZST** ($> 0$) | `Ok(new_ptr)` | Storage resized (in-place or new block allocated + old freed). | $\min(\text{old}, \text{new})$ bytes preserved byte-for-byte. |
| **non-ZST** ($> 0$) | **non-ZST** ($> 0$) | `Err(AllocError)` | Reallocation fails (OOM). | **Old block at `ptr` remains valid, untouched, and caller-owned.** |
| **non-ZST** ($> 0$) | **ZST** ($== 0$) | `Ok(zst_ptr)` | Old physical storage deallocated; canonical ZST pointer returned. | Cannot fail; zero bytes preserved. |
| **ZST** ($== 0$) | **non-ZST** ($> 0$) | `Ok(new_ptr)` | Fresh storage allocated for `new_size` bytes. | Zero bytes preserved (old was ZST). |
| **ZST** ($== 0$) | **non-ZST** ($> 0$) | `Err(AllocError)` | Fresh allocation fails (OOM). | **Old ZST state remains completely valid.** No memory lost. |
| **ZST** ($== 0$) | **ZST** ($== 0$) | `Ok(zst_ptr)` | No-op; canonical ZST pointer returned. | Cannot fail. |

---

## 6. Global vs. Custom Allocator Boundary

### 6.1 The System Default: `GlobalAlloc`
The default system heap allocator is represented by the zero-sized unit struct `GlobalAlloc`:
- Implements the `Allocator` trait.
- Domain is `AllocatorDomain::Global`.
- Instance identity is statically unified (`instance_id() == 0`).
- Satisfies the `Allocator` contract by delegating directly to the native runtime memory subsystem. (The 1-to-1 FFI function bindings `__mellis_alloc`, `__mellis_dealloc`, `__mellis_realloc` are specified and frozen in P4).

### 6.2 Custom Allocators (Tier 2 Container Integration)
For custom allocators (e.g. `ArenaAllocator`, `RegionAllocator`):
- Containers parameterized by an allocator (`RawVec<T, A: Allocator>`) must retain the exact allocator instance associated with each allocation.
- The container must invoke `self.alloc.deallocate` using that retained allocator instance, ensuring `self.alloc.instance_id() == allocation.allocator_instance_id`.

---

## 7. Summary & Transition Criteria to P4

With this P3 Hardened Specification:
1. `AllocatorInstanceId` is decoupled from `AllocatorDomain`, guaranteeing exact instance deallocation pairing.
2. The reallocation matrix is corrected: ZST $\to$ non-ZST OOM leaves the old ZST state valid.
3. Raw-byte preservation is cleanly separated from object relocation and drop semantics.
4. `AllocError::UnsupportedLayout` is separated from `LayoutError`.
5. Infallible execution is established as a caller policy, keeping the `Allocator` trait pure.
6. `GlobalAlloc` contract is established without prematurely embedding P4 FFI lowering details.

Upon review and approval, the Tier 1 roadmap advances to **P4: Runtime ABI Integration (`__mellis_alloc`, `__mellis_dealloc`, `__mellis_realloc`)**.
