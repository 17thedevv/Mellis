# RFC: Visibility-02 — Public Field Default

**Status:** APPROVED WITH AMENDMENTS
**Champions:** 17thedevv
**Created:** 2026-09-14
**Supersedes:** N/A (complements VIS-STRUCT v1)

---

## Summary

Change struct field visibility default from **private** to **public**, where field visibility is declared independently from the struct's type visibility.

## Canonical Rule

```
Struct type visibility:
    governed by struct declaration.

Field visibility:
    unmodified → Public
    export     → Public
    private    → Private

Access check:
    first validate containing struct accessibility;
    then validate field accessibility.

A private containing type does not change the field's declared visibility.
```

---

## Motivation

### Current State (VIS-STRUCT v1)

Currently, all struct fields default to `private` unless explicitly marked `export`:

```luna
struct Point {
    export x: i32,  // explicit public
    export y: i32,  // explicit public
}
```

### Problems with Private-by-Default

1. **Boilerplate burden**: Every field in every public struct requires `export` modifier
2. **Ergonomic penalty**: Simple data types like `Point`, `Vec2`, `Color` become verbose
3. **Inverted intuition**: Most struct fields in systems code are data (public) not representation (private)
4. **Stdlib verbosity**: Current stdlib has many `export` keywords that could be implicit

### Solution

Data is public unless representation needs protection:

```luna
// Plain data struct — fields implicitly public
struct Point {
    x: i32,
    y: i32,
}

// Abstractions with invariants — explicit private
struct String {
    private vec: Vec<u8>,
}

struct HashMap<K, V> {
    private table: ...,
    private len: u64,
}
```

---

## Design

### Syntax Change

| Declaration | Visibility | Notes |
|-------------|------------|-------|
| `field: T` | Public | **New default** |
| `export field: T` | Public | Explicit spelling, for readability |
| `private field: T` | Private | Explicit encapsulation |

### Access Check Order

When accessing `obj.field`:

1. **Struct type check**: Is `obj`'s type accessible from current scope?
2. **Field visibility check**: Is the field `Public` or `Private`?

If step 1 fails, the error is "type `T` is private".
If step 1 passes but step 2 fails, the error is "field `field` of struct `T` is private".

**Key invariant**: A private struct does not change the field's declared visibility. A `public` field inside a `private` struct is still declared as `Public`; it simply cannot be accessed externally because step 1 gates it first.

### VIS-STRUCT Invariants (Updated)

| ID | Rule | Status |
|----|------|--------|
| VIS-STRUCT-1 | Field visibility is independent from struct visibility | **Reaffirmed** |
| VIS-STRUCT-2 | ~~A private struct cannot export fields~~ | **Removed** (superseded) |
| VIS-STRUCT-3 | Private field access permitted from defining module scope/descendants | Unchanged |
| VIS-STRUCT-4 | Struct type visibility checked before field visibility | Unchanged |
| VIS-STRUCT-5 | External construction requires access to every required field | Unchanged |
| VIS-STRUCT-6 | All 6 access paths (read/write/borrow/project/destructure/construct) enforced | Unchanged |
| VIS-STRUCT-7 | Source and .llib preserve identical semantics | Unchanged |
| VIS-STRUCT-8 | **Field default is `Public` when declared without modifier** | **New** |

### Breaking Change Note

This is a breaking change for source code. Migration requires:

```
Scan every public struct
    ↓
unmarked field
    ↓
is this intentionally private?
    ├── yes → add `private`
    └── no  → leave implicit public
```

**Migration audit is mandatory** for stdlib before this can be frozen. See Migration Audit section.

---

## Acceptance Criteria

### TC-VIS-02-1: Public Struct + Implicit Public Fields

```luna
export struct Point {
    x: i32,
    y: i32,
}

dec p = Point { x: 1, y: 2 };
dec v = p.x;        // OK
p.x = 10;           // OK
dec rx = &p.x;      // OK
```

### TC-VIS-02-2: Public Struct + Explicit Private Fields

