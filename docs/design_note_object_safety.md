# Design Note: Object Safety and Lifetime Parameters

## Context
In Mellis, trait objects (`dyn Trait`) are used to achieve dynamic dispatch. For a trait to be converted into a trait object, it must be "object-safe". The object-safety rules are designed to ensure that all methods in the trait can be dispatched through a vtable. 

One of the strict rules for object safety is that **trait methods cannot have generic type parameters**. This is because generic methods are monomorphized (copied and specialized) for each type they are called with, which means there is no single function pointer that can be placed in a fixed-size vtable.

However, Mellis also supports `&self` methods, which implicitly introduce a **lifetime parameter**. Initially, the presence of any generic parameter (including implicit lifetimes) caused the trait to be flagged as non-object-safe.

## Decision
We decided to **exclude lifetime parameters from the object-safety checks**. 

Lifetime parameters do not generate new monomorphized copies of a function in the compiled binary. They are strictly a compile-time construct used for borrow checking and do not affect the binary layout, the size of the trait object, or the structure of the vtable. Therefore, a method with generic lifetime parameters (like `&self`) can still be safely dispatched dynamically.

## Alternatives
1. **Require explicit lifetimes for object-safe traits**: This would add unnecessary boilerplate and make the language less ergonomic, forcing developers to write verbose code for simple dynamic dispatch scenarios.
2. **Keep the strict check and ban `&self` in trait objects**: This is entirely unviable as it would prevent passing references to trait objects, breaking almost all practical uses of dynamic dispatch.

## Trade-offs
- **Complexity**: The compiler's object-safety check must now explicitly filter out `GenericParamKind::Lifetime` when inspecting a method's generic parameters. This adds a slight bit of logic to the semantic analysis phase.
- **Ergonomics**: This decision vastly improves ergonomics. Users can write natural `&self` and `&mut self` methods in their traits and use them as trait objects without unexpected `E-OBJECT-SAFE` errors.

## Status
**Implemented**. 

The `ObjectSafety::isObjectSafe` function has been updated to iterate over `method->genericParams` and ignore any parameter where `kind == GenericParamKind::Lifetime`. 

### Verification
- `&self` with inferred lifetime -> **PASS**
- `fn foo<T>(&self, x: T)` -> **FAIL** (Contains a type-level generic parameter)
- `fn foo(self)` -> **FAIL** (Passes `self` by value, violating another object-safety rule)

This ensures that only true type-level generic parameters trigger the object-safety violation, while lifetimes are correctly permitted.
