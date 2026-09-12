# Mellis Compiler RFC: Layout Model Specification (P1 Hardened)

- **Status**: FROZEN SPECIFICATION CANDIDATE (P1 Hardened)
- **Milestone**: Tier 1 — Allocation & Memory Layout Foundation (P1)
- **Scope**: Memory Layout Representation, Size, Alignment, Stride, Composition Algebra, Representability, ZST Layout, Representation Selection Boundary
- **Authority**: Canonical Single Source of Truth (SSOT) for Mellis Data Layout ("How is memory laid out?")

---

## 1. Architectural Motivation & Layering Boundary

While **P0 (Memory Model Contract)** defines what memory *means* (lifecycle, provenance, ZST contract, realloc invariants, allocator pairing), **P1 (Layout Model)** defines how data is *spatially arranged in memory*.

### 1.1 The Two-Tier Representation Architecture

To allow zero-cost abstractions such as niche optimization (`Option<&T>`, `Result<T, E>`) without distorting the middle-end type system, Mellis cleanly separates **Abstract Representation Requirements** from **Target Representation Selection**:

```text
┌────────────────────────────────────────────────────────┐
│                   Type Layout Model                    │
│  Computes abstract layout algebra, validity bounds,    │
│  size, alignment, and field placement constraints      │
└───────────────────────────┬────────────────────────────┘
                            │
┌───────────────────────────▼────────────────────────────┐
│         Abstract Representation Requirements           │
│  Defines what representations are valid (size bounds,  │
│  alignment bounds, field offsets, variant constraints) │
└───────────────────────────┬────────────────────────────┘
                            │
┌───────────────────────────▼────────────────────────────┐
│            Target Representation Selection             │
│  Selects the concrete ABI layout on a given target:    │
│  - Baseline tagged representation                      │
│  - Niche-optimized representation (e.g. tagless &T)    │
│  - Target-specific ABI packing                         │
└────────────────────────────────────────────────────────┘
```

P1 locks **what representations are valid**, without prematurely restricting **which valid representation a target architecture or backend selects**.

---

## 2. Layout Structure & Core Invariants

### 2.1 The `Layout` Value Type

