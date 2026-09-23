# Mellis Compiler Diagnostic System Audit

## Executive Summary

The Mellis diagnostic system is **structurally minimal** and **functionally incomplete**. While it handles the basic error→render pipeline, it lacks nearly every quality-of-life and engineering feature expected of a production compiler's error reporting.

---

## 1. Infrastructure Assessment

### 1.1 Diagnostic Struct ([diagnostic.rs](file:///d:/fdlang/mellis-rs/crates/mellis-common/src/diagnostic.rs))

```rust
pub struct Diagnostic {
    pub level: DiagnosticLevel,    // Error | Warning | Note
    pub span: Option<Span>,        // Optional source location
    pub message: String,           // Free-form text
}
```

> [!CAUTION]
> **Critical gaps in the struct itself:**

| Feature | Status | Impact |
|---------|--------|--------|
| Error Code (e.g. `E0001`) | ❌ Missing | No way to reference, filter, suppress, or document specific errors |
| Secondary Spans / Labels | ❌ Missing | Cannot point to "expected here" vs "found here" |
| Help / Suggestion text | ❌ Missing | No "did you mean?" or fix suggestions |
| Notes (attached sub-diagnostics) | ❌ Missing | Cannot chain "note: X defined here" to a primary error |
| Severity filtering | ❌ Missing | No way to turn warnings into errors or suppress specific diagnostics |
| Machine-readable output (JSON) | ❌ Missing | No IDE/LSP integration possible |

### 1.2 DiagnosticLevel Usage

| Level | Definition | Actually Used? |
|-------|-----------|----------------|
| `Error` | ✅ Defined | ✅ ~100+ emission sites |
| `Warning` | ✅ Defined | ❌ **NEVER USED** anywhere in the entire codebase |
| `Note` | ✅ Defined | ❌ **NEVER USED** anywhere in the entire codebase |

> [!WARNING]
> `Warning` and `Note` are dead code. The compiler has exactly **one diagnostic level**: hard error. There are no warnings for unused variables, unused imports, shadowing, deprecated features, or any other non-fatal condition.

### 1.3 Rendering ([diagnostic.rs:32-63](file:///d:/fdlang/mellis-rs/crates/mellis-common/src/diagnostic.rs#L32-L63))

The renderer is bare-minimum:
- ✅ Shows `error: <message>`
- ✅ Shows `  --> file:line:col`
- ✅ Shows the offending source line with `^^^` carets
- ❌ **No color output** (no ANSI escape codes)
- ❌ **No multi-line span rendering** (only highlights `start..end` on a single line)
- ❌ **No secondary label rendering** ("expected X, found Y")
- ❌ **No suggestion/fix rendering**
- ❌ **No error count summary** ("6 errors emitted; aborting")

### 1.4 Multi-file Rendering Bug ([lib.rs:382-393](file:///d:/fdlang/mellis-rs/crates/mellis-driver/src/lib.rs#L382-L393))

```rust
pub fn render_diagnostics(input: &str, diagnostics: &[Diagnostic]) -> String {
    let mut session = CompilerSession::new();
    session.source_manager.add_file("dummy.ms".to_string(), input.to_string());
    // ...
}
```

> [!CAUTION]
> `render_diagnostics` creates a **brand new** `SourceManager` with only **one file** named `"dummy.ms"`. Any diagnostic from an imported module (with a different `FileId`) will fail `get_file()` and render **without any source context** — just `"error: <message>"` with no location. The original `CompilerSession`'s `SourceManager` (which tracked all files) is discarded.

---

## 2. Diagnostic Quality by Phase

### 2.1 Parser ([mellis-parser](file:///d:/fdlang/mellis-rs/crates/mellis-parser/src/lib.rs))

| Quality | Status |
|---------|--------|
| Emission count | ~2 sites (invalid token + expect failure) |
| Spans | ✅ Always attached |
| Message quality | ⚠️ Generic ("Invalid token encountered", `expect()` message) |
| Recovery | ⚠️ Minimal — first error often causes cascade |
| Missing | No "unexpected X, expected Y" format; no recovery suggestions |

### 2.2 Resolver ([mellis-semantic/resolver.rs](file:///d:/fdlang/mellis-rs/crates/mellis-semantic/src/resolver.rs))

| Quality | Status |
|---------|--------|
| Emission count | ~28 sites (all via `declare_symbol` or direct push) |
| Spans | ✅ Always attached (passed through `declare_symbol`) |
| Message quality | ✅ Good for duplicates ("Duplicate definition of symbol `X`") |
| Missing | No "first defined here" secondary span on duplicate errors |

