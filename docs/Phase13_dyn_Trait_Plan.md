# Phase 13: `dyn Trait` — Dynamic Dispatch Implementation Plan

> **Document Purpose**: Standalone export for external agent audit. Contains Phase 13 plan, sub-phases, implementation details, and verification criteria.

---

## 1. Overview

Phase 13 implements **`dyn Trait`** (trait objects) for dynamic dispatch in the Mellis compiler. This allows runtime polymorphism where the concrete type is determined at runtime rather than compile time.

### Key Features
- **Trait objects**: `dyn MyTrait` type representing any type implementing `MyTrait`
- **vtable**: Virtual table containing function pointers for trait methods
- **Fat pointer ABI**: `dyn Trait` = `{ data_ptr: ptr, vtable_ptr: ptr }`
- **Object safety**: Rules for which traits can be used as trait objects
- **Coercion**: Automatic coercion from `&T` or `&mut T` to `&dyn Trait`

### Dependencies
```
Phase 12 (Closures) ──→ Phase 13 (dyn Trait)
                         ├──→ Phase 16 (async/await) [futures need dyn Future]
```

---

## 2. Feature Breakdown

| # | Feature | Priority | Description |
|---|---|---|---|
| 13A | `dyn Trait` Type | P0 | New `SemanticType::DynTrait` variant |
| 13B | vtable Layout | P0 | Static vtable with method pointers |
| 13C | Fat Pointer ABI | P0 | `{ data_ptr, vtable_ptr }` representation |
| 13D | Object Safety | P0 | Reject non-object-safe traits |
| 13E | Method Dispatch | P0 | Virtual call through vtable |
| 13F | Coercion | P1 | `&T` → `&dyn Trait` |
| 13G | Cross-module | P1 | vtable via MLib |
| 13H | `dyn Trait` in MLIR | P2 | Lowering to LLVM |

---

## 3. Sub-Phase Details

### 13A — `dyn Trait` Type System (P0)

#### Deliverables
- [ ] **AST**: `Type::TraitObject { trait: SymbolId }` parsing
- [ ] **Semantic**: New `SemanticType::DynTrait { trait_sym: SymbolId }` variant
- [ ] **Type Context**: Add to `subst`, `clone_type_from`, `needs_drop`
- [ ] **Lookup**: Resolve `dyn MyTrait` to trait definition

#### Grammar (already exists)
```ebnf
trait_object_type ::= KW_DYN type_path
```

#### Example Syntax
```mellis
trait Printable {
    fn print(self);
}

fn print_all(items: []dyn Printable) {
    for item in items {
        item.print();  // Dynamic dispatch!
    }
}
```

#### Key Files to Modify
- `crates/mellis-ast/src/ty.rs` — `Type::TraitObject` variant
- `crates/mellis-semantic/src/ty.rs` — `SemanticType::DynTrait`
- `crates/mellis-semantic/src/typechecker.rs` — type lowering
- `crates/mellis-semantic/src/resolver.rs` — trait resolution

---

### 13B — vtable Layout (P0)

#### Deliverables
- [ ] **vtable struct**: Generate static vtable per trait with method offsets
- [ ] **Method pointers**: Each vtable entry is a function pointer
- [ ] **Layout**: `{ method1: fn ptr, method2: fn ptr, ... }`

#### Design Decision: Static vs Dynamic vtable

| Option | Pros | Cons |
|--------|------|------|
| **Static (LLVM global)** | Simple, no runtime allocation | Fixed layout, no added methods |
| Dynamic (heap) | Can add methods at runtime | Complexity, GC issues |

**Decision**: Static vtables. Each concrete type's impl gets a single static vtable initialized with its method function pointers.

#### vtable Structure
```rust
// For trait with 2 methods
struct Printable_vtable {
    print_fn: fn(env: ptr) -> void,
    debug_fn: fn(env: ptr) -> String,
}
```

#### Key Files to Modify
- `crates/mellis-semantic/src/semantic_tables.rs` — `trait_vtables: HashMap<SymbolId, VtableInfo>`
- `crates/mellis-mvir/src/mvir.rs` — `Instruction::VtablePtr { trait_sym: SymbolId }`

---

### 13C — Fat Pointer ABI (P0)

#### Deliverables
- [ ] **Fat pointer layout**: `{ data_ptr: ptr, vtable_ptr: ptr }`
- [ ] **LLVM mapping**: `map_type(dyn Trait)` → `llvm_struct { ptr, ptr }`
- [ ] **Size**: Always 16 bytes on 64-bit

