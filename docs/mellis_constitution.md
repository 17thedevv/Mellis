# Mellis Constitution (Hiến pháp Mellis)

Mellis is a **Large-Scale Systems Language**, designed for architectures spanning hundreds of thousands to millions of lines of code. It does not aim to replace Rust or optimize for quick scripting (Luna handles that), but rather to combine the philosophies of three distinct languages into a unique reason to exist.

## The Three Pillars + The Missing Fourth Pillar

### 1. Rust: Memory Safety (Ownership)
- **Philosophy**: Memory safety as a compiler invariant.
- **Core Mechanism**: Ownership, Move Semantics, and Borrow Checking.
- **Guarantee**: No use-after-free, no double-free, no data races.
- **Implementation Layer**: Enforced deeply through the parser, resolver, typechecker, and finally the borrow checker.

### 2. TypeScript: Strong Type System
- **Philosophy**: Extreme expressiveness for modeling complex architectures at compile-time.
- **Core Mechanism**: Deep Type Inference, Associated Types (`type Item`), and Generic Constraints (`fn sort<T: Ord>`).
- **What Mellis Keeps**: Nominal typing (strict and unambiguous).
- **What Mellis Drops**: Structural typing, `any`, dynamic widening, and implicit optionals. Architecture demands explicit boundaries.

### 3. Ada: The Package System & Contract Programming
- **Philosophy**: Designing for systems that live for decades.
- **Core Mechanism (Packages)**: Hard separation of interface and implementation.
  - `math.msi` (Interface)
  - `math.ms` (Implementation)
- **Core Mechanism (Contracts)**: Enforcing behavior via `requires` and `ensures`.
- **Guarantee**: The compiler can resolve, type-check, and distribute interfaces without needing to parse the underlying implementation, perfectly complementing the `.mlib` system.

### 4. Compiler-Visible Architecture (The Missing Pillar)
- **Philosophy**: Architecture is a first-class citizen, not merely a convention.
- **Core Mechanism**: The compiler understands the dependency graph, visibility (e.g., `export`, `internal`), ABI boundaries, and module layouts natively.
- **Integration**: `.mlib` isn't just an output file; it's a protocol—a contract between the compiler and the entire ecosystem (IDE, Package Manager, Profiler, Debugger).

---

## 4 Architectural Invariants (Bốn nguyên tắc bất khả xâm phạm)

1. **Memory Safety by Ownership**: No undefined behavior due to memory mismanagement.
2. **Type Safety by Design**: No `any` escapes; every abstraction is verified at compile-time.
3. **Architecture as a First-class Citizen**: Packages, modules, visibility, and ABIs are integral parts of the language.
4. **Compiler Boundaries Never Leak**: 
   - AST dies before MVIR.
   - Macros die before MVIR.
   - `.mlib` serves as the universal protocol for the entire ecosystem.