A memory layout in Mellis is an immutable value type:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layout {
    size: usize,
    align: usize,
}
```

- **`size`**: The exact number of occupied bytes. In an intermediate layout (during sequential struct composition), `size` is not required to be a multiple of `align`.
- **`align`**: The required power-of-two memory address boundary.

### 2.2 Core Invariants (LAY-1 to LAY-5)

- **LAY-1 (Semantic Alignment Invariant)**:
  Every valid `Layout` must satisfy:
  $$align > 0 \quad \land \quad (align \ \& \ (align - 1)) == 0$$
  This is an intrinsic language law, independent of any platform or compiler backend.
- **LAY-2 (Decoupled Target Capability Limit)**:
  Target architectures and runtime allocators define a target capability limit:
  $$align \le \text{TARGET\_MAX\_SUPPORTED\_ALIGN}$$
  For the current x86-64 target and `mellis-runtime.lib`, $\text{TARGET\_MAX\_SUPPORTED\_ALIGN} = 4096$. If a future architecture or OS requires 16KB/64KB pages or 8192-byte SIMD alignments, the target capability limit is updated without modifying the language's semantic layout definition.
- **LAY-3 (Pointer-Distance Representability Limit)**:
  To guarantee that pointer difference `p2 - p1` between any two points within an object, array, or slice is strictly representable within a signed `isize` without arithmetic overflow:
  $$\text{MAX\_OBJECT\_SIZE} = \text{isize::MAX as usize} \quad (\approx 2^{63}-1 \text{ on 64-bit})$$
  $$size \le \text{MAX\_OBJECT\_SIZE}$$
  Any layout request with $size > \text{MAX\_OBJECT\_SIZE}$ is rejected as unrepresentable (`LayoutError::ExceedsAddressSpaceLimit`).
- **LAY-4 (Zero-Sized Layouts)**:
  A layout with $size == 0$ is valid for any power-of-two alignment:
  $$\text{Layout::from\_size\_align}(0, align) \quad \text{is valid } \forall align \in \text{PowerOfTwo}$$
- **LAY-5 (Checked Alignment Padding)**:
  The padding required to advance an unaligned offset $s$ to alignment $a$ is computed without arithmetic overflow:
  $$\text{rem} = s \ \& \ (a - 1)$$
  $$\text{padding}(s, a) = \begin{cases} 0 & \text{if } \text{rem} == 0 \\ a - \text{rem} & \text{if } \text{rem} > 0 \end{cases}$$
  $$\text{round\_up}(s, a) = \text{checked\_add}(s, \text{padding}(s, a))$$
  If `round_up` overflows or exceeds $\text{MAX\_OBJECT\_SIZE}$, it returns `Err(LayoutError::ArithmeticOverflow)`.

---

## 3. Derived Metrics: Size, Stride, and Array Size

Mellis strictly formalizes three distinct geometric quantities:

$$\text{size}(T) \quad \le \quad \text{stride}(T) \quad \le \quad \text{array\_size}(T, n)$$

### 3.1 Formal Definitions (MET-1 to MET-4)

- **MET-1 (`size`)**:
  The physical bytes occupied by the type's values. Intermediate layouts during struct composition may have `size % align != 0`. A **complete type layout** enforces trailing padding such that its complete size equals its stride:
  $$\text{complete\_type\_size}(T) = \text{stride}(T)$$
- **MET-2 (`stride`)**:
  The distance in bytes between consecutive elements in an array, slice, or sequence:
  $$\text{stride}(T) = \begin{cases}
  0 & \text{if } \text{size}(T) == 0 \\
  \text{round\_up}(\text{size}(T), \text{align}(T)) & \text{if } \text{size}(T) > 0
  \end{cases}$$
- **MET-3 (`array_size`)**:
  The total byte extent of an array of $n$ elements of type $T$:
  $$\text{array\_size}(T, n) = \begin{cases}
  0 & \text{if } n == 0 \lor \text{size}(T) == 0 \\
  \text{checked\_mul}(n, \text{stride}(T)) & \text{otherwise}
  \end{cases}$$
  Any multiplication exceeding $\text{MAX\_OBJECT\_SIZE}$ fails with `LayoutError::ArithmeticOverflow`.
- **MET-4 (ZST Array Contract)**:
  For any Zero-Sized Type $T$ ($\text{size}(T) == 0$):
  $$\text{size}(ZST) = 0, \quad \text{stride}(ZST) = 0, \quad \text{align}(ZST) = \text{align}(T) \ge 1$$
  $$\text{array\_size}(ZST, n) = 0 \quad (\forall n \ge 0)$$
  An empty array `[T; 0]` of ANY type (even with large alignment such as 4096) is always valid and produces:
  $$\text{size} = 0, \quad \text{stride} = 0, \quad \text{align} = \text{align}(T)$$

---

## 4. Composition Algebra: `extend` and `repeat`

Complex layouts are generated by functional composition over primitive layouts.

### 4.1 Field Extension: `layout.extend(next)`

Places field layout `next` immediately after intermediate layout `self`:

```rust
impl Layout {
    pub fn extend(&self, next: &Layout) -> Result<(Layout, usize), LayoutError>;
}
```

#### Step-by-Step Composition Algorithm:
1. **Field Offset**:
   Compute aligned placement offset for `next`:
   $$\text{offset} = \text{round\_up}(self.size, next.align)?$$
2. **New Size**:
   $$\text{new\_size} = \text{checked\_add}(\text{offset}, next.size)?$$
   If $\text{new\_size} > \text{MAX\_OBJECT\_SIZE}$, return `Err(LayoutError::ExceedsAddressSpaceLimit)`.
3. **Composite Alignment**:
   $$\text{new\_align} = \max(self.align, next.align)$$
4. **Result**:
   Returns `Ok((Layout { size: new_size, align: new_align }, offset))`.

### 4.2 Repetition / Array Composition: `layout.repeat(n)`

Constructs the layout for an array or slice buffer of $n$ elements of `self`:

```rust
impl Layout {
    pub fn repeat(&self, n: usize) -> Result<(Layout, usize), LayoutError>;
}
```

#### Step-by-Step Repetition Algorithm:
1. **Element Stride**:
   $$\text{elem\_stride} = self.stride()$$
2. **Total Size**:
   - If $n == 0$ or $self.size == 0$: $\text{total\_size} = 0$.
   - If $n > 0$ and $self.size > 0$:
     $$\text{total\_size} = \text{checked\_mul}(n, \text{elem\_stride})?$$
     If $\text{total\_size} > \text{MAX\_OBJECT\_SIZE}$, return `Err(LayoutError::ArithmeticOverflow)`.
3. **Resulting Layout**:
   - Alignment is preserved: $\text{array\_align} = self.align$.
   - Returns `Ok((Layout { size: total_size, align: array_align }, elem_stride))`.
   - **ZST Invariant**: For ZST ($self.size == 0$), returns `Ok((Layout { size: 0, align: self.align }, 0))`.

---

## 5. Type-Level Layout Rules

### 5.1 Primitive Scalars & Sized Pointers

On the 64-bit architecture:
- `bool`, `u8`, `i8`: `size = 1, align = 1`
- `u16`, `i16`: `size = 2, align = 2`
- `u32`, `i32`, `f32`, `char`: `size = 4, align = 4`
- `u64`, `i64`, `f64`, `usize`, `isize`: `size = 8, align = 8`
- `u128`, `i128`: `size = 16, align = 16`
- **Sized Pointers & References**:
  For any sized type $T$ ($T: \text{Sized}$):
  $$\text{size}(*T) = 8, \quad \text{align}(*T) = 8$$
  $$\text{size}(*rw\ T) = 8, \quad \text{align}(*rw\ T) = 8$$
  $$\text{size}(\&T) = 8, \quad \text{align}(\&T) = 8$$
  $$\text{size}(\&rw\ T) = 8, \quad \text{align}(\&rw\ T) = 8$$
- **Dynamically Sized Types (DST) / Fat Pointers Scope**:
  Pointers to unsized types (`*[T]`, `*dyn Trait`, `&[T]`, `&dyn Trait`) carry metadata. Their full provenance, metadata layout, and dereference rules are formally specified in **P2**. On the current 64-bit target, baseline representations are 2 machine words (16 bytes, align 8).

### 5.2 Structs and Tuples

Sequential field composition followed by complete-object trailing padding:

1. Initialize accumulator: $L_0 = \text{Layout::from\_size\_align}(0, 1)$.
2. For each field $F_i$ ($i \in 1..K$):
   $$(L_i, \text{offset}_i) = L_{i-1}.\text{extend}(\text{Layout}(F_i))$$
3. Trailing padding:
   $$\text{struct\_align} = L_K.align$$
   $$\text{struct\_size} = \text{round\_up}(L_K.size, \text{struct\_align})$$
   $$\text{Layout}(\text{Struct}) = \text{Layout} \{ \text{size}: \text{struct\_size}, \text{align}: \text{struct\_align} \}$$

### 5.3 Formal Enum Baseline Layout

For an enum with discriminant tag $Tag$ and $K$ variants $V_1, \dots, V_K$:

```text
┌────────────┬──────────────────┬─────────────────────────┬───────────────────┐
│ tag        │ padding          │ payload variant union   │ trailing padding  │
│ [0, sz_tag)│ [sz_tag, off_pay)│ [off_pay, off_pay+sz_pay) [end_pay, total_sz) │
└────────────┴──────────────────┴─────────────────────────┴───────────────────┘
```

#### Formal Composition Rules:
1. **Payload Alignment**:
   $$\text{payload\_align} = \max_{i=1}^K \text{align}(V_i) \quad (\text{or } 1 \text{ if } K == 0)$$
2. **Payload Offset**:
   The payload begins at the first byte satisfying `payload_align` after the tag:
   $$\text{payload\_offset} = \text{round\_up}(\text{size}(Tag), \text{payload\_align})$$
3. **Payload Size**:
   $$\text{payload\_size} = \max_{i=1}^K \text{size}(V_i) \quad (\text{or } 0 \text{ if } K == 0)$$
4. **Enum Alignment**:
   $$\text{enum\_align} = \max(\text{align}(Tag), \text{payload\_align})$$
5. **Enum Total Size**:
   $$\text{unpadded\_size} = \text{payload\_offset} + \text{payload\_size}$$
   $$\text{enum\_size} = \text{round\_up}(\text{unpadded\_size}, \text{enum\_align})$$
   $$\text{Layout}(\text{Enum}) = \text{Layout} \{ \text{size}: \text{enum\_size}, \text{align}: \text{enum\_align} \}$$

---

## 6. Representation Selection Boundary

To enable optimizations without violating the semantic layout model, Mellis defines explicit representation criteria:

1. **Baseline Tagged Representation**:
   Always valid for any enum. Uses the explicit layout computed in Section 5.3.
2. **Niche Optimization Eligibility**:
   An enum is eligible for niche optimization if:
   - It possesses exactly two variants: one empty variant (e.g. `None`) and one payload variant (e.g. `Some(T)`).
   - The payload type $T$ has at least one **niche** — meaning its representation has one or more bit patterns that are provably invalid for valid values of that type (e.g., the all-zero `null` bit pattern in a non-null reference `&T` or `&rw T`).
   - *Note on raw pointers*: Because primitive raw pointers (`*T`, `*rw T`) can legally represent `null`, raw pointers do not inherently have a null niche. A niche exists only if a type representation enforces an explicit non-null validity invariant.
   - In this representation, the discriminant tag is absorbed into the niche of $T$:
     $$\text{size}(\text{Option}\langle \&T \rangle) = \text{size}(\&T) = 8, \quad \text{align} = 8$$
3. **Target ABI Selection**:
   The middle-end (`mellis-semantic`) validates representation requirements; the target backend selects the most efficient representation matching those requirements.

---

## 7. Error Model: `LayoutError`

Checked arithmetic, representability bounds, and alignment invariants emit strongly-typed errors:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    ZeroAlignment,
    NonPowerOfTwoAlignment(usize),
    ExceedsTargetCapability { requested: usize, max_supported: usize },
    ArithmeticOverflow,
    ExceedsAddressSpaceLimit { requested: usize, max_allowed: usize },
}
```

---

## 8. Summary & Transition Criteria to P2

With this P1 Hardened RFC:
1. `round_up` is guaranteed overflow-free via checked arithmetic.
2. Enum baseline layout is mathematically formalized with exact payload offsets and trailing padding.
3. Semantic Layout Requirements are decoupled from Target Representation Selection.
4. Sized pointers are explicitly scoped, deferring DST metadata details to P2.
5. `Layout.size` and `Layout.stride()` are cleanly distinguished.
6. `repeat(0)` and ZST repetition invariants are locked.
7. `MAX_OBJECT_SIZE` is specified as a pointer-distance representability requirement.

Upon review and approval, the roadmap advances to **P2: Raw Pointer Semantics & Provenance Rules**.
