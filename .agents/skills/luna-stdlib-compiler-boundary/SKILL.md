---
name: luna-stdlib-compiler-boundary
description: Architectural boundary protocol between Luna standard library and the compiler. Enforces that stdlib is a consumer of the language, not an extension of the compiler's type system, prohibiting type-specific compiler branches, opcodes, or magic.
---

# Luna Stdlib–Compiler Boundary

## Purpose

This skill defines the architectural boundary between Luna standard-library implementations and the Luna compiler.

The standard library is a **consumer of the language**, not an extension of the compiler's type system.

Stdlib implementations MUST be expressible using existing Luna language semantics, generic ownership/borrowing/lifetime rules, traits, modules, intrinsics, runtime ABI, and standard library primitives.

The compiler MUST NOT acquire type-specific or library-specific semantic knowledge merely to make a stdlib component easier to implement.

---

## 1. Core Rule

> **Stdlib is separate from the compiler, except for explicitly defined language intrinsics and `lang_item` contracts.**

A standard-library type such as:

```text
Box<T>
Vec<T>
String
HashMap<K,V>
HashSet<T>
Rc<T>
Arc<T>
Mutex<T>
```

MUST remain an ordinary library abstraction unless the language specification explicitly freezes a new semantic contract.

The compiler MUST NOT recognize a type merely because its name is:

```text
"Box"
"Vec"
"String"
"HashMap"
"HashSet"
...
```

---

## 2. Allowed Compiler Knowledge

Compiler-level coupling is permitted only for two controlled categories.

### 2.1 Intrinsics

An intrinsic is a deliberately specified compiler/runtime primitive whose semantics cannot be expressed purely in Luna source.

Examples:

```text
ptr::read
ptr::write
ptr::drop_in_place
mem::size_of
mem::align_of
```

Intrinsics MUST be:

* explicitly declared by the language specification;
* generic where possible;
* independent of concrete library container names;
* documented with precise safety and ownership semantics;
* usable by multiple independent library abstractions.

Adding an intrinsic solely for one stdlib type is prohibited.

Bad:

```text
BoxFree
StringLengthIntrinsic
HashMapProbeIntrinsic
```

Good:

```text
HeapFree
drop_in_place<T>
size_of<T>
```

---

### 2.2 `lang_item`

A `lang_item` identifies a language-defined semantic contract that the compiler must recognize.

Examples include foundational traits/contracts such as:

```text
Drop
Copy
Try
FromResidual
```

A `lang_item` represents a **language-level concept**, not a specific stdlib implementation.

The compiler MAY recognize:

```text
Drop
```

because Drop is part of Luna's semantic model.

The compiler MUST NOT recognize:

```text
String
Vec
HashMap
Box
```

as language items merely because they are important stdlib types.

---

## 3. Generic Compiler Fix Rule

Stdlib development frequently exposes compiler defects.

When this happens, classify the issue before modifying the compiler.

### Valid

```text
String implementation
    ↓
TypeAlias bug discovered
    ↓
Fix generic TypeAlias machinery
```

```text
HashMap implementation
    ↓
GenericParam trait dispatch missing
    ↓
Fix generic trait dispatch
```

```text
Vec implementation
    ↓
Drop impl not rooted by monomorphization
    ↓
Fix generic Drop rooting
```

These fixes MUST benefit arbitrary user-defined types as well.

### Invalid

```text
String implementation
    ↓
add if type == String
```

```text
HashMap implementation
    ↓
add HashMap-specific compiler branch
```

```text
Vec implementation
    ↓
add Vec-specific MIR opcode
```

Such changes violate this skill.

---

## 4. Capability-Gap Protocol

When a stdlib implementation encounters a compiler limitation:

```text
Stdlib implementation
        ↓
Capability gap discovered
        ↓
Is the limitation generic?
        │
   ┌────┴────┐
   │         │
  YES        NO
   │         │
   ▼         ▼
Generic   Architecture
compiler  review required
fix       before proceeding
   │
   ▼
Add generic regression tests
   │
   ▼
Return to stdlib implementation
```

A compiler change MUST NOT be merged into a stdlib phase without identifying the generic language capability it fixes.

---

## 5. No Stdlib-Specific IR

The compiler MUST NOT introduce IR instructions or backend operations for a particular stdlib container.

