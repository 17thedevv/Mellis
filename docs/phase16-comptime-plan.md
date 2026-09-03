# Phase 16: Comptime — Compile-Time Execution System
## Architecture, Formal Semantic Model & Implementation Plan

> **Document Version**: 1.0.0  
> **Status**: APPROVED ARCHITECTURE DESIGN  
> **Target**: Mellis Compiler v1  

---

## 1. Core Philosophy & Architectural Overview

Mellis strictly distinguishes three compile-time mechanisms to maintain a clean, orthogonal, and debuggable compilation model:

```
┌────────────────────────────────────────────────────────────────────────┐
│                          MELLIS COMPILE-TIME TIERS                     │
├──────────────────┬──────────────────────────┬──────────────────────────┤
│ Mechanism        │ Signature                │ Primary Responsibility   │
├──────────────────┼──────────────────────────┼──────────────────────────┤
│ Macros           │ `TokenTree -> TokenTree` │ Syntactic metaprogramming│
│ Comptime         │ `Code -> ComptimeValue`  │ Semantic computation     │
│ Introspection    │ `Type -> SemanticMeta`   │ Type/Layout inspection   │
└──────────────────┴──────────────────────────┴──────────────────────────┘
```

### Core Invariants:
1. **Comptime produces values, never AST or syntax**: `comptime` executes Mellis code within a sandbox at compile time and yields a strongly-typed `ComptimeValue`. It cannot directly synthesize AST nodes, inject type declarations, or alter grammar.
2. **Macros produce syntax, never values**: Declarative macros (`macro!`) operate purely at the syntactic level before type checking.
3. **Unified Language Semantics**: `comptime` does not introduce a secondary, crippled mini-language. Normal Mellis functions, control flow (`if/else`, `match`, `while`, `for`), ownership, borrowing, pattern matching, generics, and the `?` operator execute under identical semantic rules at compile time.
4. **Deterministic & Side-Effect Isolated**: Compile-time execution is hermetic, side-effect free, deterministic across host environments, and strictly bounded by step/recursion/memory budgets.

---

## 2. Compiler Pipeline Placement

Comptime execution sits at the interface between **Semantic Analysis (TypeChecking)** and **Intermediate Representation (MVIR)**:

```
Source Code (.ms)
   │
   ▼
Lexer & Parser ──────────► AST Arena
   │
   ▼
Module Importer ─────────► Provider / Artifact Resolution
   │
   ▼
Attribute / Derive ──────► #[derive(...)] AST Generation
   │
   ▼
Macro Engine ────────────► Macro Expansion (AST -> AST)
   │
   ▼
Name Resolver ───────────► Scopes, Symbol Table
   │
   ▼
┌────────────────────────────────────────────────────────┐
│               TypeChecker & Comptime Engine            │
│                                                        │
│  1. Early Comptime: Array sizes, Type-level constants  │
│  2. Full Comptime: const items, comptime { ... }       │
│  3. Reflection: sizeof, alignof, type_info             │
└──────────────────────────┬─────────────────────────────┘
                           │
                           ▼
               Monomorphization Collector
                           │
                           ▼
                     MVIR Generator
            (Injects Comptime Constants as Literals)
                           │
                           ▼
             BorrowChecker / NLL / Move / Drop
                           │
                           ▼
                     MVIR Optimizer
                           │
                           ▼
                    LLVM Backend / MLib
```

### Evaluation Tiers:
- **Tier 1 (Type-Level Constant Evaluation)**: Evaluates constant expressions required to determine types during type checking (e.g., array bounds `[u8; SIZE]`, generic const arguments).
- **Tier 2 (Statement/Block Comptime Evaluation)**: Evaluates `const X = expr;` declarations and standalone `comptime { ... }` expressions once the inner expressions are typed.

---

## 3. Answers to the 13 Architectural Questions

### Q1: Where does comptime evaluation happen in the compiler pipeline?
**Answer**: Comptime evaluation runs during **Semantic Analysis (TypeChecking)** in `mellis-semantic`. Tier 1 runs on demand during type resolution (e.g. evaluating array length expressions); Tier 2 runs immediately after typechecking a `const` declaration or `comptime` block.

### Q2: Should evaluation happen on typed AST, semantic IR, or another representation?
**Answer**: Evaluation happens on the **Typed AST** with direct access to `SemanticContext` (symbol table, type table, struct/enum layouts). Evaluating typed AST avoids the overhead of converting to full MVIR for simple constants, while ensuring that all symbol IDs, types, and method resolutions are already fixed and validated.

