# Mellis Roadmap: LangItem Candidate Catalog & Architectural Checklist

## Purpose

Following the freezing of the **Mellis Core Language Contract v1.0**, this document establishes the definitive classification framework and catalog of candidates across the Mellis language roadmap.

By codifying this framework, future semantic features (such as `Copy`, `Clone`, closures, `for..in` iteration, `async/await`, operator overloading, and comptime reflection) will adhere to a uniform architectural protocol rather than inventing ad-hoc designs.

---

## 1. The 5 Semantic Archetypes

Every construct, type, trait, and function under language design must be assigned to exactly one of the following five archetypes:

```
                               ┌─────────────────────────────┐
                               │     Language Construct      │
                               └──────────────┬──────────────┘
                                              │
         ┌───────────────────┬────────────────┼───────────────────┬───────────────────┐
         ▼                   ▼                ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌───────────┐ ┌───────────────────────┐ ┌───────────┐
│Compiler Contract│ │  Compiler Hook  │ │ Intrinsic │ │ User-defined Protocol │ │ Core API  │
└─────────────────┘ └─────────────────┘ └───────────┘ └───────────────────────┘ └───────────┘
```

| Archetype | Definition | Compiler Mechanism | Example |
| :--- | :--- | :--- | :--- |
| **1. Compiler Contract** | A Trait or Type representing a fundamental language invariant or protocol required for desugaring or typechecking. | `#[lang("...")]` on Trait/Type → `LangItem` | `Drop`, `Try`, `Copy`, `Future`, `FnOnce` |
| **2. Compiler Hook** | A specific method, associated item, or enum variant inside a Compiler Contract that the compiler invokes or matches against during lowering. | `#[lang("...")]` on Method/Variant → `LangItem` | `DropFn`, `TryBranch`, `ControlFlowContinue`, `FuturePoll` |
| **3. Intrinsic** | A primitive compiler/backend built-in operation that cannot be expressed in normal Mellis source code. | `IntrinsicKind` dispatch (via compiler-owned ID) | `size_of`, `align_of`, `transmute`, `type_info` |
| **4. User-defined Protocol**| A trait intended for library polymorphism and standard idioms, where the compiler performs **zero** implicit lowering or desugaring. | Standard trait declaration (no compiler annotations) | `Display`, `Debug`, `Default`, `Hash` |
| **5. Ordinary Core API** | Standard library types, structs, enums, helper methods, and protocol implementations built purely in user-space. | Standard declaration (no compiler annotations) | `Result`, `Infallible`, `Option`, `Vec`, `String` |

---

## 2. Roadmap Candidate Catalog

Below is the structured classification of all planned language and runtime capabilities.

### 2.1 Ownership, Destruction & Copy Semantics
*   **`Copy`** → **Compiler Contract** (`#[lang("copy")]`, Trait)
    *   *Role*: Marker trait indicating that assignments and moves perform a bitwise copy rather than invalidating the source.
    *   *Policy*: Checked by Borrowck/Typechecker during move validation.
*   **`Drop`** → **Compiler Contract** (`#[lang("drop")]`, Trait) — *[Frozen in v1.0]*
*   **`DropFn`** → **Compiler Hook** (`#[lang("drop_fn")]`, Method) — *[Frozen in v1.0]*
*   **`Clone`** → **User-defined Protocol** (`Clone`, Trait)
    *   *Role*: Explicit duplicate creation (`x.clone()`).
    *   *Compiler knowledge*: None. The compiler does not implicitly insert `clone()`. (If `#[derive(Clone)]` is implemented, macro expansion generates normal calls).
*   **`CloneFn`** → **User-defined Protocol Hook** (`clone`, Method)

