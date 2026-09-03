# Phase 15: async/await — Implementation Plan

> **Redesign Phase 15 — Mellis Async/Await**

Implement Phase 15 only after auditing the existing Mellis compiler, especially:
- ownership / move / Drop
- borrow checker + NLL + provenance
- closures
- generics / traits
- `?`
- MVIR
- LLVM backend
- module / MLib system

IMPORTANT:
Do NOT translate Rust's async model directly into Mellis.
Do NOT make `Pin`, `Context`, `Waker`, `dyn Future`, or a specific executor mandatory language concepts.
Design async around Mellis's existing ownership/provenance model.

## Core Philosophy

Mellis async is a language-level suspend/resume mechanism.

`async`:
    creates a compiler-generated resumable state machine.

`await`:
    creates a suspension boundary in that state machine.

Future/executor/scheduler/waker:
    library/runtime abstractions unless the compiler genuinely needs a minimal ABI.

The user-facing model must remain simple even if the compiler internally needs stronger invariants.

## Problems this design MUST avoid

1. Runtime lock-in / ecosystem fragmentation
   - Compiler must not depend on Tokio-style or any specific executor.
   - Generated async state machines must be consumable by multiple runtimes.

2. Borrowing across `await`
   - Existing Mellis provenance/NLL must remain the source of truth.
   - A borrow that crosses suspension must be represented as part of the suspended state.
   - Reject any state transition that can make the provenance invalid.
   - Do not introduce a second lifetime/borrow system just for async.

3. Self-referential state-machine unsoundness
   - Do not assume moving a generated future is always safe.
   - Audit whether generated state can contain references to its own fields.
   - Prefer layouts/transformations that avoid self-referential pointers where possible.
   - If address stability is required, hide it in compiler/runtime representation rather than exposing Rust-style `Pin` syntax to users.
   - Document the exact invariant that guarantees references remain valid.

4. Cancellation / destruction
   - Treat dropping/destroying an incomplete async computation as a first-class semantic case.
   - Every live owned value must be dropped exactly once when an async state machine is destroyed/cancelled.
   - Borrows held by the state machine must be released correctly.
   - Partial initialization across suspension must not cause double-drop or leak.

5. Async + `?`
   - `?` keeps its existing Mellis semantics.
   - On early error return, all active state-machine-owned locals must be cleaned up before completion.
   - Verify borrow provenance through `await` + `?`.

6. Async closures / captures
   - The existing closure capture modes (`SharedBorrow`, `MutableBorrow`, `Move`) must compose with async.
   - Do not invent a separate capture/lifetime system for async closures.
   - A future produced by an async closure must correctly carry the closure environment/provenance.

7. Async traits / dynamic dispatch
   - Basic async/await must NOT depend on `dyn Trait`.
   - Keep async trait objects as an advanced layer after dynamic dispatch infrastructure exists.

## Target semantic model

Example:

```mellis
async fn load() -> Result<Data, Error> {
    dec a = await read_a()?;
    dec b = await read_b()?;
    return Ok(combine(a, b));
}
```

Conceptually compile to:

```
AsyncStateMachine {
    state,
    locals that must survive suspension,
}
```

The state machine must explicitly represent:
- current state
- initialized/live locals
- owned values
- suspended borrows/provenance
- awaited computation state

Do not expose these implementation details to the programmer.

## Async return type

Do NOT automatically hardcode a built-in runtime `Future` type into the language.

First determine whether Mellis should use:
- an opaque compiler-generated future type,
- a library protocol implemented by generated futures,
- or another minimal abstraction.

Choose the smallest model that preserves:
- composability
- executor independence
- static typing
- ownership safety

Document the decision before implementation.

## `await`

`await expr` must:
1. evaluate `expr`
2. determine whether it is awaitable through the chosen protocol
3. if incomplete, suspend the current state machine
4. preserve all required locals/borrows
5. resume later with the resulting value

Do not add a high-level `Instruction::Await` unless it is necessary.
Prefer lowering into existing MVIR control-flow/state primitives.

## Suspension analysis

Before lowering an async function:
1. Build control-flow/liveness information.
2. Identify every `await`.
3. Determine which locals are live across each suspension point.
4. Determine which owned values must be stored in the async environment.
5. Determine which borrows/provenance relationships cross the boundary.
6. Reject unsafe escaping/internal-reference configurations.

This analysis must integrate with existing NLL/provenance instead of duplicating it.

## State machine representation

Prefer a representation conceptually like:

```
StateMachine {
    state: integer/tag,
    fields for values live across suspension,
}
```

