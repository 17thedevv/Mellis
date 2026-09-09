# Mellis Normative Language Rules: Foundations (P0 & P1)

## Status: FROZEN (v1.0)
**Context**: Finalized following *Mellis Language Semantic Audit (Specification-Level)*.  
**Purpose**: Establish four foundational, mathematically sound normative rules to serve as the unshakeable source of truth for the Mellis type system, borrow checker, and compiler architecture.

---

## Rule A: Mutual Exclusivity of `Copy` and `Drop` (P0)

### A.1 Core Invariant
$$\forall T \in \text{ConcreteTypes}, \quad \neg (\text{satisfies}(T, \text{Copy}) \land \text{satisfies}(T, \text{Drop}))$$

A concrete type cannot satisfy both `Copy` and `Drop`.

### A.2 Rationale
`Copy` guarantees that bitwise duplication of a value's memory footprint produces a distinct, fully independent instance without lifecycle hooks. `Drop` guarantees that a value owns resources requiring custom finalization logic executed upon scope exit. Permitting a concrete type to satisfy both creates an irreconcilable soundness paradox: bitwise copying duplicates the instance without invoking lifecycle hooks, leading to guaranteed double-free or resource corruption when the destructor executes on multiple bitwise copies.

### A.3 Enforcement Paths
1. **Direct Implementation**:
   - Given `impl Drop for T`, any declaration `impl Copy for T` (or vice-versa) is a static compile-time error (`E_COPY_DROP_CONFLICT`).
2. **Derived Implementation**:
   - `#[derive(Copy)]` on a struct or enum is rejected if the type itself implements `Drop`.
3. **Transitive Field Constraints**:
   - A composite type (struct, enum, tuple) cannot satisfy or derive `Copy` if any transitive field $f_i$ satisfies `Drop`.
4. **Separation from Generic Bounds**:
   - The invariant applies strictly to **concrete types**.
   - A generic bound such as `fn foo<T: Copy + Drop>(x: T)` specifies an impossible constraint for any valid Mellis type. Whether the compiler rejects this bound at definition-time (via an optional bound-consistency checker) or at instantiation-time (when no concrete type can satisfy both) is a diagnostic design choice, and does not alter the core invariant.

---

## Rule B: Prohibition of Subplace Moves under `Drop` (P0)

### B.1 Core Invariant
$$\forall \text{Place } p, \forall \text{Place } a \text{ such that } a \sqsubset p, \quad \text{satisfies}(\text{typeof}(a), \text{Drop}) \implies \neg\text{can\_move}(p)$$

A proper subplace of a `Drop` place cannot be moved out. Whole-place move remains legal.

### B.2 Rationale
When a place $a$ of a type implementing `Drop` goes out of scope, the destructor hook `Drop::drop(&rw a)` must execute on a fully valid, fully initialized memory place. Moving a proper subplace $p = a.\text{field}$ leaves $a$ in a partially deinitialized state. Invoking `Drop::drop` on a partially deinitialized struct causes undefined behavior.

### B.3 Place Semantics
1. **Proper Subplace Move**:
   - Given a place $c$ whose type satisfies `Drop`: evaluating `move c.field` (by variable binding, function argument, or pattern match) is **strictly forbidden** (`E_PARTIAL_MOVE_UNDER_DROP`).
2. **Hierarchical Propagation**:
   - If $a \sqsubset p$ (i.e., $a$ is any strict prefix or ancestor place of $p$) and $\text{typeof}(a)$ satisfies `Drop`, moving $p$ is forbidden.
3. **Whole-Place Move**:
   - Moving the entire container $c$ (`dec a = c;`) transfers full ownership of all fields and shifts the destructor obligation to the new binding. **Whole-place move is fully permitted.**
4. **Field Borrowing**:
   - Borrowing a subplace (`&c.field` or `&rw c.field`) does not deinitialize the container. It is **fully permitted** under normal non-lexical borrow check rules.
5. **Field Reassignment / Overwrite**:
   - Evaluating `c.field = new_value;` on an already-initialized container $c$ does not produce a partially moved state. The previous field value is dropped in-place and replaced with `new_value`. The container $c$ remains fully initialized. **Field assignment is fully permitted.**

---

## Rule C: Associated Type Projections & The Monomorphization Barrier (P0)

### C.1 Core Invariant
Associated type projections may remain symbolic during generic semantic analysis, but no unresolved projection may cross the monomorphization boundary into MVIR and backend code generation.

