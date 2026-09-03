# Phase 14B: Attribute System — Implementation Plan

> **Attribute/derive macro protocol cho Mellis**

---

## 1. Overview

Phase 14B implement Attribute System — cho phép `#[derive(...)]`, `#[repr(...)]`, `#[test]`, etc.

**Key Principle**: Attribute là macro protocol, compiler chỉ biết gọi, nội dung do macro/compiler builtins xử lý.

---

## 2. Design Summary

Xem `Phase14_Macros_Design.md` Section 1 cho full design.

**Quick Reference:**
- `#[derive(Debug, Clone)]` → generate `impl Debug` và `impl Clone`
- `#[repr(C)]` → compiler semantic hook cho struct layout
- `#[test]` → compiler directive cho test discovery

---

## 3. Grammar

```ebnf
// Attribute annotation
annotation      ::= AT_BRACKET IDENTIFIER ( "(" annotation_args ")" )?
annotation_args ::= annotation_arg ("," annotation_arg)*
annotation_arg  ::= IDENTIFIER | literal | IDENTIFIER "=" literal

// Áp dụng cho declarations
struct_item     ::= annotation* "struct" IDENTIFIER ...
function_item   ::= annotation* "fn" IDENTIFIER ...
enum_item      ::= annotation* "enum" IDENTIFIER ...
```

---

## 4. Implementation Phases

### 14B.1 — Syntax & AST

**Files:**
- `mellis-lexer/src/token.rs` — Token `AT_BRACKET` (đã tồn tại)
- `mellis-ast/src/annotation.rs` — (new) `Annotation`, `AnnotationArg` structs
- `mellis-ast/src/decl.rs` — Thêm `annotations: Vec<Annotation>` vào decl variants
- `mellis-ast/src/lib.rs` — Export annotation types

**Deliverables:**
- [ ] `Annotation` struct với `name: Span`, `args: Vec<AnnotationArg>`
- [ ] `AnnotationArg` enum: `Ident(String)`, `Literal(Token)`, `KeyValue(key, value)`
- [ ] Thêm `annotations` field vào `Decl::Struct`, `Decl::Function`, `Decl::Enum`
- [ ] Parser: `parse_annotation()`, `parse_annotations()`

### 14B.2 — Semantic Analysis

**Files:**
- `mellis-semantic/src/annotation.rs` — (new) `AttributeProcessor`
- `mellis-semantic/src/typechecker.rs` — Gọi attribute processor sau populate_signatures

**Deliverables:**
- [ ] `AttributeProcessor` struct với `process_decl(decl_id, annotations)`
- [ ] Lookup attribute macro: `__derive__<Name>` hoặc builtin handler
- [ ] Thứ tự xử lý: `#[repr]` → `#[derive]` → `#[test]`
- [ ] Gắn generated declarations vào module

### 14B.3 — Built-in Attribute Handlers

**14B.3.1 — `#[derive(...)]`**

```rust
// Built-in derive traits
enum DeriveTrait {
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Default,
}

fn handle_derive(decl: &Decl, traits: &[DeriveTrait]) -> Vec<Decl> {
    let mut generated = Vec::new();
    for trait_name in traits {
        match trait_name {
            DeriveTrait::Debug => generated.push(generate_debug_impl(decl)),
            DeriveTrait::Clone => generated.push(generate_clone_impl(decl)),
            DeriveTrait::PartialEq => generated.push(generate_partial_eq_impl(decl)),
            // ...
        }
    }
    generated
}
```

**14B.3.2 — `#[repr(...)]`**

```rust
enum ReprKind {
    C,           // #[repr(C)]
    Transparent, // #[repr(transparent)]
    Primitive,  // #[repr(u8)], #[repr(i32)], etc.
}

fn handle_repr(decl: &mut Decl, repr: ReprKind) {
    match decl {
        Decl::Struct { annotations, .. } => {
            annotations.push(Annotation::CompilerIntrinsic("repr".into(), repr));
        }
        Decl::Enum { repr_type, ... } => {
            // Set enum tag size
        }
    }
}
```

**14B.3.3 — `#[test]`**

```rust
fn handle_test(decl: &Decl) {
    // Thêm vào test registry
    // Compiler tạo test runner entry point
}
```

### 14B.4 — Derive Macro Generation

**14B.4.1 — Debug**

```mellis
// Input
#[derive(Debug)]
struct Point { x: f64, y: f64 }

// Generated
impl Debug for Point {
    fn fmt(self, f: &mut Formatter) -> void {
        f.write_str("Point { ");
        f.write_str("x: ");
        Debug::fmt(self.x, f);
        f.write_str(", ");
        f.write_str("y: ");
        Debug::fmt(self.y, f);
        f.write_str(" }");
    }
}
```

**14B.4.2 — Clone**

```mellis
// Input
#[derive(Clone)]
struct Buffer { data: [u8; 100] }

// Generated
impl Clone for Buffer {
    fn clone(self) -> Buffer {
        Buffer { data: self.data }
    }
}
```

**14B.4.3 — PartialEq**

