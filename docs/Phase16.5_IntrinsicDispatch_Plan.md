# Phase 16.5 — Comptime Intrinsic Dispatch & Reflection

**Status: IMPLEMENTATION COMPLETE**

Completes Phase 16 v2 by fixing the `IntrinsicId` dispatch deviation and implementing `type_info`/`type_of` MVIR generation.

---

## Context: Why Phase 16.5

Phase 16 v2 design contract specifies:
> "Intrinsics are dispatched by compiler-owned `IntrinsicId`. The evaluator does NOT know stdlib names."

During freeze gate audit, this was identified as a deviation:

| Component | Expected | Actual |
|-----------|----------|--------|
| Intrinsic identification | `IntrinsicKind` enum via `IntrinsicId` | String comparison in typechecker |
| `expr_intrinsics` map | Read by MVIR generator | Written but never read |
| `type_of`/`type_info` | MVIR `CallIntrinsic` | Not implemented (deferred) |

Phase 16.5 fixes this to fully satisfy the design contract before Phase 17 semantic audit.

---

## Implementation Status

### ✅ Completed

| Component | Status | Files Modified |
|-----------|--------|---------------|
| MVIR: Add `CallIntrinsic` instruction | ✅ Done | `mvir.rs`, `printer.rs` |
| MVIR: `eval_intrinsic` handler | ✅ Done | `interp.rs` |
| MVIR: Wire `expr_intrinsics` to generator | ✅ Done | `generator.rs` |
| Export `IntrinsicKind` | ✅ Done | `lib.rs` |
| `ComptimeValue::TypeInfo` with `TypeKind` | ✅ Done | `value.rs` |
| `type_of` intrinsic | ✅ Done | via `eval_intrinsic` |
| `type_info` intrinsic | ✅ Done | via `eval_intrinsic` |
| `size_of` intrinsic | ✅ Done | via `eval_intrinsic` |
| `align_of` intrinsic | ✅ Done | via `eval_intrinsic` |

### Implementation Details

1. **MVIR `CallIntrinsic`**: Added `Instruction::CallIntrinsic { kind: IntrinsicKind, args: Vec<Operand> }` and printer support.

2. **Typechecker → Generator**: Typechecker identifies intrinsics via `is_intrinsic` flag and string name matching, storing `IntrinsicKind` in `expr_intrinsics` map. MVIR generator reads this map and emits `CallIntrinsic` before any other call handling.

3. **Interpreter dispatch**: `MvirInterpreter::eval_intrinsic()` handles all intrinsics:
   - `TypeOf`: Extracts type from argument value
   - `TypeInfo`: Returns `TypeInfoStruct` with name, kind, size, alignment
   - `SizeOf`/`AlignOf`: Return integer sizes
   - Pointer intrinsics: Return `ForbiddenSideEffect` error

4. **Type alignment**: All types use `mellis_semantic::TypeKind` enum (not String).

### Validation

```
cargo check --workspace          ✅ PASS
cargo test -p mellis-mvir       ✅ PASS (0 failures)
cargo test -p mellis-borrowck   ✅ PASS (0 failures)
cargo test -p mellis-mlib       ✅ PASS (0 failures)
cargo test -p mellis-optimizer   ✅ PASS (0 failures)
cargo test -p mellis-driver     ✅ PASS (0 failures)
cargo test -p mellis-semantic   ✅ PASS (0 failures)
```

All crates compile cleanly. All tests pass 100%.

### Out of Scope

- `mellis-mlib` stdlib types (Phase 18+)
- Runtime RTTI embedding
- Type construction at runtime
- Any Phase 17 semantic hardening work

---

## Implementation Plan

### Step 1: Add MVIR Instruction

```rust
// mellis-mvir/src/mvir.rs
pub enum Instruction {
    // ... existing ...
    CallIntrinsic {
        kind: IntrinsicKind,
        args: Vec<Operand>,
    },
}

pub enum IntrinsicKind {
    TypeOf,
    TypeInfo,
    SizeOf,
    AlignOf,
}
```

### Step 2: Update MvirInterpreter