### C.2 Rationale
During generic typechecking and constraint solving, associated types (such as `<T as Try>::Output` or `<T as Try>::Residual`) cannot always be normalized immediately because the concrete type argument for `T` is unknown. Demanding premature normalization before borrow checking would cripple generic analysis. However, downstream phases (monomorphic MVIR lowering, memory layout calculation, LLVM code generation) require completely concrete types.

### C.3 Phase-by-Phase Lifecycle
```
Source Generic AST
       │
       ▼
Generic Semantic & Borrow Analysis ──► Projections (<T as Trait>::Assoc) may remain symbolic
       │                               Obligations & equality constraints (T::Assoc == U) recorded
       ▼
Trait Solving & Normalization ──────► Normalized eagerly where concrete impls are known
       │
       ▼
=== MONOMORPHIZATION BARRIER ===
       │
       ▼
Monomorphic MVIR & Backend Lowering ─► STRICT INVARIANT: Zero unresolved projections,
                                       zero GenericParam, and zero InferenceVar allowed.
```

### C.4 Soundness of the `Try` Protocol
For the desugaring of `val?`:
1. In a generic function with `T: Try`, the branch return type is symbolically `<T as Try>::Residual` and `<T as Try>::Output`.
2. The caller return constraint is unified: $\text{ReturnType} : \text{FromResidual}<<T \text{ as Try}>::\text{Residual}>$.
3. Upon monomorphization with concrete type $T = \text{Result}<i32, \text{Error}>$, the projections normalize to:
   - `<T as Try>::Output` $\longrightarrow i32$
   - `<T as Try>::Residual` $\longrightarrow \text{Result}<\text{Infallible}, \text{Error}>$
4. Full type soundness is verified prior to MVIR emission.

---

## Rule D: Trait Coherence & The Nominal Head Orphan Rule (P1)

### D.1 Core Invariant
$$\forall (\text{Trait}, \text{Type}) \text{ in program}, \quad \text{applicable\_impls}(\text{Trait}, \text{Type}) \le 1$$

For every canonical $(\text{Trait}, \text{Type})$ pair in a program, there is at most one applicable implementation.

**Overlap Invariant**: Two `impl` declarations are illegal if their canonical trait and self-type patterns can unify (i.e., coherence is checked over unifiable pattern overlap, not merely syntactic equality).

### D.2 The Nominal Head Orphan Rule
An implementation `impl<...> Trait for TargetType` is legal within provider $P$ if and only if at least one of the following conditions is satisfied:

$$\text{is\_declared\_in}(Trait, P) \lor \text{is\_declared\_in}(\text{nominal\_head}(TargetType), P)$$

1. **Local Trait**: The `Trait` is declared within provider $P$.
2. **Local Nominal Head Type**: The root constructor (nominal head) of `TargetType` is declared within provider $P$.

*Note: The compiler strictly checks the nominal head constructor of the Self type. It does NOT inspect generic arguments to discover nested local types.*

### D.3 Precise Matrix for Generic Wrappers

| Implementation Declaration | Provider of Nominal Head | Provider of Inner Type / Generic Param | Legal in Provider $P$? | Reason |
| :--- | :--- | :--- | :--- | :--- |
| `impl<T> ExternalTrait for LocalWrapper<T>` | Local ($P$) | External / Generic | **YES** | `LocalWrapper` is the local nominal head. |
| `impl<T> ExternalTrait for ExternalWrapper<LocalType>` | External | Local ($P$) | **NO (REJECT)** | Nominal head `ExternalWrapper` is external. |
| `impl<T> ExternalTrait for ExternalWrapper<T>` | External | Generic Param | **NO (REJECT)** | Nominal head `ExternalWrapper` is external. |
| `impl ExternalTrait for ExternalType` | External | External | **NO (REJECT)** | Pure orphan implementation. Strictly forbidden. |

### D.4 Consequence for Language Extensibility
* A user cannot write `impl Try for i32` or `impl Drop for String` because neither the trait nor the nominal type head is local.
* A user can write `impl Try for MyResult<T, E>` because `MyResult` is a local nominal type.
* Fundamental types (such as `&T`, `*T`, `rw T`) will be governed by a separate *Fundamental Type Semantics* specification and do not alter this base rule.
* This guarantees that independent packages can be compiled and linked together without silent implementation collisions or coherence breakdowns.

---

## Rule E: Asynchronous Borrow Invariant & Future Storage (P1-B)

### E.1 Core Invariant
$$\forall \text{Future } F, \forall \text{Place } p, q \in \text{Env}(F), \quad \neg (\text{points\_to}(p, q))$$

