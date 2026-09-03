use crate::{SemanticContext, ty::{SemanticTypeId, SemanticType, BuiltinType}};

use mellis_ast::{AstArena, Item, Stmt, Expr, Decl};

use mellis_lexer::{BuiltinKind, TokenKind};

use mellis_common::diagnostic::Diagnostic;



pub struct TypeChecker<'a> {

    ctx: &'a mut SemanticContext,

    arena: &'a AstArena,

    source: &'a str,

    is_unsafe_context: bool,

    active_lambdas: Vec<mellis_ast::ExprId>,

    current_self_type: Option<SemanticTypeId>,

    current_async_fn: Option<mellis_ast::DeclId>,

    comptime_engine: Option<&'a dyn crate::ComptimeEngine>,

    current_return_type: Vec<SemanticTypeId>,

}



impl<'a> TypeChecker<'a> {

    pub fn new(ctx: &'a mut SemanticContext, arena: &'a AstArena, source: &'a str) -> Self {

        Self { ctx, arena, source, is_unsafe_context: false, active_lambdas: Vec::new(), current_self_type: None, current_async_fn: None, comptime_engine: None, current_return_type: Vec::new() }

    }



    pub fn new_with_engine(ctx: &'a mut SemanticContext, arena: &'a AstArena, source: &'a str, comptime_engine: &'a dyn crate::ComptimeEngine) -> Self {

        Self { ctx, arena, source, is_unsafe_context: false, active_lambdas: Vec::new(), current_self_type: None, current_async_fn: None, comptime_engine: Some(comptime_engine), current_return_type: Vec::new() }

    }



    pub fn eval_comptime_expr(&mut self, expr_id: mellis_ast::ExprId) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> {

        let new_instances = {

            let mut mono = crate::mono::MonoCollector::new(self.ctx, self.arena);

            mono.run_on_expr(expr_id);

            mono.instantiated.into_values().collect::<Vec<_>>()

        };

        self.ctx.instantiated_functions.extend(new_instances);



        if let Some(engine) = self.comptime_engine {

            engine.eval_expr(self.arena, self.ctx, self.source, expr_id)

        } else {

            let mut comptime_ctx = crate::comptime::ComptimeContext::new();

            for (sym, val) in &self.ctx.const_values {

                comptime_ctx.consts.insert(*sym, val.clone());

            }

            let evaluator = crate::comptime::ComptimeEvaluator::new(self.arena, self.ctx, self.source);

            evaluator.eval_expr(expr_id, &mut comptime_ctx)

        }

    }



