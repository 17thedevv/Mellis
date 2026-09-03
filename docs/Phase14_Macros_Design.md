# Phase 14: Macros — Macro Language v1 Design

> **Design Decision Document** — Trước khi implementation, cần chốt các design decisions cơ bản.

---

## 0. Metaprogramming Taxonomy

Mellis có **3 loại metaprogramming** rõ ràng:

```
┌──────────────────────────────────────────────────────────────┐
│                    METAPROGRAMMING LAYERS                     │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  1. ATTRIBUTE / DERIVE                                        │
│     transform: declaration → declaration                       │
│     #[derive(Debug, Clone)] → impl Debug for Foo              │
│                                                               │
│  2. DECLARATIVE MACRO                                         │
│     transform: syntax → syntax (at call site)                  │
│     macro foo!(...) → expanded syntax                         │
│                                                               │
│  3. COMPTIME                                                 │
│     transform: nothing → value (compile-time computation)       │
│     const N = comptime calculate();                           │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**Boundary quan trọng:**
```
#[derive(...)]
    → "hãy generate/transform DECLARATION này"

macro!(...)
    → "hãy generate SYNTAX ở VỊ TRÍ này"

comptime
    → "hãy tính GIÁ TRỊ này lúc compile-time"
```

---

## 1. ATTRIBUTE SYSTEM (`#[...]`) — Phase 14B

### Design: Attribute là macro protocol

**Status: 🔴 Chưa implement**

**Ý tưởng:**
- `#[derive(Debug, Clone)]` là một **attribute invocation**
- Compiler chỉ biết cách gọi attribute macro
- `Debug`, `Clone` được implement bởi macro definitions hoặc compiler builtins

### Grammar
```ebnf
annotation  ::= AT_BRACKET IDENTIFIER ( "(" arguments ")" )? "]"
annotations ::= annotation*
```

### Attribute Types

| Type | Behavior | Examples |
|------|----------|---------|
| **Derive** | Gọi derive macro để generate impls | `#[derive(Debug, Clone)]` |
| **Compiler** | Compiler xử lý semantic trực tiếp | `#[repr(C)]`, `#[inline]` |
| **Function** | Gắn metadata vào declaration | `#[test]`, `#[deprecated]` |

### Built-in Derive Macros

```mellis
// User code
#[derive(Debug, Clone, PartialEq)]
struct User {
    id: int_32;
    name: str;
}
```

**Compiler Pipeline:**
```
1. Parse annotations: [derive(Debug), derive(Clone)]
2. Expand derive macros
3. Generated AST:
     struct User { ... }
     impl Debug for User { ... }
     impl Clone for User { ... }
     impl PartialEq for User { ... }
```

### Built-in Compiler Attributes

| Attribute | Behavior |
|-----------|----------|
| `#[repr(C)]` | C struct layout |
| `#[inline]` | Inline hint to backend |
| `#[no_mangle]` | Export with original name |
| `#[link(name)]` | Link against library |

### Attribute Syntax Examples

```mellis
// Struct với derive
#[derive(Debug, Clone)]
struct Point {
    x: f64;
    y: f64;
}

// Struct với repr
#[repr(C)]
struct Header {
    magic: u32;
    size: u64,
}

// Function với attribute
#[test]
fn test_add() {
    assert!(1 + 1 == 2);
}

#[inline]
fn hot_path() {
    // ...
}

// Enum với repr
#[repr(u32)]
enum Opcode {
    Nop = 0,
    Add = 1,
    Sub = 2,
}
```

---

## 2. ATTRIBUTE MACRO PROTOCOL

### How Derive Works

```rust
// Pseudo-code: derive macro protocol
// derive_macro.expand(decl: &Decl, args: &[Ident]) -> Vec<Decl>

// Ví dụ: #[derive(Debug)]
expand_derive(decl: Decl::Struct(User), args: [Debug]) -> vec![
    Decl::Impl { trait: Debug, impl: ... }
]
```

### Built-in Derive Implementations

Mellis compiler cung cấp sẵn:

```rust
// Compiler built-in derive macros:
// - Debug: generate debug formatting
// - Clone: generate clone implementation  
// - PartialEq: generate equality comparison
// - Eq: marker trait for transitive equality
// - Hash: generate hash computation
// - Default: generate default constructor
```

### Pipeline