1. **Prohibition of Self-Referential Futures**: No place or reference within an async state machine's environment (`EnvStruct`) may point to another place within the same future instance (`LocalLoan`).
2. **External Borrow Provenance**: A reference parameter or borrowed place crossing an await point is sound if and only if its lifetime/provenance originates from outside the future (`FutureLoan`).
3. **Future Loan Lifecycle**:
   - A `FutureLoan` attached to a future remains active while the future place/value is live under NLL.
   - Moving a future (`fut1 -> fut2`) transfers the `FutureLoan` obligation and provenance to `fut2`.
   - When the future terminates (via normal completion / consumption `await fut`, explicit destruction `drop(fut)`, or scope exit), the loan terminates.

---

## Rule F: Async × Drop & State-Dispatched Cancellation (P1-C)

### F.1 Core Invariant
$$\forall T, \quad \text{needs\_drop}(\text{Future}<T>) = \text{true}$$

Every Mellis `Future<T>` owns a runtime execution environment (`EnvStruct`) whose destruction requires a specialized future destructor `{func}_drop`, regardless of whether $T$ implements `Drop`.

### F.2 State-Dispatched Cleanup Protocol
Future destruction is dispatched based on the dynamic state discriminant stored in the environment:
1. **`Initial` (State 0)**:
   - Drops only owned places that are initialized and live at cancellation time (function parameters and pre-await initializations).
   - Branches directly to `free_env`.
2. **`Suspended(k)` (State $k > 0$)**:
   - **Child Future Cancellation Cascade**: Checks if the active child future pointer in `env` is non-null (`child != null`), and if so, recursively invokes its specialized future destructor (`{child}_drop`).
   - **State-Specific Drop Set**: Drops all owned places belonging to:
     $$\text{DropSet}(k) = \text{LiveAt}(k) \cap \text{InitializedAt}(k) \cap \text{OwnedAt}(k) \cap \neg\text{MovedAt}(k)$$
   - Branches to `free_env`.
3. **`Completed` (State -1, `FUTURE_STATE_COMPLETED`)**:
   - All owned locals and child futures have already finalized during normal execution.
   - Destructor fast-paths directly to `free_env` without double-dropping.
4. **`Poisoned` (State -2, `FUTURE_STATE_POISONED`)**:
   - Reserved; not routed to `free_env` to prevent silent resource leaks without a formal poison unwinding specification.

### F.3 Ownership Transfer on Future Move
Future values obey standard affine move semantics:
- Moving `fut1` into `fut2` transfers the single destruction obligation to `fut2`.
- The source place `fut1` is deinitialized; borrowck eliminates redundant drops on `fut1`.
- Cancellation or scope exit guarantees exactly one destructor execution for the active future instance.

### F.4 Completion vs Cancellation Separation
- **Normal Completion**: The future state transitions to `FUTURE_STATE_COMPLETED` (-1), all local variables are dropped at their respective lexical scope exits in the resume function, and `env.child_future` is nullified.
- **Cancellation**: Invoked via `{func}_drop` while in state $k \ge 0$. It queries the state, conditionally frees the active child future, cleans up precisely $\text{DropSet}(k)$, and deallocates the environment.

---

## Rule G: Const × Comptime & Compile-Time Evaluation Boundary (P2-A)

### G.1 Core Invariant & Phase Separation
$$\text{Phase}(\text{Runtime}) \cap \text{Phase}(\text{Comptime}) = \emptyset$$
$$\forall v \in \text{ComptimeValue}, \quad \text{escapes\_to\_runtime}(v) \implies \text{is\_representable}(v) \land \neg\text{points\_to\_vm\_memory}(v)$$

1. **Strict Phase Separation**: Compile-time evaluation and runtime execution are strictly disjoint phases. The compiler shall never defer a failed or unsupported compile-time operation to runtime execution.
2. **Zero Fallback**: Any failure during compile-time evaluation (arithmetic overflow, division by zero, resource escape, dependency cycle, step limit exhaustion) must result in a fatal compile-time diagnostic.

### G.2 Unified Execution Engine with Dual Admission Channels
Execution is unified on MVIR via `MvirInterpreter`:
```
const Expr ─────────► [is_const_evaluable check] ──────┐
                                                       │
                                                       ▼
comptime { Expr } ──► [effect containment check] ──► MVIR ──► MvirInterpreter ──► ComptimeValue
```