### 2.3 Type Checker ([mellis-semantic/typechecker.rs](file:///d:/fdlang/mellis-rs/crates/mellis-semantic/src/typechecker.rs))

This is the **largest emitter** with ~50+ diagnostic sites.

> [!WARNING]
> **~20 diagnostics are emitted WITHOUT spans.** They produce output like:
> ```
> error: type mismatch: cannot unify i32 with f64
> ```
> ...with NO file, line, or column information. The user has no idea where the error is.

#### Spanless Diagnostics (Partial List)

| Line | Message | Why spanless |
|------|---------|--------------|
| 1240 | `"if condition must be a boolean"` | No span extracted from `condition` expr |
| 1251 | `"while condition must be a boolean"` | Same |
| 1263 | `"for condition must be a boolean"` | Same |
| 1277 | `"type mismatch: {e}"` (return) | No span from `Stmt::Return` |
| 1282 | `"type mismatch: {e}"` (void return) | Same |
| 1294 | `"`break` outside of a loop"` | No span from `Stmt::Break` |
| 1299 | `"`continue` outside of a loop"` | Same |
| 1370 | `"Pointer arithmetic requires unsafe"` | No span |
| 1377 | `"type mismatch: {e}"` (assignment) | No span |
| 1420 | `"type mismatch: {e}"` (call arg) | No span |
| 1480 | `"type mismatch: {e}"` (call arg) | No span |
| 1749 | `"Array literal elements must have the same type"` | No span |
| 1767 | `"Tuple index N out of bounds"` | No span |
| 1774 | `"Cannot index into a non-tuple type"` | No span |
| 1824 | `"type mismatch: {e}"` (dyn dispatch arg) | No span |
| 1868 | `"type mismatch: {e}"` (method call arg) | No span |
| 2067 | `"Lambda return type mismatch"` | No span |
| 2103 | `"Cannot mutably capture immutable variable"` | No span |

#### Generic "type mismatch" Messages

The message `"type mismatch: {e}"` appears **10+ times** across the typechecker. The `e` is the raw `UnifyError` string, which typically looks like:
```
type mismatch: cannot unify i32 with f64
```

This tells the user **what** mismatched but not **where** (no span) or **why** (no "expected X because of return type annotation" context).

#### Inconsistent Message Casing and Style

| Example | Style Issue |
|---------|------------|
| `"Unknown struct '{}'"` | Title case |
| `"type mismatch: {}"` | Lowercase, no period |
| `"Cannot mutate immutable variable"` | Title case |
| `"if condition must be a boolean"` | Lowercase |
| `"Array literal elements must have the same type"` | Title case |
| `"expected struct, found another type"` | Lowercase |
| `"Pointer arithmetic requires an unsafe block."` | Period at end |
| `"Lambda return type mismatch"` | No detail |

There is **no style guide** enforced. Some messages end with periods, some don't. Some are capitalized, some aren't. Some include the problematic symbol name, some don't.

### 2.4 Borrow Checker ([mellis-borrowck](file:///d:/fdlang/mellis-rs/crates/mellis-borrowck/src))

| Quality | Status |
|---------|--------|
| Emission count | ~10 sites |
| Spans | ⚠️ Uses `func.values[val_id].span` — may be `None` for synthetic values |
| Message quality | ✅ Good ("Cannot borrow 'X' as &rw because it is already borrowed as &") |
| Variable names | ❌ Uses MVIR internal names (`%v0`, `%v1`) instead of source names |
| Missing | No "first borrow here" secondary span; no lifetime annotation suggestions |

> [!IMPORTANT]
> Borrow checker errors display **MVIR value IDs** (`%v3`) instead of source-level variable names. This is completely opaque to users.

### 2.5 Driver / Backend ([mellis-driver](file:///d:/fdlang/mellis-rs/crates/mellis-driver/src))

| Quality | Status |
|---------|--------|
| Emission count | ~15 sites |
| Spans | ⚠️ Mixed — import errors have spans, backend errors don't |
| Message quality | ⚠️ Backend errors are raw strings ("Backend Error: ...", "Link Error: ...") |

---

## 3. Systematic Issues

### 3.1 No Error Recovery / Error Cascading