```
Source (.ms)
    │
    ├─ Parse annotations
    │     #[derive(Debug, Clone)]
    │     struct Foo { ... }
    │
    ▼
Parse Declarations + Annotations
    │
    ├─ Decl::Struct { annotations: [derive(Debug), derive(Clone)], ... }
    │
    ▼
Attribute Macro Expansion
    │
    ├─ derive.expand(Decl::Struct, [Debug, Clone])
    │     → vec![impl Debug, impl Clone]
    │
    ▼
Expanded Declarations
    │
    ├─ Decl::Struct { ... }
    ├─ Decl::Impl { trait: Debug, ... }
    └─ Decl::Impl { trait: Clone, ... }
    │
    ▼
Continue: Resolver → TypeChecker → MVIR
```

---

## 3. DECLARATIVE MACRO (`macro!`)

### Design: Matcher → Transcriber với `@` notation

### Grammar
```ebnf
macro_decl ::= annotation* "export"? "macro" IDENTIFIER 
               "{" macro_rule+ "}"
               
macro_rule ::= "(" macro_pattern ")" "=>" block_stmt

macro_pattern ::= macro_pattern_item ("," macro_pattern_item)*

macro_pattern_item ::= 
    | "@" IDENTIFIER ":" fragment_spec      // metavariable
    | "@" IDENTIFIER ":" fragment_spec "..."  // variadic
    | LITERAL_TOKEN                          // literal match
    | "(" macro_pattern ")"                 // nested group

fragment_spec ::= "expr" | "ident" | "ty" | "stmt" | "block" | "item"
```

### Ví dụ Declarative Macro

```mellis
// Vec macro với declarative rules
macro vec {
    // Rule 1: single element
    (@x: expr) => { [@x] }
    
    // Rule 2: variadic
    (@xs: expr...) => { [@xs] }
}

dec v1 = vec!(42);           // Rule 1
dec v2 = vec!(1, 2, 3);     // Rule 2
```

### Invocation Delimiters

```mellis
vec!(1, 2, 3);     // paren delimiter
vec![1, 2, 3];     // bracket delimiter
vec!{1, 2, 3};     // brace delimiter
```

---

## 4. ATTRIBUTE vs MACRO DISTINCTION

| Aspect | Attribute `#[...]` | Macro `macro!` |
|--------|---------------------|----------------|
| **Position** | Before declaration | At expression/statement position |
| **Transforms** | Declaration → Declaration | Expression/Statement → Expression/Statement |
| **Arguments** | Named identifiers | Token stream |
| **Example** | `#[derive(Debug)]` | `vec!(1, 2, 3)` |
| **Purpose** | Generate impls, add metadata | Control structures, syntax sugar |

### Correct Usage

```mellis
// ATTRIBUTE: gắn vào struct để generate code
#[derive(Debug, Clone)]
struct Point { x: f64, y: f64 }

// MACRO: gọi tại vị trí expression để expand
dec v = vec![1, 2, 3];  // vec! macro expand tại đây
```

---

## 5. MACRO PATTERN DATA STRUCTURE

### Pattern Tree

```rust
pub enum MacroPattern {
    /// Literal token phải khớp chính xác
    Token(Token),
    
    /// Metavariable - capture fragment
    Var {
        name: SymbolId,
        kind: FragmentKind,
    },
    
    /// Variadic metavariable - capture zero or more
    VarArgs {
        name: SymbolId,
        kind: FragmentKind,
        separator: Option<TokenKind>,
    },
    
    /// Group - khớp token group với delimiter
    Group {
        delimiter: Delimiter,
        content: Vec<MacroPattern>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind {
    Expr, Ident, Ty, Stmt, Block, Item,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Parenthesis, Bracket, Brace,
}
```

### Template Tree

```rust
pub enum MacroTemplate {
    Token(Token),
    Var(SymbolId),                    // Thay bằng captured
    VarArgs(SymbolId),               // Expand captured list
    Group {
        delimiter: Delimiter,
        content: Vec<MacroTemplate>,
    },
    Repeat {
        content: Vec<MacroTemplate>,
        separator: Option<Token>,
    },
}
```

---

## 6. FRAGMENT MATCHING: Parser-backed

### Decision: Dùng parser, không greedy

**Lý do:**
- Greedy ad-hoc matching sẽ duplicate parser logic
- Mellis parser đã có grammar chuẩn
- Đúng semantics cho nested expressions

