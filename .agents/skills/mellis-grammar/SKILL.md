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
- `using namespace`: Mellis does NOT have `using namespace` (C++ style). Mellis requires `using <path> as <alias>`.
- `using <path>;`: Bare `using` without `as <alias>` is rejected. Always use `using <path> as <alias>;`.
- `import <a::b>`: Mellis `import <...>` only accepts a single logical module name. No `::`.
- `import "foo.ms"` or `import "foo.mlib"`: File paths in local imports must not contain extensions (use `import "foo";`).
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
dec p: *i32 = ...;
dec rw p2: *rw i32 = ...;
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

### Imports & Module System
```rust
import <std>;          // External provider
import "local_module"; // Local provider
```

### Namespace Aliases (`using`)
`using` creates a **local alias** to an existing namespace. It does NOT import symbols, does NOT glob, and does NOT create provider dependencies.

**Grammar:** `using_decl ::= "using" module_path "as" IDENTIFIER ";"`

**Rules:**
- Target must be a module/namespace, not a leaf symbol (type, function, etc.)
- `using` aliases are compilation-local — they are NOT exported via MLib
- `export using` is rejected
- `using <path>;` without `as` is rejected
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
| `import`  | Make a provider available | `import <alloc>;` |
| `module`  | Define/export a namespace | `module std { ... }` |
| `using`   | Local namespace alias | `using std::collections as col;` |
| `::`      | Qualified lookup | `col::Vec<i32>` |

## Related Documents
- [grammar.ebnf](./grammar.ebnf): Machine-readable formal EBNF definition of the Mellis syntax.
- [docs/grammar.md](../../../docs/grammar.md): Original comprehensive grammar specification.
