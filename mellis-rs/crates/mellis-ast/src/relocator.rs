use crate::{AstArena, Decl, Expr, Pattern, Stmt, Type};
use crate::{DeclId, ExprId, PatId, StmtId, TypeId};
use mellis_common::ids::{FileId, Span};

/// Relocates all IDs in an AstArena by given offsets.
/// This is used when loading an `.mlib` into the host's `AstArena`.
pub struct AstRelocator {
    pub expr_offset: u32,
    pub stmt_offset: u32,
    pub decl_offset: u32,
    pub type_offset: u32,
    pub pat_offset: u32,
    pub new_file_id: FileId,
    pub span_offset: u32,
}

impl AstRelocator {
    pub fn new(
        expr_offset: u32,
        stmt_offset: u32,
        decl_offset: u32,
        type_offset: u32,
        pat_offset: u32,
        new_file_id: FileId,
        span_offset: u32,
    ) -> Self {
        Self {
            expr_offset,
            stmt_offset,
            decl_offset,
            type_offset,
            pat_offset,
            new_file_id,
            span_offset,
        }
    }

    pub fn relocate_arena(&self, arena: &mut AstArena) {
        for expr in &mut arena.exprs {
            self.relocate_expr(expr);
        }
        for stmt in &mut arena.stmts {
            self.relocate_stmt(stmt);
        }
        for decl in &mut arena.decls {
            self.relocate_decl(decl);
        }
        for ty in &mut arena.types {
            self.relocate_type(ty);
        }
        for pat in &mut arena.pats {
            self.relocate_pat(pat);
        }
    }

    pub fn shift_expr_id(&self, id: ExprId) -> ExprId {
        ExprId(id.0 + self.expr_offset)
    }

    pub fn shift_stmt_id(&self, id: StmtId) -> StmtId {
        StmtId(id.0 + self.stmt_offset)
    }

    pub fn shift_decl_id(&self, id: DeclId) -> DeclId {
        DeclId(id.0 + self.decl_offset)
    }

    pub fn shift_type_id(&self, id: TypeId) -> TypeId {
        TypeId(id.0 + self.type_offset)
    }

    pub fn shift_pat_id(&self, id: PatId) -> PatId {
        PatId(id.0 + self.pat_offset)
    }

    pub fn shift_span(&self, span: &mut Span) {
        span.file_id = self.new_file_id;
        span.start += self.span_offset;
        span.end += self.span_offset;
    }