```mellis
// Input
#[derive(PartialEq)]
struct Vec2 { x: f64, y: f64 }

// Generated
impl PartialEq for Vec2 {
    fn eq(self, other: &Vec2) -> bool {
        self.x == other.x && self.y == other.y
    }
}
```

### 14B.5 — Integration

**Files:**
- `mellis-driver/src/lib.rs` — Gọi attribute expansion trước macro expansion
- `mellis-semantic/src/lib.rs` — Export `AttributeProcessor`

**Pipeline Order:**
```
Source (.ms)
    │
    ▼
┌─────────────────┐
│   Parser        │  Parse annotations, declarations
└────────┬────────┘
         ▼
┌─────────────────┐
│  Attribute Exp  │  14B: expand #[derive], handle #[repr]
└────────┬────────┘
         ▼
┌─────────────────┐
│  Macro Exp      │  14A: expand macro!() calls
└────────┬────────┘
         ▼
┌─────────────────┐
│   Resolver      │  Resolve symbols
└────────┬────────┘
         ▼
    TypeChecker...
```

---

## 5. Key Data Structures

### AST

```rust
// mellis-ast/src/annotation.rs (new)
#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: Span,
    pub args: Vec<AnnotationArg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum AnnotationArg {
    Ident(String),
    Literal(Token),
    KeyValue { key: String, value: Token },
}

#[derive(Debug, Clone)]
pub enum BuiltinAttribute {
    Derive(Vec<DeriveTrait>),
    Repr(ReprKind),
    Test,
    Inline,
    NoMangle,
    Link { name: String },
}
```

### Semantic

```rust
// mellis-semantic/src/annotation.rs (new)
pub struct AttributeProcessor<'a> {
    arena: &'a AstArena,
    source: &'a str,
    generated_decls: Vec<Item>,
}

pub trait AttributeHandler {
    fn name(&self) -> &str;
    fn apply(&self, decl_id: DeclId, args: &[AnnotationArg]) -> Vec<Decl>;
}
```

---

## 6. Test Plan

### 14B.1 — Parser Tests

| Test | Description |
|------|-------------|
| `annotation_basic.ms` | `#[derive(Debug)] struct Foo { ... }` |
| `annotation_multi.ms` | `#[derive(Debug, Clone, PartialEq)]` |
| `annotation_repr.ms` | `#[repr(C)]` |
| `annotation_error.ms` | `#[unknown_attr]` diagnostic |

### 14B.2 — Derive Tests

| Test | Description |
|------|-------------|
| `derive_debug.ms` | `#[derive(Debug)]` generates Debug impl |
| `derive_clone.ms` | `#[derive(Clone)]` generates Clone impl |
| `derive_partial_eq.ms` | `#[derive(PartialEq)]` generates PartialEq impl |
| `derive_multi.ms` | Multiple derives on one struct |
| `derive_struct.ms` | Struct with multiple fields |
| `derive_enum.ms` | `#[derive(Debug)]` on enum |

### 14B.3 — Repr Tests

| Test | Description |
|------|-------------|
| `repr_c.ms` | `#[repr(C)]` on struct |
| `repr_enum.ms` | `#[repr(u32)]` on enum |

### 14B.4 — Error Tests

| Test | Description |
|------|-------------|
| `derive_unknown.ms` | `#[derive(Unknown)]` → error |
| `repr_invalid.ms` | `#[repr(invalid)]` → error |

---

## 7. Timeline Estimate

```
14B.1 (Parser/AST)       2-3 days
14B.2 (Semantic)         2-3 days
14B.3 (Builtin Handlers) 3-4 days
14B.4 (Derive Gen)       4-5 days
14B.5 (Integration)      1-2 days
14B.6 (Tests)            2-3 days
───────────────────────────────
Total:                    ~14-20 days
```

---

## 8. Deliverables Summary

### Core
- [ ] `Annotation` AST struct
- [ ] Parser: `parse_annotation()`, `parse_annotations()`
- [ ] `AttributeProcessor` semantic handler
- [ ] `#[derive(Debug, Clone, PartialEq, ...)]` expansion
- [ ] `#[repr(C)]` struct layout semantic
- [ ] UI tests: 8+ test files

### Built-in Derive
- [ ] `Debug` impl generation
- [ ] `Clone` impl generation
- [ ] `PartialEq` impl generation
- [ ] `Eq` impl generation (marker)

### Optional (v1 scope)
- [ ] `#[test]` test discovery
- [ ] `#[inline]` / `#[no_mangle]`
- [ ] `#[link(...)]`

---

## 9. Open Questions

1. **Derive vs Macro**: `#[derive]` nên gọi một macro hay compiler built-in?
   - Macro: cleaner architecture, user có thể override
   - Built-in: simpler, predictable

2. **Custom derive**: User có thể định nghĩa `#[derive(MyTrait)]` không?
   - Cần attribute macro protocol (`#[attribute_macro]`)

3. **Enum derives**: `#[derive(Debug)]` trên enum nên format như thế nào?
   - Rust style: `VariantName { field: value }`

---

## Status: 🔴 Not Started

**Pre-requisites:**
- Phase 14 (Declarative Macros) ✅
