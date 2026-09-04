
import json
content = open("crates/mellis-semantic/src/mono.rs", "r").read()

old_code = """        let closure_capture_bindings = closure_bindings.into_iter().map(|mut binding| {
            binding.ty = self.substitute(binding.ty);
            binding.env_ty = self.substitute(binding.env_ty);
            binding
        }).collect();
        let instantiated_fn = InstantiatedFunction {
            instance: instance.clone(),
            expr_types: std::mem::take(&mut self.current_expr_types),
            symbol_types: std::mem::take(&mut self.current_symbol_types),
            pat_types: std::mem::take(&mut self.current_pat_types),"""

new_code = """        let closure_capture_bindings = closure_bindings.into_iter().map(|mut binding| {
            binding.ty = self.substitute(binding.ty);
            binding.env_ty = self.substitute(binding.env_ty);
            binding
        }).collect();
        
        // --- CONCRETIZATION BARRIER ---
        // Ensure that no unresolved InferenceVars or GenericParams cross into MVIR for this monomorphic unit.
        let mut has_unresolved = false;
        
        for (expr_id, ty) in self.current_expr_types.iter_mut() {
            let resolved = self.substitute(*ty);
            *ty = resolved;
            if !self.ctx.types.is_monomorphic(resolved) {
                has_unresolved = true;
                let span = match &self.arena.exprs[expr_id.0 as usize] {
                    mellis_ast::Expr::Literal(tok, _) => Some(tok.span),
                    mellis_ast::Expr::Identifier { segments, .. } => segments.first().copied(),
                    mellis_ast::Expr::MethodCall { method_name, .. } => Some(*method_name),
                    _ => None,
                }.unwrap_or(mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0));
                
                let err_msg = if self.ctx.types.contains_inference_var(resolved) {
                    "cannot infer type for expression"
                } else {
                    "unresolved generic parameter in monomorphic context"
                };
                self.ctx.diagnostics.push(mellis_common::diagnostic::Diagnostic::error(err_msg).with_span(span));
                *ty = self.ctx.types.intern(crate::ty::SemanticType::Error);
            }
        }
        
        for (sym_id, ty) in self.current_symbol_types.iter_mut() {
            let resolved = self.substitute(*ty);
            *ty = resolved;
            if !self.ctx.types.is_monomorphic(resolved) {
                has_unresolved = true;
                let span = mellis_common::Span::new(mellis_common::ids::FileId(0), 0, 0); // fallback span
                let err_msg = if self.ctx.types.contains_inference_var(resolved) {
                    "cannot infer type for variable"
                } else {
                    "unresolved generic parameter for variable"
                };
                self.ctx.diagnostics.push(mellis_common::diagnostic::Diagnostic::error(err_msg).with_span(span));
                *ty = self.ctx.types.intern(crate::ty::SemanticType::Error);
            }
        }
        
        let instantiated_fn = InstantiatedFunction {
            instance: instance.clone(),
            expr_types: std::mem::take(&mut self.current_expr_types),
            symbol_types: std::mem::take(&mut self.current_symbol_types),
            pat_types: std::mem::take(&mut self.current_pat_types),"""

if old_code in content:
    content = content.replace(old_code, new_code)
    open("crates/mellis-semantic/src/mono.rs", "w").write(content)
    print("Replaced mono barrier")
else:
    print("Could not find old_code")