1. **`const` Expression Admission (`is_const_evaluable`)**:
   - A `const` expression is a restricted, expression-oriented subset: deterministic, pure, non-mutating, non-allocating.
   - Permitted: arithmetic, comparisons, bitwise ops, primitive casts, pure aggregate constructors, scalar field/index projections, `sizeof`, `alignof`, and references to other `const` items.
   - Prohibited in raw `const`: loops, local mutable bindings (`dec rw`), assignments, and heap allocations.
   - **Encapsulated Computation**: If loops or local mutation are required to compute a constant, they must be explicitly encapsulated in an embedded `comptime { ... }` block:
     `const X: T = comptime { ... };` is legal if and only if the block satisfies comptime admission and the resulting value is a representable constant.
2. **`comptime` Block Admission (Effect Containment)**:
   - A `comptime { ... }` block executes within a compiler-owned VM sandbox.
   - Permitted: local mutable state (`dec rw`), imperative control flow (`while`, `for`, `loop`, `if`, `match`, `break`, `continue`), local heap structures, moves, drops, and pure function calls.
   - Effect containment: The block must have zero observable external side-effects. Prohibited: I/O, network access, async `await`, calling `extern fn`, and OS/thread interactions.

### G.3 Const Dependency Graph & Cycle Detection
- Declarations of `const` items (both module-level and local) are evaluated according to a topological dependency order.
- Cyclic dependencies (e.g. `const A = B + 1; const B = A + 1;`) are detected on the item dependency graph before evaluation and rejected with `E_CONST_DEPENDENCY_CYCLE`. Cyclic evaluation must never lead to compiler stack overflow.

### G.4 VM Memory Isolation & Runtime-Observable Resource Escape
$$\text{VM Memory} \not\longrightarrow \text{Runtime}$$
$$\text{VM Value} \longrightarrow \text{Runtime Materialization} \longrightarrow \text{Runtime-Owned Representation}$$

1. **Resource Escape Invariant**: No VM-allocated memory or host-bound resource may escape across the compile-time boundary into runtime code (`E_COMPTIME_RESOURCE_ESCAPE`).
2. **Prohibited Escapes**:
   - VM stack pointers (`Address::Stack`, `&T`)
   - Raw VM heap pointers (`Address::Heap`, `*T`)
   - Closures capturing VM environments
   - Unawaited futures
   - Opaque host handles or runtime-opaque types
3. **Runtime Materialization**:
   - Compile-time values (`ComptimeValue`) crossing into runtime are transformed into runtime-owned representations:
     - Primitives (`Int`, `Float`, `Bool`, `Char`) are materialized as immediate SSA operands.
     - Large aggregates, strings, and constant arrays are materialized into runtime-owned immutable storage (e.g. `.rodata` global sections), producing runtime-valid references. At no point does a runtime pointer reference internal VM memory.

### G.5 Affine Ownership, Moves, and Drops in the VM
- Memory slots in the VM maintain affine states: `Uninitialized`, `Initialized`, `Moved`.
- Reading a moved slot triggers `E_USE_OF_MOVED_VALUE`.
- Scope exits in the VM invoke drop functions for initialized places implementing `Drop`. Moved places are skipped, preventing double-frees at compile-time.
- Unfreed VM heap allocations at `comptime` block termination emit `E_COMPTIME_RESOURCE_LEAK`.

### G.6 Configurable Execution Boundaries
- The compile-time VM enforces an execution step limit (default: 1,000,000 steps) and a call recursion limit (default: 512 frames), configurable via compiler options (`--comptime-steps`, `--comptime-depth`).
- Exceeding these limits is a fatal diagnostic (`E_COMPTIME_STEP_LIMIT`, `E_COMPTIME_RECURSION_LIMIT`), never a runtime fallback.

---

## Rule H: Raw Pointer Safety & Unsafe Boundary (P2-B)

### H.1 Core Invariant: Separation of References and Raw Pointers
$$\forall p \in \text{PointerTypes}, \quad p \in \text{Reference}(\&T, \&\text{rw } T) \iff \text{Tracked}(\text{Lifetime}, \text{Provenance})$$
$$\forall p \in \text{PointerTypes}, \quad p \in \text{RawPointer}(*T, *\text{rw } T) \iff \neg\text{Tracked}(\text{Lifetime}, \text{Provenance}) \land \text{RequiresCapability}(*p, \text{UnsafeContext})$$

1. **References (`&T`, `&rw T`)**: Safe, compiler-tracked lifetimes and provenance. Dereferencing is fully safe.
2. **Raw Pointers (`*T`, `*rw T`)**: Second-class untracked addresses, `Copy` by default, carrying zero lifetime or provenance guarantees.
3. **Capability Boundary**: Dereferencing a raw pointer (`*p`), performing raw pointer arithmetic (`p + n`, `p - q`), and calling an `unsafe fn` (direct or indirect) strictly require an enclosing `unsafe` context (`unsafe { ... }` block or `unsafe fn` body). Any violation emits `E_UNSAFE_DEREF_OUTSIDE_UNSAFE`, `E_POINTER_ARITHMETIC_OUTSIDE_UNSAFE`, or `E_CALL_UNSAFE_FN_OUTSIDE_UNSAFE`.

