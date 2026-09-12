# Mellis Compiler Diagnostic System v1.0 — Architecture & Contract Specification

**Status**: FROZEN / CANONICAL SPECIFICATION  
**Scope**: All Compiler Phases (Parser, Resolver, TypeChecker, Borrowck, Async/Comptime, Mono, Driver/Backend)  
**Authority**: Single Source of Truth (SSOT) for Diagnostic Codes, Structures, and Invariants

---

## 1. Overview & Architectural Principles

The Mellis Compiler diagnostic subsystem is a first-class compiler boundary responsible for reporting all syntactic, semantic, borrow-checking, capability, monomorphization, and backend errors.

Ad-hoc error strings and unformatted diagnostic messages are strictly forbidden. Every compiler diagnostic is a structured, strongly-typed machine-readable object governed by the invariants defined below.

```text
┌──────────────────────────────────────────────────────────────────────────────────┐
│                                   Diagnostic                                     │
├───────────────┬───────────────────┬──────────────────┬───────────────────────────┤
│ level: Error  │ code: E3001       │ message: "..."   │ suggestions: Vec<...>     │
├───────────────┴───────────────────┴──────────────────┴───────────────────────────┤
│ primary: Some(Label { span: access_span, message: "value used here after move" }) │
│ related: [Label { span: move_span, message: "value previously moved here" }]      │
│ notes: ["values implementing Drop cannot be accessed after move"]                │
└──────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Diagnostic Invariants Contract (DIAG-1 to DIAG-10)

All diagnostics emitted by the Mellis compiler must strictly uphold the following 10 invariants:

- **DIAG-1 (Code Mandatory)**: Every user-visible compiler error MUST have a strongly-typed `DiagnosticCode`. Ad-hoc diagnostics without a code are prohibited.
- **DIAG-2 (Primary Span Mandatory for Source Errors)**: Every source-originated diagnostic (`< 6000`) MUST have a primary source `Span` attached at construction time via `Diagnostic::source_error` or `source_warning`. `Diagnostic::global_error` is forbidden for phases 0–5.
- **DIAG-3 (Structural Related Spans)**: Secondary/related source locations (e.g., previous definition, active conflicting loan, move origin, trait declaration) MUST be represented structurally in `related: Vec<Label>`, NEVER formatted into message text.
- **DIAG-4 (Message Independence)**: Diagnostic identity, categorizations, and test assertions MUST NOT depend on rendered message text. Tests assert on `d.code == DiagnosticCode::...` and label spans.
- **DIAG-5 (Poison Containment)**: Semantic and type-level errors MUST produce `SemanticType::Error` to suppress cascading false-positive diagnostics in downstream expressions.
- **DIAG-6 (Strict Determinism)**: Diagnostic emission order and deduping MUST be strictly deterministic across compilation runs.
- **DIAG-7 (Regression Coverage)**: Every `DiagnosticCode` in the registry MUST be covered by automated test assertions verifying code identity and span attachment.
- **DIAG-8 (Phase Ownership)**: Every diagnostic code belongs to exactly one owning compiler phase. No phase may emit diagnostic codes belonging to another phase.
- **DIAG-9 (Event Origin Traceability)**: In borrow checking and move analysis, diagnostics must record both the invalid event (primary span) and the antecedent cause (related span, e.g. move origin or active loan).
- **DIAG-10 (Stable Code Uniqueness & Parity)**: Numerical discriminants and code strings are immutable and 1-to-1 unique. Automated tests verify exact parity between this specification and `DiagnosticCode::ALL`.

---

## 3. Data Structures & Constructors

### 3.1 Severity Levels

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Error,
    Warning,
}
```
*Note: DiagnosticLevel does NOT have a `Note` variant. Notes are structural elements (`notes: Vec<String>`) within a Diagnostic.*

### 3.2 Label and Suggestion

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Suggestion {
    pub span: Span,
    pub replacement: String,
    pub message: String,
}
```

### 3.3 Constructors

```rust
impl Diagnostic {
    /// Constructs a source-originated Error with a mandatory primary source span.
    pub fn source_error(code: DiagnosticCode, span: Span, message: impl Into<String>) -> Self;

    /// Constructs a source-originated Warning with a mandatory primary source span.
    pub fn source_warning(code: DiagnosticCode, span: Span, message: impl Into<String>) -> Self;

    /// Constructs a global infrastructure/backend Error (only valid for code >= 6000).
    pub fn global_error(code: DiagnosticCode, message: impl Into<String>) -> Self;

    /// Attaches a related secondary source location.
    pub fn with_related(mut self, span: Span, message: impl Into<String>) -> Self;

