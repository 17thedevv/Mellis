            Expr::Call { callee, generic_args, args } => {
                println!("DEBUG AST Call: callee={:?}, generic_args_len={}", callee, generic_args.len());
                let callee_expr = &self.arena.exprs[callee.0 as usize];
                if let Expr::Identifier { segments, .. } = callee_expr {
                    let name_str = segments.iter().map(|seg| &self.source[seg.start as usize..seg.end as usize]).collect::<Vec<_>>().join("::");
                    if name_str == "drop" {
                        let diag = Diagnostic::error("Explicit calls to drop() are forbidden. Values are dropped automatically at end of scope.".to_string());
                        let diag = if let Some(span) = segments.last() { diag.with_span(*span) } else { diag };
                        self.ctx.diagnostics.push(diag);
                    }
                }
                
                let callee_ty_id = self.typecheck_expr(callee);
                let mut ret_ty_id = self.ctx.types.new_inference_var();
                let callee_ty = self.ctx.types.get(callee_ty_id).clone();
                
                // Generic bounds checking
                if !generic_args.is_empty() {
                    println!("DEBUG: generic_args is not empty!");
                    if let Some(&callee_sym) = self.ctx.tables.expr_symbols.get(callee) {
                        println!("DEBUG: callee_sym found: {:?}", callee_sym);
                        if let Some(&callee_decl_id) = self.ctx.tables.symbol_decls.get(&callee_sym) {
                            println!("DEBUG: callee_decl_id found: {:?}", callee_decl_id);
                            if let mellis_ast::Decl::Function { generic_params, .. } = &self.arena.decls[callee_decl_id.0 as usize] {
                                println!("DEBUG: generic_params len: {}", generic_params.len());
                                let mut subst = crate::ty::Substitution::new();
                                for (gp_idx, gp) in generic_params.iter().enumerate() {
                                    if let Some(&concrete_type_id) = generic_args.get(gp_idx) {
                                        let concrete_ty = self.lower_type(concrete_type_id);
                                        if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(callee_decl_id, gp_idx)) {
                                            subst.insert(gp_sym, concrete_ty);
                                            let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();
                                            for bound in bounds {
                                                let resolved_ty = self.ctx.types.resolve(concrete_ty);
                                                let mut peeled_ty = resolved_ty;
                                                while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {
                                                    peeled_ty = *inner;
                                                }
                                                if let SemanticType::Struct(struct_sym, _) | SemanticType::Enum(struct_sym, _) = self.ctx.types.get(peeled_ty) {
                                                    let impl_key = crate::semantic_tables::ImplKey {
                                                        trait_id: Some(bound.trait_id),
                                                        self_type_def: *struct_sym,
                                                    };
                                                    if !self.ctx.tables.trait_impls.contains_key(&impl_key) {
                                                        let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();
                                                        let type_name = self.ctx.symbol_table.get_symbol(*struct_sym).name.clone();
                                                        let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];
                                                        let mut diag = Diagnostic::error(format!(
                                                            "The type `{}` does not implement trait `{}` (required by generic parameter `{}`)",
                                                            type_name, trait_name, gp_name
                                                        ));
                                                        if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }
                                                        self.ctx.diagnostics.push(diag);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                let is_dyn_call = self.ctx.tables.dyn_method_indices.contains_key(callee);
                let is_struct_method = if let Expr::Member { .. } = callee_expr { self.ctx.tables.expr_symbols.contains_key(callee) } else { false };
                let is_method_call = (is_dyn_call || is_struct_method) && if let SemanticType::Function { ref params, .. } = callee_ty { params.len() == args.len() + 1 } else { false };
                
                if let SemanticType::Function { params, return_type } = callee_ty {
                    ret_ty_id = return_type;
                    let expected_params = if is_method_call { &params[1..] } else { &params[..] };
                    for (i, arg) in args.iter().enumerate() {
                        let arg_ty = self.typecheck_expr(&arg.value);
                        if let Some(&expected_p) = expected_params.get(i) {
                            if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                let _ = self.unify(expected_p, arg_ty);
                            }
                        }
                    }
                } else {
                    for arg in args {
                        self.typecheck_expr(&arg.value);
                    }
                }
                ret_ty_id
            }
            Expr::MethodCall { object, method_name, args, generic_args } => {
                let obj_ty_id = self.typecheck_expr(object);
                let obj_ty = self.ctx.types.get(obj_ty_id).clone();
                let member_name = &self.source[method_name.start as usize..method_name.end as usize];

                let dyn_trait_sym = match &obj_ty {
                    SemanticType::DynTrait(sym) => Some(*sym),
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        if let SemanticType::DynTrait(sym) = self.ctx.types.get(*inner) {
                            Some(*sym)
                        } else { None }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);
                            if sym.name == member_name {
                                self.ctx.tables.dyn_method_indices.insert(*expr_id, idx as u32);
                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                    if let SemanticType::Function { params, return_type } = self.ctx.types.get(m_ty).clone() {
                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };
                                        for (i, arg) in args.iter().enumerate() {
                                            let arg_ty = self.typecheck_expr(&arg.value);
                                            if let Some(&expected_p) = expected_params.get(i) {
                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                                    let _ = self.unify(expected_p, arg_ty);
                                                }
                                            }
                                        }
                                        return return_type;
                                    }
                                }
                            }
                        }
                    }
                }

                let peeled_ty = match &obj_ty {
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        self.ctx.types.get(*inner).clone()
                    }
                    _ => obj_ty.clone(),
                };

                if let SemanticType::Struct(sym_id, struct_generic_args) = peeled_ty {
                    let mut matching_impls = Vec::new();
                    for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {
                        if impl_key.self_type_def == sym_id {
                            matching_impls.push(impl_decl_id);
                        }
                    }
                    for impl_decl_id in matching_impls {
                        if let mellis_ast::Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                            for &m_id in methods {
                                if let mellis_ast::Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {
                                    let m_name = &self.source[name.start as usize..name.end as usize];
                                    if m_name == member_name {
                                        if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {
                                            self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);
                                            if let Some(&m_ty_id) = self.ctx.tables.symbol_types.get(&m_sym) {
                                                let mut subst = crate::ty::Substitution::new();
                                                if let mellis_ast::Decl::Impl { generic_params: impl_gps, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                                    for (i, _) in impl_gps.iter().enumerate() {
                                                        if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(impl_decl_id, i)) {
                                                            if let Some(&arg_ty) = struct_generic_args.get(i) {
                                                                subst.insert(gp_sym, arg_ty);
                                                            }
                                                        }
                                                    }
                                                }
                                                let mut is_inferred_generic = false;
                                                let mut generic_param_symbols = Vec::new();
                                                if let mellis_ast::Decl::Function { generic_params: method_gps, .. } = &self.arena.decls[m_id.0 as usize] {
                                                    for (i, gp) in method_gps.iter().enumerate() {
                                                        if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(m_id, i)) {
                                                            if let Some(&arg_ty_id) = generic_args.get(i) {
                                                                let arg_ty = self.lower_type(arg_ty_id);
                                                                subst.insert(gp_sym, arg_ty);
                                                                let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();
                                                                for bound in bounds {
                                                                    let resolved_ty = self.ctx.types.resolve(arg_ty);
                                                                    let mut peeled_ty = resolved_ty;
                                                                    while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {
                                                                        peeled_ty = *inner;
                                                                    }
                                                                    if let SemanticType::Struct(struct_sym, _) | SemanticType::Enum(struct_sym, _) = self.ctx.types.get(peeled_ty) {
                                                                        let impl_key = crate::semantic_tables::ImplKey {
                                                                            trait_id: Some(bound.trait_id),
                                                                            self_type_def: *struct_sym,
                                                                        };
                                                                        if !self.ctx.tables.trait_impls.contains_key(&impl_key) {
                                                                            let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();
                                                                            let type_name = self.ctx.symbol_table.get_symbol(*struct_sym).name.clone();
                                                                            let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];
                                                                            let mut diag = Diagnostic::error(format!(
                                                                                "The type `{}` does not implement trait `{}` (required by generic parameter `{}`)",
                                                                                type_name, trait_name, gp_name
                                                                            ));
                                                                            if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }
                                                                            self.ctx.diagnostics.push(diag);
                                                                        }
                                                                    }
                                                                }
                                                            } else {
                                                                let infer_var = self.ctx.types.new_inference_var();
                                                                subst.insert(gp_sym, infer_var);
                                                                is_inferred_generic = true;
                                                                generic_param_symbols.push((gp_sym, gp.clone()));
                                                            }
                                                        }
                                                    }
                                                }
                                                let ret_ty_id = self.substitute_type(m_ty_id, &subst);
                                                if let SemanticType::Function { params, return_type } = self.ctx.types.get(ret_ty_id).clone() {
                                                    let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };
                                                    for (i, arg) in args.iter().enumerate() {
                                                        let arg_ty = self.typecheck_expr(&arg.value);
                                                        if let Some(&expected_p) = expected_params.get(i) {
                                                            if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                                                let _ = self.unify(expected_p, arg_ty);
                                                            }
                                                        }
                                                    }
                                                    if is_inferred_generic {
                                                        if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id) {
                                                            let mut subst_clone = subst.clone();
                                                            for (gp_sym, gp) in generic_param_symbols {
                                                                if let Some(&infer_var) = subst_clone.map.get(&gp_sym) {
                                                                    let resolved_ty = self.ctx.types.resolve(infer_var);
                                                                    subst_clone.map.insert(gp_sym, resolved_ty);
                                                                    let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();
                                                                    for bound in bounds {
                                                                        let mut peeled_ty = resolved_ty;
                                                                        while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {
                                                                            peeled_ty = *inner;
                                                                        }
                                                                        if let SemanticType::Struct(struct_sym, _) | SemanticType::Enum(struct_sym, _) = self.ctx.types.get(peeled_ty) {
                                                                            let impl_key = crate::semantic_tables::ImplKey {
                                                                                trait_id: Some(bound.trait_id),
                                                                                self_type_def: *struct_sym,
                                                                            };
                                                                            if !self.ctx.tables.trait_impls.contains_key(&impl_key) {
                                                                                let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();
                                                                                let type_name = self.ctx.symbol_table.get_symbol(*struct_sym).name.clone();
                                                                                let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];
                                                                                let mut diag = Diagnostic::error(format!(
                                                                                    "The type `{}` does not implement trait `{}` (required by inferred generic parameter `{}`)",
                                                                                    type_name, trait_name, gp_name
                                                                                ));
                                                                                if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }
                                                                                self.ctx.diagnostics.push(diag);
                                                                            }
                                                                        } else if matches!(self.ctx.types.get(resolved_ty), SemanticType::InferenceVar(_)) {
                                                                            let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];
                                                                            let mut diag = Diagnostic::error(format!("Could not infer type for generic parameter `{}`", gp_name));
                                                                            if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }
                                                                            self.ctx.diagnostics.push(diag);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            self.ctx.tables.expr_substs.insert(*expr_id, subst_clone);
                                                        }
                                                    }
                                                    return return_type;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                for arg in args {
                    self.typecheck_expr(&arg.value);
                }
                self.ctx.types.new_inference_var()
            }
            Expr::Assign { lvalue, value, .. } => {
                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(lvalue).copied() {
                    for lambda_id in &self.active_lambdas {
                        if self.ctx.tables.expr_captures.get(lambda_id).map_or(false, |caps| caps.contains(&sym_id)) {
                            self.ctx.tables.closure_mutated_captures.entry(*lambda_id).or_default().insert(sym_id);
                        }
                    }
                }
                let l_ty = self.typecheck_expr(lvalue);
                let r_ty = self.typecheck_expr(value);
                self.enforce_mutability(lvalue);
                if !self.try_coerce_dyn(*value, r_ty, l_ty) {
                    let _ = self.unify(l_ty, r_ty);
                }
                self.ctx.types.intern(SemanticType::Void)
            }
            Expr::Member { object, member } => {
                let obj_ty_id = self.typecheck_expr(object);
                let obj_ty = self.ctx.types.get(obj_ty_id).clone();
                let member_name = &self.source[member.start as usize..member.end as usize];
                
                // 1. Check if obj_ty is DynTrait or pointer/ref to DynTrait
                let dyn_trait_sym = match &obj_ty {
                    SemanticType::DynTrait(sym) => Some(*sym),
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        if let SemanticType::DynTrait(sym) = self.ctx.types.get(*inner) {
                            Some(*sym)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);
                            if sym.name == member_name {
                                self.ctx.tables.dyn_method_indices.insert(*expr_id, idx as u32);
                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                    self.ctx.tables.expr_types.insert(*expr_id, m_ty);
                                    return m_ty;
                                }
                            }
                        }
                    }
                    self.ctx.diagnostics.push(Diagnostic::error(format!("Trait does not contain method '{}'", member_name)).with_span(*member));
                    let infer = self.ctx.types.new_inference_var();
                    self.ctx.tables.expr_types.insert(*expr_id, infer);
                    return infer;
                }
                
                let peeled_ty = match &obj_ty {
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        self.ctx.types.get(*inner).clone()
                    }
                    _ => obj_ty.clone(),
                };
                
                if let SemanticType::Struct(sym_id, field_tys) = peeled_ty {
                    if let Some(decl_id) = self.ctx.tables.symbol_decls.get(&sym_id) {
                        if let Decl::Struct { fields, .. } = &self.arena.decls[decl_id.0 as usize] {
                            if let Some((index, _)) = fields.iter().enumerate().find(|(_, field)| {
                                &self.source[field.name.start as usize..field.name.end as usize] == member_name
                            }) {
                                self.ctx.tables.expr_member_indices.insert(*expr_id, index as u32);
                                let f_ty = field_tys[index];
                                self.ctx.tables.expr_types.insert(*expr_id, f_ty);
                                return f_ty;
                            }
                        }
                    }

                    // Look up methods on this struct (from impl blocks)
                    for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {
                        if impl_key.self_type_def == sym_id {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                for &m_id in methods {
                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {
                                        let m_name = &self.source[name.start as usize..name.end as usize];
                                        if m_name == member_name {
                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {
                                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                                    self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);
                                                    self.ctx.tables.expr_types.insert(*expr_id, m_ty);
                                                    return m_ty;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown field or method '{}'", member_name)).with_span(*member));
                    let infer = self.ctx.types.new_inference_var();
                    self.ctx.tables.expr_types.insert(*expr_id, infer);
                    return infer;
                }
                self.ctx.diagnostics.push(Diagnostic::error("Member access requires a struct or trait object").with_span(*member));
                self.ctx.types.new_inference_var()
            }
            Expr::StructInit { path, fields, .. } => {
                let Some(name_span) = path.last() else {
                    return self.ctx.types.new_inference_var();
                };
                let name = &self.source[name_span.start as usize..name_span.end as usize];
                let Some(symbol) = self.ctx.symbol_table.lookup(name, crate::ScopeId(0)) else {
                    self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown struct '{}'", name)).with_span(*name_span));
                    return self.ctx.types.new_inference_var();
                };
                let struct_ty = self.ctx.tables.symbol_types.get(&symbol).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());
                let SemanticType::Struct(_, field_tys) = self.ctx.types.get(struct_ty).clone() else {
                    self.ctx.diagnostics.push(Diagnostic::error(format!("'{}' is not a struct", name)).with_span(*name_span));
                    return self.ctx.types.new_inference_var();
                };
                let Some(decl_id) = self.ctx.tables.symbol_decls.get(&symbol).copied() else {
                    return struct_ty;
                };
                let Decl::Struct { fields: declared_fields, .. } = &self.arena.decls[decl_id.0 as usize] else {
                    return struct_ty;
                };
                let mut init_indices = Vec::with_capacity(fields.len());
                for field in fields {
                    let field_name = &self.source[field.name.start as usize..field.name.end as usize];
                    if let Some((index, _)) = declared_fields.iter().enumerate().find(|(_, declared)| {
                        &self.source[declared.name.start as usize..declared.name.end as usize] == field_name
                    }) {
                        let value_ty = self.typecheck_expr(&field.value);
                        if self.unify(field_tys[index], value_ty).is_err() {
                            self.ctx.diagnostics.push(Diagnostic::error(format!("Type mismatch for field '{}'", field_name)).with_span(field.name));
                        }
                        init_indices.push(index as u32);
                    } else {
                        self.ctx.diagnostics.push(Diagnostic::error(format!("Unknown field '{}' for struct '{}'", field_name, name)).with_span(field.name));
                        self.typecheck_expr(&field.value);
                        init_indices.push(u32::MAX); // Error recovery
                    }
                }
                self.ctx.tables.expr_struct_init_indices.insert(*expr_id, init_indices);
                struct_ty
            }
            Expr::ArrayLiteral { elements } => {
                let mut elem_ty = self.ctx.types.new_inference_var();
                for (i, el) in elements.iter().enumerate() {
                    let ty = self.typecheck_expr(el);
                    if i == 0 {
                        elem_ty = ty;
                    } else if self.unify(elem_ty, ty).is_err() {
                        self.ctx.diagnostics.push(Diagnostic::error("Array literal elements must have the same type"));
                    }
                }
                self.ctx.types.intern(SemanticType::Array(elem_ty, elements.len() as u64))
            }
            Expr::TupleLiteral { elements } => {
                let mut elem_tys = Vec::new();
                for el in elements {
                    elem_tys.push(self.typecheck_expr(el));
                }
                self.ctx.types.intern(SemanticType::Tuple(elem_tys))
            }
            Expr::TupleIndex { object, index } => {
                let base_ty_id = self.typecheck_expr(object);
                let base_ty = self.ctx.types.get(base_ty_id).clone();
                match base_ty {
                    SemanticType::Tuple(elem_tys) => {
                        if *index as usize >= elem_tys.len() {
                            self.ctx.diagnostics.push(Diagnostic::error(format!("Tuple index {} out of bounds (tuple has {} elements)", index, elem_tys.len())));
                            self.ctx.types.new_inference_var()
                        } else {
                            elem_tys[*index as usize]
                        }
                    }
                    _ => {
                        self.ctx.diagnostics.push(Diagnostic::error("Cannot index into a non-tuple type"));
                        self.ctx.types.new_inference_var()
                    }
                }
            }
            Expr::Index { base, index } => {
                let base_ty_id = self.typecheck_expr(base);
                self.typecheck_expr(index);
                
                let base_ty = self.ctx.types.get(base_ty_id).clone();
                match base_ty {
                    SemanticType::Array(el, _) => el,
                    SemanticType::Slice(el) => el,
                    _ => self.ctx.types.new_inference_var()
                }
            }
            Expr::MethodCall { object, method_name, args, .. } => {
                let obj_ty_id = self.typecheck_expr(object);
                let obj_ty = self.ctx.types.get(obj_ty_id).clone();
                let member_name = &self.source[method_name.start as usize..method_name.end as usize];
                
                // 1. Check if obj_ty is DynTrait or pointer/ref to DynTrait
                let dyn_trait_sym = match &obj_ty {
                    SemanticType::DynTrait(sym) => Some(*sym),
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        if let SemanticType::DynTrait(sym) = self.ctx.types.get(*inner) {
                            Some(*sym)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(trait_sym) = dyn_trait_sym {
                    if let Some(method_syms) = self.ctx.tables.trait_methods.get(&trait_sym) {
                        for (idx, &m_sym) in method_syms.iter().enumerate() {
                            let sym = self.ctx.symbol_table.get_symbol(m_sym);
                            if sym.name == member_name {
                                self.ctx.tables.dyn_method_indices.insert(*expr_id, idx as u32);
                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                    if let SemanticType::Function { params, return_type } = self.ctx.types.get(m_ty).clone() {
                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };
                                        for (i, arg) in args.iter().enumerate() {
                                            let arg_ty = self.typecheck_expr(&arg.value);
                                            if let Some(&expected_p) = expected_params.get(i) {
                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                                    let _ = self.unify(expected_p, arg_ty);
                                                }
                                            }
                                        }
                                        return return_type;
                                    }
                                }
                            }
                        }
                    }
                }

                // 2. Struct method lookup
                let peeled_ty = match &obj_ty {
                    SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) => {
                        self.ctx.types.get(*inner).clone()
                    }
                    _ => obj_ty.clone(),
                };

                if let SemanticType::Struct(sym_id, _) = peeled_ty {
                    for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {
                        if impl_key.self_type_def == sym_id {
                            if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {
                                for &m_id in methods {
                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {
                                        let m_name = &self.source[name.start as usize..name.end as usize];
                                        if m_name == member_name {
                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {
                                                self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);
                                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {
                                                    if let SemanticType::Function { params, return_type } = self.ctx.types.get(m_ty).clone() {
                                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };
                                                        for (i, arg) in args.iter().enumerate() {
                                                            let arg_ty = self.typecheck_expr(&arg.value);
                                                            if let Some(&expected_p) = expected_params.get(i) {
                                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {
                                                                    let _ = self.unify(expected_p, arg_ty);
                                                                }
                                                            }
                                                        }
                                                        return return_type;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                for arg in args {
                    self.typecheck_expr(&arg.value);
                }

                self.ctx.types.new_inference_var()
            }