### 2.2 Error Handling & Control Flow
*   **`Try`** → **Compiler Contract** (`#[lang("try")]`, Trait) — *[Frozen in v1.0]*
*   **`TryBranch`** → **Compiler Hook** (`#[lang("try_branch")]`, Method) — *[Frozen in v1.0]*
*   **`TryFromOutput`** → **Compiler Hook** (`#[lang("try_from_output")]`, Method) — *[Frozen in v1.0]*
*   **`FromResidual`** → **Compiler Contract** (`#[lang("from_residual")]`, Trait) — *[Frozen in v1.0]*
*   **`FromResidualFn`** → **Compiler Hook** (`#[lang("from_residual_fn")]`, Method) — *[Frozen in v1.0]*
*   **`ControlFlow`** → **Compiler Contract** (`#[lang("control_flow")]`, Enum) — *[Frozen in v1.0]*
*   **`ControlFlowContinue` / `ControlFlowBreak`** → **Compiler Hooks** (`#[lang("continue")]`, `#[lang("break")]`, EnumVariants) — *[Frozen in v1.0]*
*   **`Result<T, E>` / `Infallible` / `Option<T>`** → **Ordinary Core APIs**
    *   *Role*: Standard error and optionality types that implement `Try` and `FromResidual`.

### 2.3 Iteration & `for .. in` Loops
*   **`IntoIterator`** → **Compiler Contract** (`#[lang("into_iterator")]`, Trait)
    *   *Role*: Identifies types that can be converted into an iterator at the start of a `for x in iter` loop.
*   **`IntoIterFn`** → **Compiler Hook** (`#[lang("into_iter_fn")]`, Method: `into_iter`)
    *   *Role*: Lowered at loop prelude: `var __iter = IntoIterator::into_iter(target)`.
*   **`Iterator`** → **Compiler Contract** (`#[lang("iterator")]`, Trait)
    *   *Role*: The core stream protocol.
*   **`IteratorNext`** → **Compiler Hook** (`#[lang("iterator_next")]`, Method: `next`)
    *   *Role*: Invoked at every loop iteration header: `match __iter.next() { ... }`.
*   *Note*: The return type of `next()` can either leverage `Option<Item>` (if `Option` variants are recognized as hooks) or return a `ControlFlow<(), Item>` to avoid multiplying enum variants in compiler contracts.

### 2.4 Closures & Callable Protocols
*   **`FnOnce`** → **Compiler Contract** (`#[lang("fn_once")]`, Trait)
    *   *Role*: Call protocol consuming the closure environment by value.
*   **`FnOnceCall`** → **Compiler Hook** (`#[lang("fn_once_call")]`, Method: `call_once`)
*   **`FnMut`** → **Compiler Contract** (`#[lang("fn_mut")]`, Trait)
    *   *Role*: Call protocol taking exclusive reference (`&rw self`).
*   **`FnMutCall`** → **Compiler Hook** (`#[lang("fn_mut_call")]`, Method: `call_mut`)
*   **`Fn`** → **Compiler Contract** (`#[lang("fn")]`, Trait)
    *   *Role*: Call protocol taking shared reference (`&self`).
*   **`FnCall`** → **Compiler Hook** (`#[lang("fn_call")]`, Method: `call`)

### 2.5 Asynchronous Suspension (`async` / `await`)
*   **`Future`** → **Compiler Contract** (`#[lang("future")]`, Trait)
    *   *Role*: The compiler-generated resumable state machine protocol.
*   **`FuturePoll`** → **Compiler Hook** (`#[lang("future_poll")]`, Method: `poll`)
    *   *Role*: The suspension resumption hook invoked at `await` boundaries.
*   **`Poll`** → **Compiler Contract** (`#[lang("poll")]`, Enum)
    *   *Role*: Return type of `Future::poll`.
*   **`PollReady` / `PollPending`** → **Compiler Hooks** (`#[lang("poll_ready")]`, `#[lang("poll_pending")]`, EnumVariants)
    *   *Role*: Target branches in MVIR async state machine lowering.
*   **`Waker` / `Context` / `Executor`** → **Ordinary Core APIs**
    *   *Role*: Runtime and library abstractions. The compiler generates the state transition logic and poll contract, but does not hardcode an executor or runtime.