    pub fn eval_comptime_stmt(&mut self, stmt_id: mellis_ast::StmtId) -> Result<crate::comptime::ComptimeValue, crate::comptime::ComptimeError> {

        let new_instances = {

            let mut mono = crate::mono::MonoCollector::new(self.ctx, self.arena);

            mono.run_on_stmt(stmt_id);

            mono.instantiated.into_values().collect::<Vec<_>>()

        };

        self.ctx.instantiated_functions.extend(new_instances);



        if let Some(engine) = self.comptime_engine {

            engine.eval_stmt(self.arena, self.ctx, self.source, stmt_id)

        } else {

            let mut comptime_ctx = crate::comptime::ComptimeContext::new();

            for (sym, val) in &self.ctx.const_values {

                comptime_ctx.consts.insert(*sym, val.clone());

            }

            let evaluator = crate::comptime::ComptimeEvaluator::new(self.arena, self.ctx, self.source);

            match evaluator.eval_stmt(stmt_id, &mut comptime_ctx) {

                Ok(crate::comptime::ComptimeControlFlow::Value(v)) | Ok(crate::comptime::ComptimeControlFlow::Return(v)) => Ok(v),

                Ok(_) => Ok(crate::comptime::ComptimeValue::Unit),

                Err(e) => Err(e),

            }

        }

    }

        }

    }

    

    pub fn unify(&mut self, expected: SemanticTypeId, actual: SemanticTypeId) -> Result<(), String> {

        let expected = self.ctx.types.resolve(expected);

        let actual = self.ctx.types.resolve(actual);

        

        if expected == actual {

            return Ok(());

        }



        let t1 = self.ctx.types.get(expected).clone();

        let t2 = self.ctx.types.get(actual).clone();

        

        match (t1, t2) {

            (SemanticType::InferenceVar(var), _) => {

                self.ctx.types.inference_bindings.insert(var, actual);

                Ok(())

            }

            (_, SemanticType::InferenceVar(var)) => {

                self.ctx.types.inference_bindings.insert(var, expected);

                Ok(())

            }

            (_, SemanticType::Never) => Ok(()),

            (SemanticType::Pointer(m1, i1), SemanticType::Pointer(m2, i2)) if m1 == m2 => {

                self.unify(i1, i2)

            }

            (SemanticType::Reference(_, m1, i1), SemanticType::Reference(_, m2, i2)) if m1 == m2 => {

                self.unify(i1, i2)

            }

            (SemanticType::Pointer(m1, i1), SemanticType::Reference(_, m2, i2)) if m1 == m2 => {

                self.unify(i1, i2)

            }

            (SemanticType::Reference(_, m1, i1), SemanticType::Pointer(m2, i2)) if m1 == m2 => {

                self.unify(i1, i2)

            }

            (SemanticType::Tuple(els1), SemanticType::Tuple(els2)) if els1.len() == els2.len() => {

                for (e1, e2) in els1.into_iter().zip(els2.into_iter()) {

                    self.unify(e1, e2)?;

                }

                Ok(())

            }

            (SemanticType::Array(i1, s1), SemanticType::Array(i2, s2)) if s1 == s2 => {

                self.unify(i1, i2)

            }

            (SemanticType::Slice(i1), SemanticType::Slice(i2)) => {

                self.unify(i1, i2)

            }

            (SemanticType::Closure(_, p1, r1), SemanticType::Closure(_, p2, r2)) if p1.len() == p2.len() => {

                for (a, b) in p1.into_iter().zip(p2.into_iter()) {

                    self.unify(a, b)?;

                }

                self.unify(r1, r2)

            }

            (SemanticType::Future(o1), SemanticType::Future(o2)) => {

                self.unify(o1, o2)

            }

            (SemanticType::Primitive(p1), SemanticType::Primitive(p2)) => {

                let is_int = |p: &crate::ty::BuiltinType| matches!(p, crate::ty::BuiltinType::I8 | crate::ty::BuiltinType::I16 | crate::ty::BuiltinType::I32 | crate::ty::BuiltinType::I64 | crate::ty::BuiltinType::I128 | crate::ty::BuiltinType::Isize | crate::ty::BuiltinType::U8 | crate::ty::BuiltinType::U16 | crate::ty::BuiltinType::U32 | crate::ty::BuiltinType::U64 | crate::ty::BuiltinType::U128 | crate::ty::BuiltinType::Usize);

                if p1 == p2 {

                    Ok(())

                } else if is_int(&p1) && is_int(&p2) {

                    Ok(())

                } else {

                    Err(format!("Type mismatch"))

                }

            }

            _ => Err(format!("Type mismatch")),

        }

    }



    pub fn is_ffi_safe(&self, ty_id: SemanticTypeId) -> Result<(), String> {

        let ty = self.ctx.types.get(ty_id).clone();

        match ty {

            SemanticType::Primitive(_) | SemanticType::Void | SemanticType::Never => Ok(()),

            SemanticType::Pointer(_, _inner) | SemanticType::Reference(_, _, _inner) => {

                // The C-ABI doesn't strictly require the pointee to be FFI-safe if it's opaque,

                // but generally it's safer to ensure pointees are also FFI-safe. 

                // We'll just allow it for now since C pointers can point to opaque structs.

                Ok(())

            },

                self.is_ffi_safe(return_type)?;

                Ok(())

            },

            _ => Err(format!("Type is not FFI-safe")),

        }

    }



    pub fn typecheck_items(&mut self, items: &[Item]) {

        self.populate_signatures(items);

        for item in items {

            self.typecheck_item(item);

        }

    }



    fn populate_signatures(&mut self, items: &[Item]) {

        // Pass 1: Declare all struct/enum/type types

        for item in items {

            if let Item::Decl(decl_id) = item {

                let decl = &self.arena.decls[decl_id.0 as usize];

                match decl {

                    Decl::Struct { fields, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        let mut field_tys = Vec::new();

                        for field in fields {

                            field_tys.push(self.lower_type(field.ty));

                        }

                        if let Some(sym_id) = sym_id_opt {

                            let struct_ty = self.ctx.types.intern(SemanticType::Struct(sym_id, field_tys));

                            self.ctx.tables.symbol_types.insert(sym_id, struct_ty);

                        }

                    }

                    Decl::Enum { variants, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        let mut variant_tys = Vec::new();

                        for variant in variants {

                            // Each variant's type is based on its fields

                            let mut field_tys = Vec::new();

                            for field_id in &variant.fields {

                                if let Decl::Param { ty, .. } = &self.arena.decls[field_id.0 as usize] {



    pub fn typecheck_items(&mut self, items: &[Item]) {

        for (i, decl) in self.arena.decls.iter().enumerate() {

            if let Decl::Function { name, generic_params, .. } = decl {

                let name_str = &self.source[name.start as usize..name.end as usize];

                println!("DEBUG TYPECHECK_START: decl_id={} name={} generic_params={:?}", i, name_str, generic_params);

            }

        }

        self.populate_signatures(items);

        for item in items {

            self.typecheck_item(item);

        }

    }



    fn populate_signatures(&mut self, items: &[Item]) {

        // Pass 1: Declare all struct/enum/type types

        for item in items {

            if let Item::Decl(decl_id) = item {

                let decl = &self.arena.decls[decl_id.0 as usize];

                match decl {

                    Decl::Struct { fields, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        let mut field_tys = Vec::new();

                        for field in fields {

                            field_tys.push(self.lower_type(field.ty));

                        }

                        if let Some(sym_id) = sym_id_opt {

                            let struct_ty = self.ctx.types.intern(SemanticType::Struct(sym_id, field_tys));

                            self.ctx.tables.symbol_types.insert(sym_id, struct_ty);

                        }

                    }

                    Decl::Enum { variants, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        let mut variant_tys = Vec::new();

                        for variant in variants {

                            // Each variant's type is based on its fields

                            let mut field_tys = Vec::new();

                            for field_id in &variant.fields {

                                if let Decl::Param { ty, .. } = &self.arena.decls[field_id.0 as usize] {

                                    if let Some(t) = ty {

                                        field_tys.push(self.lower_type(*t));

                        if let Some(sym_id) = sym_id_opt {

                            let enum_ty = self.ctx.types.intern(SemanticType::Enum(sym_id, variant_tys));

                            self.ctx.tables.symbol_types.insert(sym_id, enum_ty);

                        }

                    }

                    _ => {}

                }

            }

        }



        // Pass 1.5: Early evaluation of top-level constants

        let mut const_inits = Vec::new();

        for item in items {

            if let Item::Decl(decl_id) = item {

                let decl = &self.arena.decls[decl_id.0 as usize];

                if let Decl::Var { is_const: true, initializer: Some(init), .. } = decl {

                    const_inits.push((*decl_id, *init));

                }

            }

        }



        for (decl_id, init) in const_inits {

            self.typecheck_expr(&init);

            if let Ok(val) = self.eval_comptime_expr(init) {

                self.ctx.comptime_values.insert(init, val.clone());

                if let Some(sym_id) = self.ctx.tables.decl_symbols.get(&decl_id).copied() {

                    self.ctx.const_values.insert(sym_id, val);

                }

            }

        }



        // Pass 2: Functions and global variables

        for item in items {

            if let Item::Decl(decl_id) = item {

                let decl = &self.arena.decls[decl_id.0 as usize];

                match decl {

                    Decl::Function { params, generic_params, return_type, is_async, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        

                        // Set generic scope so lower_type can resolve TypeParam names

                        let prev_generic_scope = self.current_generic_scope;

                        if !generic_params.is_empty() {

                            if let Some(sym_id) = sym_id_opt {

                                self.current_generic_scope = self.ctx.symbol_table.get_symbol(sym_id).inner_scope;

                            }

                        }

                        

                        // Populate trait_bounds for each generic parameter

                        for (gp_idx, gp) in generic_params.iter().enumerate() {

                            if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(*decl_id, gp_idx)) {

                                let mut bounds = Vec::new();

                                for &bound_ty_id in &gp.bounds {

                                    // Resolve the bound type as a trait symbol from global scope

                                    let bound_ast_ty = &self.arena.types[bound_ty_id.0 as usize];

                                    if let mellis_ast::Type::Named { segments, .. } = bound_ast_ty {

                                        let bound_name = segments.last().map(|s| &self.source[s.start as usize..s.end as usize]);

                                        if let Some(trait_sym) = bound_name.and_then(|n| self.ctx.symbol_table.lookup(n, crate::ScopeId(0))) {

                                            bounds.push(crate::semantic_tables::TraitBound { param: gp_sym, trait_id: trait_sym });

                                        }

                                    }

                                }

                                self.ctx.tables.trait_bounds.insert(gp_sym, bounds);

                            }

                        }

                        

                        let mut param_tys = Vec::new();

                        for param_id in params {

                            if let Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {

                                let param_ty = if let Some(t) = ty { self.lower_type(*t) } else { self.ctx.types.new_inference_var() };

                                param_tys.push(param_ty);

                                if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {

                                    self.ctx.tables.symbol_types.insert(*param_sym, param_ty);

                                }

                            }

                        }

                        let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };

                        let actual_ret_ty = if *is_async {

                            self.ctx.types.intern(SemanticType::Future(ret_ty))

                        } else {

                            ret_ty

                        };

                        let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: actual_ret_ty });

                        if let Some(sym_id) = sym_id_opt {

                            self.ctx.tables.symbol_types.insert(sym_id, func_ty);

                            self.ctx.tables.function_effects.entry(sym_id).or_insert_with(crate::effect::EffectSet::pure);

                        }

                        

                        self.current_generic_scope = prev_generic_scope;

                    }

                    Decl::Var { type_annot, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        let mut ty = self.ctx.types.new_inference_var();

                        if let Some(annot) = type_annot {

                            ty = self.lower_type(*annot);

                        }

                        if let Some(sym_id) = sym_id_opt {

                            self.ctx.tables.symbol_types.insert(sym_id, ty);

                        }

                    }

                    Decl::Trait { methods, .. } => {

                        for method_id in methods {

                            let method_decl = &self.arena.decls[method_id.0 as usize];

                            if let Decl::Function { params, return_type, .. } = method_decl {

                                let sym_id_opt = self.ctx.tables.decl_symbols.get(method_id).copied();

                                let mut param_tys = Vec::new();

                                for param_id in params {

                                    if let Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {

                                        let param_ty = if let Some(t) = ty { self.lower_type(*t) } else { self.ctx.types.new_inference_var() };

                                        param_tys.push(param_ty);

                                        if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {

                                            self.ctx.tables.symbol_types.insert(*param_sym, param_ty);

                                        }

                                    }

                                }

                                let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };

                                let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: ret_ty });

                                if let Some(sym_id) = sym_id_opt {

                                    self.ctx.tables.symbol_types.insert(sym_id, func_ty);

                                }

                            }

                        }

                    }

                    Decl::Impl { generic_params, self_type, methods, .. } => {

                        if generic_params.is_empty() {

                            let self_sem_ty = self.lower_type(*self_type);

                            let prev_self = self.current_self_type;

                            self.current_self_type = Some(self_sem_ty);

                            for method_id in methods {

                                let method_decl = &self.arena.decls[method_id.0 as usize];

                                if let Decl::Function { params, return_type, is_async, .. } = method_decl {

                                    let sym_id_opt = self.ctx.tables.decl_symbols.get(method_id).copied();

                                    let mut param_tys = Vec::new();

                                    for param_id in params {

                                        if let Decl::Param { ty, is_self, .. } = &self.arena.decls[param_id.0 as usize] {

                                            let param_ty = if let Some(t) = ty {

                                                self.lower_type(*t)

                                            } else if *is_self {

                                                self_sem_ty

                                            } else {

                                                self.ctx.types.new_inference_var()

                                            };

                                            param_tys.push(param_ty);

                                            if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {

                                                self.ctx.tables.symbol_types.insert(*param_sym, param_ty);

                                            }

                                        }

                                    }

                                    let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };

                                    let actual_ret_ty = if *is_async {

                        let prev_self = self.current_self_type;

                        self.current_self_type = Some(self_sem_ty);

                        for method_id in methods {

                            let method_decl = &self.arena.decls[method_id.0 as usize];

                            if let Decl::Function { params, return_type, is_async, .. } = method_decl {

                                    let sym_id_opt = self.ctx.tables.decl_symbols.get(method_id).copied();

                                    let mut param_tys = Vec::new();

                                    for param_id in params {

                                        if let Decl::Param { ty, is_self, .. } = &self.arena.decls[param_id.0 as usize] {

                                            let param_ty = if let Some(t) = ty {

                                                self.lower_type(*t)

                                            } else if *is_self {

                                                self_sem_ty

                                            } else {

                                                self.ctx.types.new_inference_var()

                                            };

                                            param_tys.push(param_ty);

                                            if let Some(param_sym) = self.ctx.tables.decl_symbols.get(param_id) {

                                                self.ctx.tables.symbol_types.insert(*param_sym, param_ty);

                                            }

                                        }

                                    }

                                    let ret_ty = if let Some(r) = return_type { self.lower_type(*r) } else { self.ctx.types.intern(SemanticType::Void) };

                                    let actual_ret_ty = if *is_async {

                                        self.ctx.types.intern(SemanticType::Future(ret_ty))

                                    } else {

                                        ret_ty

                                    };

                                    let func_ty = self.ctx.types.intern(SemanticType::Function { params: param_tys, return_type: actual_ret_ty });

                                    if let Some(sym_id) = sym_id_opt {

                                        self.ctx.tables.symbol_types.insert(sym_id, func_ty);

                                    }

                                }

                            }

                            self.current_self_type = prev_self;

                            self.current_generic_scope = prev_generic_scope;

                        }

                    }

                    _ => {}

                }

            }

        }



        // Pass 3: Check Drop Impls & Trait Impls

        for item in items {

            if let Item::Decl(decl_id) = item {

                let decl = &self.arena.decls[decl_id.0 as usize];

                if let Decl::Impl { trait_type, self_type, methods, .. } = decl {

                    let self_ast_ty = &self.arena.types[self_type.0 as usize];

                    let self_name_opt = if let mellis_ast::Type::Named { segments, .. } = self_ast_ty {

                        segments.last().map(|s| &self.source[s.start as usize..s.end as usize])

                    

                    let self_sym_opt = self_name_opt.and_then(|name| {

                        self.ctx.symbol_table.lookup(name, crate::ScopeId(0))

                    });



                    if let Some(trait_ty_id) = trait_type {

                        let trait_ast_ty = &self.arena.types[trait_ty_id.0 as usize];

                        if let mellis_ast::Type::Named { segments, .. } = trait_ast_ty {

                            if let Some(last_seg) = segments.last() {

                                let trait_name = &self.source[last_seg.start as usize..last_seg.end as usize];

                        if let Some(last_seg) = segments.last() {

                            let ret_name = &self.source[last_seg.start as usize..last_seg.end as usize];

                            if ret_name == "Self" {

                                let mut diag = Diagnostic::error(format!(

                                    "Trait `{}` cannot be made into an object because method `{}` returns `Self`",

                                    self.ctx.symbol_table.get_symbol(trait_sym).name,

                                    method_name

                                ));

                                if let Some(sp) = span { diag = diag.with_span(sp); }

                                self.ctx.diagnostics.push(diag);

                                return false;

                            }

                        }

                    }

                if let Decl::Param { is_self, .. } = first_param {

                    if !*is_self {

                        let mut diag = Diagnostic::error(format!(

                            "Trait `{}` cannot be made into an object because method `{}` has no `self` receiver",

                            self.ctx.symbol_table.get_symbol(trait_sym).name,

                            method_name

                        ));

                        if let Some(sp) = span { diag = diag.with_span(sp); }

                        self.ctx.diagnostics.push(diag);

                        return false;

                    }

                }

                

                // Rule 3: Return type cannot be unboxed `Self`

                if let Some(ret_ty_id) = return_type {

                    let ret_ast = &self.arena.types[ret_ty_id.0 as usize];

                    if let mellis_ast::Type::Named { segments, .. } = ret_ast {

                        if let Some(last_seg) = segments.last() {

                            let ret_name = &self.source[last_seg.start as usize..last_seg.end as usize];

                            if ret_name == "Self" {

                                let mut diag = Diagnostic::error(format!(

            }

            (_, SemanticType::DynTrait(trait_sym)) => {

                (from_ty, *trait_sym)

            }

            _ => return false,

        };

        

        let f_ty = self.ctx.types.get(from_inner).clone();

        let concrete_sym = match f_ty {

            SemanticType::Struct(s_sym, _) => s_sym,

            _ => return false,

        };

        

        let key = crate::semantic_tables::ImplKey {

    }



    pub fn try_coerce_dyn(&mut self, expr_id: mellis_ast::ExprId, from_ty: SemanticTypeId, to_ty: SemanticTypeId) -> bool {

        let from = self.ctx.types.get(from_ty).clone();

        let to = self.ctx.types.get(to_ty).clone();

        

        let (from_inner, to_trait) = match (&from, &to) {

            (SemanticType::Pointer(_, f_in), SemanticType::Pointer(_, t_in))

            | (SemanticType::Reference(_, _, f_in), SemanticType::Reference(_, _, t_in))

            | (SemanticType::Pointer(_, f_in), SemanticType::Reference(_, _, t_in))



    pub fn try_coerce_dyn(&mut self, expr_id: mellis_ast::ExprId, from_ty: SemanticTypeId, to_ty: SemanticTypeId) -> bool {

        let from = self.ctx.types.get(from_ty).clone();

        let to = self.ctx.types.get(to_ty).clone();

        

        let (from_inner, to_trait) = match (&from, &to) {

            (SemanticType::Pointer(_, f_in), SemanticType::Pointer(_, t_in))

            | (SemanticType::Reference(_, _, f_in), SemanticType::Reference(_, _, t_in))

            | (SemanticType::Pointer(_, f_in), SemanticType::Reference(_, _, t_in))

            | (SemanticType::Reference(_, _, f_in), SemanticType::Pointer(_, t_in)) => {

                let t_ty = self.ctx.types.get(*t_in).clone();

                if let SemanticType::DynTrait(trait_sym) = t_ty {

                    (*f_in, trait_sym)

                } else {

                    return false;

                }

            }

            (_, SemanticType::DynTrait(trait_sym)) => {

                (from_ty, *trait_sym)

            }

            _ => return false,

        };

        

        let f_ty = self.ctx.types.get(from_inner).clone();

        let concrete_sym = match f_ty {

            SemanticType::Struct(s_sym, _) => s_sym,

            _ => return false,

        };

        

        let key = crate::semantic_tables::ImplKey {

            trait_id: Some(to_trait),

            self_type_def: concrete_sym,

        };

        if self.ctx.tables.trait_impls.contains_key(&key) {

            self.ctx.tables.dyn_coercions.insert(expr_id, (to_trait, concrete_sym));

            return true;

        }

        false

    }



    pub fn lower_type(&mut self, ast_ty_id: mellis_ast::TypeId) -> SemanticTypeId {

                    BuiltinKind::I128 => BuiltinType::I128,

                    BuiltinKind::Isize => BuiltinType::Isize,

                    BuiltinKind::U8 => BuiltinType::U8,

                    BuiltinKind::U16 => BuiltinType::U16,

                    BuiltinKind::U32 => BuiltinType::U32,

                    BuiltinKind::U64 => BuiltinType::U64,

                    BuiltinKind::U128 => BuiltinType::U128,

                    BuiltinKind::Usize => BuiltinType::Usize,

                    BuiltinKind::F32 => BuiltinType::F32,

                    BuiltinKind::F64 => BuiltinType::F64,

                    BuiltinKind::Bool => BuiltinType::Bool,

                    BuiltinKind::Str => BuiltinType::String,

                    BuiltinKind::Char => BuiltinType::Char,

                    BuiltinKind::Void => return self.ctx.types.intern(SemanticType::Void),

                };

                self.ctx.types.intern(SemanticType::Primitive(builtin))

            }

            Type::Named { segments, generic_args, .. } => {

                let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);

                if let Some("Self") = name {

            Type::Slice { inner } => {

                let inner_ty = self.lower_type(*inner);

                self.ctx.types.intern(SemanticType::Slice(inner_ty))

            }

            Type::Array { element_type, size } => {

                let inner_ty = self.lower_type(*element_type);

                let resolved_size = match self.eval_comptime_expr(*size) {

                    Ok(val) => val.as_usize().unwrap_or(0) as u64,

                    Err(e) => {

                    BuiltinKind::Bool => BuiltinType::Bool,

                    BuiltinKind::Str => BuiltinType::String,

                    BuiltinKind::Char => BuiltinType::Char,

                    BuiltinKind::Void => return self.ctx.types.intern(SemanticType::Void),

                };

                self.ctx.types.intern(SemanticType::Primitive(builtin))

            }

            Type::Named { segments, generic_args, .. } => {

                let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);

                if let Some("Self") = name {

                    if let Some(self_ty) = self.current_self_type {

                        return self_ty;

                    }

                }

                if let Some("Future") = name {

                    if let Some(&arg) = generic_args.first() {

                        let out_ty = self.lower_type(arg);

                        return self.ctx.types.intern(SemanticType::Future(out_ty));

                    }

                }

                // Check for TypeParam symbols in the current generic scope first

                if let Some(name_str) = name {

                    if let Some(generic_scope) = self.current_generic_scope {

                        if let Some(sym) = self.ctx.symbol_table.lookup_exact(name_str, generic_scope) {

                            if matches!(self.ctx.symbol_table.get_symbol(sym).kind, crate::SymbolKind::TypeParam) {

                                return self.ctx.types.intern(SemanticType::GenericParam(sym));

                            }

                        }

                    }

                    // Fall back to global scope for concrete named types

                    if let Some(symbol) = self.ctx.symbol_table.lookup(name_str, crate::ScopeId(0)) {

                        self.ctx.tables.symbol_types.get(&symbol).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())

                    } else {

                        self.ctx.types.new_inference_var()

                    }

    pub fn lower_type(&mut self, ast_ty_id: mellis_ast::TypeId) -> SemanticTypeId {

        use mellis_ast::Type;

        let ast_ty = &self.arena.types[ast_ty_id.0 as usize];

        let sem_ty_id = match ast_ty {

            Type::Builtin(kind) => {

                let builtin = match kind {

                    BuiltinKind::I8 => BuiltinType::I8,

                    BuiltinKind::I16 => BuiltinType::I16,

                    BuiltinKind::I32 => BuiltinType::I32,

                    BuiltinKind::I64 => BuiltinType::I64,

                    BuiltinKind::I128 => BuiltinType::I128,

                    BuiltinKind::Isize => BuiltinType::Isize,

                    BuiltinKind::U8 => BuiltinType::U8,

                    BuiltinKind::U16 => BuiltinType::U16,

                    BuiltinKind::U32 => BuiltinType::U32,

                    BuiltinKind::U64 => BuiltinType::U64,

                    BuiltinKind::U128 => BuiltinType::U128,

                    BuiltinKind::Usize => BuiltinType::Usize,

                    BuiltinKind::F32 => BuiltinType::F32,

                    BuiltinKind::F64 => BuiltinType::F64,

                    BuiltinKind::Bool => BuiltinType::Bool,

                    BuiltinKind::Str => BuiltinType::String,

                    BuiltinKind::Char => BuiltinType::Char,

                    BuiltinKind::Void => return self.ctx.types.intern(SemanticType::Void),

                };

            Type::Named { segments, generic_args, .. } => {

                let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);

                if let Some("Self") = name {

                    if let Some(self_ty) = self.current_self_type {

                        return self_ty;

                    }

                }

                if let Some("Future") = name {

                    if let Some(&arg) = generic_args.first() {

                        let out_ty = self.lower_type(arg);

                self.ctx.types.intern(SemanticType::Primitive(builtin))

            }

            Type::Named { segments, generic_args, .. } => {

                let name = segments.last().map(|span| &self.source[span.start as usize..span.end as usize]);

                if let Some("Self") = name {

                    if let Some(self_ty) = self.current_self_type {

                        return self_ty;

                    }

                }

                if let Some("Future") = name {

                    if let Some(&arg) = generic_args.first() {

                        let out_ty = self.lower_type(arg);

                        return self.ctx.types.intern(SemanticType::Future(out_ty));

                    }

                }

                // Check for TypeParam symbols in the current generic scope first

                if let Some(name_str) = name {

                    if let Some(generic_scope) = self.current_generic_scope {

                        if let Some(sym) = self.ctx.symbol_table.lookup_exact(name_str, generic_scope) {

                            if matches!(self.ctx.symbol_table.get_symbol(sym).kind, crate::SymbolKind::TypeParam) {

                                return self.ctx.types.intern(SemanticType::GenericParam(sym));

                            }

                        }

                    }

                    // Fall back to global scope for concrete named types

                    if let Some(symbol) = self.ctx.symbol_table.lookup(name_str, crate::ScopeId(0)) {

                        self.ctx.tables.symbol_types.get(&symbol).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())

                    } else {

                        self.ctx.types.new_inference_var()

                    }

                } else {

                    self.ctx.types.new_inference_var()

                }

            }

            Type::Tuple { elements } => {

                let mut elem_tys = Vec::new();

                for &e in elements {

                    elem_tys.push(self.lower_type(e));

                }

                self.ctx.types.intern(SemanticType::Tuple(elem_tys))

            }

                        if let Some(body_stmt) = body {

                            self.typecheck_stmt(body_stmt);

                        }

                        self.current_return_type.pop();

                        self.current_async_fn = prev_async;

                    }

                    Decl::Var { name, initializer, pattern, type_annot, is_const, .. } => {

                        let mut init_ty = if let Some(init) = initializer {

                            self.typecheck_expr(init)

                        } else {

                        

                        if let Some(pat_id) = pattern {

                            self.typecheck_pattern(pat_id, init_ty);

                            if !self.is_pattern_irrefutable(pat_id, init_ty) {

                    Decl::Var { name, initializer, pattern, type_annot, is_const, .. } => {

                        let mut init_ty = if let Some(init) = initializer {

                            self.typecheck_expr(init)

                        } else {

                            self.ctx.types.new_inference_var()

                        self.current_return_type.pop();

                        self.current_async_fn = prev_async;

                        self.current_generic_scope = prev_generic_scope;

                    }

                    Decl::Var { name, initializer, pattern, type_annot, is_const, .. } => {

                        let mut init_ty = if let Some(init) = initializer {

                            self.typecheck_expr(init)

                        } else {

                            self.ctx.types.new_inference_var()

                        };

                        

                        if let Some(annot) = type_annot {

                            let expected_ty = self.lower_type(*annot);

                            if let Some(init) = initializer {

                                if !self.try_coerce_dyn(*init, init_ty, expected_ty) {

                                    if self.unify(expected_ty, init_ty).is_err() {

                                        println!("typeof unify fail: {:?} vs {:?}", self.ctx.types.get(expected_ty), self.ctx.types.get(init_ty));

                                        self.emit_type_mismatch("Type mismatch in variable initialization", init);

                                    }

                                } else {

                                    init_ty = expected_ty;

                                }

                            }

                        }

                        

                        if *is_const {

                            if let Some(init) = initializer {

                                match self.eval_comptime_expr(*init) {

                                    Ok(val) => {

                                        self.ctx.comptime_values.insert(*init, val.clone());

                                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {

                                            self.ctx.const_values.insert(sym_id, val);

                                        }

                                    }

                                    Err(e) => {

    fn typecheck_item(&mut self, item: &Item) {

        match item {

            Item::Decl(decl_id) => {

                let decl = &self.arena.decls[decl_id.0 as usize];

                match decl {

                    Decl::Function { body, is_async, return_type, generic_params, .. } => {

                        let prev_async = self.current_async_fn;

                        let prev_generic_scope = self.current_generic_scope;

                        if *is_async {

                            self.current_async_fn = Some(*decl_id);

                        } else {

                            self.current_async_fn = None;

                        }

                        // Set generic scope for the body so lower_type can see TypeParam symbols

                        if !generic_params.is_empty() {

                            if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {

                                self.current_generic_scope = self.ctx.symbol_table.get_symbol(sym_id).inner_scope;

                            }

                        }

                        let ret_ty = if let Some(r) = return_type {

                            self.lower_type(*r)

                        } else {

                            self.ctx.types.intern(SemanticType::Void)

                        };

                        self.current_return_type.push(ret_ty);

                        if let Some(body_stmt) = body {

                            self.typecheck_stmt(body_stmt);

                        }

                        self.current_return_type.pop();

                        self.current_async_fn = prev_async;

                        self.current_generic_scope = prev_generic_scope;

                    }

                    Decl::Var { name, initializer, pattern, type_annot, is_const, .. } => {

                        let mut init_ty = if let Some(init) = initializer {

                            self.typecheck_expr(init)

                        } else {

                            self.ctx.types.new_inference_var()

                        };

                        

                        if let Some(annot) = type_annot {

                            let expected_ty = self.lower_type(*annot);

                            if let Some(init) = initializer {

                                if !self.try_coerce_dyn(*init, init_ty, expected_ty) {

                                    if self.unify(expected_ty, init_ty).is_err() {

                                        println!("typeof unify fail: {:?} vs {:?}", self.ctx.types.get(expected_ty), self.ctx.types.get(init_ty));

                                        self.emit_type_mismatch("Type mismatch in variable initialization", init);

                                    }

                                } else {

                                    init_ty = expected_ty;

                                }

                            }

                        }

                        

                        if *is_const {

                            if let Some(init) = initializer {

                                match self.eval_comptime_expr(*init) {

                                    Ok(val) => {

                                        self.ctx.comptime_values.insert(*init, val.clone());

                                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {

                                            self.ctx.const_values.insert(sym_id, val);

                                        }

                                    }

                                    Err(e) => {

                                        self.ctx.diagnostics.push(Diagnostic::error(format!("cannot evaluate constant in comptime: {}", e)).with_span(*name));

                                    }

                                }

                            }

                        }



                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(decl_id).copied() {

                            let mut needs_update = false;

                            if let Some(current_ty) = self.ctx.tables.symbol_types.get(&sym_id).copied() {

                                let resolved = self.ctx.types.get(current_ty).clone();

                                if matches!(resolved, SemanticType::InferenceVar(_)) {

                                    needs_update = true;

                                }

                    Decl::Extern { func, .. } => {

                        // The inner func is registered, we must check its FFI safety

                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(func).copied() {

                            let mut eff = crate::effect::EffectSet::pure();

                            eff.add(crate::effect::Effect::Extern);

                            eff.add(crate::effect::Effect::IO);

                            self.ctx.tables.function_effects.insert(sym_id, eff);



                            if let Some(func_ty) = self.ctx.tables.symbol_types.get(&sym_id).copied() {

                                if let Err(e) = self.is_ffi_safe(func_ty) {

                                    self.ctx.diagnostics.push(Diagnostic::error(format!("Extern function signature is not FFI-safe: {}", e)));

                                }

                            }

                        }

                    }

                    _ => {}

                }

            }

            Item::Stmt(stmt_id) => {

                    Decl::Impl { generic_params, self_type, methods, .. } => {

                        let sym_id_opt = self.ctx.tables.decl_symbols.get(decl_id).copied();

                        let prev_generic_scope = self.current_generic_scope;

                        if !generic_params.is_empty() {

                            if let Some(sym_id) = sym_id_opt {

                                self.current_generic_scope = self.ctx.symbol_table.get_symbol(sym_id).inner_scope;

                            }

                        }



                        let self_sem_ty = self.lower_type(*self_type);

                        let prev_self = self.current_self_type;

                        self.current_self_type = Some(self_sem_ty);

                        for method_id in methods {

                            let method_decl = &self.arena.decls[method_id.0 as usize];

                            if let Decl::Function { body: Some(body_stmt), is_async, return_type, .. } = method_decl {

                                let prev_async = self.current_async_fn;

                                if *is_async {

                                    self.current_async_fn = Some(*method_id);

                                } else {

                                    self.current_async_fn = None;

                                }

                                let ret_ty = if let Some(r) = return_type {

                                    self.lower_type(*r)

                                } else {

                                    self.ctx.types.intern(SemanticType::Void)

                                };

                                self.current_return_type.push(ret_ty);

                                self.typecheck_stmt(body_stmt);

                                self.current_return_type.pop();

                                self.current_async_fn = prev_async;

                            }

                        }

                        self.current_self_type = prev_self;

                        self.current_generic_scope = prev_generic_scope;

                    }

                    Decl::Extern { func, .. } => {

                        // The inner func is registered, we must check its FFI safety

                        if let Some(sym_id) = self.ctx.tables.decl_symbols.get(func).copied() {

                            let mut eff = crate::effect::EffectSet::pure();

                            eff.add(crate::effect::Effect::Extern);

                            eff.add(crate::effect::Effect::IO);

                            self.ctx.tables.function_effects.insert(sym_id, eff);



                            if let Some(func_ty) = self.ctx.tables.symbol_types.get(&sym_id).copied() {

                                if let Err(e) = self.is_ffi_safe(func_ty) {

                                    self.ctx.diagnostics.push(Diagnostic::error(format!("Extern function signature is not FFI-safe: {}", e)));

                                }

                            }

                        }

                    }

                    _ => {}

                    if let crate::symbol::SymbolKind::EnumVariant(variant_idx) = variant_sym.kind {

                        if let Some(decl_id) = variant_sym.decl_id {

                            if let mellis_ast::Decl::Enum { variants, .. } = &self.arena.decls[decl_id.0 as usize] {

                                // Must be exactly 1 variant for enum destructuring to be irrefutable!

                                if variants.len() == 1 {

                                    let mut variant_payload_tys = Vec::new();

                                    if let Some(variant) = variants.get(variant_idx as usize) {

                                        for &param_decl_id in &variant.fields {

                                            if let mellis_ast::Decl::Param { ty: Some(ty_id), .. } = &self.arena.decls[param_decl_id.0 as usize] {

                                                let sem_ty = self.ctx.tables.ast_type_to_semantic.get(ty_id).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());

                                                variant_payload_tys.push(sem_ty);

                                            } else {

                                                variant_payload_tys.push(self.ctx.types.new_inference_var());

                                            }

                                        }

                                    }

                                    return fields.iter().enumerate().all(|(i, field)| {

                                        let field_ty = variant_payload_tys.get(i).copied().unwrap_or_else(|| self.ctx.types.new_inference_var());

                                        self.is_pattern_irrefutable(field, field_ty)

                                    });

            Stmt::Unsafe { body } => {

                let old = self.is_unsafe_context;

                self.is_unsafe_context = true;

                self.typecheck_stmt(body);

                self.is_unsafe_context = old;

            }

            _ => {}

        }

    }



    fn resolve_type(&mut self, ty_id: mellis_ast::TypeId) -> SemanticTypeId {

        self.ctx.tables.ast_type_to_semantic.get(&ty_id).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())

    }



    fn typecheck_expr(&mut self, expr_id: &mellis_ast::ExprId) -> SemanticTypeId {

        let expr = &self.arena.exprs[expr_id.0 as usize];

        let ty_id = match expr {

            Expr::Literal(tok) => {

                // Determine type based on literal token type

                let kind = match tok.kind {

                    TokenKind::IntegerLiteral => SemanticType::Primitive(BuiltinType::I32), // Default to i32

                    TokenKind::FloatLiteral => SemanticType::Primitive(BuiltinType::F64), // Default to f64

                    TokenKind::StringLiteral => SemanticType::Primitive(BuiltinType::String),

                    TokenKind::CharLiteral => SemanticType::Primitive(BuiltinType::Char),

                    TokenKind::KwTrue | TokenKind::KwFalse => SemanticType::Primitive(BuiltinType::Bool),

                    _ => SemanticType::Error,

                };

                self.ctx.types.intern(kind)

            }

            Expr::Identifier { .. } => {

                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {

                    if let Some(ty) = self.ctx.tables.symbol_types.get(sym_id) {

                        *ty

                    } else {

                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);

                        if let crate::SymbolKind::EnumVariant(_) = symbol.kind {

                            if let Some(decl_id) = symbol.decl_id {

                                if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {

                                    if let Some(ty) = self.ctx.tables.symbol_types.get(enum_sym_id) {

                                        *ty

                // For simplified logic: require left and right to be same, return left type

                let _ = self.unify(l_ty, r_ty); 

                l_ty 

    fn typecheck_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {

        let stmt = &self.arena.stmts[stmt_id.0 as usize];

        match stmt {

            Stmt::Block { body, tail_expr } => {

                self.typecheck_items(body);

                if let Some(expr) = tail_expr {

                    self.typecheck_expr(expr);

                }

            }

            Stmt::Expr { expr, .. } => {

                self.typecheck_expr(expr);

            }

            Stmt::If { condition, then_branch, else_branch } => {

                let cond_ty = self.typecheck_expr(condition);

                let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                if self.unify(bool_ty, cond_ty).is_err() {

                    self.ctx.diagnostics.push(Diagnostic::error("if condition must be a boolean"));

                }

                self.typecheck_stmt(then_branch);

                if let Some(else_br) = else_branch {

                    self.typecheck_stmt(else_br);

                }

            }

            Stmt::While { condition, body, .. } => {

                let cond_ty = self.typecheck_expr(condition);

                let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                if self.unify(bool_ty, cond_ty).is_err() {

                    self.ctx.diagnostics.push(Diagnostic::error("while condition must be a boolean"));

                }

                self.typecheck_stmt(body);

            }

            Stmt::For { init, cond, step, body, iterable, .. } => {

                if let Some(item) = init { self.typecheck_item(item); }

                if let Some(c) = cond { 

                    let cond_ty = self.typecheck_expr(c); 

                    let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                    if self.unify(bool_ty, cond_ty).is_err() {

                        self.ctx.diagnostics.push(Diagnostic::error("for condition must be a boolean"));

                    }

                }

                if let Some(s) = step { self.typecheck_expr(s); }

                if let Some(iter) = iterable { self.typecheck_expr(iter); }

                self.typecheck_stmt(body);

            }

            Stmt::Return { value } => {

                let expected_ty = self.current_return_type.last().copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Void));

                if let Some(val) = value {

                    let val_ty = self.typecheck_expr(val);

                    let _ = self.unify(expected_ty, val_ty);

                } else {

                    let void_ty = self.ctx.types.intern(SemanticType::Void);

                    let _ = self.unify(expected_ty, void_ty);

                }

            }

            Stmt::Unsafe { body } => {

                let old = self.is_unsafe_context;

                self.is_unsafe_context = true;

                self.typecheck_stmt(body);

                self.is_unsafe_context = old;

            }

            _ => {}

        }

    }



    fn resolve_type(&mut self, ty_id: mellis_ast::TypeId) -> SemanticTypeId {

        self.ctx.tables.ast_type_to_semantic.get(&ty_id).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())

    }



    fn typecheck_expr(&mut self, expr_id: &mellis_ast::ExprId) -> SemanticTypeId {

        let expr = &self.arena.exprs[expr_id.0 as usize];

        let ty_id = match expr {

            Expr::Literal(tok) => {

                // Determine type based on literal token type

                let kind = match tok.kind {

                    TokenKind::IntegerLiteral => SemanticType::Primitive(BuiltinType::I32), // Default to i32

                    TokenKind::FloatLiteral => SemanticType::Primitive(BuiltinType::F64), // Default to f64

                    TokenKind::StringLiteral => SemanticType::Primitive(BuiltinType::String),

                    TokenKind::CharLiteral => SemanticType::Primitive(BuiltinType::Char),

                    TokenKind::KwTrue | TokenKind::KwFalse => SemanticType::Primitive(BuiltinType::Bool),

                    _ => SemanticType::Error,

                };

                self.ctx.types.intern(kind)

            }

            Expr::Identifier { .. } => {

                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {

                    if let Some(ty) = self.ctx.tables.symbol_types.get(sym_id) {

                        *ty

                    } else {

                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);

                        if let crate::SymbolKind::EnumVariant(_) = symbol.kind {

                            if let Some(decl_id) = symbol.decl_id {

                                if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {

                                    if let Some(ty) = self.ctx.tables.symbol_types.get(enum_sym_id) {

                                        *ty

                                    } else {

            Expr::Identifier { .. } => {

                if let Some(item) = init { self.typecheck_item(item); }

                if let Some(c) = cond { 

                    let cond_ty = self.typecheck_expr(c); 

                    let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                    if self.unify(bool_ty, cond_ty).is_err() {

                        self.ctx.diagnostics.push(Diagnostic::error("for condition must be a boolean"));

                    }

                }

                if let Some(s) = step { self.typecheck_expr(s); }

                if let Some(iter) = iterable { self.typecheck_expr(iter); }

                self.typecheck_stmt(body);

            }

            Stmt::Return { value } => {

                let expected_ty = self.current_return_type.last().copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Void));

                if let Some(val) = value {

                    let val_ty = self.typecheck_expr(val);

                    let _ = self.unify(expected_ty, val_ty);

                } else {

                    let void_ty = self.ctx.types.intern(SemanticType::Void);

                    let _ = self.unify(expected_ty, void_ty);

                }

            }

            Stmt::Unsafe { body } => {

                let old = self.is_unsafe_context;

                self.is_unsafe_context = true;

                self.typecheck_stmt(body);

                self.is_unsafe_context = old;

            }

            _ => {}

        }

    }

                let l_sem_ty = self.ctx.types.get(l_ty).clone();

                let r_sem_ty = self.ctx.types.get(r_ty).clone();

                

                if matches!(l_sem_ty, SemanticType::Pointer(_, _)) {

                    use mellis_ast::expr::BinaryOp;

    fn typecheck_expr(&mut self, expr_id: &mellis_ast::ExprId) -> SemanticTypeId {

        let expr = &self.arena.exprs[expr_id.0 as usize];

    fn typecheck_stmt(&mut self, stmt_id: &mellis_ast::StmtId) {

        let stmt = &self.arena.stmts[stmt_id.0 as usize];

        match stmt {

            Stmt::Block { body, tail_expr } => {

                self.typecheck_items(body);

                if let Some(expr) = tail_expr {

                    self.typecheck_expr(expr);

                }

            }

            Stmt::Expr { expr, .. } => {

                self.typecheck_expr(expr);

            }

            Stmt::If { condition, then_branch, else_branch } => {

                let cond_ty = self.typecheck_expr(condition);

                let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                if self.unify(bool_ty, cond_ty).is_err() {

                    self.ctx.diagnostics.push(Diagnostic::error("if condition must be a boolean"));

                }

                self.typecheck_stmt(then_branch);

                if let Some(else_br) = else_branch {

                    self.typecheck_stmt(else_br);

                }

            }

            Stmt::While { condition, body, .. } => {

                let cond_ty = self.typecheck_expr(condition);

                let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                if self.unify(bool_ty, cond_ty).is_err() {

                    self.ctx.diagnostics.push(Diagnostic::error("while condition must be a boolean"));

                }

                self.typecheck_stmt(body);

            }

            Stmt::For { kind, init, cond, step, body, iterable, pattern, .. } => {

                if *kind == mellis_ast::stmt::ForKind::ForEach {

                    if let Some(iter) = iterable {

                        let iter_ty = self.typecheck_expr(iter);

                        if let Some(pat) = pattern {

                            if let SemanticType::Range(inner_ty) = self.ctx.types.get(iter_ty) {

                                let inner_ty_id = *inner_ty;

                                self.typecheck_pattern(pat, inner_ty_id);

                            }

                        }

                    }

                }



                if let Some(item) = init { self.typecheck_item(item); }

                if let Some(c) = cond { 

                    let cond_ty = self.typecheck_expr(c); 

                    let bool_ty = self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::Bool));

                    if self.unify(bool_ty, cond_ty).is_err() {

                        self.ctx.diagnostics.push(Diagnostic::error("for condition must be a boolean"));

                    }

                }

                if let Some(s) = step { self.typecheck_expr(s); }

                if let Some(iter) = iterable { self.typecheck_expr(iter); }

                self.typecheck_stmt(body);

            }

            Stmt::Return { value } => {

                let expected_ty = self.current_return_type.last().copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Void));

                if let Some(val) = value {

                    let val_ty = self.typecheck_expr(val);

                    if self.unify(expected_ty, val_ty).is_err() {

                        self.emit_type_mismatch("Type mismatch in return statement", val);

                    }

                } else {

                    let void_ty = self.ctx.types.intern(SemanticType::Void);

                    if self.unify(expected_ty, void_ty).is_err() {

                        self.ctx.diagnostics.push(Diagnostic::error("Type mismatch in return statement"));

                    }

                }

            }

            Stmt::Unsafe { body } => {

                let old = self.is_unsafe_context;

                self.is_unsafe_context = true;

                self.typecheck_stmt(body);

                self.is_unsafe_context = old;

            }

            _ => {}

        }

    }



    fn resolve_type(&mut self, ty_id: mellis_ast::TypeId) -> SemanticTypeId {

        self.ctx.tables.ast_type_to_semantic.get(&ty_id).copied().unwrap_or_else(|| self.ctx.types.new_inference_var())

    }



    fn typecheck_expr(&mut self, expr_id: &mellis_ast::ExprId) -> SemanticTypeId {

        let expr = &self.arena.exprs[expr_id.0 as usize];

        let ty_id = match expr {

            Expr::Literal(tok) => {

                // Determine type based on literal token type

                let kind = match tok.kind {

                    TokenKind::IntegerLiteral => SemanticType::Primitive(BuiltinType::I32), // Default to i32

                    TokenKind::FloatLiteral => SemanticType::Primitive(BuiltinType::F64), // Default to f64

                    TokenKind::StringLiteral => SemanticType::Primitive(BuiltinType::String),

                    TokenKind::CharLiteral => SemanticType::Primitive(BuiltinType::Char),

                    TokenKind::KwTrue | TokenKind::KwFalse => SemanticType::Primitive(BuiltinType::Bool),

                    _ => SemanticType::Error,

                };

                self.ctx.types.intern(kind)

            }

            Expr::Identifier { .. } => {

                if let Some(sym_id) = self.ctx.tables.expr_symbols.get(expr_id) {

                    if let Some(ty) = self.ctx.tables.symbol_types.get(sym_id) {

                        *ty

                    } else {

                        let symbol = self.ctx.symbol_table.get_symbol(*sym_id);

                        if let crate::SymbolKind::EnumVariant(_) = symbol.kind {

                            if let Some(decl_id) = symbol.decl_id {

                                if let Some(enum_sym_id) = self.ctx.tables.decl_symbols.get(&decl_id) {

                                    if let Some(ty) = self.ctx.tables.symbol_types.get(enum_sym_id) {

                                        *ty

                                    } else {

                                        self.ctx.types.new_inference_var()

                                    }

                                } else {

                                    self.ctx.types.new_inference_var()

                                }

                            } else {

                                self.ctx.types.new_inference_var()

                            }

                        } else {

                            self.ctx.types.new_inference_var()

                        }

                    }

                                    if let Some(&concrete_type_id) = generic_args.get(gp_idx) {

                                        // Resolve the concrete type provided at call site

                                        let concrete_ty = self.lower_type(concrete_type_id);

                                        // Record the substitution for this generic param

                                        if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(callee_decl_id, gp_idx)) {

                                            subst.insert(gp_sym, concrete_ty);

                                            // Check trait bounds for this generic param

                                            let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                                            for bound in &bounds {

                                                // Get the self_type_def SymbolId from the concrete type

                                                let resolved_ty = self.ctx.types.resolve(concrete_ty);

                                                let maybe_struct_sym = match self.ctx.types.get(resolved_ty) {

                                                    SemanticType::Struct(sym, _) => Some(*sym),

                                                    SemanticType::Enum(sym, _) => Some(*sym),

                                                    _ => None,

                                                };

                                                let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                                                if let Some(struct_sym) = maybe_struct_sym {

                                                    let impl_key = crate::semantic_tables::ImplKey {

                                                        trait_id: Some(bound.trait_id),

                                                        self_type_def: struct_sym,

                                                    };

                                                    if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                                        let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                                        let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

                                                        let mut diag = Diagnostic::error(format!(

                                                            "The type `{}` does not implement trait `{}` (required by generic parameter `{}`)",

                                                            type_name, trait_name, gp_name

                                                        ));

                                                        if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }

                                                            .with_span(gp.name)

                                                        );

                                                    }

                                                } else if matches!(self.ctx.types.get(resolved_ty), SemanticType::InferenceVar(_)) {

                                                    // Type could not be resolved — skip bound check (infer will catch it)

                                                } else {

                                                    // Non-nominal types (primitives, etc.) can't implement traits unless special-cased

                                                    // For now, allow primitives to pass without impl check

                                                }

            Expr::Call { callee, generic_args, args } => {

                let callee_expr = &self.arena.exprs[callee.0 as usize];

                if let Expr::Identifier { segments, .. } = callee_expr {

                    let name_str = segments.iter().map(|seg| &self.source[seg.start as usize..seg.end as usize]).collect::<Vec<_>>().join("::");

                    if name_str == "drop" {

                        let diag = Diagnostic::error("Explicit calls to drop() are forbidden. Values are dropped automatically at end of scope.".to_string());

                        let diag = if let Some(span) = segments.last() { diag.with_span(*span) } else { diag };

                        self.ctx.diagnostics.push(diag);

                    }

                }

                

                let mut is_inferred_generic = false;

                let mut generic_param_symbols = Vec::new();

                

                // If there are explicit generic args, check trait bounds before resolving callee

                if !generic_args.is_empty() {

                    if let Some(&callee_sym) = self.ctx.tables.expr_symbols.get(callee) {

                        if let Some(&callee_decl_id) = self.ctx.tables.symbol_decls.get(&callee_sym) {

                            let decl = &self.arena.decls[callee_decl_id.0 as usize];

                            if let mellis_ast::Decl::Function { generic_params, .. } = decl {

                                let generic_params = generic_params.clone();

                                // Build substitution and check bounds for each generic param

                                let mut subst = crate::ty::Substitution::new();

                                for (gp_idx, gp) in generic_params.iter().enumerate() {

                                    if let Some(&concrete_type_id) = generic_args.get(gp_idx) {

                                        // Resolve the concrete type provided at call site

                                        let concrete_ty = self.lower_type(concrete_type_id);

                                        // Record the substitution for this generic param

                                        if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(callee_decl_id, gp_idx)) {

                                            subst.insert(gp_sym, concrete_ty);

                                            // Check trait bounds for this generic param

                                            let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                                            for bound in &bounds {

                                                // Get the self_type_def SymbolId from the concrete type

                                                let resolved_ty = self.ctx.types.resolve(concrete_ty);

                                                let mut peeled_ty = resolved_ty;

                                                while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {

                                                    peeled_ty = *inner;

                                                }

                                                let maybe_struct_sym = match self.ctx.types.get(peeled_ty) {

                                                    SemanticType::Struct(sym, _) => Some(*sym),

                                                    SemanticType::Enum(sym, _) => Some(*sym),

                                                    _ => None,

                                                };

                                                let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                                                if let Some(struct_sym) = maybe_struct_sym {

                                                    let impl_key = crate::semantic_tables::ImplKey {

                                                        trait_id: Some(bound.trait_id),

                                                        self_type_def: struct_sym,

                                                    };

                                                    if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                                        let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                                        let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

                                                        let mut diag = Diagnostic::error(format!(

                                                            "The type `{}` does not implement trait `{}` (required by generic parameter `{}`)",

                                                            type_name, trait_name, gp_name

                                                        ));

                                                        if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }

                                                        self.ctx.diagnostics.push(diag);

                                                    }

                                                } else if matches!(self.ctx.types.get(resolved_ty), SemanticType::InferenceVar(_)) {

                            }

                        }

                    }

                    

                    if is_inferred_generic {

                        // Read back the resolved types of the InferenceVars and verify trait bounds

                        if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id) {

                            let mut subst_clone = subst.clone();

                            for (gp_sym, gp) in generic_param_symbols {

                                if let Some(&infer_var) = subst_clone.map.get(&gp_sym) {

                                    let resolved_ty = self.ctx.types.resolve(infer_var);

                                    subst_clone.map.insert(gp_sym, resolved_ty);

                                    

                                    let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                                    for bound in &bounds {

                                        let maybe_struct_sym = match self.ctx.types.get(resolved_ty) {

                                            SemanticType::Struct(sym, _) => Some(*sym),

                                            SemanticType::Enum(sym, _) => Some(*sym),

                                            _ => None,

                                        };

                                        let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                                        if let Some(struct_sym) = maybe_struct_sym {

                                            let impl_key = crate::semantic_tables::ImplKey {

                                                trait_id: Some(bound.trait_id),

                                                self_type_def: struct_sym,

                                            };

                                            if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                                let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                                let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

                                                        if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {

                                                            if self.unify(expected_p, arg_ty).is_err() {

                                                                self.emit_type_mismatch("Type mismatch in method argument", &arg.value);

                                                            }

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



                for arg in args {

                    self.typecheck_expr(&arg.value);

                }



                        }

                    }

                } else {

                    // Check if it's an inferred generic call

                    if let Some(&callee_sym) = self.ctx.tables.expr_symbols.get(callee) {

                        if let Some(&callee_decl_id) = self.ctx.tables.symbol_decls.get(&callee_sym) {

                            let decl = &self.arena.decls[callee_decl_id.0 as usize];

                            if let mellis_ast::Decl::Function { generic_params, .. } = decl {

                                if !generic_params.is_empty() {

                                    is_inferred_generic = true;

                                    let mut subst = crate::ty::Substitution::new();

                                    for (gp_idx, gp) in generic_params.iter().enumerate() {

                                        if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(callee_decl_id, gp_idx)) {

                                            let infer_var = self.ctx.types.new_inference_var();

                                            subst.insert(gp_sym, infer_var);

                                            generic_param_symbols.push((gp_sym, gp.clone()));

                                        }

                                    }

                                    self.ctx.tables.expr_substs.insert(*expr_id, subst);

                                }

                            }

                        }

                    }

                }

                

                let callee_ty_id = self.typecheck_expr(callee);

                

                // Substitute the callee type if it's a generic call (explicit or inferred)

                let callee_ty_id = if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id) {

                    let subst_clone = subst.clone();

                    self.substitute_type(callee_ty_id, &subst_clone)

                } else {

                    callee_ty_id

                };

                

                let mut ret_ty_id = self.ctx.types.new_inference_var();

                let callee_ty = self.ctx.types.get(callee_ty_id).clone();

                let is_dyn_call = self.ctx.tables.dyn_method_indices.contains_key(callee);

                let is_struct_method = if let Expr::Member { .. } = callee_expr { self.ctx.tables.expr_symbols.contains_key(callee) } else { false };

                let is_method_call = (is_dyn_call || is_struct_method) && if let SemanticType::Function { ref params, .. } = callee_ty { params.len() == args.len() + 1 } else { false };

                if let SemanticType::Function { params, return_type } = callee_ty {

                    ret_ty_id = return_type;

                    let expected_params = if is_method_call {

                        &params[1..]

                    } else {

                        &params[..]

                    };

                    for (i, arg) in args.iter().enumerate() {

                        let arg_ty = self.typecheck_expr(&arg.value);

                        if let Some(&expected_p) = expected_params.get(i) {

                            if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {

                                if self.unify(expected_p, arg_ty).is_err() {

                                    self.emit_type_mismatch("Type mismatch in function argument", &arg.value);

                                }

                            }

                        }

                    }

                    

                    if is_inferred_generic {

                        // Read back the resolved types of the InferenceVars and verify trait bounds

                        if let Some(subst) = self.ctx.tables.expr_substs.get(expr_id) {

                            let mut subst_clone = subst.clone();

                            for (gp_sym, gp) in generic_param_symbols {

                                if let Some(&infer_var) = subst_clone.map.get(&gp_sym) {

                                    let resolved_ty = self.ctx.types.resolve(infer_var);

                                    subst_clone.map.insert(gp_sym, resolved_ty);

                                    

                                    let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                                    for bound in &bounds {

                                        let mut peeled_ty = resolved_ty;

                                        while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {

                                            peeled_ty = *inner;

                                        }

                                        let maybe_struct_sym = match self.ctx.types.get(peeled_ty) {

                                            SemanticType::Struct(sym, _) => Some(*sym),

                                            SemanticType::Enum(sym, _) => Some(*sym),

                                            _ => None,

                                        };

                                        let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                                        if let Some(struct_sym) = maybe_struct_sym {

                                            let impl_key = crate::semantic_tables::ImplKey {

                                                trait_id: Some(bound.trait_id),

                                                self_type_def: struct_sym,

                                            };

                                            if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                                let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                                let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

                                                let mut diag = Diagnostic::error(format!(

                                                    "The type `{}` does not implement trait `{}` (required by inferred generic parameter `{}`)",

                                                    type_name, trait_name, gp_name

                                                ));

                                                if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }

                                                self.ctx.diagnostics.push(diag);

                                            }

                                        } else if matches!(self.ctx.types.get(resolved_ty), SemanticType::InferenceVar(_)) {

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

                } else if let SemanticType::Enum(..) = callee_ty {

                    ret_ty_id = callee_ty_id;

                    // TODO: Unify args with variant fields

                    for arg in args {

                        self.typecheck_expr(&arg.value);

                    }

                } else {

                    for arg in args {

                        self.typecheck_expr(&arg.value);

                    }

                }

                ret_ty_id

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

                            let sym = self.ctx.symbol_table.get_symbol(m_sym);

                            if sym.name == member_name {

                                self.ctx.tables.dyn_method_indices.insert(*expr_id, idx as u32);

                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {

                                    self.ctx.tables.expr_types.insert(*expr_id, m_ty);

                                    return m_ty;

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

                println!("DEBUG MEMBER ACCESS peeled_ty={:?} obj_ty={:?}", peeled_ty, obj_ty);

                

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

                } else if let SemanticType::GenericParam(sym_id) = peeled_ty {

                    if let Some(bounds) = self.ctx.tables.trait_bounds.get(&sym_id) {

                        for bound in bounds {

                            if let Some(method_syms) = self.ctx.tables.trait_methods.get(&bound.trait_id) {

                                for (idx, &m_sym) in method_syms.iter().enumerate() {

                                    let sym = self.ctx.symbol_table.get_symbol(m_sym);

                                    if sym.name == member_name {

                                        self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);

                                        if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {

                                            self.ctx.tables.expr_types.insert(*expr_id, m_ty);

                                            return m_ty;

                                        }

                                    }

                                }

                            }

                        }

                    }

                }

                self.ctx.diagnostics.push(Diagnostic::error("Member access requires a struct or trait object").with_span(*member));

                self.ctx.types.new_inference_var()

            }

            Expr::StructInit { path, fields, generic_args, .. } => {

                let Some(name_span) = path.last() else {

                    return self.ctx.types.new_inference_var();

                let mut subst = crate::ty::Substitution::new();

                let mut instantiated_args = Vec::new();

                

                for (i, gp) in generic_params.iter().enumerate() {

                    let gp_sym = self.ctx.tables.generic_param_symbols.get(&(decl_id, i)).copied().unwrap();

                    let arg_ty = if let Some(&ast_arg) = generic_args.get(i) {

                        self.lower_type(ast_arg)

                    } else {

                        self.ctx.types.new_inference_var()

                    };

                    subst.insert(gp_sym, arg_ty);

                    instantiated_args.push(arg_ty);

                    

                    // Lazily check trait bounds

                    let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                    for bound in &bounds {

                        let resolved_ty = self.ctx.types.resolve(arg_ty);

                        let mut peeled_ty = resolved_ty;

                        while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {

                            peeled_ty = *inner;

                        }

                        let maybe_struct_sym = match self.ctx.types.get(peeled_ty) {

                            SemanticType::Struct(sym, _) => Some(*sym),

                            SemanticType::Enum(sym, _) => Some(*sym),

                            _ => None,

                        };

                        let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                        if let Some(struct_sym) = maybe_struct_sym {

                            let impl_key = crate::semantic_tables::ImplKey {

                                trait_id: Some(bound.trait_id),

                                self_type_def: struct_sym,

                            };

                            if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

                                for &m_id in methods {

                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {

                                        let m_name = &self.source[name.start as usize..name.end as usize];

                                        if m_name == member_name {

                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {

                                                self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);

                                                if let Some(&m_ty_id) = self.ctx.tables.symbol_types.get(&m_sym) {

                                                    let mut subst = crate::ty::Substitution::new();

                                                    

                                if let Some(&m_ty) = self.ctx.tables.symbol_types.get(&m_sym) {

                                    if let SemanticType::Function { params, return_type } = self.ctx.types.get(m_ty).clone() {

                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };

                                        for (i, arg) in args.iter().enumerate() {

                                            let arg_ty = self.typecheck_expr(&arg.value);

                                            if let Some(&expected_p) = expected_params.get(i) {

                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {

                                                    if self.unify(expected_p, arg_ty).is_err() {

                                                        self.emit_type_mismatch("Type mismatch in method argument", &arg.value);

                                                    }

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



                if let SemanticType::Struct(sym_id, struct_generic_args) = peeled_ty {

                    println!("DEBUG: peeled_ty is Struct({:?})", sym_id);

                    let mut matching_impls = Vec::new();

                    for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {

                        println!("DEBUG: checking impl_key {:?} == {:?}", impl_key.self_type_def, sym_id);

                        if impl_key.self_type_def == sym_id {

                };



                if let SemanticType::Struct(sym_id, struct_generic_args) = peeled_ty {

                    println!("DEBUG: peeled_ty is Struct({:?})", sym_id);

                    let mut matching_impls = Vec::new();

                    for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {

                        println!("DEBUG: checking impl_key {:?} == {:?}", impl_key.self_type_def, sym_id);

                        if impl_key.self_type_def == sym_id {

                            matching_impls.push(impl_decl_id);

                        }

                    }

                    println!("DEBUG: matching_impls = {:?}", matching_impls);

                    for impl_decl_id in matching_impls {

                        if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {

                                for &m_id in methods {

                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {

                                        let m_name = &self.source[name.start as usize..name.end as usize];

                                        println!("DEBUG: m_name = {}, member_name = {}", m_name, member_name);

                                        if m_name == member_name {

                                            println!("DEBUG: match! has symbol? {:?}", self.ctx.tables.decl_symbols.get(&m_id));

                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {

                                                self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);

                                                if let Some(&m_ty_id) = self.ctx.tables.symbol_types.get(&m_sym) {

                                                    let mut subst = crate::ty::Substitution::new();

                                                    

                                                    if let Decl::Impl { generic_params: impl_gps, .. } = &self.arena.decls[impl_decl_id.0 as usize] {

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

                                                    println!("DEBUG AST DECL for m_name {} (m_id={:?}) = {:?}", m_name, m_id, &self.arena.decls[m_id.0 as usize]);

                                                    if let Decl::Function { generic_params: method_gps, .. } = &self.arena.decls[m_id.0 as usize] {

                                                        println!("DEBUG METHOD_GPS for m_name {} = {:?}", m_name, method_gps);

            Expr::MethodCall { object, method_name, args, generic_args } => {

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

                                                    if self.unify(expected_p, arg_ty).is_err() {

                                                        self.emit_type_mismatch("Type mismatch in method argument", &arg.value);

                                                    }

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



                if let SemanticType::Struct(sym_id, struct_generic_args) = peeled_ty {

                    println!("DEBUG: peeled_ty is Struct({:?})", sym_id);

                    let mut matching_impls = Vec::new();

                    for (impl_key, &impl_decl_id) in &self.ctx.tables.trait_impls {

                        println!("DEBUG: checking impl_key {:?} == {:?}", impl_key.self_type_def, sym_id);

                        if impl_key.self_type_def == sym_id {

                            matching_impls.push(impl_decl_id);

                        }

                    }

                    println!("DEBUG: matching_impls = {:?}", matching_impls);

                    for impl_decl_id in matching_impls {

                        if let Decl::Impl { methods, .. } = &self.arena.decls[impl_decl_id.0 as usize] {

                                for &m_id in methods {

                                    if let Decl::Function { name, .. } = &self.arena.decls[m_id.0 as usize] {

                                        let m_name = &self.source[name.start as usize..name.end as usize];

                                        println!("DEBUG: m_name = {}, member_name = {}", m_name, member_name);

                                        if m_name == member_name {

                                            println!("DEBUG: match! has symbol? {:?}", self.ctx.tables.decl_symbols.get(&m_id));

                                            if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(&m_id) {

                                                self.ctx.tables.expr_symbols.insert(*expr_id, m_sym);

                                                if let Some(&m_ty_id) = self.ctx.tables.symbol_types.get(&m_sym) {

                                                    let mut subst = crate::ty::Substitution::new();

                                                    

                                                    if let Decl::Impl { generic_params: impl_gps, .. } = &self.arena.decls[impl_decl_id.0 as usize] {

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

                                                    println!("DEBUG AST DECL for m_name {} (m_id={:?}) = {:?}", m_name, m_id, &self.arena.decls[m_id.0 as usize]);

                                                    if let Decl::Function { generic_params: method_gps, .. } = &self.arena.decls[m_id.0 as usize] {

                                                        println!("DEBUG METHOD_GPS for m_name {} = {:?}", m_name, method_gps);

                                                        for (i, gp) in method_gps.iter().enumerate() {

                                                            if let Some(&gp_sym) = self.ctx.tables.generic_param_symbols.get(&(m_id, i)) {

                                                                if let Some(&arg_ty_id) = generic_args.get(i) {

                                                                    let arg_ty = self.lower_type(arg_ty_id);

                                                                    subst.insert(gp_sym, arg_ty);

                                                                    let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                                                                    for bound in &bounds {

                                                                        let resolved_ty = self.ctx.types.resolve(arg_ty);

                                                                        let mut peeled_ty = resolved_ty;

                                                                        while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {

                                                                            peeled_ty = *inner;

                                                                        }

                                                                        let maybe_struct_sym = match self.ctx.types.get(peeled_ty) {

                                                                            SemanticType::Struct(sym, _) => Some(*sym),

                                                                            SemanticType::Enum(sym, _) => Some(*sym),

                                                                            _ => None,

                                                                        };

                                                                        let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                                                                        if let Some(struct_sym) = maybe_struct_sym {

                                                                            let impl_key = crate::semantic_tables::ImplKey {

                                                                                trait_id: Some(bound.trait_id),

                                                                                self_type_def: struct_sym,

                                                                            };

                                                                            if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                                                                let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                                                                let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

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

                                                                    is_inferred_generic = true;

                                                                    let infer_var = self.ctx.types.new_inference_var();

                                                                    subst.insert(gp_sym, infer_var);

                                                                    generic_param_symbols.push((gp_sym, gp.clone()));

                                                                }

                                                            }

                                                        }

                                                    }

                                                    

                                                    if is_inferred_generic {

                                                        self.ctx.tables.expr_substs.insert(*expr_id, subst.clone());

                                                    }

                                                    

                                                    let m_ty_subst_id = self.substitute_type(m_ty_id, &subst);

                                                    

                                                    if let SemanticType::Function { params, return_type } = self.ctx.types.get(m_ty_subst_id).clone() {

                                                        let expected_params = if params.len() == args.len() + 1 { &params[1..] } else { &params[..] };

                                                        for (i, arg) in args.iter().enumerate() {

                                                            let arg_ty = self.typecheck_expr(&arg.value);

                                                            if let Some(&expected_p) = expected_params.get(i) {

                                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {

                                                                    if self.unify(expected_p, arg_ty).is_err() {

                                                                        self.emit_type_mismatch("Type mismatch in method argument", &arg.value);

                                                                    }

                                                                }

                                                            }

                                                        }

                                                        

                                                        if is_inferred_generic {

                                                            if let Some(mut subst_clone) = self.ctx.tables.expr_substs.get(expr_id).cloned() {

                                                                for (gp_sym, gp) in generic_param_symbols {

                                                                    if let Some(&infer_var) = subst_clone.map.get(&gp_sym) {

                                                                        let resolved_ty = self.ctx.types.resolve(infer_var);

                                                                        subst_clone.map.insert(gp_sym, resolved_ty);

                                                        for (i, arg) in args.iter().enumerate() {

                                                            let arg_ty = self.typecheck_expr(&arg.value);

                                                            if let Some(&expected_p) = expected_params.get(i) {

                                                                if !self.try_coerce_dyn(arg.value, arg_ty, expected_p) {

                                                                    if self.unify(expected_p, arg_ty).is_err() {

                                                                        self.emit_type_mismatch("Type mismatch in method argument", &arg.value);

                                                                    }

                                                                }

                                                            }

                                                        }

                                                        

                                                        if is_inferred_generic {

                                                            if let Some(mut subst_clone) = self.ctx.tables.expr_substs.get(expr_id).cloned() {

                                                                for (gp_sym, gp) in generic_param_symbols {

                                                                    if let Some(&infer_var) = subst_clone.map.get(&gp_sym) {

                                                                        let resolved_ty = self.ctx.types.resolve(infer_var);

                                                                        subst_clone.map.insert(gp_sym, resolved_ty);

                                                                        

                                                                        let bounds = self.ctx.tables.trait_bounds.get(&gp_sym).cloned().unwrap_or_default();

                                                                        for bound in &bounds {

                                                                            let mut peeled_ty = resolved_ty;

                                                                            while let SemanticType::Pointer(_, inner) | SemanticType::Reference(_, _, inner) = self.ctx.types.get(peeled_ty) {

                                                                                peeled_ty = *inner;

                                                                            }

                                                                            let maybe_struct_sym = match self.ctx.types.get(peeled_ty) {

                                                                                SemanticType::Struct(sym, _) => Some(*sym),

                                                                                SemanticType::Enum(sym, _) => Some(*sym),

                                                                                _ => None,

                                                                            };

                                                                            let gp_name = &self.source[gp.name.start as usize..gp.name.end as usize];

                                                                            if let Some(struct_sym) = maybe_struct_sym {

                                                                                let impl_key = crate::semantic_tables::ImplKey {

                                                                                    trait_id: Some(bound.trait_id),

                                                                                    self_type_def: struct_sym,

                                                                                };

                                                                                if !self.ctx.tables.trait_impls.contains_key(&impl_key) {

                                                                                    let trait_name = self.ctx.symbol_table.get_symbol(bound.trait_id).name.clone();

                                                                                    let type_name = self.ctx.symbol_table.get_symbol(struct_sym).name.clone();

                                                                                    let mut diag = Diagnostic::error(format!(

                                                                                        "The type `{}` does not implement trait `{}` (required by inferred generic parameter `{}`)",

                                                                                        type_name, trait_name, gp_name

                                                                                    ));
                                                                                    if let Some(sp) = self.get_expr_span_for_diag(expr_id) { diag = diag.with_span(sp); }
                                                                                    self.ctx.diagnostics.push(diag);
                                                                                }
                                                                            } else if matches!(self.ctx.types.get(resolved_ty), SemanticType::InferenceVar(_)) {

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

            Expr::Sizeof { target_type } => {

                self.lower_type(*target_type);

                self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::I32))

            }

            Expr::Alignof { target_type } => {

                self.lower_type(*target_type);

                self.ctx.types.intern(SemanticType::Primitive(crate::ty::BuiltinType::I32))

            }

            Expr::Cast { expr: e, target_type } => {

                self.typecheck_expr(e);

                self.lower_type(*target_type)

            }

            Expr::Match { subject, arms, match_span } => {

                let subject_ty_id = self.typecheck_expr(subject);

                let subject_ty = self.ctx.types.get(subject_ty_id).clone();

                let mut result_ty = self.ctx.types.new_inference_var();

                

                let mut covered_variants = std::collections::HashSet::new();

                let mut covered_bools = std::collections::HashSet::new();

                let mut has_irrefutable = false;



                for (i, arm) in arms.iter().enumerate() {

                    self.typecheck_pattern(&arm.pattern, subject_ty_id);

                    

                    if self.is_pattern_irrefutable(&arm.pattern, subject_ty_id) {

                        has_irrefutable = true;

                    }



                    match &self.arena.pats[arm.pattern.0 as usize] {

                        mellis_ast::Pattern::Identifier { .. } => {

                            if let Some(sym_id) = self.ctx.tables.pat_symbols.get(&arm.pattern) {

                                let sym = self.ctx.symbol_table.get_symbol(*sym_id);

                                let decl = &self.arena.decls[decl_id.0 as usize];

                                if let mellis_ast::Decl::Enum { variants, .. } = decl {

                                    if covered_variants.len() < variants.len() {

                                        self.ctx.diagnostics.push(

                                            mellis_common::diagnostic::Diagnostic::error(format!("Match is not exhaustive. Covered {}/{} enum variants.", covered_variants.len(), variants.len()))

                                                .with_span(*match_span)

                                        );

                                    }

                                }

                            }

                        }

                        SemanticType::InferenceVar(_) | SemanticType::Error => {}

                        _ => {

                            self.ctx.diagnostics.push(

                                mellis_common::diagnostic::Diagnostic::error("Match is not exhaustive. An irrefutable pattern (like `_`, a variable, or a full struct/tuple pattern) is required for this type.")

                                    .with_span(*match_span)

                            );

                        }

                    }

                }

                

                result_ty

            }

            Expr::Lambda { body, params, return_type, is_move } => {

                let mut param_tys = Vec::new();

                for param_id in params {

                    if let mellis_ast::Decl::Param { ty, .. } = &self.arena.decls[param_id.0 as usize] {

                        let ty_id = if let Some(param_ty) = ty {

                            self.lower_type(*param_ty)

                        } else {

                            self.ctx.types.new_inference_var()

                        };

                        if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(param_id) {

                            self.ctx.tables.symbol_types.insert(sym_id, ty_id);

                        }

                        param_tys.push(ty_id);

                    } else {

                        param_tys.push(self.ctx.types.new_inference_var());

                    }

                }

                let ret_ty_id = if let Some(ret_ty) = return_type {

                    self.lower_type(*ret_ty)

                } else {

                    self.ctx.types.new_inference_var()

                };

                

                self.active_lambdas.push(*expr_id);

                self.current_return_type.push(ret_ty_id);

                self.typecheck_stmt(body);

                self.current_return_type.pop();

                self.active_lambdas.pop();

                if let Some(body_ty) = self.infer_stmt_value_type(body) {

                    if let Err(_) = self.unify(ret_ty_id, body_ty) {

                        self.ctx.diagnostics.push(Diagnostic::error("Lambda return type mismatch"));

                    }

                }

                let ret_ty_id = self.ctx.types.resolve_inference(ret_ty_id);

                

                let mut env_tys = Vec::new();

                let mut capture_bindings = Vec::new();

                if let Some(captures) = self.ctx.tables.expr_captures.get(expr_id).cloned() {

                    for (field, sym_id) in captures.into_iter().enumerate() {

                        let sym_ty = self.ctx.tables.symbol_types.get(&sym_id).copied().unwrap_or_else(|| self.ctx.types.intern(SemanticType::Error));

                        let is_mutated = self.ctx.tables.closure_mutated_captures.get(expr_id).map_or(false, |mutated| mutated.contains(&sym_id));

                        let mode = if *is_move {

                            crate::semantic_tables::CaptureMode::Move

                        } else if is_mutated {

                            crate::semantic_tables::CaptureMode::MutableBorrow

                        } else {

                            crate::semantic_tables::CaptureMode::SharedBorrow

                        };

                        

                        let env_ty = match mode {

                            crate::semantic_tables::CaptureMode::MutableBorrow => {

                                self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Mutable, sym_ty))

                            }

                            crate::semantic_tables::CaptureMode::SharedBorrow => {

                                self.ctx.types.intern(SemanticType::Pointer(crate::ty::Mutability::Immutable, sym_ty))

                            }

                            crate::semantic_tables::CaptureMode::Move => sym_ty,

                        };

                        env_tys.push(env_ty);

                        let is_mutated = self.ctx.tables.closure_mutated_captures.get(expr_id).map_or(false, |mutated| mutated.contains(&sym_id));

                        let is_mutable = self.ctx.tables.symbol_decls.get(&sym_id).and_then(|decl_id| match &self.arena.decls[decl_id.0 as usize] {

                            Decl::Var { is_mutable, .. } => Some(*is_mutable),

                            Decl::Param { .. } => Some(true),

                            _ => None,

                        }).unwrap_or(false);

                        if is_mutated && !is_mutable {

                            self.ctx.diagnostics.push(Diagnostic::error("Cannot mutably capture immutable variable"));

                        }

                        capture_bindings.push(crate::semantic_tables::CaptureBinding {

                            symbol: sym_id,

                            mode,

                            env_field: field as u32,

                            ty: sym_ty,

                            env_ty,

                        });

                    }

                }

                self.ctx.tables.closure_capture_bindings.insert(*expr_id, capture_bindings);

                let env_tuple_ty = self.ctx.types.intern(SemanticType::Tuple(env_tys));

                self.ctx.tables.closure_env_types.insert(*expr_id, env_tuple_ty);

                let env_ptr_ty = self.ctx.types.intern(SemanticType::Pointer(

                    crate::ty::Mutability::Immutable,

                    env_tuple_ty,

                ));

                self.ctx.tables.closure_env_ptr_types.insert(*expr_id, env_ptr_ty);

                

                self.ctx.types.intern(SemanticType::Closure(

                    *expr_id,

                    param_tys,

                    ret_ty_id,

                ))

            }

            Expr::Try { expr: e, .. } => {

                self.typecheck_expr(e)

            }

            Expr::Await { expr } => {

                if self.current_async_fn.is_none() {

                    let mut diag = Diagnostic::error("`await` is only allowed inside `async` functions");

                    if let Some(span) = self.get_expr_span_for_diag(expr) {

                        diag.span = Some(span);

                    }

                    self.ctx.diagnostics.push(diag);

                }

                let inner_ty_id = self.typecheck_expr(expr);

                let inner_ty = self.ctx.types.get(inner_ty_id).clone();

                match inner_ty {

                    SemanticType::Future(out_ty) => out_ty,

            Expr::MethodCall { method_name, .. } => Some(*method_name),

            Expr::Await { expr } => self.get_expr_span_for_diag(expr),

            Expr::Cast { expr, .. } => self.get_expr_span_for_diag(expr),

            Expr::Binary { left, .. } => self.get_expr_span_for_diag(left),

            Expr::Unary { operand, .. } => self.get_expr_span_for_diag(operand),

            Expr::Assign { lvalue, .. } => self.get_expr_span_for_diag(lvalue),

            Expr::Member { member, .. } => Some(*member),

            _ => None

        }

    }



    fn substitute_type(&mut self, ty_id: SemanticTypeId, subst: &crate::ty::Substitution) -> SemanticTypeId {

        let ty = self.ctx.types.get(ty_id).clone();

        match ty {

            SemanticType::GenericParam(sym) => {

                if let Some(&new_ty) = subst.map.get(&sym) {

                    new_ty

                } else {

                    ty_id

                }

            }

            SemanticType::Pointer(mutability, inner) => {

                let new_inner = self.substitute_type(inner, subst);

                if new_inner == inner { ty_id } else { self.ctx.types.intern(SemanticType::Pointer(mutability, new_inner)) }

            }

            SemanticType::Reference(lifetime, mutability, inner) => {

                let new_inner = self.substitute_type(inner, subst);

                if new_inner == inner { ty_id } else { self.ctx.types.intern(SemanticType::Reference(lifetime, mutability, new_inner)) }

            }

            SemanticType::Array(inner, size) => {

                let new_inner = self.substitute_type(inner, subst);

                if new_inner == inner { ty_id } else { self.ctx.types.intern(SemanticType::Array(new_inner, size)) }

            }

            SemanticType::Slice(inner) => {

                let new_inner = self.substitute_type(inner, subst);

                if new_inner == inner { ty_id } else { self.ctx.types.intern(SemanticType::Slice(new_inner)) }

            }

            SemanticType::Tuple(els) => {

                let mut new_els = Vec::new();

                let mut changed = false;

                for el in els.iter() {

                    let new_el = self.substitute_type(*el, subst);

                    if new_el != *el { changed = true; }

                    new_els.push(new_el);

                }

                if changed { self.ctx.types.intern(SemanticType::Tuple(new_els)) } else { ty_id }

            }

            SemanticType::Function { params, return_type } => {

                let mut new_params = Vec::new();

                let mut changed = false;

                for p in params.iter() {

                    let new_p = self.substitute_type(*p, subst);

                    if new_p != *p { changed = true; }

                    new_params.push(new_p);

                }

                let new_ret = self.substitute_type(return_type, subst);

                if new_ret != return_type { changed = true; }

                if changed { self.ctx.types.intern(SemanticType::Function { params: new_params, return_type: new_ret }) } else { ty_id }

            }

            SemanticType::Closure(env, params, return_type) => {

                let mut new_params = Vec::new();

                let mut changed = false;

                for p in params.iter() {

                    let new_p = self.substitute_type(*p, subst);

                    if new_p != *p { changed = true; }

                    new_params.push(new_p);

                }

                let new_ret = self.substitute_type(return_type, subst);

                if new_ret != return_type { changed = true; }

                if changed { self.ctx.types.intern(SemanticType::Closure(env, new_params, new_ret)) } else { ty_id }

            }

            SemanticType::Future(inner) => {

                let new_inner = self.substitute_type(inner, subst);

                if new_inner == inner { ty_id } else { self.ctx.types.intern(SemanticType::Future(new_inner)) }

            }

            _ => ty_id,

        }

    }



    fn emit_type_mismatch(&mut self, msg: &str, expr_id: &mellis_ast::ExprId) {

        let mut diag = Diagnostic::error(msg.to_string());

        if let Some(sp) = self.get_expr_span_for_diag(expr_id) {

            diag = diag.with_span(sp);

        }

        self.ctx.diagnostics.push(diag);

    }



    fn enforce_mutability(&mut self, expr_id: &mellis_ast::ExprId) {

        let expr = &self.arena.exprs[expr_id.0 as usize];

        if let mellis_ast::Expr::Identifier { segments, .. } = expr {