    fn relocate_expr(&self, expr: &mut Expr) {
        match expr {
            Expr::Literal(_, _) => {}
            Expr::Identifier { segments, generic_args } => {
                for s in segments { self.shift_span(s); }
                for ga in generic_args { *ga = self.shift_type_id(*ga); }
            }
            Expr::Binary { left, right, .. } => {
                *left = self.shift_expr_id(*left);
                *right = self.shift_expr_id(*right);
            }
            Expr::Unary { operand, .. } => *operand = self.shift_expr_id(*operand),
            Expr::Assign { lvalue, value, .. } => {
                *lvalue = self.shift_expr_id(*lvalue);
                *value = self.shift_expr_id(*value);
            }
            Expr::Call { callee, generic_args, args } => {
                *callee = self.shift_expr_id(*callee);
                for ga in generic_args { *ga = self.shift_type_id(*ga); }
                for arg in args {
                    if let Some(l) = &mut arg.label { self.shift_span(l); }
                    arg.value = self.shift_expr_id(arg.value);
                }
            }
            Expr::MethodCall { object, method_name, generic_args, args } => {
                *object = self.shift_expr_id(*object);
                self.shift_span(method_name);
                for ga in generic_args { *ga = self.shift_type_id(*ga); }
                for arg in args {
                    if let Some(l) = &mut arg.label { self.shift_span(l); }
                    arg.value = self.shift_expr_id(arg.value);
                }
            }
            Expr::Index { base, index } => {
                *base = self.shift_expr_id(*base);
                *index = self.shift_expr_id(*index);
            }
            Expr::Member { object, member } => {
                *object = self.shift_expr_id(*object);
                self.shift_span(member);
            }
            Expr::TupleIndex { object, .. } => {
                *object = self.shift_expr_id(*object);
            }
            Expr::Cast { expr: inner, target_type } => {
                *inner = self.shift_expr_id(*inner);
                *target_type = self.shift_type_id(*target_type);
            }
            Expr::ArrayLiteral { elements } => {
                for element in elements {
                    *element = self.shift_expr_id(*element);
                }
            }
            Expr::TupleLiteral { elements } => {
                for element in elements {
                    *element = self.shift_expr_id(*element);
                }
            }
            Expr::StructInit { path, generic_args, fields } => {
                for p in path { self.shift_span(p); }
                for arg in generic_args { *arg = self.shift_type_id(*arg); }
                for field in fields {
                    self.shift_span(&mut field.name);
                    field.value = self.shift_expr_id(field.value);
                }
            }
            Expr::Match { match_span, subject, arms } => {
                self.shift_span(match_span);
                *subject = self.shift_expr_id(*subject);
                for arm in arms {
                    arm.pattern = self.shift_pat_id(arm.pattern);
                    arm.body = self.shift_stmt_id(arm.body);
                }
            }
            Expr::Lambda { params, return_type, body, .. } => {
                for param in params {
                    *param = self.shift_decl_id(*param);
                }
                if let Some(rt) = return_type {
                    *rt = self.shift_type_id(*rt);
                }
                *body = self.shift_stmt_id(*body);
            }
            Expr::Try { expr: inner, .. } => *inner = self.shift_expr_id(*inner),
            Expr::Await { expr: inner } => *inner = self.shift_expr_id(*inner),
            Expr::Sizeof { target_type } => *target_type = self.shift_type_id(*target_type),
            Expr::Alignof { target_type } => *target_type = self.shift_type_id(*target_type),
            Expr::MacroCall { name, path, span, .. } => {
                self.shift_span(name);
                for seg in path {
                    self.shift_span(seg);
                }
                self.shift_span(span);
            }
            Expr::Comptime { body } => *body = self.shift_stmt_id(*body),
        }
    }