```luna
export struct BankAccount {
    balance: i64,           // public: OK to read
    private pin: u32,       // private: encapsulation
}

dec acc = BankAccount { balance: 1000, pin: 1234 };
dec b = acc.balance;        // OK
acc.balance = 2000;         // OK
dec p = acc.pin;            // ERR: private field
```

External construction with private field:

```luna
// In external module:
dec acc = BankAccount { balance: 1000, pin: 1234 };
// ERR: field `pin` of struct `BankAccount` is private
```

### TC-VIS-02-3: Private Struct + Fields

```luna
struct Internal {
    x: i32,              // declared Public, but struct is private
    private y: i32,     // declared Private
}

// Same module:
dec acc = Internal { x: 1, y: 2 };  // OK
dec v = acc.x;                      // OK (same module)
```

The `x` field is still declared as `Public` — it simply cannot be accessed externally because `Internal` itself is private.

### TC-VIS-02-4: Nested Module Access

```luna
module outer {
    struct Outer {
        data: i32,           // declared Public
    }

    module inner {
        dec test(o: Outer) -> i32 {
            return o.data;    // OK: inner is descendant of outer
        }
    }
}
```

Descendant modules can access `Public` fields of private structs.

### TC-VIS-02-5: Source/.llib Parity

Field visibility defaults must be identical between `.ln` and `.llib`.

### TC-VIS-02-6: Pattern/Destructuring

```luna
export struct Point {
    x: i32,
    y: i32,
}

dec p = Point { x: 1, y: 2 };
dec Point { x, .. } = p;  // OK: destructuring public fields
```

### TC-VIS-02-7: All 6 Access Paths

All VIS-STRUCT-6 access paths must work for implicitly public fields:
1. Field read: `p.x`
2. Field write: `p.x = val`
3. Field borrow: `&p.x`, `&rw p.x`
4. Struct literal: `Point { x: ..., y: ... }`
5. Destructuring: `dec Point { x, .. } = p`
6. Nested projection: `rect.origin.x`

### TC-VIS-02-8: Public Struct + Private Field Blocks External Construction

```luna
export struct Config {
    name: str,           // public field
    private secret: str, // private field
}

// External module:
dec c = Config { name: "app" };
// ERR: missing required field `secret`

dec c = Config { name: "app", secret: "xyz" };
// ERR: field `secret` of struct `Config` is private
```

---

## Implementation

### Parser Changes

In the parser, when parsing `StructField`, if no visibility keyword is present, set `visibility: Visibility::Public`:

```rust
// luna-parser/src/decl.rs
let f_vis = if self.match_token(TokenKind::KwExport) {
    Visibility::Public
} else if self.match_token(TokenKind::KwPrivate) {
    Visibility::Private
} else {
    Visibility::Public  // new default
};
```

### Semantic Changes

1. **Resolver**: Remove VIS-STRUCT-2 check (`private struct cannot export fields`)
2. **TypeChecker**: `check_field_visibility` unchanged; struct accessibility check already gates field access
3. **No new `Visibility` variant needed**: `ImplicitPublic` is unnecessary — parser directly sets `Visibility::Public`

### Lint

Do **not** add lint for redundant `export` in RFC v1. `export field` is explicit source intent; linting is a tooling decision for a future iteration.

---

## Migration Audit (Mandatory)

Before freezing this RFC, a full stdlib audit is required:

### High-Risk Types

These types expose internal representation that must become `private`:

```luna
// MUST ADD private
export struct String {
    private vec: Vec<u8>,
}

export struct Vec<T> {
    private data: *T,
    private len: u64,
    private cap: u64,
}

export struct HashMap<K, V> {
    private table: ...,
    private len: u64,
    private tombstones: u64,
}

// Iterator internals
struct IterStruct<T> {
    private ptr: *T,
    private end: *T,
}
```

### Audit Checklist

```
□ String.vec          → private
□ Vec.data, len, cap   → private
□ HashMap.table, len   → private
□ Option.none internals
□ Result.err internals
□ Iterator state fields
□ Builder internals
□ Parser/tokenizer state
□ Any @repr(C) fields that need specific layout
```

---

## References

- VIS-STRUCT v1 (frozen)
- Box-FROZEN ownership abstraction
- Luna Semantic Compliance Protocol (SKILL.md)