### H.2 Model B Pointer Passing: Safe Creation, Copying, and Passing
- Creating a raw pointer from a reference (`&x as *T`, `&rw x as *rw T`), assigning/copying raw pointers (`dec q = p;`), and passing raw pointers as arguments or return values across safe function boundaries are **safe operations**.
- The act of holding or transmitting a raw pointer does not risk undefined behavior; only the capability-demanding operations (dereference, arithmetic, and unsafe calls) require `unsafe`.

### H.3 `unsafe` Is an Admission Boundary, NOT a Borrowck Bypass
$$\text{InUnsafeContext} \not\implies \text{BypassBorrowck}$$
1. Entering an `unsafe` block or `unsafe fn` grants admission to raw dereferences, pointer arithmetic, and unsafe function calls.
2. Standard borrowck, move analysis, and affine lifetime invariants remain 100% active inside `unsafe` for all safe places and references:
   - Mutating a safe place while an active shared reference exists inside an `unsafe` block is rejected with `E_BORROW_CONFLICT`.
   - Using a moved value inside an `unsafe` block is rejected with `E_USE_OF_MOVED_VALUE`.

### H.4 Access Mutability vs Object Mutability
- `*T` grants **read-only access capability** through that pointer, rather than asserting that the underlying target allocation is immutable.
- Writing through an immutable pointer (`*p = val`) or taking a mutable reference through it (`&rw (*p)`) is statically forbidden with `E_CANNOT_MUTATE_IMMUTABLE_POINTER`.
- `*rw T` grants **read/write capability** through that pointer.
- Multiple `*rw T` pointers may alias the exact same target allocation and perform interleaved mutations inside `unsafe` without triggering borrowck conflicts, because raw pointers are explicitly untracked.

### H.5 Raw-Derived References & Local Validity Regions
- Re-borrowing through a raw pointer (`&(*p)` or `&rw (*p)`) synthesizes a fresh reference.
- The synthesized reference's loan and lifetime are bound to the local enclosing statement or block validity region.
- Once created, the synthesized reference immediately participates in standard borrowck and must not conflict with other active safe borrows in that validity region.

### H.6 Element-Stride Pointer Arithmetic
- Offset arithmetic (`p + n`, `p - n`) scales by the stride of the pointee type:
  $$\text{OffsetBytes} = n \times \text{sizeof}(T)$$
  Lowered to LLVM `getelementptr inbounds` (GEP).
- Pointer distance subtraction (`p1 - p2`) computes the signed element count as `isize`:
  $$\text{Distance} = \frac{\text{int}(p1) - \text{int}(p2)}{\text{sizeof}(T)}$$
  Lowered via LLVM `ptrtoint` and integer subtraction with exact division, or LLVM `build_ptr_diff`.
- Pointer arithmetic on non-pointer operands or outside `unsafe` is rejected.

### H.7 Deterministic Comptime VM Traps vs Runtime Zero-Cost
1. **Comptime Sandboxed VM**:
   - The MVIR interpreter traps raw pointer violations deterministically at compile-time:
     - Null pointer dereference (`0x0`) emits `E_NULL_POINTER_DEREFERENCE`.
     - Use-after-free or dangling address dereference emits `E_USE_AFTER_FREE`.
     - Out-of-bounds pointer arithmetic or dereference emits `E_OUT_OF_BOUNDS_DEREF`.
2. **Runtime Production**:
   - Compiles to native unadorned hardware loads and stores with zero runtime check overhead, preserving optimal C-ABI foreign interoperability and high performance. Target hardware memory protection faults (e.g. SIGSEGV) trap at the CPU level.

---

## Rule I: Unsize / dyn Trait Coercion Formalization (P3)

### I.1 Core Invariant: Unsized Type Admission & Value-Position Rejection
$$\forall T \in \text{Types}, \quad \text{is\_unsized}(T) \iff T \in \{\text{dyn Trait}, [U]\}$$
$$\forall v \in \text{ValuePositions}, \quad \text{type\_of}(v) = T \land \text{is\_unsized}(T) \implies \text{Error}(\text{E\_UNSIZED\_TYPE\_IN\_VALUE\_POSITION})$$