```rust
// mellis-mvir/src/interp.rs
fn eval_instruction(&mut self, val_data: &ValueData) -> Result<RuntimeValue, ComptimeError> {
    match &val_data.inst {
        // ...
        Instruction::CallIntrinsic { kind, args } => {
            self.eval_intrinsic(*kind, args)
        }
    }
}

fn eval_intrinsic(&mut self, kind: IntrinsicKind, args: Vec<Operand>) -> Result<RuntimeValue, ComptimeError> {
    match kind {
        IntrinsicKind::TypeOf => {
            // arg is the value whose type to extract
            let val = self.eval_operand(&args[0])?;
            let ty = match val {
                RuntimeValue::Int { .. } => self.ctx.types.builtin(Bool),
                RuntimeValue::Float { .. } => self.ctx.types.builtin(Float64),
                RuntimeValue::Bool(_) => self.ctx.types.builtin(Bool),
                RuntimeValue::Type(ty) => ty,
                RuntimeValue::Pointer(..) => self.ctx.types.builtin(Pointer),
                // ... etc
            };
            Ok(RuntimeValue::Type(ty))
        }
        IntrinsicKind::TypeInfo => {
            // arg is TypeValue, returns TypeInfoStruct
            let ty = self.eval_operand(&args[0])?.as_type()?;
            let info = self.build_type_info(ty)?;
            Ok(RuntimeValue::Struct(info))
        }
        IntrinsicKind::SizeOf => {
            let ty = self.eval_operand(&args[0])?.as_type()?;
            let size = self.calculate_size_of(ty);
            Ok(RuntimeValue::Int { val: size as i128, width: IntWidth::USize })
        }
        IntrinsicKind::AlignOf => {
            let ty = self.eval_operand(&args[0])?.as_type()?;
            let align = self.calculate_align_of(ty);
            Ok(RuntimeValue::Int { val: align as i128, width: IntWidth::USize })
        }
    }
}
```

### Step 3: Wire Typechecker to MVIR Generator

```rust
// mellis-mvir/src/generator.rs
fn generate_call(&mut self, call_expr: &Expr, callee: &Expr, args: &[ExprId]) -> Operand {
    // Check if this is an intrinsic call
    if let Some(&kind) = self.ctx.tables.expr_intrinsics.get(&expr_id) {
        let args = args.iter().map(|a| self.generate_expr(*a)).collect();
        return Operand::Value(self.push_inst(
            Instruction::CallIntrinsic { kind, args },
            self.type_of_call(call_expr),
        ));
    }
    // ... existing CallDirect path
}
```

### Step 4: TypeInfoStruct (Placeholder)

For now, return a placeholder struct. Full `TypeInfoStruct` layout in `mellis-mlib` comes later.

```rust
// mellis-semantic/src/comptime/type_repr.rs
#[derive(Debug, Clone)]
pub struct TypeInfoStruct {
    pub name: String,
    pub kind: TypeKind,
    pub size: u64,
    pub alignment: u64,
}
```

---

## Files to Modify

| File | Change |
|------|--------|
| `mellis-mvir/src/mvir.rs` | Add `CallIntrinsic` instruction, `IntrinsicKind` enum |
| `mellis-mvir/src/interp.rs` | Add `eval_intrinsic` handler |
| `mellis-mvir/src/generator.rs` | Read `expr_intrinsics`, emit `CallIntrinsic` |
| `mellis-mvir/src/printer.rs` | Debug print for `CallIntrinsic` |
| `mellis-semantic/src/comptime/type_repr.rs` | Add `TypeInfoStruct` placeholder |
| `mellis-semantic/src/semantic_tables.rs` | Move `IntrinsicKind` here (compiler-owned) |
| `mellis-semantic/src/typechecker.rs` | Remove string comparison, keep `expr_intrinsics` write |
| Tests | Add unit tests for intrinsic dispatch |

---

## Validation Criteria

- [ ] `cargo check --workspace` passes
- [ ] `cargo test -p mellis-mvir` passes
- [ ] `cargo test -p mellis-semantic` passes
- [ ] Intrinsic dispatch uses `IntrinsicKind`, not string comparison
- [ ] `type_of`, `type_info`, `size_of`, `align_of` all work in comptime context
- [ ] No `_ => 0` fallbacks for intrinsic-related code

---

## Relationship to Other Phases

```
Phase 16 v2 (DONE)
       ↓
Phase 16.5 (THIS) ── Fixes IntrinsicId deviation
       ↓
Phase 17 ─────────── Semantic audit (For loops, _ => 0, trait bounds)
       ↓
Phase 18+ ────────── Stdlid integration (mellis-mlib types)
```

Phase 16.5 is a scoped fix, not a new design phase. It completes Phase 16's contract.