Generated resume/poll logic:
state 0 -> execute until await
state 1 -> resume after await
state 2 -> continue
...
final state -> completed

The exact runtime entrypoint is implementation-defined.

Do NOT copy Rust's `poll(self: Pin<&mut Self>, cx: &mut Context)` unless the audit proves Mellis actually needs an equivalent contract.

## Cancellation / Drop model

Define explicitly:

- What happens when an incomplete async value is destroyed?
- What fields are initialized in each state?
- How are initialized fields dropped?
- How are borrows released?
- Can cancellation occur at every suspension point?
- Is cancellation guaranteed to run deterministic cleanup?

The generated state machine must behave like a normal Mellis owned value:
- move rules apply
- Drop rules apply
- partial initialization rules apply
- borrow rules apply

## Async + borrow examples that MUST be verified

VALID where sound:

```mellis
async fn f() {
    dec x = ...;
    await g();
    use(x);
}
```

VALID where the reference safely survives:

```mellis
async fn f() {
    dec x = ...;
    dec r = &x;
    await g();
    use(r);
}
```

INVALID when mutation conflicts with a live borrow across await:

```mellis
async fn f() {
    dec x = ...;
    dec r = &x;
    await g();
    x = ...;
}
```

Also test `&rw`:

```mellis
async fn f() {
    dec rw x = ...;
    dec r = &rw x;
    await g();
    use(r);
}
```

Reject overlapping mutable access according to existing Mellis rules.

## Async + Drop

Test:
- owned local before await
- owned local after await
- multiple locals across multiple suspension points
- early `?`
- explicit return
- cancellation/destruction before completion
- partial move before await
- partial move after await

Prove exactly-once destruction.

## Async + closures

Test:
- non-capturing async closure
- shared capture
- mutable capture
- move capture
- closure/future escape
- closure dropped before completion
- closure future moved
- provenance across await

## Async + loops and match

Test:
- await inside while
- await inside for
- await inside match
- nested await
- multiple await points
- break/continue around await

## Async + generics

Test:
- generic async function
- generic awaited value
- generic Result
- generic Drop
- generic state-machine fields
- cross-module generic async function

## Runtime independence

The compiler must generate a stable semantic/ABI boundary that different runtimes can drive.

Do not add a mandatory global executor to the language.

A minimal library/runtime should be enough to demonstrate:
- create async computation
- resume it
- detect completion
- retrieve result

More advanced scheduling/waker systems should live outside the language core.

## Compiler architecture

Prefer:

AST
→ async semantic analysis
→ suspension/liveness analysis
→ state-machine lowering
→ normal ownership/borrow verification
→ MVIR
→ LLVM

Not:

AST
→ async special-case codegen
→ LLVM magic

The borrow checker must remain responsible for safety.

## Suggested implementation order

15A — Semantic design audit
- finalize awaitable protocol
- finalize generated future/state-machine type
- finalize move/borrow/drop semantics
- finalize cancellation semantics

15B — Suspension analysis
- CFG
- liveness across await
- locals crossing suspension
- provenance across suspension

15C — State-machine lowering
- state representation
- transitions
- resume paths
- cleanup paths

15D — Ownership / Drop integration
- move
- partial move
- Drop
- cancellation cleanup
- NLL

15E — Runtime/library protocol
- minimal runtime interface
- no mandatory executor

15F — LLVM/backend
- layout
- resume ABI
- function pointers if needed
- no async semantics in LLVM

15G — Integration tests

## Mandatory tests

- `async_basic.ms`
- `async_multiple_await.ms`
- `async_local_across_await.ms`
- `async_borrow_across_await.ms`
- `async_mut_borrow_across_await.ms`
- `async_move_across_await.ms`
- `async_partial_move_across_await.ms`
- `async_drop_across_await.ms`
- `async_cancel_cleanup.ms`
- `async_loop.ms`
- `async_match.ms`
- `async_try.ms`
- `async_generic.ms`
- `async_closure.ms`
- `async_closure_borrow.ms`
- `async_closure_move.ms`

## Completion criteria

Do not mark Phase 15 complete merely because:
`async fn foo() -> 42` compiles.

Phase 15 is complete only when:

- async/await works end-to-end
- suspension/resume semantics are specified
- no async-specific unsound ownership model exists
- borrows across await are sound
- `&rw` uniqueness remains sound
- move/partial-move works across suspension
- Drop is exactly-once on completion and cancellation
- `?` cleanup remains correct
- generated state machines do not create dangling self-references
- runtime is executor-independent
- generic/cross-module cases work
- positive and negative tests pass
- `cargo test --workspace` passes
- unsupported edge cases have diagnostics
- final audit documents every remaining limitation