1. **Unsized Types in Mellis v1**:
   - `dyn Trait` (trait object dynamically sized type) and `[T]` (dynamically sized slice type) are unsized.
   - Mellis v1 does **not** introduce an implicit `T: Sized` trait solver bound. Instead, the type checker directly rejects unsized types in all value positions:
     - Variable declarations (`dec x: dyn Trait`, `dec x: [T]`)
     - Function parameters (`fn f(x: dyn Trait)`)
     - Function return types (`fn f() -> dyn Trait`)
     - Struct field definitions (`struct S { x: dyn Trait }`)
     Any occurrence in these positions emits `E_UNSIZED_TYPE_IN_VALUE_POSITION`.
2. **Permitted Indirect Positions**:
   Unsized types may appear exclusively behind indirection:
   - `&T`, `&rw T` (safe fat references)
   - `*T`, `*rw T` (raw fat pointers)
   - `Box<T>` (heap-allocated unique ownership, subject to representation support).

### I.2 Unified Coercion Engine
$$\text{CoercionKind} \in \{\text{RefMutToShared}, \text{ArrayToSlice}, \text{ConcreteToDyn}\}$$

1. **Architectural Separation**: Subtyping/coercions and type unification remain strictly separated. The coercion engine is unified under `coercion::try_coerce(actual, expected) -> bool`, recording `CoercionKind` in `tables.coercions`.
2. **Explicit MVIR Lowering**: Coercions are explicit instructions in MVIR:
   - `ArrayToSlice`: Lowered to `Instruction::MakeSlice { data_ptr, len }`.
   - `ConcreteToDyn`: Lowered to `Instruction::MakeTraitObject { data_ptr, vtable, trait_sym, concrete_sym }`.
   - `RefMutToShared`: Lowered without runtime transformation (retaining the inner pointer).
3. **Mutability Narrowing**:
   - `&rw [T; N]` coerces to `&rw [T]` and `&[T]`.
   - `&[T; N]` coerces to `&[T]`, but coercing `&[T; N]` to `&rw [T]` is rejected.
   - `&rw T` coerces to `&rw dyn Trait` and `&dyn Trait`.
   - `&T` coerces to `&dyn Trait`, but coercing `&T` to `&rw dyn Trait` is rejected.

### I.3 Fat Pointer Representation
$$\text{Representation}(\&[T]) = \text{Representation}(*[T]) = \{ \text{data}: *T, \, \text{len}: \text{i64} \}$$
$$\text{Representation}(\&\text{dyn Trait}) = \text{Representation}(*\text{dyn Trait}) = \{ \text{data}: *\text{Void}, \, \text{vtable}: *\text{Vtable} \}$$

1. **Slice Fat Pointers**:
   A 2-word aggregate `{ data: ptr, len: i64 }`. Length represents element count (not byte count).
2. **Trait Object Fat Pointers**:
   A 2-word aggregate `{ data: ptr, vtable: ptr }`. `data` points to the concrete payload; `vtable` points to the statically initialized global vtable for that `(Trait, ConcreteType)` pair.

### I.4 Mellis Trait Object ABI v1
$$\text{VtableLayout} = [ \text{drop\_glue}: *\text{Void}, \, \text{size}: \text{usize}, \, \text{align}: \text{usize}, \, \text{method}_0, \, \text{method}_1, \dots ]$$

1. **Slot Allocation**:
   - **Slot 0**: `drop_glue`. Function pointer to the concrete destructor, or `null` if the type does not implement `Drop`.
   - **Slot 1**: `size`. Size of the concrete type in bytes as a pointer-sized integer (`inttoptr`).
   - **Slot 2**: `align`. Alignment of the concrete type in bytes as a pointer-sized integer (`inttoptr`).
   - **Slot $3 + i$**: Function pointer to the $i$-th trait method, indexed in source declaration order.
2. **Destructor Execution & Null Guard**:
   - `Instruction::DropVirt { obj }` extracts `drop_glue = vtable[0]`.
   - Backend codegen emits an explicit null guard (`if drop_glue != null { call drop_glue(data); }`). If `drop_glue == null`, destructor execution is a safe no-op.
3. **Method Dispatch**:
   - `Instruction::CallVirt { obj, method_idx, args }` indexes the vtable at slot `method_idx + 3`, passing `obj.data` as receiver.