The compiler uses `return Err(diagnostics)` at the **first phase** that produces errors. There is no attempt to:
- Continue past parse errors to find more errors
- Continue past type errors in one function to check other functions
- Limit cascade errors (where one root cause produces 10+ downstream errors)

### 3.2 No Deduplication

If the same error condition is hit multiple times (e.g., same unresolved type used in 5 places), the compiler emits 5 identical diagnostics with no deduplication.

### 3.3 Debug Prints Leaked to Production

| File | Line | Content |
|------|------|---------|
| [lib.rs](file:///d:/fdlang/mellis-rs/crates/mellis-driver/src/lib.rs#L38) | 38 | `eprintln!("DEBUG DRIVER: Items before attr_processor: {}")` |
| [lib.rs](file:///d:/fdlang/mellis-rs/crates/mellis-driver/src/lib.rs#L40) | 40 | `eprintln!("DEBUG DRIVER: Items after attr_processor: {}")` |
| [borrow_analysis.rs](file:///d:/fdlang/mellis-rs/crates/mellis-borrowck/src/borrow_analysis.rs#L232) | 232 | `println!("DEBUG: check_access ...")` |
| [borrow_analysis.rs](file:///d:/fdlang/mellis-rs/crates/mellis-borrowck/src/borrow_analysis.rs#L380) | 380 | `println!("DEBUG: CallDirect ...")` |
| [borrow_analysis.rs](file:///d:/fdlang/mellis-rs/crates/mellis-borrowck/src/borrow_analysis.rs#L501) | 501 | `println!("DEBUG: arg {:?} ...")` |
| [borrow_analysis.rs](file:///d:/fdlang/mellis-rs/crates/mellis-borrowck/src/borrow_analysis.rs#L506) | 506-509 | 2× `println!("DEBUG: ...")` |

> [!CAUTION]
> These `println!` and `eprintln!` calls produce noise on every compilation. They are **not** gated behind any `--verbose` or debug flag.

### 3.4 No Error Limit

There is no `--max-errors N` flag. If a file has 500 type errors, all 500 are printed.

### 3.5 `_ => {}` Swallows Entire AST Variants

In `typecheck_stmt` at [line 1302](file:///d:/fdlang/mellis-rs/crates/mellis-semantic/src/typechecker.rs#L1302):
```rust
_ => {}
```
And in `typecheck_expr` at [line 2187](file:///d:/fdlang/mellis-rs/crates/mellis-semantic/src/typechecker.rs#L2187):
```rust
_ => self.ctx.types.intern(SemanticType::Error)
```

Any unhandled AST node silently produces no diagnostic and gets typed as `Error`. This means new AST variants added to the parser can silently pass through the entire semantic layer.

---

## 4. Missing Diagnostic Categories

| Category | Exists? | Notes |
|----------|---------|-------|
| Unused variable | ❌ | |
| Unused import | ❌ | |
| Unreachable code | ❌ | |
| Shadowing warning | ❌ | |
| Dead code | ❌ | |
| Deprecated feature | ❌ | |
| Implicit type coercion | ❌ | |
| Missing return type annotation | ❌ | |
| Unused function | ❌ | |
| Recursive type (infinite size) | ⚠️ | Detected in Phase 5 audit but no diagnostic emitted pre-backend |
| Missing match arm detail | ⚠️ | Says "not exhaustive" but doesn't list missing variants |

---

## 5. Priority Recommendations

### P0 — Blocks Usability
1. **Add spans to all ~20 spanless diagnostics** in `typechecker.rs`
2. **Fix `render_diagnostics`** to use the real `SourceManager` instead of creating a dummy one
3. **Remove all `println!("DEBUG: ...")`** from production code
4. **Add ANSI color output** to the renderer

### P1 — Improves Developer Experience
5. **Add secondary spans** (at minimum: "expected type" annotation location + "found type" expression location)
6. **Add error codes** (`E0001`–`E9999`) for all diagnostics
7. **Standardize message style** (lowercase, no trailing period, include relevant names)
8. **Add error count summary** ("aborting due to N previous errors")
9. **Display source-level names** in borrow checker errors instead of MVIR `%vN`

### P2 — Production Quality
10. **Add `Warning` level diagnostics** for unused variables, shadowing, etc.
11. **Add `Note` level** for "first defined here", "trait defined here" context
12. **Add `--max-errors`** flag
13. **Add JSON output** for IDE/LSP integration
14. **Add help/suggestion text** ("did you mean `x`?", "try adding `rw` keyword")
15. **Add diagnostic deduplication**