### Q3: How are normal function calls executed during comptime?
**Answer**: When an ordinary Mellis function `fn f(x: T) -> R` is called within a comptime context, the evaluator retrieves the function's AST body from `AstArena` using its resolved `SymbolId`. The evaluator creates a new local frame in the `ComptimeContext`, binds arguments, and interprets the body. No separate `comptime fn` keyword is required; any function is comptime-callable if it satisfies the purity/effect constraints.

### Q4: How are effects/capabilities represented and checked?
**Answer**: Mellis uses a capability-based effect model extending `mellis-borrowck::effect`. Comptime is executed with a `ComptimeCap::Pure` capability token. Any operation requiring external side effects (`IO`, `ExternCall`, raw volatile pointer access, modifying runtime static memory) is rejected with a compiler diagnostic before or during execution.

### Q5: How is temporary comptime memory represented and reclaimed?
**Answer**: Memory allocated during comptime (tuples, arrays, structs, boxed items) is owned by an evaluator-managed `ComptimeArena`. Allocations are tracked with local reference lifetimes and cleaned up using Mellis's normal `Drop` rules upon leaving scope. When comptime evaluation finishes, the arena is either serialized into constant data or discarded.

### Q6: How are pointers/references treated inside the compile-time domain?
**Answer**: References (`&T`, `&rw T`) within comptime are modeled as fat place pointers `PlaceRef { alloc_id, offset, ty }` inside the evaluator arena. They are subject to compile-time borrow rules: immutable aliasing is allowed, mutable aliasing is exclusive, and dangling pointers to expired stack frames are prevented. Raw pointers (`*T`) cannot be dereferenced unless they point to valid compile-time allocated objects.

### Q7: What values are allowed to escape from comptime into runtime?
**Answer**: Only **pure, serializable values** may escape from comptime into runtime code (integers, floats, booleans, chars, strings, pure value structs, enums, fixed-size arrays, and tuples). Unresolved pointers, references to comptime stack locations, open handles, or closure environment pointers cannot escape and trigger an `UnserializableComptimeValue` diagnostic.

### Q8: How are generic instances evaluated?
**Answer**: Generic functions called in comptime are evaluated under concrete `Substitution` mappings provided by the caller's call-site. Types within the function body are resolved against the call-site `Substitution` before evaluation.

### Q9: How are recursion and loops bounded?
**Answer**: The evaluator maintains strict execution budgets:
- **Step Budget**: Max $1{,}000{,}000$ operations (configurable).
- **Call Stack Depth**: Max $512$ nested frames.
- **Memory Budget**: Max $64\text{ MB}$ total arena allocation.
Exceeding any budget emits a clean `ComptimeBudgetExhausted` diagnostic with full call stack trace, preventing compiler hangs.

### Q10: How are comptime diagnostics mapped to source spans?
**Answer**: The evaluator maintains a `ComptimeCallStack` containing `(SymbolId, Span)`. When an error occurs (e.g. division by zero, assert failure, out-of-bounds index), a primary diagnostic is attached to the exact `Span` of the failing expression, accompanied by note diagnostics displaying the comptime execution backtrace.

### Q11: How does constant propagation feed type checking (e.g. array lengths)?
**Answer**: When `TypeChecker` encounters `Type::Array { element_type, size }`, it invokes `ComptimeEvaluator::eval_expr(size)`. If evaluation succeeds with `ComptimeValue::Int(n, _)`, `n` is validated ($n \ge 0$) and the type is interned as `SemanticType::Array(elem_ty, n as u64)`. If evaluation fails, an error diagnostic is emitted and `SemanticType::Error` is returned.

### Q12: Which parts, if any, must be represented in MVIR?
**Answer**: The *result* of comptime evaluation is materialized in MVIR as pure literal constants (`Instruction::Assign(Operand::Number(...))`, string literals, or composite aggregate initializers). The internal comptime execution loop itself never enters runtime MVIR.

### Q13: Which parts are purely semantic and never reach runtime codegen?
**Answer**: `comptime { ... }` blocks, `const` declarations (unless explicitly exported as public constants), static assertions, reflection queries (`sizeof`, `alignof`, `type_info`), and unreferenced comptime helper functions are purely semantic and leave zero footprint in the final executable.

---

## 4. Comptime Value Model (`ComptimeValue`)

