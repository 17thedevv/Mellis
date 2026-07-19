# freedomLanguage Vision

> A modern systems programming language focused on compiler and tooling innovation.

---

# Why fdlang Exists

Modern systems languages have largely solved runtime performance and memory safety.

However, large software projects—such as game engines, AI systems, and backend infrastructure—still suffer from slow build times, complex toolchains, and inefficient development workflows.

fdlang exists to improve the development experience of large-scale native software without sacrificing performance or safety.

---

# Mission

Build a systems programming language that provides:

- Native performance
- Memory safety
- Explicit control
- Excellent compiler tooling
- Fast iteration for large projects

The compiler is not merely a translator.

The compiler is a core part of the language.

---

# Core Principles

## Performance

Programs written in fdlang should be capable of achieving performance comparable to modern C++ and Rust.

Runtime performance is never traded for convenience unless explicitly requested by the programmer.

---

## Safety

Memory safety is the default.

Unsafe operations must always be explicit.

Undefined behavior should be minimized wherever possible.

---

## Explicitness

Code should clearly express ownership, intent and cost.

The compiler should avoid hidden behavior whenever practical.

---

## Zero-Cost Abstractions

High-level abstractions should compile down to code with no unnecessary runtime overhead.

---

## Tooling First

The compiler, build system and developer tooling are first-class citizens.

Fast incremental builds, diagnostics and IDE support are part of the language experience.

---

# What fdlang Intentionally Inherits

fdlang does not attempt to redesign concepts that have already been proven.

It intentionally adopts:

- Ownership
- Borrow Checking
- Move Semantics
- Safe / Unsafe boundary
- LLVM backend
- Native compilation

These technologies provide a strong and well-understood foundation.

---

# What fdlang Wants to Innovate

fdlang focuses its innovation on compiler architecture rather than memory management.

Key areas include:

- FLIR
- .flo module format
- Incremental compilation
- Module caching
- Hot reload
- Compiler tooling
- Fast iteration
- Build pipeline

---

# The Compiler Philosophy

The compiler is designed as a collection of independent passes.

Source
    ↓
Lexer
    ↓
Parser
    ↓
Resolver
    ↓
Type Checker
    ↓
Borrow Checker
    ↓
FLIR
    ↓
.flo
    ↓
Backend
    ↓
Executable

Every stage should have a single responsibility.

---

# Libraries Over Language Features

Whenever possible, features belong in libraries instead of the language itself.

Example:

Game-specific concepts such as:

- Entity
- Component
- MonoBehaviour

should never become language keywords.

The language must remain general-purpose.

---

# Decision Filter

Before introducing a language feature, ask:

"If the game engine disappeared tomorrow, would this feature still be valuable?"

If yes:

→ It belongs in fdlang.

If no:

→ It belongs in a library, framework or engine.

---

# Trade-offs

fdlang willingly accepts:

- More compiler complexity
- Longer initial compilation
- A larger toolchain

In exchange for:

- Better runtime performance
- Memory safety
- Faster iteration on large projects
- Better scalability

---

# Non-goals

fdlang is not designed to:

- Replace every programming language.
- Be the simplest language.
- Hide memory management.
- Include engine-specific features.
- Sacrifice performance for convenience.

---

# Long-term Vision

The long-term goal of fdlang is to become a complete ecosystem for native software development.

The language, compiler, module system and tooling should work together to make building large software projects significantly more productive while maintaining the performance expected from a modern systems language.