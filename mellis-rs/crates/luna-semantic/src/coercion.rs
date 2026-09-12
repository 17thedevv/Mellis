use crate::ty::{SemanticType, SemanticTypeId, Mutability};
use crate::semantic_tables::ImplKey;
use crate::SemanticContext;
use luna_common::ids::SymbolId;

/// Semantic evidence for a representation-changing coercion.
/// Each variant carries all metadata needed by the MVIR generator
/// to emit the correct explicit instruction. The backend never
/// independently infers representation changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoercionKind {
    /// &rw T -> &T (no representation change, just permission narrowing)
    RefMutToShared,

    /// &[T; N] -> &[T] or *[T; N] -> *[T]
    /// Representation change: ptr -> { ptr, N }
    ArrayToSlice {
        element_type: SemanticTypeId,
        length: u64,
    },

    /// &ConcreteType -> &dyn Trait or *ConcreteType -> *dyn Trait
    /// Representation change: ptr -> { ptr, vtable_ptr }
    ConcreteToDyn {
        trait_sym: SymbolId,
        concrete_sym: SymbolId,
    },
}

/// Helper to check if mutability allows coercion.
/// Mutable can coerce to Mutable or Immutable (narrowing).
/// Immutable can only coerce to Immutable (cannot widen).
fn mutability_allows(from_mut: Mutability, to_mut: Mutability) -> bool {
    match (from_mut, to_mut) {
        (Mutability::Mutable, Mutability::Mutable) => true,
        (Mutability::Mutable, Mutability::Immutable) => true, // narrowing allowed
        (Mutability::Immutable, Mutability::Immutable) => true,
        (Mutability::Immutable, Mutability::Mutable) => false, // cannot widen
    }
}

/// Single semantic entrypoint for all coercions.
///
/// Returns `Some(CoercionKind)` if `from_ty` can be coerced to `to_ty`,
/// along with sufficient metadata for MVIR generation.
/// Returns `None` if no coercion is applicable.
pub fn try_coerce(
    ctx: &SemanticContext,
    from_ty: SemanticTypeId,
    to_ty: SemanticTypeId,
) -> Option<CoercionKind> {
    let from_resolved = ctx.types.resolve_inference(from_ty);
    let to_resolved = ctx.types.resolve_inference(to_ty);

    let from = ctx.types.get(from_resolved).clone();
    let to = ctx.types.get(to_resolved).clone();

    // 1. Reference -> Reference coercions
    if let (SemanticType::Reference(_, f_mut, f_inner),
            SemanticType::Reference(_, t_mut, t_inner)) = (&from, &to) {
        if !mutability_allows(f_mut.clone(), t_mut.clone()) {
            return None;
        }

        // Sub-case 1a: &rw T -> &T (same inner type, narrowing mutability)
        if f_inner == t_inner && *f_mut == Mutability::Mutable && *t_mut == Mutability::Immutable {
            return Some(CoercionKind::RefMutToShared);
        }

        // Sub-case 1b: &[T; N] -> &[T] or &rw [T; N] -> &rw [T] or &rw [T; N] -> &[T]
        let f_inner_ty = ctx.types.get(ctx.types.resolve_inference(*f_inner)).clone();
        let t_inner_ty = ctx.types.get(ctx.types.resolve_inference(*t_inner)).clone();

        if let (SemanticType::Array(arr_elem, arr_len), SemanticType::Slice(slice_elem)) = (&f_inner_ty, &t_inner_ty) {
            let arr_elem_res = ctx.types.resolve_inference(*arr_elem);
            let slice_elem_res = ctx.types.resolve_inference(*slice_elem);
            if arr_elem_res == slice_elem_res {
                return Some(CoercionKind::ArrayToSlice {
                    element_type: arr_elem_res,
                    length: *arr_len,
                });
            }
        }

        // Sub-case 1c: &Concrete -> &dyn Trait or &rw Concrete -> &rw dyn Trait
        if let SemanticType::DynTrait(trait_sym) = t_inner_ty {
            return try_concrete_to_dyn(ctx, *f_inner, trait_sym);
        }
    }

    // 2. Pointer -> Pointer coercions
    if let (SemanticType::Pointer(f_mut, f_inner),
            SemanticType::Pointer(t_mut, t_inner)) = (&from, &to) {
        if !mutability_allows(f_mut.clone(), t_mut.clone()) {
            return None;
        }

        // Sub-case 2a: *[T; N] -> *[T] or *rw [T; N] -> *rw [T] or *rw [T; N] -> *[T]
        let f_inner_ty = ctx.types.get(ctx.types.resolve_inference(*f_inner)).clone();
        let t_inner_ty = ctx.types.get(ctx.types.resolve_inference(*t_inner)).clone();

        if let (SemanticType::Array(arr_elem, arr_len), SemanticType::Slice(slice_elem)) = (&f_inner_ty, &t_inner_ty) {
            let arr_elem_res = ctx.types.resolve_inference(*arr_elem);
            let slice_elem_res = ctx.types.resolve_inference(*slice_elem);
            if arr_elem_res == slice_elem_res {
                return Some(CoercionKind::ArrayToSlice {
                    element_type: arr_elem_res,
                    length: *arr_len,
                });
            }
        }

        // Sub-case 2b: *Concrete -> *dyn Trait or *rw Concrete -> *rw dyn Trait
        if let SemanticType::DynTrait(trait_sym) = t_inner_ty {
            return try_concrete_to_dyn(ctx, *f_inner, trait_sym);
        }
    }

    // 3. Direct / bare value -> DynTrait (if checking compatibility directly)
    if let SemanticType::DynTrait(trait_sym) = to {
        return try_concrete_to_dyn(ctx, from_resolved, trait_sym);
    }

    None
}

/// Attempt concrete type -> dyn Trait coercion.
/// Returns `Some(ConcreteToDyn)` if an impl of the trait for the concrete type exists.
fn try_concrete_to_dyn(
    ctx: &SemanticContext,
    concrete_ty: SemanticTypeId,
    trait_sym: SymbolId,
) -> Option<CoercionKind> {
    let resolved = ctx.types.resolve_inference(concrete_ty);
    let f_ty = ctx.types.get(resolved).clone();
    let concrete_sym = match f_ty {
        SemanticType::Struct(s_sym, _, _) => s_sym,
        SemanticType::Enum(e_sym, _, _) => e_sym,
        _ => return None,
    };

    let key = ImplKey {
        trait_id: Some(trait_sym),
        self_type_def: concrete_sym,
    };
    if ctx.tables.trait_impls.contains_key(&key) {
        Some(CoercionKind::ConcreteToDyn {
            trait_sym,
            concrete_sym,
        })
    } else {
        None
    }
}
