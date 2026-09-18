---
name: luna-stdlib-architecture
description: Defines the canonical architecture for Luna's standard library source and artifact layout (STD-ARCH-01).
---

# Luna Stdlib Architecture

## Purpose

This skill defines the canonical architecture for Luna's standard library source/artifact layout.

This is an architectural refactor, not a new language feature.

The goal is to eliminate monolithic stdlib provider files such as `core.ln` and `alloc.ln` while preserving Luna's frozen provider/module semantics and existing public behavior.

---

## 1. Frozen Semantic Principle

A provider is NOT a namespace.

The provider filename, provider logical name, or provider artifact path MUST NOT automatically create a source namespace.

Examples:

```text
import <core>;

does NOT imply:

core::Option
core::Iterator

Likewise:

import <alloc>;

does NOT imply:

alloc::Vec
alloc::String

Namespaces exist only when explicitly declared with:

module std {
    ...
}

Multiple providers MAY contribute declarations to the same explicit logical module.

Therefore:

alloc/vec.ln
alloc/string.ln
alloc/hashmap.ln

may all contribute to:

module std {
    ...
}

and consumers use:

std::Vec
std::String
std::HashMap

rather than provider-derived namespaces.

## 2. Canonical Source Layout

The intended stdlib structure is:

sysroot/
├── lang/
│   ├── drop.ln
│   ├── option.ln
│   ├── iterator.ln
│   └── into_iterator.ln
│
├── core/
│   ├── iter_adapters.ln
│   ├── iter_consumers.ln
│   └── ...
│
├── alloc/
│   ├── vec.ln
│   ├── box.ln
│   ├── string.ln
│   ├── raw_table.ln
│   ├── hashmap.ln
│   └── hashset.ln
│
└── io/
    └── io.ln

Corresponding artifacts SHOULD follow the same logical organization:

sysroot/
├── lang/
│   ├── drop.llib
│   ├── option.llib
│   ├── iterator.llib
│   └── into_iterator.llib
│
├── core/
│   ├── iter_adapters.llib
│   └── iter_consumers.llib
│
├── alloc/
│   ├── vec.llib
│   ├── box.llib
│   ├── string.llib
│   ├── raw_table.llib
│   ├── hashmap.llib
│   └── hashset.llib
│
└── io/
    └── io.llib

The exact physical root directory MAY differ according to the existing sysroot/build layout, but the logical decomposition MUST remain equivalent.

## 3. Component Ownership

### lang/

Contains language-level foundational contracts required by the compiler/language.

Current foundational contracts:

Drop
Option
Iterator
IntoIterator

These are NOT ordinary convenience utilities.

They participate directly in language semantics.

Examples:

for lowering depends on IntoIterator
iterator protocol depends on Iterator
Option::Some / Option::None represent language-level iteration results
Drop participates in ownership/drop semantics

Do NOT move ordinary convenience APIs into lang/.

### core/

Contains generic foundational library functionality that does not define compiler language contracts.

Examples:

iterator adapters
iterator terminal consumers
generic utility APIs

Core functionality is NOT automatically visible merely because it is in core/.

### alloc/

Contains heap-backed ownership abstractions and allocation-related containers.

Examples:

Vec
Box
String
RawTable
HashMap
HashSet

These remain ordinary Luna library code.

No container becomes compiler-built-in merely because it lives under alloc/.

### io/

Contains I/O facilities such as:

print
println
eprintln

I/O is NOT a language contract and MUST NOT be auto-visible as foundational language infrastructure.

## 4. Logical std Module

User-facing standard-library components should normally contribute to the explicit logical module:

module std {
    ...
}

Example:

alloc/vec.ln:

module std {
    struct Vec<T> {
        ...
    }
}

alloc/string.ln:

module std {
    struct String {
        ...
    }
}

A consumer therefore sees:

std::Vec
std::String

not:

vec::Vec
string::String
alloc::Vec
alloc::String

unless a separate explicit module declaration establishes such namespaces.

## 5. Multi-provider Module Merge

Multiple providers MAY contribute to the same explicit module.

For example:

alloc/vec.ln
alloc/string.ln
alloc/hashmap.ln

may all provide declarations inside:

module std

This is intentional.

The resolver MUST merge declarations according to the existing frozen provider/module semantics.

The implementation MUST NOT assume:

provider filename == namespace

## 6. Collision Rules

The existing resolver semantics remain authoritative.

Different providers exporting the same root symbol MUST continue to produce duplicate-definition diagnostics.

Likewise, collisions inside an explicit logical module MUST remain deterministic.

The architecture refactor MUST NOT silently alter symbol collision rules.

## 7. No Semantic Refactor During Layout Refactor

STD-ARCH-01 MUST NOT simultaneously:

redesign ownership semantics
redesign borrowing
redesign lifetime contracts
add const generics
redesign trait coherence
introduce compiler-native Vec/String/HashMap knowledge
alter frozen runtime ABI
alter provider/module semantics
change language syntax

This phase changes organization and bootstrap/loading architecture.

Feature work belongs to separate phases.

## 8. No Compiler Knowledge of Containers

Moving:

Vec
String
HashMap
HashSet
Box

between files MUST NOT cause new special cases in:

parser
resolver
type checker
borrow checker
monomorphization
MVIR
LLVM backend
reflection
coherence

Existing generic mechanisms remain the implementation mechanism.

## 9. Artifact Migration

The refactor must preserve source/artifact parity.

For every migrated component:

component.ln
component.llib

must represent the same logical provider/component.

Artifacts MUST be regenerated through the official compiler/build path.

Do not manually patch .llib binaries.

Do not use stale artifacts to claim migration success.

## 10. Import Semantics Must Remain Explicit

Ordinary core/alloc/io functionality remains explicitly imported.

Example:

import <core>;
import <alloc>;
import <io>;

The fact that files now exist under:

core/
alloc/
io/

does not itself make their declarations visible.

## 11. What This Refactor Is Solving

The previous architecture allowed large monolithic providers:

core.ln
alloc.ln

which creates poor component boundaries and makes independent artifact rebuilding difficult.

The target architecture provides:

component-level source ownership
component-level .llib artifacts
explicit logical module composition
cleaner bootstrap boundaries
smaller semantic units
easier artifact freshness tracking
less coupling between unrelated stdlib components

## 12. Required Validation

A valid implementation MUST verify:

every migrated component still parses
every migrated component still typechecks
source and .llib providers remain semantically equivalent
explicit std module contributions merge correctly
provider names do not create namespaces
unrelated core/alloc/io symbols are not accidentally auto-visible
language contracts remain available according to the language-contract architecture
existing stdlib behavior remains unchanged
existing ownership/lifetime/borrow behavior remains unchanged
old monolithic artifacts are not accidentally selected as the canonical representation

## 13. Stop Conditions

STOP instead of improvising if:

the current resolver cannot support multiple providers contributing to one explicit module
language-contract auto-loading requires a new semantic rule
source/.llib parity cannot be preserved
a migration appears to require container-specific compiler logic
an existing frozen semantic rule conflicts with the proposed layout

This skill defines architecture, not permission to redesign unrelated compiler semantics.
