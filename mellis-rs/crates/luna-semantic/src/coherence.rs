use std::collections::HashMap;
use luna_common::ids::SymbolId;
use luna_common::{Diagnostic, Span};
use crate::ty::{SemanticType, SemanticTypeId, Substitution};
use crate::SemanticContext;

impl SemanticContext {
    pub fn is_symbol_local(&self, sym_id: SymbolId) -> bool {
        self.symbol_table.is_symbol_local(sym_id, self.current_provider)
    }

    pub fn is_symbol_foreign(&self, sym_id: SymbolId) -> bool {
        self.symbol_table.is_symbol_foreign(sym_id, self.current_provider)
    }

    pub fn nominal_head(&self, ty_id: SemanticTypeId) -> Option<SymbolId> {
        let ty_id = self.types.resolve(ty_id);
        match self.types.get(ty_id) {
            SemanticType::Struct(s, _, _) => Some(*s),
            SemanticType::Enum(e, _, _) => Some(*e),
            _ => None,
        }
    }

    pub fn check_impl_locality(
        &mut self,
        trait_sym_opt: Option<SymbolId>,
        self_ty: SemanticTypeId,
        span: Span,
    ) -> Result<(), ()> {
        let self_ty_res = self.types.resolve(self_ty);
        let nominal_head = self.nominal_head(self_ty_res);
        let is_head_local = match nominal_head {
            Some(sym) => self.is_symbol_local(sym),
            None => false,
        };
        let is_trait_local = match trait_sym_opt {
            Some(t_sym) => self.is_symbol_local(t_sym),
            None => false,
        };

        if let Some(trait_sym) = trait_sym_opt {
            if !is_trait_local && !is_head_local {
                let trait_name = self.symbol_table.get_symbol(trait_sym).name.clone();
                let self_name = nominal_head
                    .map(|s| self.symbol_table.get_symbol(s).name.clone())
                    .unwrap_or_else(|| format!("{:?}", self.types.get(self_ty_res)));
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "E_ORPHAN_IMPL: Cannot implement foreign trait `{}` for foreign type `{}`. Either the trait or the target type nominal head must be defined in the current module.",
                        trait_name, self_name
                    )).with_span(span)
                );
                return Err(());
            }
        } else {
            // Inherent impl
            if !is_head_local {
                let self_name = nominal_head
                    .map(|s| self.symbol_table.get_symbol(s).name.clone())
                    .unwrap_or_else(|| format!("{:?}", self.types.get(self_ty_res)));
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "E_ORPHAN_IMPL: Cannot define inherent methods for foreign type `{}`. The target type must be defined in the current module.",
                        self_name
                    )).with_span(span)
                );
                return Err(());
            }
        }
        Ok(())
    }

    pub fn check_impl_coherence(
        &mut self,
        trait_sym: SymbolId,
        self_ty: SemanticTypeId,
        generic_params: &[SymbolId],
        span: Span,
    ) -> Result<(), ()> {
        for entry in &self.tables.trait_impl_entries {
            if entry.trait_id == trait_sym {
                if self.types_overlap(entry.self_type, &entry.generic_params, self_ty, generic_params) {
                    let trait_name = self.symbol_table.get_symbol(trait_sym).name.clone();
                    let self_ty_res = self.types.resolve(self_ty);
                    let self_type_str = match self.types.get(self_ty_res) {
                        SemanticType::Struct(s, _, _) => self.symbol_table.get_symbol(*s).name.clone(),
                        SemanticType::Enum(e, _, _) => self.symbol_table.get_symbol(*e).name.clone(),
                        _ => format!("{:?}", self.types.get(self_ty_res)),
                    };
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "E_CONFLICTING_TRAIT_IMPL: conflicting implementations for trait `{}` for `{}`",
                            trait_name, self_type_str
                        )).with_span(span)
                    );
                    return Err(());
                }
            }
        }
        Ok(())
    }

    pub fn types_overlap(
        &self,
        ty_a: SemanticTypeId,
        generics_a: &[SymbolId],
        ty_b: SemanticTypeId,
        generics_b: &[SymbolId],
    ) -> bool {
        let mut subst = HashMap::new();
        self.can_unify_patterns(ty_a, ty_b, generics_a, generics_b, &mut subst)
    }

    fn deref_type(
        &self,
        mut ty: SemanticTypeId,
        generics_1: &[SymbolId],
        generics_2: &[SymbolId],
        subst: &HashMap<SymbolId, SemanticTypeId>,
    ) -> SemanticTypeId {
        let is_var = |sym: SymbolId| generics_1.contains(&sym) || generics_2.contains(&sym);
        loop {
            ty = self.types.resolve(ty);
            if let SemanticType::GenericParam(gp) = self.types.get(ty) {
                if is_var(*gp) {
                    if let Some(&next) = subst.get(gp) {
                        ty = next;
                        continue;
                    }
                }
            }
            break;
        }
        ty
    }

    fn occurs_in_pattern(
        &self,
        target: SymbolId,
        ty: SemanticTypeId,
        generics_1: &[SymbolId],
        generics_2: &[SymbolId],
        subst: &HashMap<SymbolId, SemanticTypeId>,
    ) -> bool {
        let ty = self.deref_type(ty, generics_1, generics_2, subst);
        match self.types.get(ty) {
            SemanticType::GenericParam(gp) => *gp == target,
            SemanticType::Struct(_, args, _) | SemanticType::Enum(_, args, _) | SemanticType::Tuple(args) => {
                args.iter().any(|&a| self.occurs_in_pattern(target, a, generics_1, generics_2, subst))
            }
            SemanticType::Reference(_, _, inner) | SemanticType::Pointer(_, inner) | SemanticType::Box(inner) | SemanticType::Slice(inner) => {
                self.occurs_in_pattern(target, *inner, generics_1, generics_2, subst)
            }
            _ => false,
        }
    }

    fn can_unify_patterns(
        &self,
        t1: SemanticTypeId,
        t2: SemanticTypeId,
        generics_1: &[SymbolId],
        generics_2: &[SymbolId],
        subst: &mut HashMap<SymbolId, SemanticTypeId>,
    ) -> bool {
        let t1 = self.deref_type(t1, generics_1, generics_2, subst);
        let t2 = self.deref_type(t2, generics_1, generics_2, subst);

        let is_var = |sym: SymbolId| generics_1.contains(&sym) || generics_2.contains(&sym);

        let g1_opt = match self.types.get(t1) {
            SemanticType::GenericParam(g1) if is_var(*g1) => Some(*g1),
            _ => None,
        };

        let g2_opt = match self.types.get(t2) {
            SemanticType::GenericParam(g2) if is_var(*g2) => Some(*g2),
            _ => None,
        };

        match (g1_opt, g2_opt) {
            (Some(g1), Some(g2)) => {
                if g1 == g2 {
                    return true;
                }
                subst.insert(g1, t2);
                return true;
            }
            (Some(g1), None) => {
                if self.occurs_in_pattern(g1, t2, generics_1, generics_2, subst) {
                    return false;
                }
                subst.insert(g1, t2);
                return true;
            }
            (None, Some(g2)) => {
                if self.occurs_in_pattern(g2, t1, generics_1, generics_2, subst) {
                    return false;
                }
                subst.insert(g2, t1);
                return true;
            }
            (None, None) => {}
        }

        match (self.types.get(t1), self.types.get(t2)) {
            (SemanticType::Primitive(p1), SemanticType::Primitive(p2)) => p1 == p2,
            (SemanticType::Void, SemanticType::Void) | (SemanticType::Never, SemanticType::Never) => true,
            (SemanticType::Struct(s1, args1, _), SemanticType::Struct(s2, args2, _)) if s1 == s2 => {
                args1.len() == args2.len()
                    && args1.iter().zip(args2.iter()).all(|(&a1, &a2)| {
                        self.can_unify_patterns(a1, a2, generics_1, generics_2, subst)
                    })
            }
            (SemanticType::Enum(e1, args1, _), SemanticType::Enum(e2, args2, _)) if e1 == e2 => {
                args1.len() == args2.len()
                    && args1.iter().zip(args2.iter()).all(|(&a1, &a2)| {
                        self.can_unify_patterns(a1, a2, generics_1, generics_2, subst)
                    })
            }
            (SemanticType::Tuple(args1), SemanticType::Tuple(args2)) => {
                args1.len() == args2.len()
                    && args1.iter().zip(args2.iter()).all(|(&a1, &a2)| {
                        self.can_unify_patterns(a1, a2, generics_1, generics_2, subst)
                    })
            }
            (SemanticType::Reference(m1, _, inner1), SemanticType::Reference(m2, _, inner2)) => {
                m1 == m2 && self.can_unify_patterns(*inner1, *inner2, generics_1, generics_2, subst)
            }
            (SemanticType::Pointer(m1, inner1), SemanticType::Pointer(m2, inner2)) => {
                m1 == m2 && self.can_unify_patterns(*inner1, *inner2, generics_1, generics_2, subst)
            }
            (SemanticType::Box(inner1), SemanticType::Box(inner2)) => {
                self.can_unify_patterns(*inner1, *inner2, generics_1, generics_2, subst)
            }
            (SemanticType::Slice(inner1), SemanticType::Slice(inner2)) => {
                self.can_unify_patterns(*inner1, *inner2, generics_1, generics_2, subst)
            }
            (SemanticType::GenericParam(gp1), SemanticType::GenericParam(gp2)) => gp1 == gp2,
            _ => false,
        }
    }

    pub fn matches_impl_pattern(
        &self,
        pattern_ty: SemanticTypeId,
        concrete_ty: SemanticTypeId,
        generic_params: &[SymbolId],
        subst: &mut Substitution,
    ) -> bool {
        let pattern_ty = self.types.resolve(pattern_ty);
        let concrete_ty = self.types.resolve(concrete_ty);
        match (self.types.get(pattern_ty), self.types.get(concrete_ty)) {
            (&SemanticType::GenericParam(gp), _) if generic_params.contains(&gp) => {
                if let Some(&existing) = subst.get(gp) {
                    self.types.resolve(existing) == self.types.resolve(concrete_ty)
                } else {
                    subst.insert(gp, concrete_ty);
                    true
                }
            }
            (&SemanticType::Primitive(ref p1), &SemanticType::Primitive(ref p2)) => p1 == p2,
            (&SemanticType::Void, &SemanticType::Void) | (&SemanticType::Never, &SemanticType::Never) => true,
            (&SemanticType::Struct(s1, ref args1, _), &SemanticType::Struct(s2, ref args2, _)) if s1 == s2 => {
                args1.len() == args2.len()
                    && args1.iter().zip(args2.iter()).all(|(&a1, &a2)| {
                        self.matches_impl_pattern(a1, a2, generic_params, subst)
                    })
            }
            (&SemanticType::Enum(e1, ref args1, _), &SemanticType::Enum(e2, ref args2, _)) if e1 == e2 => {
                args1.len() == args2.len()
                    && args1.iter().zip(args2.iter()).all(|(&a1, &a2)| {
                        self.matches_impl_pattern(a1, a2, generic_params, subst)
                    })
            }
            (&SemanticType::Tuple(ref args1), &SemanticType::Tuple(ref args2)) => {
                args1.len() == args2.len()
                    && args1.iter().zip(args2.iter()).all(|(&a1, &a2)| {
                        self.matches_impl_pattern(a1, a2, generic_params, subst)
                    })
            }
            (&SemanticType::Reference(ref m1, _, inner1), &SemanticType::Reference(ref m2, _, inner2)) => {
                m1 == m2 && self.matches_impl_pattern(inner1, inner2, generic_params, subst)
            }
            (&SemanticType::Pointer(ref m1, inner1), &SemanticType::Pointer(ref m2, inner2)) => {
                m1 == m2 && self.matches_impl_pattern(inner1, inner2, generic_params, subst)
            }
            (&SemanticType::Box(inner1), &SemanticType::Box(inner2)) => {
                self.matches_impl_pattern(inner1, inner2, generic_params, subst)
            }
            (&SemanticType::Slice(inner1), &SemanticType::Slice(inner2)) => {
                self.matches_impl_pattern(inner1, inner2, generic_params, subst)
            }
            (&SemanticType::GenericParam(gp1), &SemanticType::GenericParam(gp2)) => gp1 == gp2,
            _ => false,
        }
    }
}