    /// Attaches a descriptive note.
    pub fn with_note(mut self, message: impl Into<String>) -> Self;

    /// Attaches a code suggestion / fix.
    pub fn with_suggestion(mut self, span: Span, replacement: impl Into<String>, message: impl Into<String>) -> Self;
}
```

---

## 4. Phase Partitioning & Error Code Scheme

The error code namespace is statically partitioned into non-overlapping ranges by compiler phase:

| Code Range | Phase | Description |
|:---|:---|:---|
| `E0001`–`E0999` | **Parser** | Lexing, token consumption, syntax errors, attribute grammar |
| `E1000`–`E1999` | **Resolver** | Symbol resolution, visibility, module providers, cyclic imports |
| `E2000`–`E2999` | **TypeChecker** | Typing, trait resolution, coherence, associated types, lifetimes |
| `E3000`–`E3999` | **Borrowck** | Move semantics, borrow conflicts, missing return, escapes |
| `E4000`–`E4999` | **Async/Comptime**| Async across await, comptime step limits, evaluation panics |
| `E5000`–`E5999` | **Mono** | Monomorphization barriers and infinite generic recursion |
| `E6000`–`E6999` | **Infrastructure**| Driver, MLib corruption, backend codegen, object emit, linker |

---

## 5. Canonical Diagnostic Code Registry

The following table lists every stable `DiagnosticCode` recognized by Mellis v1.0.

| Code | Value | Name | Owning Phase | Description / Invariant |
|:---|:---|:---|:---|:---|
| `E0001` | 1 | ExpectedToken | Parser | Expected specific token kind (e.g. delimiter, identifier) |
| `E0002` | 2 | UnexpectedToken | Parser | Unexpected token found in grammar position |
| `E0003` | 3 | InvalidSyntax | Parser | General syntax violation or malformed statement |
| `E0004` | 4 | UnclosedDelimiter | Parser | Unclosed parentheses, bracket, or brace |
| `E0005` | 5 | InvalidAnnotation | Parser | Malformed or unknown compiler attribute (`#[...]`) |
| `E1001` | 1001 | UnresolvedSymbol | Resolver | Identifier not found in local or imported scope |
| `E1002` | 1002 | DuplicateDefinition | Resolver | Symbol declared multiple times in same scope |
| `E1003` | 1003 | PrivateSymbolAccess | Resolver | Access to non-exported private item across boundary |
| `E1004` | 1004 | UnresolvedModuleProvider | Resolver | Module provider not found in search paths or registry |
| `E1005` | 1005 | CyclicModuleDependency | Resolver | Circular import cycle detected between providers |
| `E1006` | 1006 | InvalidVisibility | Resolver | Visibility modifier used on invalid declaration |
| `E1007` | 1007 | WildcardImportProhibited | Resolver | Wildcard glob import prohibited by Mellis contract |
| `E2001` | 2001 | TypeMismatch | TypeChecker | Incompatible types in assignment, call, or binary op |
| `E2002` | 2002 | CannotDereference | TypeChecker | Dereference operator applied to non-pointer/reference |
| `E2003` | 2003 | CannotIndex | TypeChecker | Index operator applied to non-indexable type |
| `E2004` | 2004 | CopyDropConflict | TypeChecker | Type cannot satisfy/derive Copy if field implements Drop |
| `E2005` | 2005 | ConflictingTraitImpl | TypeChecker | Duplicate or conflicting trait implementation |
| `E2006` | 2006 | OrphanImpl | TypeChecker | Trait impl violates orphan rule (neither local) |
| `E2007` | 2007 | UnresolvedTraitImpl | TypeChecker | Required trait implementation not found for type |
| `E2008` | 2008 | AssociatedTypeCycle | TypeChecker | Recursion cycle detected while normalizing associated type |
| `E2009` | 2009 | InfiniteSizeRecursiveType | TypeChecker | Struct/enum has infinite size due to unboxed cycle |
| `E2010` | 2010 | NonObjectSafeTrait | TypeChecker | Trait cannot be made into `dyn Trait` object |
| `E2011` | 2011 | InvalidLvalue | TypeChecker | Left-hand side of assignment is not an assignable place |
| `E2012` | 2012 | InvalidUnaryOp | TypeChecker | Unary operator cannot be applied to operand type |
| `E2013` | 2013 | InvalidBinaryOp | TypeChecker | Binary operator cannot be applied to operand types |
| `E2014` | 2014 | DuplicateField | TypeChecker | Field name duplicated in struct definition |
| `E2015` | 2015 | DuplicateVariant | TypeChecker | Variant name duplicated in enum definition |
| `E2016` | 2016 | LifetimeConstraintViolation | TypeChecker | Lifetime outlives or provenance constraint not satisfied |
| `E2017` | 2017 | MissingField | TypeChecker | Struct pattern or literal missing required field |
| `E2018` | 2018 | NoAssociatedType | TypeChecker | Trait implementation does not define requested associated type |
| `E2019` | 2019 | AmbiguousAssociatedType | TypeChecker | Multiple candidate traits define conflicting associated type |
| `E2020` | 2020 | CannotCallNonFunction | TypeChecker | Value is not callable as a function or method |
| `E2021` | 2021 | TraitBoundNotSatisfied | TypeChecker | Generic parameter trait bound is not satisfied by argument |
| `E2022` | 2022 | NonExhaustivePattern | TypeChecker | Match expression does not cover all variants or values |
| `E2023` | 2023 | CannotMutateImmutable | TypeChecker | Mutation attempted through immutable variable or reference |
| `E2024` | 2024 | InvalidMainSignature | TypeChecker | `main` function does not match allowed ABI signatures |
| `E2025` | 2025 | UnsafeOperationOutsideUnsafe | TypeChecker | Unsafe function call or raw deref outside `unsafe` block |
| `E2026` | 2026 | InvalidCast | TypeChecker | Invalid type cast (e.g. fat to thin pointer) |
| `E2027` | 2027 | AwaitOutsideAsync | TypeChecker | `await` expression used outside `async` function |
| `E2028` | 2028 | InvalidTryOperator | TypeChecker | `?` operator applied to type not implementing `Try` |
| `E2029` | 2029 | LoopControlOutsideLoop | TypeChecker | `break` or `continue` used outside loop body |
| `E2030` | 2030 | NonFfiSafeType | TypeChecker | Extern function signature contains non-FFI-safe types |
| `E3001` | 3001 | UseAfterMove | Borrowck | Value accessed or borrowed after having been moved |
| `E3002` | 3002 | PartialMoveUnderDrop | Borrowck | Partial move attempted out of a type implementing Drop |
| `E3003` | 3003 | BorrowConflict | Borrowck | Aliasing violation (multiple mutable or mut/immut loans) |
| `E3004` | 3004 | MissingReturnValue | Borrowck | Non-void function reaches end without returning value |
| `E3005` | 3005 | LocalBorrowEscape | Borrowck | Reference to local variable escapes function scope |
| `E4001` | 4001 | AsyncBorrowAcrossAwait | Async/Comptime | Borrow active across `.await` point in async function |
| `E4002` | 4002 | ComptimeStepLimitExceeded | Async/Comptime | Comptime execution exceeded instruction step limit |
| `E4003` | 4003 | ComptimeUnserializableEscape| Async/Comptime | Comptime value escaping to runtime cannot be serialized |
| `E4004` | 4004 | ComptimePanic | Async/Comptime | Explicit panic or assertion failure during comptime |
| `E4005` | 4005 | ComptimeEvaluationFailed | Async/Comptime | General evaluation failure during comptime block/const |
| `E5001` | 5001 | MonomorphizationBarrier | Mono | Unresolved generic projection blocked monomorphization |
| `E5002` | 5002 | InfiniteMonomorphizationRecursion | Mono | Recursive generic instantiation depth limit reached |
| `E6001` | 6001 | BackendInvariantViolation | Infrastructure | Internal compiler or backend code generation invariant failed |
| `E6002` | 6002 | ObjectEmissionFailure | Infrastructure | LLVM object file emission failure |
| `E6003` | 6003 | LinkerFailure | Infrastructure | System linker invocation failed |

---

## 6. Rendering Specification

The terminal renderer outputs multi-label diagnostic reports adhering to this standard layout:

```text
error[E3001]: UseAfterMove: Use of moved value `x`
  --> src/main.ms:12:9
   |
12 |     foo(x);
   |         ^ value used here after move
   |
  ::: src/main.ms:8:9
   |
 8 |     bar(x);
   |         - value previously moved here
   |
note: values implementing Drop cannot be accessed after move
suggestion: consider borrowing `&x` instead
```

1. **Header**: `<level>[<code>]: <Name>: <message>`
2. **Primary Location**: `  --> <file>:<line>:<col>`
3. **Primary Source Slice**: Underlined with `^^^^^` and primary label text.
4. **Related Context Slices**: `  ::: <file>:<line>:<col>` with `-----` and related message text.
5. **Notes**: `note: <text>`
6. **Suggestions**: `suggestion: <text>`