### I.5 Object Safety Specification
A trait `Trait` is object-safe (admitting `dyn Trait`) if and only if all of the following hold:
1. **No Generic Methods**: No method in `Trait` declares type parameters (`generic_params.is_empty()`).
2. **Receiver Requirement**: Every method in `Trait` has a receiver (`self`, `&self`, or `&rw self`) as its first parameter.
3. **No Unboxed Self Return**: No method in `Trait` returns unboxed `Self`.
4. **No Self in Non-Receiver Parameters**: No parameter other than the receiver references `Self`.
Violations emit `E_TRAIT_NOT_OBJECT_SAFE`.

### I.6 Frozen v1 Restrictions
1. **Single Trait Only**: `dyn Trait` admits exactly one trait name. Multi-trait composition (`dyn Foo + Bar`) is unsupported in v1.
2. **No Associated Types on dyn**: Traits declaring associated types cannot be made into trait objects (`E_DYN_ASSOCIATED_TYPE_UNSUPPORTED`).
3. **No Supertrait dyn Composition**: Traits declaring supertraits (`trait Child : Parent`) cannot be made into trait objects (`E_DYN_SUPERTRAIT_UNSUPPORTED`).
4. **No Box of Unsized**: `Box<dyn Trait>` and `Box<[T]>` are rejected in v1 with `E_UNSUPPORTED_BOX_UNSIZED`.

### I.7 Monomorphization Rooting of Dynamic Implementations
In the presence of dynamic dispatch, methods called via `CallVirt` cannot be discovered by static call-graph traversal alone.
The monomorphization engine (`MonoCollector`) roots and instantiates all methods from `impl Trait for Concrete` (and corresponding `impl Drop for Concrete`) whenever `CoercionKind::ConcreteToDyn { trait_sym, concrete_sym }` is recorded during semantic analysis.

### I.8 Entrypoint ABI Exception
The compiler-generated program entry point `main` may receive the unsized command-line argument slice `[str]` by value. This is a privileged ABI boundary; it does not generalize unsized value-position semantics to ordinary functions, bindings, structs, or returns.

---

## Rule J: Module Ancestry Visibility & Import Boundary Invariants (M1–M5)

### J.1 Core Invariant: Module Ancestry Visibility Rule
$$\forall \text{Symbol } S, \forall \text{Scope } C, \quad \text{Accessible}(S, C) \iff \text{is\_public}(S) \lor \text{enclosing\_mod}(S) = \text{enclosing\_mod}(C) \lor \text{is\_descendant}(\text{enclosing\_mod}(C), \text{enclosing\_mod}(S))$$

1. **Enclosing Module Boundary**:
   $\text{enclosing\_mod}(\sigma)$ walks up parent scopes until the first scope of kind `ScopeKind::Module` or `ScopeKind::Global` is reached.
2. **Access Invariant**:
   - A `public` (`export`) symbol is accessible everywhere within its provider and exported in cross-provider interfaces.
   - A `private` symbol is accessible:
     - Within the lexical body of its declaring module ($\text{enclosing\_mod}(S) = \text{enclosing\_mod}(C)$).
     - Within any child, grandchild, or descendant module nested inside its declaring module ($\text{is\_descendant}(\text{enclosing\_mod}(C), \text{enclosing\_mod}(S))$).
   - A `private` symbol is **strictly inaccessible** to sibling modules, parent modules, ancestor modules, or foreign providers. Violations emit `Symbol '...' is private and cannot be accessed from this scope`.

### J.2 Rule M1: Policy Disjointness of Local vs External Imports
$$\text{Domain}(\text{import } \text{"pkg"}) \cap \text{Domain}(\text{import } \langle\text{pkg}\rangle) = \emptyset$$

1. **Local Provider Resolution (`import "foo";`)**:
   Searches strictly within user-configured `search_paths` for `foo.ms` or `foo.mlib`. Never resolves against external sysroot components.
2. **External Package Resolution (`import <foo>;`)**:
   Searches strictly within `sysroot.external_dir()` and registered sysroot components. Never falls back to local `search_paths`.
3. **Cross-Contamination Prohibition**:
   No local import may silently resolve a bootstrapped external component (`core`), and no external import may silently resolve a local project file.

### J.3 Rule M2: End-to-End Declaration Container Traversal
`Decl::Module` is a structural declaration container, not merely a resolver namespace alias.
All compiler phases must recursively traverse `Decl::Module` subtrees:
1. **TypeChecker**: Traverses items in Pass 1 (nominal types), Pass 1.8 (trait impls), Pass 2 (function headers), constant initializers, and Pass 3 / `typecheck_item`.
2. **MonoCollector**: Recursively discovers and roots generic instantiations nested inside modules.
3. **MvirGenerator**: Emits MVIR function declarations and basic blocks for items inside modules.
4. **Borrowck**: Audits borrow and move semantics across all functions lowered from nested modules.