### 2.6 Operator Overloading
*   **Arithmetic Traits (`Add`, `Sub`, `Mul`, `Div`, `Rem`)** → **Compiler Contracts** (`#[lang("add")]`, etc.)
*   **Arithmetic Methods (`add`, `sub`, etc.)** → **Compiler Hooks** (`#[lang("add_fn")]`, etc.)
*   **Index Protocol (`Index`, `IndexMut`)** → **Compiler Contracts** (`#[lang("index")]`, `#[lang("index_mut")]`)
*   **Index Methods (`index`, `index_mut`)** → **Compiler Hooks** (`#[lang("index_fn")]`, `#[lang("index_mut_fn")]`)
*   **Deref Protocol (`Deref`, `DerefMut`)** → **Compiler Contracts** / **Compiler Hooks**

### 2.7 Compiler Intrinsics (Phase 16 / 16.5)
*   **`size_of<T>()`** → **Intrinsic** (`IntrinsicKind::SizeOf`)
*   **`align_of<T>()`** → **Intrinsic** (`IntrinsicKind::AlignOf`)
*   **`type_info<T>()`** → **Intrinsic** (`IntrinsicKind::TypeInfo`)
*   **`type_of<T>(val)`** → **Intrinsic** (`IntrinsicKind::TypeOf`)
*   **`transmute<T, U>(val)`** → **Intrinsic** (`IntrinsicKind::Transmute`)
*   **`black_box<T>(val)`** → **Intrinsic** (`IntrinsicKind::BlackBox`)
*   *Rule*: Intrinsics do NOT use `#[lang("...")]` traits or methods. They are mapped via `#[intrinsic]` declarations and dispatched via `IntrinsicKind` in the MVIR generator and VM evaluator.

---

## 3. Fixed Architectural Checklist for New Semantic Features

Whenever a new language feature is proposed or implemented, engineers and agents **MUST** follow this 6-step checklist:

### Step 1: Archetype Classification
*   [ ] Is this feature a **Compiler Contract**, **Compiler Hook**, **Intrinsic**, **User-defined Protocol**, or **Ordinary Core API**?
*   [ ] Does the compiler really need to know about it? If it can be implemented with an existing contract (like `Result` implementing `Try`), it **must remain an Ordinary Core API**.

### Step 2: Language Surface & Grammar Authority
*   [ ] Consult `mellis-grammar` skill and `grammar.ebnf`.
*   [ ] Verify whether source keywords or operators (e.g., `?`, `for`, `await`, `async`) already exist.
*   [ ] Propose grammar changes before writing parser or AST code.

### Step 3: Canonical Identification
*   [ ] Assign a canonical `#[lang("...")]` string attribute for contracts and hooks.
*   [ ] Add the variant to `LangItem` enum in `mellis-semantic`.
*   [ ] Define target kind (`LangItemTarget::Trait`, `Method`, `Enum`, `EnumVariant`).

### Step 4: Strict Separation of Identification vs. Policy
*   [ ] Does `LangItem` strictly identify the declaration? (Yes)
*   [ ] Is behavioral policy (e.g. forbidding manual call, requiring bounds, borrow lifetimes) kept in Typechecker/Borrowck? (Yes)
*   [ ] Never attribute behavioral constraints to the `LangItem` itself.

### Step 5: Lowering & Rename Independence
*   [ ] Lowering in Typechecker, Borrowck, and MVIR must query `ctx.lang_items.get(LangItem)`.
*   [ ] **Zero string matching**: No `name == "..."` in compiler passes.
*   [ ] Ensure `CanonicalSymbolId` preserves identity across module/provider boundaries.

### Step 6: Bootstrap & Verification Suite
*   [ ] Declare the item in `core` component.
*   [ ] Ensure dynamic discovery via Sysroot Resolution without hardcoded file paths.
*   [ ] Write a Rename Independence test (proving the language mechanism works when lexical user functions share or don't share the same names).