#### Fat Pointer Layout
```
┌─────────────────────────────────────┐
│  dyn Trait                          │
│  ┌────────────┐  ┌──────────────┐ │
│  │ data_ptr   │  │ vtable_ptr   │ │
│  │ (8 bytes)  │  │ (8 bytes)    │ │
│  └────────────┘  └──────────────┘ │
└─────────────────────────────────────┘
```

#### Key Files to Modify
- `crates/mellis-semantic/src/ty.rs` — `needs_drop(DynTrait)` returns true (may drop via vtable)
- `crates/mellis-backend/src/llvm_codegen.rs` — `map_type` for `DynTrait`

---

### 13D — Object Safety (P0)

#### Deliverables
- [ ] **Object-safe rules**: Implement trait object safety checks
- [ ] **Reject non-object-safe**: Emit diagnostic for invalid `dyn Trait`
- [ ] **Validation**: `is_object_safe(trait_sym) -> bool`

#### Object Safety Rules (per design_note_object_safety.md)

A trait is **object-safe** if ALL of:
1. No generic methods (method's impl doesn't require type specialization)
2. `Self` is not used in return position (except `-> Self`)
3. No static methods (all methods must have `&self` or `&mut self` receiver)

#### Implementation
```rust
fn is_object_safe(trait_sym: SymbolId, ctx: &SemanticContext) -> bool {
    let trait_decl = /* get trait decl */;
    for method in trait_decl.methods {
        // Check 1: No generic params (type-level only, lifetimes are OK)
        if method.generic_params.contains_type_params() { return false; }
        // Check 2: No Self in return (except -> Self)
        if has_returned_associated_type(method) { return false; }
        // Check 3: Has receiver
        if !has_receiver(method) { return false; }
    }
    true
}
```

#### Key Files to Modify
- `crates/mellis-semantic/src/semantic_tables.rs` — `is_object_safe(trait_sym) -> bool`
- `crates/mellis-semantic/src/typechecker.rs` — emit diagnostic if non-object-safe

---

### 13E — Method Dispatch (P0)

#### Deliverables
- [ ] **Virtual call**: Generate `CallVirt { obj: Operand, method_idx: u32 }`
- [ ] **vtable lookup**: `obj.vtable_ptr[method_idx]`
- [ ] **Indirect call**: Call the resolved method pointer

#### Call Sequence
```
1. Load vtable_ptr from object+8
2. GEP vtable_ptr[method_idx] to get method function ptr
3. Build indirect call with data_ptr as implicit first arg
```

#### Example
```mellis
trait Drawable {
    fn draw(self);
}

struct Circle { radius: f64 }
impl Drawable for Circle {
    fn draw(self) { /* ... */ }
}

fn render(obj: dyn Drawable) {
    obj.draw();  // CallVirt(drawable, 0)
}
```

#### Key Files to Modify
- `crates/mellis-mvir/src/mvir.rs` — `Instruction::CallVirt { obj, method_idx }`
- `crates/mellis-mvir/src/generator.rs` — method call lowering
- `crates/mellis-backend/src/llvm_codegen.rs` — vtable lookup + indirect call

---

### 13F — Coercion (P1)

#### Deliverables
- [ ] **Auto-coercion**: `&T` → `&dyn Trait` when `T: Trait`
- [ ] **Vtable pointer**: Embed vtable address in fat pointer
- [ ] **Validation**: Ensure `T` implements `Trait`

#### Coercion Example
```mellis
fn takes_trait(obj: dyn Drawable) { obj.draw(); }

let circle = Circle { radius: 1.0 };
takes_trait(&circle);  // Auto-coerce &Circle -> &dyn Drawable
```

#### Coercion Process
```
1. Check T implements Trait (via trait resolution)
2. Get static vtable address for T's impl
3. Build fat pointer: { data_ptr: &circle, vtable_ptr: Circle_Drawable_vtable }
```

#### Key Files to Modify
- `crates/mellis-semantic/src/typechecker.rs` — coercion insertion
- `crates/mellis-mvir/src/generator.rs` — fat pointer construction

---

### 13G — Cross-Module via MLib (P1)

#### Deliverables
- [ ] **vtable serialization**: Serialize vtable pointers in MLib
- [ ] **Cross-module dispatch**: Use vtable from imported module
- [ ] **ABI stability**: Stable vtable layout across modules

