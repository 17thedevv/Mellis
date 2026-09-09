---
name: mellis-grammar
description: Canonical Single Source of Truth (SSOT) for Mellis Language Grammar and Syntax. Provides strict guidelines, forbidden syntax, and canonical examples for the language surface.
---

# Mellis Grammar and Syntax Guidelines

This skill is the **Single Source of Truth** for Mellis syntax. Do not infer Mellis syntax from Rust, C++, or any other language. Always refer to this document and the accompanying `grammar.ebnf` when modifying the parser, AST, or semantic analysis, or when writing `.ms` code.

## Grammar Authority Rule

Agents MUST adhere to the following rules at all times:
1. **Read this skill** before modifying parser, AST, or semantic syntax.
2. **Never introduce a new keyword** without first updating the grammar in `docs/grammar.md`, `grammar.ebnf`, and this skill.
3. **Never reuse deprecated or removed keywords**.
4. **Never infer Mellis syntax from Rust/C++ syntax**. (e.g., Do not use `mut`, `let`, `mod`, `use`).
5. **Add or update parser tests** when changing grammar.
6. **Reuse, don't redesign**: If a syntax already exists, use it. If not, explicitly propose a grammar change to the user before implementation.

## Forbidden Syntax

The following keywords and syntax constructs are strictly **FORBIDDEN** in Mellis:
- `mut`: Mellis uses `rw` (rewritable) for mutability.
- `let`: Mellis uses `dec` and `const` for variable bindings.
- `use`: Mellis uses `import` for providers and `using ... as ...` for namespace aliases. The keyword `use` does not exist in Mellis.
- `mod`: Mellis uses `module` for namespaces, or `import` for module providers. (Avoid Rust's `mod` assumptions).
- `module foo;`: File-level module declarations without a body are forbidden. Mellis only allows inline `module foo { ... }`. Provider identity is determined by file name / artifact resolution.
- `using namespace`: Mellis does NOT have `using namespace` (C++ style). Mellis requires `using <path> as <alias>`.
- `using <path>;`: Bare `using` without `as <alias>` is rejected. Always use `using <path> as <alias>;`.
- `export using` / `export import`: Mellis v1.0 does not support module re-export. Both are hard syntax errors.
- `import <a::b>`: Mellis `import <...>` only accepts a single logical provider name. No `::`.
- `import "foo.ms"` or `import "foo.mlib"`: File paths in local imports must not contain extensions (use `import "foo";`).
- Prefix `await`: Mellis v1.0 strictly uses postfix `.await` (e.g. `fut.await`). Prefix `await fut` is forbidden.
- Semicolons `;` as struct field delimiters: Struct fields must strictly use comma `,` delimiters (e.g. `struct Point { x: f64, y: f64 }`).
- `=>`: Mellis uses `->` for match arms and lambda return types. Note: `=>` is strictly used as the macro rule separator (`macro foo { (pattern) => { template } }`).
- `$`: Mellis uses `@` for macro placeholders/metavariables (`@name: expr`). `$` is forbidden in Mellis macro definitions.

## Canonical Examples

### Variable Bindings & Mutability
```rust
dec x = 10;
dec rw y = 20;

// References
dec r: &i32 = &x;
dec rw r2: &rw i32 = &rw y;

// Raw Pointers
dec p: *i32 = &x as *i32;
dec rw p2: *rw i32 = &rw y as *rw i32;
```

### Raw Pointers and Unsafe Blocks / Functions
```rust
dec x = 10;
dec rw y = 20;

// Creating and passing raw pointers is safe
dec p: *i32 = &x as *i32;
dec p_mut: *rw i32 = &rw y as *rw i32;
dec q = p; // trivially copyable

// Dereferencing and pointer arithmetic require unsafe
unsafe {
    dec val = *p;         // read through immutable raw pointer
    *p_mut = 30;          // write through mutable raw pointer
    *rw p_mut = 40;       // explicit mutable dereference syntax
    dec next = p_mut + 1; // pointer arithmetic (element stride)
}

// Unsafe functions
unsafe fn danger(ptr: *rw i32) {
    *ptr = 999;
}

// Calling unsafe functions requires unsafe
unsafe {
    danger(p_mut);
}
```

### Generics
Mellis utilizes `<...>` for explicit generic arguments at BOTH type-level and value-level.
`@<...>` is NOT generic syntax.

- Type generic arguments: `<T>`
- Value generic arguments: `<T>`

```rust
struct Box<T> { ... }

impl<T> Box<T> { ... }

fn identity<T>(x: T) -> T {
    return x;
}

// Usage
dec x: Box<i32>;
dec y = identity<i32>(42);
```

### Struct Declarations
Struct fields **strictly use comma `,` delimiters**. Semicolons `;` are forbidden in v1.0.
Trailing comma is permitted.
```rust
struct Point {
    x: f64,
    y: f64,
}

struct Container<T> {
    value: T,
    id: i32,
}
```

### Imports & Module Architecture (Provider vs Namespace)
Mellis strictly separates **artifact providers** from **module namespaces**:
- `import` selects a **provider** (file/artifact name, no file extensions).
- `module` defines a **namespace / declaration container** inside a provider file.
- Symbols are accessed via their namespace (`geometry::Point`), NOT via their provider name.
- Multiple providers can contribute to the same module namespace (e.g. `alloc` and `core` contributing to `module std`).

```rust
// In math.ms (provider = "math"):
module geometry {
    export struct Point {
        x: f64,
        y: f64,
    }
}

// In consumer.ms:
import "math";     // Provider loaded (finds math.ms / math.mlib)
import <core>;     // External sysroot provider loaded

// Accessed via namespace, NOT provider name:
dec pt = geometry::Point { x: 1.0, y: 2.0 };
```

### Namespace Aliases (`using`)
`using` creates a **local alias** to an existing namespace path. It does NOT import symbols, does NOT glob, does NOT export, and does NOT create provider dependencies.

**Grammar:** `using_decl ::= "using" module_path "as" IDENTIFIER ";"`

**Rules:**
- Target must be a qualified module/namespace path, not a leaf symbol (type, function, etc.)
- `using` aliases are compilation-local — they are NOT exported via MLib
- `export using` is a compile-time syntax error
- `using <path>;` without `as` is a syntax error
- Standard duplicate-scope rules apply

```rust
import <alloc>;

// Alias a namespace
using std::collections as col;

dec v: col::Vec<i32>;       // Same as std::collections::Vec<i32>

// Deep path alias
using application::network::protocol as proto;

dec r: proto::Request;
dec s: proto::Response;

// INVALID — target is a type, not a namespace:
// using std::Vec as V;       // ERROR

// INVALID — missing 'as':
// using std::collections;    // ERROR

// INVALID — cannot export:
// export using std as s;     // ERROR
```

### Methods & Receiver Parameters
Mellis defines canonical shorthand for method receivers, with strict semantic equivalence to explicit forms:

| Canonical Shorthand | Explicit Equivalent | Semantics |
|:---|:---|:---|
| `&self` | `self: &Self` | Immutable reference receiver |
| `&rw self` | `self: &rw Self` | Mutable / rewritable reference receiver |
| `self` | `self: Self` | By-value ownership receiver |

```rust
impl Point {
    fn distance(&self, other: &Self) -> f64 {
        // ...
    }

    fn translate(&rw self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    fn consume(self) {
        // consumes ownership
    }
}
```

### Async & Await
Mellis strictly uses **postfix `.await`** (`expr.await`). Prefix `await expr` is strictly forbidden to eliminate operator precedence ambiguity and harmonize with the member-access family (`.field`, `.method()`, `[i]`, `.await`).

```rust
async fn fetch() -> i32 {
    dec fut = get_data();
    dec res = fut.await;
    return res;
}
```

### Loops (Foreach & C-style)
Mellis supports both foreach and C-style index loops as complementary systems constructs:

```rust
// 1. Foreach loop: for ( <pattern> in <expression> )
for (item in collection) {
    process(item);
}

// 2. C-style index loop: for ( <init> ; <condition> ; <step> )
for (dec rw i = 0; i < n; i += 1) {
    buffer[i] = 0;
}
```

### Match Statements
Match arms use `->` instead of `=>`.
```rust
match opt {
    MyOption::Some(x) -> x,
    MyOption::None -> 0,
}
```

### Functions
```rust
fn do_something(x: i32) -> i32 {
    return x + 1;
}
```

### Declarative Macros (Phase 14B)
Declarative macros use `@` for placeholders, `=>` to separate pattern and transcriber, and `!` for invocation.
```rust
macro foo {
    (@x: expr) => {
        @x * 2
    }

    (@x: expr, @y: expr) => {
        @x + @y
    }
}

// Invocations:
dec a = foo!(21);
dec b = foo![10, 20];
dec c = foo!{30};
```

## Module System Primitives

Mellis v1 has exactly 4 module system primitives:

| Primitive | Purpose | Example |
|-----------|---------|---------|
| `import`  | Make an artifact provider available | `import <alloc>;` / `import "math";` |
| `module`  | Define an inline namespace container | `module std { ... }` |
| `using`   | Local namespace alias | `using std::collections as col;` |
| `::`      | Qualified namespace lookup | `col::Vec<i32>` |

## Grammar Layer Invariant

$$\text{AST accepted by parser} \iff \text{Exactly defined by grammar.ebnf} \iff \text{Exactly supported by parser} \iff \text{No accepted syntax silently ignored}$$

## Related Documents
- [grammar.ebnf](./grammar.ebnf): Machine-readable formal EBNF definition of the Mellis syntax.
- [docs/grammar.md](../../../docs/grammar.md): Original comprehensive grammar specification.