### J.4 Rule M3: Multi-Provider Namespace Aggregation
$$\text{Provider}(P_1) \cup \text{Provider}(P_2) \longrightarrow \text{Namespace}(N)$$

Per Mellis Rule 7, providers and module namespaces are orthogonal:
1. Multiple providers (e.g. `alloc.ms` and `core.ms`) may contribute declarations to the same public module namespace (e.g. `module std { ... }`).
2. Provider export extraction (`extract_scope`) recurses into `SymbolKind::Module` without filtering on `provider_id`. Child declarations retain individual provider ownership.
3. Symbol injection merges contributions into existing module scopes without overwriting or discarding declarations from preceding providers.

### J.5 Rule M5: Syntax Containment & Re-Export Diagnostics
1. Mellis v1 does not support re-exporting modules via imports or aliases.
2. `export using` and `export import` are strictly prohibited and must emit clear compile-time diagnostics. Neither construct may silently succeed or silently no-op.

### J.6 Negative Pipeline Invariant
$$\text{Module / Import Error} \implies \text{Diagnostic} \land \text{No Provider Corruption} \land \text{No Partial Namespace} \land \text{No MVIR} \land \text{No Backend} \land \text{No Artifact} \land \text{No Panic}$$
Any failure during module resolution, file parsing, cyclic detection, or visibility check immediately halts compilation before downstream intermediate representations are produced.

---

## Rule K: Surface Syntax & Grammar Authority Invariants

### K.1 Core Invariant: Grammar Authority & Isomorphism
$$\text{AST accepted by parser} \iff \text{Exactly defined by grammar.ebnf} \iff \text{Exactly supported by parser} \iff \text{No accepted syntax silently ignored}$$

The `mellis-grammar` skill and `grammar.ebnf` are the canonical Single Source of Truth for Mellis syntax. No syntax may be accepted by the parser unless formalized in `grammar.ebnf`, and no accepted syntax may be silently ignored or discarded.

### K.2 Struct Field Delimiter Invariant
$$\text{struct\_body} ::= \text{"\{" } (\text{struct\_field } (\text{"," } \text{struct\_field})^* \text{","}? )? \text{ "\}"}$$

Struct fields strictly use comma `,` delimiters (`struct Point { x: f64, y: f64 }`). Semicolons `;` as struct field separators are excluded from the v1.0 standard syntax to ensure unambiguous formatting, macro expansion, and syntax tooling.

### K.3 Postfix Await Operator Invariant
$$\text{postfix\_op} ::= \dots \mid \text{"." } \text{KW\_AWAIT}$$

Mellis v1.0 strictly standardizes on postfix `.await` (`expr.await`). Prefix `await expr` is forbidden to eliminate operator precedence ambiguities (e.g. `await foo().bar()`) and align with the postfix member-access hierarchy (`.field`, `.method()`, `[i]`, `.await`).

### K.4 Dual Loop Invariant (Foreach & C-style)
Mellis supports two complementary loop constructs:
1. **Foreach Loop**: `for ( <pattern> in <expression> ) block_stmt`
2. **C-style Index Loop**: `for ( <for_init>? ; <expression>? ; <expression>? ) block_stmt`
The two loop forms have disjoint syntactic heads and are parsed without ambiguous heuristics.

### K.5 Method Receiver Equivalence Invariant
The canonical method receiver shorthands have strict, bidirectional semantic equivalence with their explicit forms:

$$\&self \equiv self : \&Self$$
$$\&rw \text{ } self \equiv self : \&rw \text{ } Self$$
$$self \equiv self : Self$$

No compiler phase or analysis pass may exhibit semantic divergence between canonical shorthand and explicit forms.

### K.6 Provider vs Module Namespace Orthogonality
$$\text{Provider(Artifact/File)} \ne \text{Namespace(Module)}$$

1. `import "foo";` and `import <foo>;` reference **providers** (artifact or file names without extensions). They do not bring symbols into scope and do not define namespaces.
2. `module foo { ... }` declares an inline **namespace / declaration container** within a provider file. File-level `module foo;` is forbidden.
3. Symbols are accessed via namespace paths (`geometry::Point`), never provider names. Multiple providers may contribute declarations to the same namespace.

### K.7 Namespace Alias Invariant
$$\text{using\_decl} ::= \text{"using" } \text{module\_path } \text{"as" } \text{IDENTIFIER } \text{";"}$$

`using` creates a compilation-local alias to a qualified module namespace path. It does not export (`export using` is a syntax error), does not glob, and does not load providers.