Forbidden examples:

```text
BoxFree
BoxNew
VecGrow
StringFree
HashMapProbe
HashSetInsert
```

Preferred generic primitives:

```text
HeapFree
Drop
Load
Store
Call
Allocation
pointer/memory primitives
```

If a proposed operation is useful only for one stdlib type, architecture review is mandatory.

---

## 6. No Stdlib-Specific Type System Cases

Semantic subsystems MUST NOT contain branches such as:

```rust
if is_box_type(...) { ... }
if is_string_type(...) { ... }
if is_hash_map_type(...) { ... }
```

This prohibition applies to:

```text
TypeChecker
Resolver
Borrowck
Lifetime analysis
Monomorphization
Coherence
Reflection
MVIR generation
LLVM backend
Llib serialization
```

unless the type has an explicitly frozen language-level semantic contract.

---

## 7. Stdlib Representation Is Library-Owned

Internal representation belongs to the stdlib implementation.

For example:

```text
Box<T>
    private ptr

Vec<T>
    private ptr
    private len
    private cap

String
    private vec

HashMap<K,V>
    private table
```

The compiler MUST NOT depend on these layouts merely because they belong to stdlib types.

Layout-sensitive semantics MUST be expressed through generic language mechanisms or explicitly specified intrinsics.

---

## 8. Runtime ABI Boundary

Runtime ABI functions may be exposed through stdlib wrappers:

```text
__luna_alloc
__luna_dealloc
__luna_print
__luna_thread_*
...
```

The runtime ABI itself is not a justification for compiler special-casing the library abstraction using it.

Preferred architecture:

```text
Luna stdlib API
        ↓
safe library wrapper
        ↓
unsafe intrinsic / extern ABI boundary
        ↓
runtime
```

not:

```text
Compiler recognizes stdlib type
        ↓
special backend lowering
```

---

## 9. Source/.llib Parity

A stdlib feature MUST preserve equivalent semantics across:

```text
source .ln
```

and:

```text
precompiled .llib
```

Compiler infrastructure may differ internally during compilation, but the semantic contract and exported behavior MUST remain identical.

A stdlib feature MUST NOT rely on source-only compiler magic that disappears at the `.llib` boundary.

---

## 10. Ownership Abstraction Compatibility

This skill is subordinate to the `BOX-FROZEN` Ownership Abstraction Guard.

Future ownership containers:

```text
Rc<T>
Arc<T>
custom smart pointers
allocator-backed containers
```

MUST reuse generic:

```text
Struct
Move
Drop
Borrow
Lifetime
ptr/mem
HeapFree
```

rather than introducing compiler-recognized container semantics.

---

## 11. Exception Test

Before accepting a compiler change made during a stdlib phase, ask:

> **Would the same compiler change improve or enable an unrelated user-defined type?**

If YES:

```text
Likely generic language machinery.
```

If NO:

```text
Likely stdlib-specific coupling.
```

Stdlib-specific coupling requires explicit language-design justification and a new frozen semantic contract.

---

## 12. Mandatory Reporting Format

Every stdlib walkthrough that changes compiler code MUST classify each compiler modification as:

```text
Compiler Change
    Capability:
    Why stdlib exposed it:
    Why it is generic:
    User-defined type benefiting:
    Tests:
    New intrinsic/lang_item?:
    Stdlib-specific branch?: NO
```

Every new intrinsic or `lang_item` MUST additionally document:

```text
Semantic contract
Safety contract
Generic applicability
ABI implications
Source/.llib behavior
Why existing primitives are insufficient
```

---

## 13. Architectural SSOT

The following documents/rules govern this boundary:

1. Frozen Luna language specification
2. Luna Semantic Core
3. Luna Lifetime Relation Model
4. Ownership Abstraction Guard (`BOX-FROZEN`)
5. Intrinsic contracts
6. `lang_item` contracts
7. This Stdlib–Compiler Boundary skill

When a stdlib implementation conflicts with a frozen semantic contract, the implementation is wrong unless a new language RFC explicitly changes the contract.

---

## Final Invariant

> **The standard library is built with the language, not built into the language.**

The only intentional compiler-facing escape hatches are:

```text
Intrinsic
LangItem
```

and both are language-level contracts, not shortcuts for individual stdlib implementations.