```rust
fn match_fragment(
    cursor: &mut TokenCursor, 
    kind: FragmentKind
) -> Result<TokenStream, MatchError> {
    match kind {
        FragmentKind::Expr => parse_expression_at_cursor(cursor),
        FragmentKind::Ty => parse_type_at_cursor(cursor),
        FragmentKind::Ident => parse_ident_at_cursor(cursor),
        FragmentKind::Stmt => parse_statement_at_cursor(cursor),
        // ...
    }
}
```

---

## 7. HYGIENE: SyntaxContext

### Structure

```rust
pub enum SyntaxContext {
    /// Code viết trực tiếp bởi user
    Origin,
    
    /// Từ một macro expansion cụ thể
    Expansion(MacroExpansionId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacroExpansionId(u32);

pub struct HygienicIdent {
    pub text: String,
    pub span: Span,
    pub context: SyntaxContext,
}
```

### Correct Behavior

```mellis
dec x = 20;  // x#Origin

macro temp {
    () => { dec x = 10; }  // x#Expansion(1)
}

temp!();
print(x);  // In 20 - x#Origin vẫn còn
```

---

## 8. CROSS-MODULE MACROS

### Decision: Export/Import hoạt động

```mellis
// module_a.ms
export macro vec {
    (@x: expr) => { [@x] }
    (@xs: expr...) => { [@xs] }
}

// module_b.ms
import "module_a";

dec v = vec!(1, 2, 3);  // ✅ vec được import
```

### MLib Storage

```rust
pub struct MlibMacro {
    pub name: String,
    pub rules: Vec<MacroRule>,
    pub span: Span,
}
```

---

## 9. MACRO NAMESPACE

### Decision: Riêng namespace, không fallback

```rust
fn resolve_macro_call(name: &str) -> Resolution {
    if let Some(macro_def) = macros.get(name) {
        return Resolution::Macro(macro_def);
    }
    // KHÔNG fallback sang function
    Resolution::Undefined
}
```

### Error Messages

```mellis
// foo!(x) nhưng không có macro foo
error: no macro named `foo` in scope
  --> test.ms:5:1
   |
5  |     foo!(x);
   |     ^^^
```

---

## 10. SUMMARY OF DESIGN DECISIONS

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **3 Layers** | Attribute + Macro + Comptime | Rõ ràng boundary |
| **Attribute** | Derive protocol | Library-first, extensible |
| **Notation** | `@` | Mellis đã dùng `@` |
| **Model** | Declarative (matcher → transcriber) | Multiple rules |
| **Fragment matching** | Parser-backed | Không duplicate parser |
| **Hygiene** | SyntaxContext | Semantic identity |
| **Cross-module** | Export/import | Hoạt động |
| **Namespace** | Separate | Không fallback |

---

## 11. PHASE STRUCTURE

```
Phase 14A — Attribute System
       ├─ AST: Annotation nodes
       ├─ Parser: #[...] parsing
       ├─ Derive macro protocol
       └─ Compiler attributes (repr, inline)

Phase 14B — Declarative Macro AST
       ├─ MacroPattern enum
       ├─ MacroTemplate enum
       ├─ MacroRule struct
       └─ FragmentKind enum

Phase 14C — Parser Integration
       ├─ Parse macro declaration
       ├─ Parse rules (matcher => transcriber)
       ├─ Fragment matcher dùng parser
       └─ Token tree capture

Phase 14D — Macro Namespace
       ├─ Separate macro scope
       ├─ Resolution không fallback
       └─ Export/import support

Phase 14E — Expansion Engine
       ├─ Match rule với pattern tree
       ├─ Capture fragments
       ├─ Variadic expansion
       ├─ Recursive expansion
       └─ Depth limit

Phase 14F — Hygiene System
       ├─ SyntaxContext enum
       ├─ MacroExpansionId
       ├─ Context tracking
       └─ Resolution với context

Phase 14G — Integration
       ├─ Expanded code pipeline
       └─ Full compilation flow

Phase 14H — MLib Cross-module
       ├─ Serialize macro definition
       ├─ Import macro definition
       └─ Re-expand at consumer
```

---

## 12. READY FOR IMPLEMENTATION

Design này đảm bảo:

1. **3 layers riêng biệt**: Attribute / Macro / Comptime không bị trộn lẫn
2. **Attribute là derive protocol**: Không phải repetition syntax
3. **`@` notation**: Consistent với Mellis placeholder
4. **Parser-backed matching**: Không greedy ad-hoc
5. **SyntaxContext hygiene**: Semantic identity
6. **Cross-module hoạt động**: Export/import