`ComptimeValue` faithfully represents all Mellis data types with exact bit-width and sign guarantees:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum IntWidth {
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    USize, ISize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FloatWidth {
    F32, F64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeValue {
    // Primitives
    Unit,
    Bool(bool),
    Int { val: i128, width: IntWidth },
    Float { val: f64, width: FloatWidth },
    Char(char),
    Str(String),
    
    // Compounds
    Tuple(Vec<ComptimeValue>),
    Array { elements: Vec<ComptimeValue>, elem_ty: SemanticTypeId },
    Struct { symbol: SymbolId, fields: Vec<(String, ComptimeValue)> },
    Enum { symbol: SymbolId, variant_index: u32, payload: Vec<ComptimeValue> },
    
    // References (intra-comptime only)
    Ref { alloc_id: u32, offset: usize, is_mutable: bool },
    
    // Type/Metadata representation
    Type(SemanticTypeId),
}
```

---

## 5. Effect & Capability Model (Purity Boundary)

Every expression evaluated in comptime is checked against the `ComptimeCapability`:

```
┌─────────────────────────────────────────────────────────────┐
│                     COMPTIME CAPABILITY                     │
├────────────────────────┬────────────────────────────────────┤
│ Allowed Capabilities   │ Denied / Forbidden Effects         │
├────────────────────────┼────────────────────────────────────┤
│ Pure Arithmetic/Logic  │ File I/O (read/write)              │
│ Local Control Flow     │ Network sockets                    │
│ Local Arena Allocation │ OS Process / Environment access    │
│ Pure Function Calls    │ Runtime Global/Static mutation     │
│ Read-only Reflection   │ Extern C / FFI calls               │
│ Constant Pattern Match │ Uninitialized memory reads         │
│ Local Drop Execution   │ Host-dependent Pointer Introspection│
└────────────────────────┴────────────────────────────────────┘
```

### Call-Site Validation:
1. When calling `f(args)` in comptime:
   - If `f` has `is_extern == true`, emit `ComptimeExternCallProhibited`.
   - If `f` has body `Some(stmt_id)`, evaluate body under current budget.
   - If `f` uses inline assembly or intrinsics not supported in comptime, emit `ComptimeUnsupportedOperation`.

---

## 6. Compile-Time Memory & Ownership Model

### Evaluator-Owned Arena (`ComptimeMemory`):
```rust
pub struct ComptimeAllocation {
    pub bytes: Vec<u8>,
    pub ty: SemanticTypeId,
    pub is_mutable: bool,
}

pub struct ComptimeMemory {
    allocations: Vec<ComptimeAllocation>,
}
```

### Comptime Move & Drop Semantics:
- Variables declared inside comptime blocks (`dec x = ...`) participate in normal Move Analysis.
- When an object goes out of scope in comptime, the evaluator checks if `ctx.needs_drop(ty)` is true. If so, it executes the type's `drop` method in compile-time sandboxed mode before reclaiming its arena allocation.

---

## 7. Semantic Reflection Model

Mellis exposes compile-time introspection purely through **value-based reflection queries**:

```rust
// Language-level builtins accessible in comptime:
sizeof<T>() -> usize
alignof<T>() -> usize
typeof(expr) -> type
type_name<T>() -> str
field_count<T>() -> usize
field_name<T>(index: usize) -> str
variant_count<T>() -> usize
variant_name<T>(index: usize) -> str
has_trait<T, Trait>() -> bool
```

These functions are recognized as compiler intrinsics in `ComptimeEvaluator` and return concrete `ComptimeValue` records. They **never** output syntax or manipulate the AST.

---

## 8. Detailed Implementation Decomposition

```
Phase 16: Comptime Execution System
├── 16A: Grammar, Lexer & AST Nodes
│   ├── `const` declaration syntax in parser
│   └── `comptime { ... }` block expression syntax
├── 16B: `ComptimeValue` & Evaluator State Architecture
│   ├── Data models, width-aware arithmetic, comparisons
│   └── `ComptimeContext`, call stack frames, budget tracking
├── 16C: Core AST-Tree Evaluator Engine
│   ├── Literals, Binary/Unary operators, Block statements
│   ├── Variable bindings (`dec`, `dec rw`), assignments
│   └── Control flow (`if/else`, `while`, `for`, `match`)
├── 16D: Function Call & Generic Instantiation in Comptime
│   ├── Call stack frame creation and argument passing
│   └── Generic function evaluation with `Substitution`
├── 16E: Effect Checker & Purity Enforcement
│   ├── Forbid I/O, FFI, runtime globals
│   └── Determinism guarantee
├── 16F: Comptime Ownership, Borrowing & Drop Execution
│   ├── Intra-comptime borrow tracker
│   └── Comptime `drop` method invocation
├── 16G: Semantic Reflection System
│   ├── `sizeof<T>`, `alignof<T>`, `type_name<T>`
│   └── Struct field & enum variant introspection queries
├── 16H: Const Propagation & Type-Level Integration
│   ├── Symbol table `const` registration
│   └── Array length evaluation `[T; comptime_len]`
├── 16I: Diagnostics, Error Spans & Budget Limits
│   ├── Step budget, recursion depth, memory limit guards
│   └── Spanned error reporting with comptime call stack notes
├── 16J: Macro & Comptime Interoperability Verification
│   ├── Macros expanding into `comptime { ... }`
│   └── Ensuring strict separation of code-gen vs value-eval
└── 16K: Comprehensive Test Matrix & Regression Freeze
    ├── Unit test suite in `mellis-semantic`
    └── End-to-end UI tests in `mellis-driver/tests/ui/`
```

---

## 9. File & Crate Impact Map

| Crate | Target Files | Nature of Change |
|---|---|---|
| `mellis-ast` | `decl.rs`, `expr.rs` | Add `Decl::Const`, `Expr::Comptime` AST variants |
| `mellis-parser` | `parser.rs`, `grammar.md` | Parse `const X: T = expr;` and `comptime { ... }` |
| `mellis-semantic` | `comptime/mod.rs` (NEW) | Main compile-time execution engine |
| `mellis-semantic` | `comptime/value.rs` (NEW) | `ComptimeValue`, typed arithmetic & conversions |
| `mellis-semantic` | `comptime/interp.rs` (NEW) | AST evaluator, control flow, function interpreter |
| `mellis-semantic` | `comptime/reflect.rs` (NEW) | Reflection intrinsics (`sizeof`, `type_info`, etc.) |
| `mellis-semantic` | `typechecker.rs` | Array bound evaluation, `const` symbol registration |
| `mellis-semantic` | `resolver.rs` | Resolving `const` symbols in scopes and modules |
| `mellis-mvir` | `generator.rs` | Emit folded `ComptimeValue` literals into MVIR |
| `mellis-driver` | `tests/ui/comptime_*.ms` | Exhaustive positive and negative test suite |

---

## 10. Test Matrix & Completion Criteria

### Positive Test Cases (`crates/mellis-driver/tests/ui/`)
1. `comptime_basic.ms`: Arithmetic, logic, bitwise ops, width checks.
2. `comptime_control_flow.ms`: Loops, `match`, `if/else`, early returns.
3. `comptime_functions.ms`: Calling pure user functions and generic functions at compile time.
4. `comptime_const.ms`: `const` variable bindings and module-level constants.
5. `comptime_array_len.ms`: Array types sized by comptime expressions (`[i32; SIZE]`).
6. `comptime_struct_enum.ms`: Constructing and destructuring structs/enums at compile time.
7. `comptime_reflection.ms`: `sizeof<T>`, `alignof<T>`, and type inspection.
8. `comptime_try_operator.ms`: `Result`/`Option` with `?` inside comptime blocks.
9. `comptime_drop.ms`: Verifying destructors run for local comptime allocations.

### Negative Test Cases (`crates/mellis-driver/tests/ui/`)
1. `comptime_div_by_zero.ms`: Compile-time division by zero diagnosed cleanly.
2. `comptime_overflow.ms`: Integer overflow during compile-time arithmetic diagnosed.
3. `comptime_infinite_loop.ms`: Exceeding step/recursion budget emits `ComptimeBudgetExhausted`.
4. `comptime_forbidden_io.ms`: Attempting I/O or FFI call inside comptime rejected.
5. `comptime_escape_ref.ms`: Attempting to leak a comptime stack reference to runtime rejected.

---

## 11. Known Risks & Mitigations

| Risk | Impact | Mitigation Strategy |
|---|---|---|
| Infinite loops in comptime | Compiler hangs indefinitely | Hard step budget ($1{,}000{,}000$ steps) & recursion depth limit ($512$) |
| Host endianness/pointer size bias | Non-deterministic cross-compilation | Emulate target integer widths and target-layout `sizeof`/`alignof` explicitly |
| Recursive const dependencies (`const A = B; const B = A;`) | Cyclic dependency deadlock | Cycle detection in `TypeChecker::eval_const` using a visiting stack |
| Confusing macro expansion with comptime | Architectural degradation | Strict phase boundary: Macros run first (AST -> AST); Comptime runs in TypeChecker (AST -> Value) |
