# Lifetime System

> Safe memory through explicit provenance tracking at compile time.

Mellis's lifetime system ensures that references never outlive the data they point to, preventing use-after-free and dangling pointer errors. Mellis uses **relation model** — explicit provenance expressions that read like natural language, without requiring lifetime annotations like Rust's `'a`, `'b`.

## Table of Contents

1. [Why Lifetimes?](#why-lifetimes)
2. [Basic Syntax](#basic-syntax)
3. [life_from()](#life_from)
4. [where outlives()](#where-outlives)
5. [Examples](#examples)
6. [Error Messages](#error-messages)
7. [Best Practices](#best-practices)

---

## Why Lifetimes?

Consider this unsafe code:

```mellis
fn dangling() -> &i32 {
    let x = 5;
    return &x;  // ERROR: x is dropped at end of function
}
```

The compiler must ensure that references are valid for their entire lifetime. Mellis tracks **provenance** — where a reference came from — and verifies that return values don't outlive their sources.

---

## Basic Syntax

Functions can declare explicit lifetime signatures after the return type:

```mellis
fn function_name(param: &Type) -> &Type [lifetime_signature]
```

The lifetime signature consists of:

1. **`life_from(ident)`** — declares the provenance of the return value
2. **`where outlives(a, b)`** — constrains relative lifetimes

---

## life_from()

### Single Origin

```mellis
fn first(a: &i32) -> &i32 life_from(a) {
    return a;  // Return has same lifetime as a
}
```

The return value's provenance is `a`. This means the return reference is valid as long as `a` is valid.

### Multiple Origins

When a function can return from one of several parameters:

```mellis
fn pick_first(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    if condition {
        return a;
    } else {
        return b;
    }
}
```

The `|` (pipe) means "or". The return can come from either `a` or `b`.

### How it Works

`life_from(x)` generates:

1. **Provenance**: The return value originates from `x`
2. **Constraint**: `'return ≤ 'x` (return lifetime is at most x's lifetime)

---

## where outlives()

Declares that one lifetime must be at least as long as another.

### Syntax

```mellis
where outlives(longer, shorter)
```

This reads as: "`longer` must outlive `shorter`" — meaning `longer`'s lifetime must be **greater than or equal to** `shorter`'s.

### Example

```mellis
fn merge(a: &i32, b: &i32) -> &i32
    life_from(a)
    where outlives(a, b)
{
    return a;
}
```

This says: "return comes from `a`, but `a` must outlive `b`".

### Why?

Consider this scenario:

```mellis
fn cache_result(result: &i32, cache: &i32) -> &i32
    life_from(result)
    where outlives(result, cache)
```

If `cache` is dropped before `result` is used, there could be issues. The `outlives` constraint ensures `result` lives at least as long as `cache`.

---

## Examples

### Example 1: Simple Reference Passing

```mellis
fn get_first(x: &i32) -> &i32 life_from(x) {
    return x;
}

fn main() {
    let value = 42;
    let ref = get_first(&value);
    printLn(*ref);  // Prints: 42
}
```

### Example 2: Conditional Return

```mellis
fn select(positive: &i32, negative: &i32, flag: bool) -> &i32
    life_from(positive | negative)
{
    if flag {
        return positive;
    }
    return negative;
}
```

### Example 3: With Lifetime Constraint

```mellis
fn longest(x: &i32, y: &i32) -> &i32
    life_from(x | y)
    where outlives(x, y)
{
    if *x > *y {
        return x;
    }
    return y;
}
```

### Example 4: Struct with Lifetime

```mellis
struct Span {
    start: &i32,
    end: &i32,
}

fn create_span(a: &i32, b: &i32) -> Span life_from(a | b) {
    return Span { start: a, end: b };
}
```

> **Note:** Mellis does NOT use lifetime annotations like Rust's `'a`, `'b`. 
> Lifetime relationships are expressed through `life_from()` and `where outlives()` only.
> The compiler infers actual lifetimes internally.

---

## Error Messages

### Unresolved Lifetime Identifier

```mellis
fn bad(x: &i32) -> &i32 life_from(y)  // Error: 'y' does not refer to a parameter
```

**Fix**: Change `y` to a valid parameter name (`x`).

### Type Mismatch

```mellis
fn bad(a: &i32, b: &str) -> &i32 life_from(a | b)
// Error: cannot use '&str' in life_from with '&i32' return
```

**Fix**: All alternatives must be coercible to the return type.

### Constraint Violation

```mellis
fn bad(a: &i32, b: &i32) -> &i32
    life_from(a)
    where outlives(a, b)
// When called with arguments where a doesn't outlive b...
```

This is caught at call site with:

```
error: argument 1 does not satisfy lifetime constraint: 'a must outlive 'b
```

> **Note:** The `'a`, `'b` in error messages refer to the parameter names, not explicit lifetime annotations.

### Return Provenance Mismatch

```mellis
fn bad(x: &i32) -> &i32 life_from(x) {
    let y = 5;
    return &y;  // Error: return does not have provenance 'x'
}
```

**Fix**: Return a reference with the correct provenance.

---

## Best Practices

### 1. Keep Signatures Simple

```mellis
// Good: Clear intent
fn get_data(data: &Buffer) -> &u8 life_from(data)

// Avoid: Overly complex
fn get_data(data: &Buffer, temp: &Buffer, cache: &Cache) -> &u8
    life_from(data | temp | cache)
    where outlives(data, temp)
    where outlives(data, cache)
```

### 2. Use Constraints Judiciously

Only add `where outlives` when there's a semantic requirement, not just to be explicit.

```mellis
// Good: Actual semantic requirement
fn cache_key(key: &str, cache: &Cache) -> &str
    life_from(key)
    where outlives(key, cache)

// Unnecessary: Both parameters have same lifetime anyway
fn pair(a: &i32, b: &i32) -> &i32
    life_from(a)
    where outlives(a, b)
```

### 3. Match Return Patterns

When returning conditionally, ensure all branches return from the declared provenance:

```mellis
// Good
fn example(a: &i32, b: &i32) -> &i32 life_from(a | b) {
    if condition {
        return a;  // OK: a is in provenance set
    }
    return b;     // OK: b is in provenance set
}

// Compile error
fn bad(a: &i32, b: &i32) -> &i32 life_from(a) {  // Only a in provenance
    return b;  // Error: b is not in provenance set
}
```

### 4. Prefer Union When Flexible

```mellis
// Good: Function can return either input
fn either(a: &i32, b: &i32) -> &i32 life_from(a | b)

// When you only return one specific input
fn first_only(a: &i32, _b: &i32) -> &i32 life_from(a)
```

---

## Appendix: Lifetime Lattice

Internally, lifetimes form a mathematical **lattice**:

```
        static (top)
           |
           |  a ≤ b means region(a) ⊆ region(b)
           |
    ┌──────┴──────┐
    │              │
   a              b
    │              │
    └──────┬──────┘
           |
        ⊥ (bottom)
```

- **static**: Lives for the entire program
- **`a ≤ b`**: Lifetime `a` is contained within `b` (a is shorter or equal)
- **⊥**: Abstract empty lifetime (internal only)

The solver uses constraint propagation to find valid lifetime assignments, checking for contradictions.

---

## See Also

- [Language Reference](../LanguageReference.md) — Full language specification
- [Borrow Checker](../borrowck/) — Ownership and move semantics
- [Memory Model](../docs/MemoryModel.md) — How Mellis manages memory safely