#### MLib Requirements
- Export vtable addresses in `ExportTable`
- Import vtable addresses from dependencies
- Handle vtable patching for cross-module trait objects

#### Key Files to Modify
- `crates/mellis-mlib/src/ir.rs` — vtable entry
- `crates/mellis-mlib/src/writer.rs` — vtable serialization
- `crates/mellis-mlib/src/reader.rs` — vtable deserialization

---

### 13H — LLVM Lowering (P2)

#### Deliverables
- [ ] **vtable as LLVM global**: `internal constant`
- [ ] **Method as function with env**: Each method takes data_ptr as first arg
- [ ] **CallVirt lowering**: Load vtable → GEP → indirect call

#### LLVM IR Example
```llvm
; vtable for Circle implementing Drawable
@Circle_Drawable_vtable = internal constant {
    ptr,  ; draw_fn
} [{
    ptr @Circle_draw
}]

; Function taking dyn Drawable
define void @render(%dyn_Drawable* %obj) {
    %vtable = getelementptr %dyn_Drawable, %dyn_Drawable* %obj, i32 0, i32 1
    %vtable_ptr = load ptr, ptr* %vtable
    %draw_fn_ptr = getelementptr { ptr }, %vtable_ptr, i32 0, i32 0
    %draw_fn = load ptr, ptr* %draw_fn_ptr
    %data = getelementptr %dyn_Drawable, %dyn_Drawable* %obj, i32 0, i32 0
    %data_ptr = load ptr, ptr* %data
    ; Call with data_ptr as implicit first argument
    call void %draw_fn(ptr %data_ptr)
}
```

---

## 4. Compiler Crate Map

| Crate | Phase 13 Changes |
|---|---|
| `mellis-ast` | `Type::TraitObject` variant |
| `mellis-parser` | `dyn Trait` parsing |
| `mellis-semantic` | `SemanticType::DynTrait`, object safety, coercion |
| `mellis-mvir` | `CallVirt`, `VtablePtr` instructions |
| `mellis-borrowck` | Virtual call borrow tracking |
| `mellis-mlib` | vtable serialization |
| `mellis-backend` | vtable global, indirect call |

---

## 5. Test Plan

### Positive Tests
- [ ] `dyn_trait_basic.ms` — Simple trait object usage
- [ ] `dyn_trait_coercion.ms` — `&T` → `&dyn Trait`
- [ ] `dyn_trait_multi_method.ms` — Multiple methods in vtable
- [ ] `dyn_trait_generic_impl.ms` — Generic impl block

### Negative Tests
- [ ] `dyn_trait_non_object_safe.ms` — Reject non-object-safe traits
- [ ] `dyn_trait_return_self.ms` — Reject `-> Self` in dyn context
- [ ] `dyn_trait_generic_method.ms` — Reject generic methods

### Cross-Module Tests
- [ ] `dyn_trait_cross_module.ms` — Provider defines trait, consumer uses dyn

---

## 6. Implementation Order

```
Week 1: 13A + 13B + 13C (Type system, vtable, fat pointer)
Week 2: 13D + 13E (Object safety, method dispatch)
Week 3: 13F + 13G (Coercion, MLib support)
Week 4: 13H + Tests (LLVM lowering, verification)
```

---

## 7. Known Challenges

### Challenge 1: vtable across compilation units
**Issue**: vtable addresses must be known at link time
**Solution**: Use LLVM `internal constant` with `@` linkage, linker merges

### Challenge 2: Object safety with generics
**Issue**: `fn foo<T>(&self, x: T)` cannot be in vtable
**Solution**: Already specified - reject at object safety check

### Challenge 3: Drop for trait objects
**Issue**: `dyn Trait` may need custom drop via vtable
**Solution**: Default drop, no custom destructor in vtable (Phase 14+)

---

## 8. Open Questions

1. **Self trait**: Should `dyn Self` be supported? (Not for MVP)
2. **Nested dyn**: Is `dyn dyn Trait` valid? (No - reject)
3. **Sized bound**: Should `dyn Trait` require `Sized`? (Yes, implicit)

---

## 9. Success Criteria

Phase 13 is complete when:
1. `dyn Trait` type parses and typechecks correctly
2. Fat pointer `{ data, vtable }` passed through all stages
3. vtable generated for each trait impl
4. Virtual calls dispatch to correct method at runtime
5. `&T` coerces to `&dyn Trait` when `T: Trait`
6. Non-object-safe traits rejected with diagnostic
7. All positive tests pass
8. All negative tests produce expected diagnostics