    fn relocate_stmt(&self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Block { body, tail_expr } => {
                for item in body {
                    match item {
                        crate::Item::Decl(d) => *d = self.shift_decl_id(*d),
                        crate::Item::Stmt(s) => *s = self.shift_stmt_id(*s),
                    }
                }
                if let Some(te) = tail_expr {
                    *te = self.shift_expr_id(*te);
                }
            }
            Stmt::Expr { expr, .. } => *expr = self.shift_expr_id(*expr),
            Stmt::If { condition, then_branch, else_branch } => {
                *condition = self.shift_expr_id(*condition);
                *then_branch = self.shift_stmt_id(*then_branch);
                if let Some(eb) = else_branch {
                    *eb = self.shift_stmt_id(*eb);
                }
            }
            Stmt::While { label, condition, body } => {
                if let Some(l) = label { self.shift_span(l); }
                *condition = self.shift_expr_id(*condition);
                *body = self.shift_stmt_id(*body);
            }
            Stmt::For { label, pattern, iterable, init, cond, step, body, .. } => {
                if let Some(l) = label { self.shift_span(l); }
                if let Some(p) = pattern { *p = self.shift_pat_id(*p); }
                if let Some(it) = iterable { *it = self.shift_expr_id(*it); }
                if let Some(i) = init {
                    match i {
                        crate::Item::Decl(d) => *d = self.shift_decl_id(*d),
                        crate::Item::Stmt(s) => *s = self.shift_stmt_id(*s),
                    }
                }
                if let Some(c) = cond { *c = self.shift_expr_id(*c); }
                if let Some(s) = step { *s = self.shift_expr_id(*s); }
                *body = self.shift_stmt_id(*body);
            }Stmt::Return { value } => {
                if let Some(v) = value { *v = self.shift_expr_id(*v); }
            }
            Stmt::Break { label } => {
                if let Some(l) = label { self.shift_span(l); }
            }
            Stmt::Continue { label } => {
                if let Some(l) = label { self.shift_span(l); }
            }
            Stmt::Unsafe { body } => *body = self.shift_stmt_id(*body),
            Stmt::Comptime { body } => *body = self.shift_stmt_id(*body),
        }
    }

    fn relocate_decl(&self, decl: &mut Decl) {
        match decl {
            Decl::Var { annotations, name, pattern, type_annot, initializer, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                if let Some(p) = pattern { *p = self.shift_pat_id(*p); }
                if let Some(t) = type_annot { *t = self.shift_type_id(*t); }
                if let Some(i) = initializer { *i = self.shift_expr_id(*i); }
            }
            Decl::Param { annotations, name, ty, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                if let Some(t) = ty { *t = self.shift_type_id(*t); }
            }
            Decl::Function { annotations, name, generic_params, params, return_type, body, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                self.relocate_generic_params(generic_params);
                for p in params { *p = self.shift_decl_id(*p); }
                if let Some(rt) = return_type { *rt = self.shift_type_id(*rt); }
                if let Some(b) = body { *b = self.shift_stmt_id(*b); }
            }
            Decl::Struct { annotations, name, generic_params, fields, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                self.relocate_generic_params(generic_params);
                for field in fields {
                    self.shift_span(&mut field.name);
                    field.ty = self.shift_type_id(field.ty);
                }
            }
            Decl::Enum { annotations, name, generic_params, variants, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                self.relocate_generic_params(generic_params);
                for variant in variants {
                    self.relocate_annotations(&mut variant.annotations);
                    self.shift_span(&mut variant.name);
                    for f in &mut variant.fields { *f = self.shift_decl_id(*f); }
                }
            }
            Decl::Trait { annotations, name, generic_params, associated_types, methods, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                self.relocate_generic_params(generic_params);
                for t in associated_types { *t = self.shift_decl_id(*t); }
                for m in methods { *m = self.shift_decl_id(*m); }
            }
            Decl::Impl { annotations, generic_params, self_type, trait_type, associated_types, methods, .. } => {
                self.relocate_annotations(annotations);
                self.relocate_generic_params(generic_params);
                *self_type = self.shift_type_id(*self_type);
                if let Some(tt) = trait_type { *tt = self.shift_type_id(*tt); }
                for t in associated_types { *t = self.shift_decl_id(*t); }
                for m in methods { *m = self.shift_decl_id(*m); }
            }
            Decl::Import { annotations, name, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
            }
            Decl::Extern { annotations, func, .. } => {
                self.relocate_annotations(annotations);
                *func = self.shift_decl_id(*func);
            }
            Decl::TypeAlias { annotations, name, generic_params, bounds, aliased_type, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                self.relocate_generic_params(generic_params);
                for b in bounds { *b = self.shift_type_id(*b); }
                if let Some(at) = aliased_type { *at = self.shift_type_id(*at); }
            }
            Decl::Module { annotations, name, items, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                for item in items {
                    *item = self.shift_decl_id(*item);
                }
            }
            Decl::Using { path, alias, span } => {
                for seg in path {
                    self.shift_span(seg);
                }
                self.shift_span(alias);
                self.shift_span(span);
            }
            Decl::Macro { annotations, name, rules, .. } => {
                self.relocate_annotations(annotations);
                self.shift_span(name);
                for rule in rules {
                    self.relocate_macro_rule(rule);
                }
            }
        }
    }

    fn relocate_macro_rule(&self, rule: &mut crate::MacroRule) {
        self.shift_span(&mut rule.span);
        self.shift_span(&mut rule.pattern.span);
        for elem in &mut rule.pattern.elements {
            self.relocate_matcher_element(elem);
        }
        self.shift_span(&mut rule.transcriber.span);
        for elem in &mut rule.transcriber.elements {
            self.relocate_transcriber_element(elem);
        }
        for m in &mut rule.matchers {
            self.shift_span(&mut m.name);
        }
        for tok in &mut rule.template_tokens {
            self.shift_span(&mut tok.span);
        }
    }

    fn relocate_matcher_element(&self, elem: &mut crate::MatcherElement) {
        match elem {
            crate::MatcherElement::MetaVar { name, span, .. } => {
                self.shift_span(name);
                self.shift_span(span);
            }
            crate::MatcherElement::Group { elements, span, .. } => {
                self.shift_span(span);
                for el in elements {
                    self.relocate_matcher_element(el);
                }
            }
            crate::MatcherElement::Leaf { token } => {
                self.shift_span(&mut token.span);
            }
            crate::MatcherElement::Repetition { elements, span, .. } => {
                self.shift_span(span);
                for el in elements {
                    self.relocate_matcher_element(el);
                }
            }
        }
    }

    fn relocate_transcriber_element(&self, elem: &mut crate::TranscriberElement) {
        match elem {
            crate::TranscriberElement::MetaVar { name, span } => {
                self.shift_span(name);
                self.shift_span(span);
            }
            crate::TranscriberElement::Group { elements, span, .. } => {
                self.shift_span(span);
                for el in elements {
                    self.relocate_transcriber_element(el);
                }
            }
            crate::TranscriberElement::Leaf { token } => {
                self.shift_span(&mut token.span);
            }
            crate::TranscriberElement::Repetition { elements, span, .. } => {
                self.shift_span(span);
                for el in elements {
                    self.relocate_transcriber_element(el);
                }
            }
        }
    }

    fn relocate_annotations(&self, annotations: &mut Vec<crate::Annotation>) {
        for ann in annotations {
            self.shift_span(&mut ann.name);
            for arg in &mut ann.args {
                if let Some(k) = &mut arg.key { self.shift_span(k); }
                arg.value = self.shift_expr_id(arg.value);
            }
        }
    }

    fn relocate_generic_params(&self, generic_params: &mut Vec<crate::GenericParam>) {
        for gp in generic_params {
            self.shift_span(&mut gp.name);
            for b in &mut gp.bounds { *b = self.shift_type_id(*b); }
        }
    }

    fn relocate_type(&self, ty: &mut Type) {
        match ty {
            Type::Builtin(_) => {}
            Type::Lifetime(span) => self.shift_span(span),
            Type::Named { segments, generic_args, associated_bindings } => {
                for s in segments { self.shift_span(s); }
                for ga in generic_args { *ga = self.shift_type_id(*ga); }
                for ab in associated_bindings {
                    self.shift_span(&mut ab.name);
                    ab.ty = self.shift_type_id(ab.ty);
                }
            }
            Type::Reference { lifetime, inner, .. } => {
                if let Some(lt) = lifetime { *lt = self.shift_type_id(*lt); }
                *inner = self.shift_type_id(*inner);
            }
            Type::Pointer { inner, .. } => *inner = self.shift_type_id(*inner),
            Type::Array { element_type, size } => {
                *element_type = self.shift_type_id(*element_type);
                *size = self.shift_expr_id(*size);
            }
            Type::Slice { inner } => *inner = self.shift_type_id(*inner),
            Type::Tuple { elements } => {
                for e in elements { *e = self.shift_type_id(*e); }
            }
            Type::Function { params, return_type, .. } => {
                for p in params { *p = self.shift_type_id(*p); }
                if let Some(rt) = return_type { *rt = self.shift_type_id(*rt); }
            }
            Type::Never => {}
            Type::TraitObject { trait_type } => *trait_type = self.shift_type_id(*trait_type),
            Type::Typeof { expr } => *expr = self.shift_expr_id(*expr),
            Type::MacroCall { path, span, .. } => {
                for p in path { self.shift_span(p); }
                self.shift_span(span);
            }
        }
    }

    fn relocate_pat(&self, pat: &mut Pattern) {
        match pat {
            Pattern::Literal(_) => {}
            Pattern::Identifier { segments } => {
                for s in segments { self.shift_span(s); }
            }
            Pattern::Struct { path, fields, .. } => {
                for p in path { self.shift_span(p); }
                for f in fields {
                    self.shift_span(&mut f.name);
                    if let Some(p) = &mut f.pattern { *p = self.shift_pat_id(*p); }
                }
            }
            Pattern::Enum { path, fields } => {
                for p in path { self.shift_span(p); }
                for f in fields { *f = self.shift_pat_id(*f); }
            }
            Pattern::Tuple { elements, .. } => {
                for e in elements { *e = self.shift_pat_id(*e); }
            }
            Pattern::Wildcard => {}
        }
    }
}
