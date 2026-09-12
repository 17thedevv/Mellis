pub trait ModuleNamespaceProvider<'a> {
    fn get_module_scope(&self, name: &str) -> Option<crate::symbol::ScopeId>;
}

pub struct ModuleNamespaceMap {
    pub scopes: std::collections::HashMap<String, crate::symbol::ScopeId>,
}

impl ModuleNamespaceMap {
    pub fn new() -> Self {
        Self {
            scopes: std::collections::HashMap::new(),
        }
    }
}
impl<'a> ModuleNamespaceProvider<'a> for ModuleNamespaceMap {
    fn get_module_scope(&self, name: &str) -> Option<crate::symbol::ScopeId> {
        self.scopes.get(name).copied()
    }
}

use crate::{ScopeId, SemanticContext, SymbolKind};
use luna_ast::{AstArena, Decl, Expr, Item, Pattern, Stmt, Visibility};
use luna_common::ids::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationContext {
    Free,
    TraitMethod(Visibility),
    ImplMethod,
}

pub struct Resolver<'a, 'b, 'c> {
    pub ctx: &'a mut SemanticContext,
    pub arena: &'b AstArena,
    pub source_manager: &'c luna_common::source::SourceManager,
    pub current_scope: ScopeId,
    pub module_provider: Option<&'a dyn ModuleNamespaceProvider<'a>>,
    pub active_lambdas: Vec<(luna_ast::ExprId, ScopeId)>,
}

impl<'a, 'b, 'c> Resolver<'a, 'b, 'c> {

    pub fn get_span_text(&self, span: luna_common::ids::Span) -> &str {
        &self.source_manager.get_file(span.file_id).unwrap().source[span.start as usize..span.end as usize]
    }

    pub fn new(ctx: &'a mut SemanticContext, arena: &'b AstArena, source_manager: &'c luna_common::source::SourceManager) -> Self {
        // Assume global scope is 0
        let global_scope = ScopeId(0);
        Self {
            ctx,
            arena,
            source_manager,
            current_scope: global_scope,
            module_provider: None,
            active_lambdas: Vec::new(),
        }
    }

    pub fn enter_scope(&mut self, kind: crate::symbol::ScopeKind) -> ScopeId {
        let new_scope = self
            .ctx
            .symbol_table
            .create_scope(kind, Some(self.current_scope));
        self.current_scope = new_scope;
        new_scope
    }

    pub fn exit_scope(&mut self) {
        if let Some(parent) = self.ctx.symbol_table.scopes[self.current_scope.0 as usize].parent {
            self.current_scope = parent;
        }
    }

    pub fn register_macros(&mut self, items: &[Item]) {
        for item in items {
            self.declare_macro_item(item);
        }
    }

    fn declare_macro_item(&mut self, item: &Item) {
        if let Item::Decl(decl_id) = item {
            let decl = &self.arena.decls[decl_id.0 as usize];
            match decl {
                Decl::Macro {
                    name, visibility, ..
                } => {
                    let name_str = self.get_span_text(*name).to_string();
                    let sym_id = self.ctx.symbol_table.declare_symbol(
                        name_str,
                        SymbolKind::Macro,
                        self.current_scope,
                        *name,
                        Some(*decl_id),
                        *visibility,
                        &mut self.ctx.diagnostics,
                    );
                    self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                    self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                    self.ctx.tables.decl_macros.insert(*decl_id, sym_id);
                    self.ctx.tables.macro_decls.insert(sym_id, *decl_id);
                }
                Decl::Module {
                    name,
                    items,
                    visibility,
                    ..
                } => {
                    let name_str = self.get_span_text(*name).to_string();
                    let sym_id = if let Some(existing) = self
                        .ctx
                        .symbol_table
                        .lookup_exact(&name_str, self.current_scope)
                    {
                        existing
                    } else {
                        self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Module,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                            &mut self.ctx.diagnostics,
                        )
                    };
                    self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                    self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                    let mod_scope = if let Some(inner) =
                        self.ctx.symbol_table.symbols[sym_id.0 as usize].inner_scope
                    {
                        inner
                    } else {
                        let s = self.enter_scope(crate::symbol::ScopeKind::Module);
                        self.ctx.symbol_table.set_inner_scope(sym_id, s);
                        s
                    };
                    let prev_scope = self.current_scope;
                    self.current_scope = mod_scope;
                    for item_id in items {
                        let inner_item = Item::Decl(*item_id);
                        self.declare_macro_item(&inner_item);
                    }
                    self.current_scope = prev_scope;
                }
                Decl::Using { path, alias, .. } => {
                    let alias_str = self.get_span_text(*alias).trim_matches('"').to_string();
                    let path_str: Vec<&str> = path.iter().map(|seg| self.get_span_text(*seg).trim_matches('"')).collect();
                    let target_scope_opt = if path_str.len() == 1 {
                        self.ctx
                            .symbol_table
                            .lookup(path_str[0], self.current_scope)
                            .and_then(|s| self.ctx.symbol_table.symbols[s.0 as usize].inner_scope)
                    } else {
                        let mut curr = self
                            .ctx
                            .symbol_table
                            .lookup(path_str[0], self.current_scope)
                            .and_then(|s| self.ctx.symbol_table.symbols[s.0 as usize].inner_scope);
                        for &seg in &path_str[1..] {
                            if let Some(scope) = curr {
                                curr =
                                    self.ctx
                                        .symbol_table
                                        .lookup_exact(seg, scope)
                                        .and_then(|s| {
                                            self.ctx.symbol_table.symbols[s.0 as usize].inner_scope
                                        });
                            } else {
                                break;
                            }
                        }
                        curr
                    };

                    if let Some(target_scope) = target_scope_opt {
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            alias_str,
                            SymbolKind::Module,
                            self.current_scope,
                            *alias,
                            Some(*decl_id),
                            Visibility::Private,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.symbol_table.set_inner_scope(sym_id, target_scope);
                    } else {
                        self.ctx.diagnostics.push(luna_common::diagnostic::Diagnostic::error(format!("unresolved module or path `{}`", alias_str)).with_span(*alias));
                    }
                }
                Decl::Import { name, kind, .. } => {
                    self.resolve_import_decl(name, kind);
                }
                _ => {}
            }
        }
    }

    fn resolve_import_decl(&mut self, name: &Span, kind: &luna_ast::ImportKind) {
        let name_str = self.get_span_text(*name).trim_matches('"').to_string();
        let target_scope = match kind {
            luna_ast::ImportKind::External => {
                if let Some(&pid) = self.ctx.provider_lookup.get(&name_str) {
                    self.ctx.provider_scopes.get(&pid).copied()
                } else if let Some(&scope) = self.ctx.external_module_scopes.get(&name_str) {
                    Some(scope)
                } else if let Some(provider) = self.module_provider {
                    provider.get_module_scope(&name_str)
                } else {
                    None
                }
            }
            luna_ast::ImportKind::Local => {
                if let Some(&pid) = self.ctx.provider_lookup.get(&name_str) {
                    self.ctx.provider_scopes.get(&pid).copied()
                } else if let Some(provider) = self.module_provider {
                    provider.get_module_scope(&name_str)
                } else {
                    self.ctx
                        .symbol_table
                        .lookup(&name_str, crate::symbol::ScopeId(0))
                        .and_then(|s| self.ctx.symbol_table.symbols[s.0 as usize].inner_scope)
                }
            }
        };

        if let Some(scope) = target_scope {
            // Collect exported root symbols and explicit module namespaces from the target provider scope
            let sym_entries: Vec<(String, luna_common::ids::SymbolId)> = {
                let scope_ref = &self.ctx.symbol_table.scopes[scope.0 as usize];
                let mut entries = Vec::new();
                for (key, sym_ids) in &scope_ref.symbols {
                    if let Some(&sym_id) = sym_ids.last() {
                        entries.push((key.name.clone(), sym_id));
                    }
                }
                entries
            };

            for (sym_name, sym_id) in sym_entries {
                let sym = self.ctx.symbol_table.get_symbol(sym_id).clone();
                // IMPORT-6: Non-exported / private provider symbols are never injected into consumer scopes
                if sym.visibility != Visibility::Public
                    && sym.kind != SymbolKind::TypeParam
                    && sym.kind != SymbolKind::Module
                {
                    continue;
                }

                // Explicit module namespaces retain their hierarchy (IMPORT-7), while root declarations
                // are injected into current_scope.
                match self.ctx.symbol_table.add_imported_symbol(
                    sym_name.clone(),
                    sym_id,
                    self.current_scope,
                ) {
                    crate::symbol::ImportSymbolResult::Success
                    | crate::symbol::ImportSymbolResult::AlreadyImported => {}
                    crate::symbol::ImportSymbolResult::Conflict { .. } => {
                        // IMPORT-5: Conflicting imported root bindings are diagnosed deterministically
                        self.ctx.diagnostics.push(
                            luna_common::diagnostic::Diagnostic::error(format!(
                                "E1002: conflicting imported symbol `{}`: symbol already defined or imported from another provider",
                                sym_name
                            ))
                            .with_span(*name),
                        );
                    }
                }
            }
        }
    }

    fn check_lang_item(
        &mut self,
        annotations: &[luna_ast::Annotation],
        sym_id: luna_common::ids::SymbolId,
        target: crate::lang_item::LangItemTarget,
    ) {
        for annot in annotations {
            let attr_name = self.get_span_text(annot.name);
            if attr_name == "lang" {
                if !self.ctx.allow_internal_lang_items {
                    self.ctx.diagnostics.push(
                        luna_common::diagnostic::Diagnostic::error(
                            "`#[lang]` attribute can only be used in trusted compiler/core contexts",
                        )
                        .with_span(annot.name),
                    );
                    continue;
                }
                if annot.args.len() != 1 {
                    self.ctx.diagnostics.push(
                        luna_common::diagnostic::Diagnostic::error(
                            "`#[lang]` attribute requires exactly one argument: `#[lang(\"name\")]`",
                        )
                        .with_span(annot.name),
                    );
                    continue;
                }
                let arg_expr_id = annot.args[0].value;
                let (tok_span, arg_str) = match &self.arena.exprs[arg_expr_id.0 as usize] {
                    luna_ast::Expr::Literal(tok, s) => {
                        if !s.starts_with('"') || !s.ends_with('"') {
                            self.ctx.diagnostics.push(
                                luna_common::diagnostic::Diagnostic::error(
                                    "`#[lang(...)]` argument must be a string literal",
                                )
                                .with_span(tok.span),
                            );
                            continue;
                        }
                        (tok.span, s[1..s.len() - 1].to_string())
                    }
                    _ => {
                        self.ctx.diagnostics.push(
                            luna_common::diagnostic::Diagnostic::error(
                                "`#[lang(...)]` argument must be a string literal",
                            )
                            .with_span(annot.name),
                        );
                        continue;
                    }
                };

                let Some(item) = crate::lang_item::LangItem::from_name(&arg_str) else {
                    self.ctx.diagnostics.push(
                        luna_common::diagnostic::Diagnostic::error(format!(
                            "unknown language item `{}`",
                            arg_str
                        ))
                        .with_span(tok_span),
                    );
                    continue;
                };

                self.ctx.lang_items.register(
                    item,
                    sym_id,
                    target,
                    annot.name,
                    &mut self.ctx.diagnostics,
                );
            }
        }
    }

    fn check_unsupported_lang_item(
        &mut self,
        annotations: &[luna_ast::Annotation],
        decl_kind: &'static str,
    ) {
        for annot in annotations {
            let attr_name = self.get_span_text(annot.name);
            if attr_name == "lang" {
                if !self.ctx.allow_internal_lang_items {
                    self.ctx.diagnostics.push(
                        luna_common::diagnostic::Diagnostic::error(
                            "`#[lang]` attribute can only be used in trusted compiler/core contexts",
                        )
                        .with_span(annot.name),
                    );
                    continue;
                }
                self.ctx.diagnostics.push(
                    luna_common::diagnostic::Diagnostic::error(format!(
                        "`#[lang]` cannot be applied to {} declarations",
                        decl_kind
                    ))
                    .with_span(annot.name),
                );
            }
        }
    }

    pub fn resolve_items(&mut self, items: &[Item]) {
        for item in items {
            self.declare_item(item, DeclarationContext::Free);
        }
        for item in items {
            self.resolve_item_body(item);
        }
    }

    fn declare_item(&mut self, item: &Item, context: DeclarationContext) {
        match item {
            Item::Decl(decl_id) => {
                let prev_scope = self.current_scope;
                if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {
                    if let Some(scope) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                        self.current_scope = scope;
                    }
                }
                self.resolve_decl_types(decl_id);
                self.current_scope = prev_scope;
                
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function {
                        name,
                        params,
                        visibility,
                        generic_params,
                        annotations,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();

                        let effective_visibility = if let DeclarationContext::TraitMethod(v) = context {
                            v
                        } else {
                            *visibility
                        };

                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Function,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            effective_visibility,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        
                        let expected_lang_target = match context {
                            DeclarationContext::TraitMethod(_) | DeclarationContext::ImplMethod => crate::lang_item::LangItemTarget::Method,
                            DeclarationContext::Free => crate::lang_item::LangItemTarget::Function,
                        };
                        self.check_lang_item(annotations, sym_id, expected_lang_target);

                        let func_scope = self.enter_scope(crate::symbol::ScopeKind::Function);
                        self.ctx.symbol_table.set_inner_scope(sym_id, func_scope);
                        self.ctx.tables.decl_scopes.insert(*decl_id, func_scope);

                        if !generic_params.is_empty() {
                            for (idx, gp) in generic_params.iter().enumerate() {
                                let gp_name_str = self.source_manager.get_file(gp.name.file_id).unwrap().source[gp.name.start as usize..gp.name.end as usize]
                                    .to_string();
                                let gp_sym_id = self.ctx.symbol_table.declare_symbol(
                                    gp_name_str,
                                    crate::symbol::SymbolKind::TypeParam,
                                    self.current_scope,
                                    gp.name,
                                    Some(*decl_id),
                                    luna_ast::Visibility::Private, // Generic params don't really have visibility
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx
                                    .tables
                                    .generic_param_symbols
                                    .insert((*decl_id, idx), gp_sym_id);
                                self.ctx.tables.symbol_decls.insert(gp_sym_id, *decl_id);
                            }
                        }

                        let mut noescapes = Vec::new();
                        for param_id in params {
                            if let Decl::Param {
                                name: p_name,
                                visibility: p_vis,
                                annotations,
                                ..
                            } = &self.arena.decls[param_id.0 as usize]
                            {
                                let mut is_noescape = false;
                                for ann in annotations {
                                    let ann_name = &self.source_manager.get_file(ann.name.file_id).unwrap().source[ann.name.start as usize..ann.name.end as usize];
                                    if ann_name == "sync_noescape" {
                                        is_noescape = true;
                                        break;
                                    }
                                }
                                noescapes.push(is_noescape);

                                let p_name_str = self.source_manager.get_file(p_name.file_id).unwrap().source[p_name.start as usize..p_name.end as usize]
                                    .to_string();
                                let p_sym_id = self.ctx.symbol_table.declare_symbol(
                                    p_name_str,
                                    SymbolKind::Variable,
                                    self.current_scope,
                                    *p_name,
                                    Some(*param_id),
                                    *p_vis,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx.tables.decl_symbols.insert(*param_id, p_sym_id);
                                self.ctx.tables.symbol_decls.insert(p_sym_id, *param_id);
                            }
                        }
                        self.ctx.tables.ffi_sync_noescape.insert(sym_id, noescapes);
                        self.resolve_lifetime_signature(decl_id);
                        self.exit_scope();
                    }
                    Decl::Var {
                        name,
                        visibility,
                        is_mutable,
                        pattern,
                        annotations,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            if *is_mutable {
                                SymbolKind::Variable
                            } else {
                                SymbolKind::Constant
                            },
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        self.check_unsupported_lang_item(annotations, "variable");
                        if let Some(pattern) = pattern {
                            self.resolve_pattern(pattern, *visibility, *is_mutable);
                        }
                    }
                    Decl::Struct {
                        name,
                        fields: _,
                        visibility,
                        generic_params,
                        annotations,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Struct,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        self.check_lang_item(annotations, sym_id, crate::lang_item::LangItemTarget::Struct);
                        
                        let prev_scope = self.current_scope;
                        let struct_scope = self.enter_scope(crate::symbol::ScopeKind::Struct);
                        self.ctx.symbol_table.set_inner_scope(sym_id, struct_scope);
                        self.ctx.tables.decl_scopes.insert(*decl_id, struct_scope);
                        
                        if !generic_params.is_empty() {
                            for (idx, gp) in generic_params.iter().enumerate() {
                                let gp_name_str = self.source_manager.get_file(gp.name.file_id).unwrap().source[gp.name.start as usize..gp.name.end as usize]
                                    .to_string();
                                let gp_sym_id = self.ctx.symbol_table.declare_symbol(
                                    gp_name_str,
                                    crate::symbol::SymbolKind::TypeParam,
                                    self.current_scope,
                                    gp.name,
                                    Some(*decl_id),
                                    luna_ast::Visibility::Private,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx
                                    .tables
                                    .generic_param_symbols
                                    .insert((*decl_id, idx), gp_sym_id);
                                self.ctx.tables.symbol_decls.insert(gp_sym_id, *decl_id);
                            }
                        }
                        
                        self.current_scope = prev_scope;
                    }
                    Decl::Extern { func, .. } => {
                        let item = Item::Decl(*func);
                        self.declare_item(&item, DeclarationContext::Free);
                    }
                    Decl::Enum {
                        name,
                        variants,
                        visibility,
                        generic_params,
                        annotations,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str.clone(),
                            SymbolKind::Struct, // Enums use Struct kind for now
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        self.check_lang_item(annotations, sym_id, crate::lang_item::LangItemTarget::Enum);

                        let prev_scope = self.current_scope;
                        let enum_scope = self.enter_scope(crate::symbol::ScopeKind::Struct);
                        self.ctx.symbol_table.set_inner_scope(sym_id, enum_scope);
                        self.ctx.tables.decl_scopes.insert(*decl_id, enum_scope);
                        
                        if !generic_params.is_empty() {
                            for (idx, gp) in generic_params.iter().enumerate() {
                                let gp_name_str = self.source_manager.get_file(gp.name.file_id).unwrap().source[gp.name.start as usize..gp.name.end as usize]
                                    .to_string();
                                let gp_sym_id = self.ctx.symbol_table.declare_symbol(
                                    gp_name_str,
                                    crate::symbol::SymbolKind::TypeParam,
                                    self.current_scope,
                                    gp.name,
                                    Some(*decl_id),
                                    luna_ast::Visibility::Private,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx
                                    .tables
                                    .generic_param_symbols
                                    .insert((*decl_id, idx), gp_sym_id);
                                self.ctx.tables.symbol_decls.insert(gp_sym_id, *decl_id);
                            }
                        }

                        // Declare each variant as a symbol accessible via VariantName inside enum scope
                        for (idx, variant) in variants.iter().enumerate() {
                            let v_name_str = self.source_manager.get_file(variant.name.file_id).unwrap().source[variant.name.start as usize..variant.name.end as usize]
                                .to_string();
                            let v_sym_id = self.ctx.symbol_table.declare_symbol(
                                v_name_str,
                                SymbolKind::EnumVariant(idx as u32),
                                self.current_scope,
                                variant.name,
                                Some(*decl_id), // Point to the Enum decl
                                *visibility,
                                &mut self.ctx.diagnostics,
                            );
                            self.check_lang_item(&variant.annotations, v_sym_id, crate::lang_item::LangItemTarget::EnumVariant);
                        }
                        
                        self.current_scope = prev_scope;
                    }
                    Decl::Trait {
                        name,
                        methods,
                        visibility,
                        generic_params,
                        annotations,
                        associated_types,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str.clone(),
                            SymbolKind::Trait,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        self.check_lang_item(annotations, sym_id, crate::lang_item::LangItemTarget::Trait);


                        let trait_scope = self.enter_scope(crate::symbol::ScopeKind::Struct);
                        self.ctx.symbol_table.set_inner_scope(sym_id, trait_scope);
                        self.ctx.tables.decl_scopes.insert(*decl_id, trait_scope);
                        
                        if !generic_params.is_empty() {
                            for (idx, gp) in generic_params.iter().enumerate() {
                                let gp_name_str = self.source_manager.get_file(gp.name.file_id).unwrap().source[gp.name.start as usize..gp.name.end as usize]
                                    .to_string();
                                let gp_sym_id = self.ctx.symbol_table.declare_symbol(
                                    gp_name_str,
                                    crate::symbol::SymbolKind::TypeParam,
                                    self.current_scope,
                                    gp.name,
                                    Some(*decl_id),
                                    luna_ast::Visibility::Private,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx
                                    .tables
                                    .generic_param_symbols
                                    .insert((*decl_id, idx), gp_sym_id);
                                self.ctx.tables.symbol_decls.insert(gp_sym_id, *decl_id);
                            }
                        }

                        let mut trait_assoc_syms = Vec::new();
                        for assoc_id in associated_types {
                            let decl = &self.arena.decls[assoc_id.0 as usize];
                            if let Decl::TypeAlias { name: assoc_name, visibility: assoc_vis, .. } = decl {
                                let name_str = self.get_span_text(*assoc_name).to_string();
                                let assoc_sym = self.ctx.symbol_table.declare_symbol(
                                    name_str.clone(),
                                    SymbolKind::AssociatedType,
                                    self.current_scope,
                                    *assoc_name,
                                    Some(*assoc_id),
                                    *assoc_vis,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx.tables.decl_symbols.insert(*assoc_id, assoc_sym);
                                self.ctx.tables.symbol_decls.insert(assoc_sym, *assoc_id);
                                self.ctx.tables.assoc_type_traits.insert(assoc_sym, sym_id);
                                self.ctx.tables.assoc_type_names.insert((sym_id, name_str), assoc_sym);
                                trait_assoc_syms.push(assoc_sym);
                            }
                        }
                        self.ctx
                            .tables
                            .trait_associated_types
                            .insert(sym_id, trait_assoc_syms);

                        let mut trait_method_syms = Vec::new();
                        for method_id in methods {
                            let item = Item::Decl(*method_id);
                            self.declare_item(&item, DeclarationContext::TraitMethod(*visibility));
                            if let Some(&meth_sym) = self.ctx.tables.decl_symbols.get(method_id) {
                                trait_method_syms.push(meth_sym);
                            }
                        }
                        self.exit_scope();
                        self.ctx
                            .tables
                            .trait_methods
                            .insert(sym_id, trait_method_syms);
                    }
                    Decl::Impl {
                        trait_type,
                        self_type,
                        methods,
                        generic_params,
                        annotations,
                        associated_types,
                        ..
                    } => {
                        self.check_unsupported_lang_item(annotations, "impl");
                        let impl_scope = self.enter_scope(crate::symbol::ScopeKind::Struct);
                        self.ctx.tables.decl_scopes.insert(*decl_id, impl_scope);

                        if !generic_params.is_empty() {
                            for (idx, gp) in generic_params.iter().enumerate() {
                                let gp_name_str = self.source_manager.get_file(gp.name.file_id).unwrap().source[gp.name.start as usize..gp.name.end as usize]
                                    .to_string();
                                let gp_sym_id = self.ctx.symbol_table.declare_symbol(
                                    gp_name_str,
                                    crate::symbol::SymbolKind::TypeParam,
                                    self.current_scope,
                                    gp.name,
                                    Some(*decl_id),
                                    luna_ast::Visibility::Private, // Generic params don't really have visibility
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx
                                    .tables
                                    .generic_param_symbols
                                    .insert((*decl_id, idx), gp_sym_id);
                                self.ctx.tables.symbol_decls.insert(gp_sym_id, *decl_id);
                            }
                        }

                        for assoc_id in associated_types {
                            let decl = &self.arena.decls[assoc_id.0 as usize];
                            if let Decl::TypeAlias { name: assoc_name, visibility: assoc_vis, .. } = decl {
                                let name_str = self.get_span_text(*assoc_name).to_string();
                                let assoc_sym = self.ctx.symbol_table.declare_symbol(
                                    name_str,
                                    SymbolKind::AssociatedType,
                                    self.current_scope,
                                    *assoc_name,
                                    Some(*assoc_id),
                                    *assoc_vis,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx.tables.decl_symbols.insert(*assoc_id, assoc_sym);
                                self.ctx.tables.symbol_decls.insert(assoc_sym, *assoc_id);
                            }
                        }

                        let method_context = if trait_type.is_some() {
                            DeclarationContext::TraitMethod(luna_ast::Visibility::Public)
                        } else {
                            DeclarationContext::ImplMethod
                        };
                        for method_id in methods {
                            let item = Item::Decl(*method_id);
                            self.declare_item(&item, method_context);
                        }
                        self.exit_scope();

                        let self_sym_opt = self.ctx.tables.type_symbols.get(self_type).copied().or_else(|| {
                            let self_ast_ty = &self.arena.types[self_type.0 as usize];
                            if let luna_ast::Type::Named { segments, .. } = self_ast_ty {
                                if segments.len() == 1 {
                                    let s = &segments[0];
                                    let name = self.get_span_text(*s);
                                    self.ctx.symbol_table.lookup_with_ctxt(name, s.ctxt, self.current_scope)
                                        .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(name, s.ctxt, crate::ScopeId(0)))
                                } else {
                                    let mut scope = self.current_scope;
                                    let mut res = None;
                                    for (i, seg) in segments.iter().enumerate() {
                                        let seg_name = self.get_span_text(*seg);
                                        let sym_id = if i == 0 {
                                            self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, scope)
                                                .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, crate::ScopeId(0)))
                                        } else {
                                            self.ctx.symbol_table.lookup_exact_with_ctxt(seg_name, seg.ctxt, scope)
                                        };
                                        if let Some(id) = sym_id {
                                            res = Some(id);
                                            if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                                scope = inner;
                                            } else if i < segments.len() - 1 {
                                                res = None;
                                                break;
                                            }
                                        } else {
                                            res = None;
                                            break;
                                        }
                                    }
                                    res
                                }
                            } else {
                                None
                            }
                        });

                        let trait_sym_opt = trait_type.and_then(|trait_ty_id| {
                            self.ctx.tables.type_symbols.get(&trait_ty_id).copied().or_else(|| {
                                let trait_ast_ty = &self.arena.types[trait_ty_id.0 as usize];
                                if let luna_ast::Type::Named { segments, .. } = trait_ast_ty {
                                    if segments.len() == 1 {
                                        let s = &segments[0];
                                        let name = self.get_span_text(*s);
                                        self.ctx.symbol_table.lookup_with_ctxt(name, s.ctxt, self.current_scope)
                                            .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(name, s.ctxt, crate::ScopeId(0)))
                                    } else {
                                        let mut scope = self.current_scope;
                                        let mut res = None;
                                        for (i, seg) in segments.iter().enumerate() {
                                            let seg_name = self.get_span_text(*seg);
                                            let sym_id = if i == 0 {
                                                self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, scope)
                                                    .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, crate::ScopeId(0)))
                                            } else {
                                                self.ctx.symbol_table.lookup_exact_with_ctxt(seg_name, seg.ctxt, scope)
                                            };
                                            if let Some(id) = sym_id {
                                                res = Some(id);
                                                if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                                    scope = inner;
                                                } else if i < segments.len() - 1 {
                                                    res = None;
                                                    break;
                                                }
                                            } else {
                                                res = None;
                                                break;
                                            }
                                        }
                                        res
                                    }
                                } else {
                                    None
                                }
                            })
                        });

                        if let Some(self_sym) = self_sym_opt {
                            let key = crate::semantic_tables::ImplKey {
                                trait_id: trait_sym_opt,
                                self_type_def: self_sym,
                            };

                            self.ctx
                                .tables
                                .trait_impls
                                .entry(key.clone())
                                .or_default()
                                .push(*decl_id);

                            let mut method_syms = Vec::new();
                            for method_id in methods {
                                if let Some(&m_sym) = self.ctx.tables.decl_symbols.get(method_id) {
                                    method_syms.push(m_sym);
                                    self.ctx.tables.method_impls.insert(m_sym, key.clone());
                                }
                            }
                            self.ctx
                                .tables
                                .impl_methods
                                .entry(key.clone())
                                .or_default()
                                .extend(method_syms);


                        }
                    }
                    Decl::Macro {
                        name, visibility, annotations, ..
                    } => {
                        if !self.ctx.tables.decl_macros.contains_key(decl_id) {
                            let name_str =
                                self.get_span_text(*name).to_string();
                            let sym_id = self.ctx.symbol_table.declare_symbol(
                                name_str,
                                SymbolKind::Macro,
                                self.current_scope,
                                *name,
                                Some(*decl_id),
                                *visibility,
                                &mut self.ctx.diagnostics,
                            );
                            self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                            self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                            self.ctx.tables.decl_macros.insert(*decl_id, sym_id);
                            self.ctx.tables.macro_decls.insert(sym_id, *decl_id);
                            self.check_unsupported_lang_item(annotations, "macro");
                        }
                    }
                    Decl::TypeAlias {
                        name,
                        visibility,
                        generic_params,
                        annotations,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();
                        let sym_id = self.ctx.symbol_table.declare_symbol(
                            name_str,
                            SymbolKind::Alias,
                            self.current_scope,
                            *name,
                            Some(*decl_id),
                            *visibility,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        self.check_unsupported_lang_item(annotations, "type alias");

                        let alias_scope = self.enter_scope(crate::symbol::ScopeKind::Struct);
                        self.ctx.symbol_table.set_inner_scope(sym_id, alias_scope);
                        
                        if !generic_params.is_empty() {
                            for (idx, gp) in generic_params.iter().enumerate() {
                                let gp_name_str = self.source_manager.get_file(gp.name.file_id).unwrap().source[gp.name.start as usize..gp.name.end as usize]
                                    .to_string();
                                let gp_sym_id = self.ctx.symbol_table.declare_symbol(
                                    gp_name_str,
                                    crate::symbol::SymbolKind::TypeParam,
                                    self.current_scope,
                                    gp.name,
                                    Some(*decl_id),
                                    luna_ast::Visibility::Private,
                                    &mut self.ctx.diagnostics,
                                );
                                self.ctx
                                    .tables
                                    .generic_param_symbols
                                    .insert((*decl_id, idx), gp_sym_id);
                                self.ctx.tables.symbol_decls.insert(gp_sym_id, *decl_id);
                            }
                        }
                        self.exit_scope();
                    }
                    Decl::Module {
                        name,
                        items,
                        visibility,
                        annotations,
                        ..
                    } => {
                        let name_str =
                            self.get_span_text(*name).to_string();
                        let sym_id = if let Some(existing) = self
                            .ctx
                            .symbol_table
                            .lookup_exact(&name_str, self.current_scope)
                        {
                            existing
                        } else {
                            self.ctx.symbol_table.declare_symbol(
                                name_str,
                                SymbolKind::Module,
                                self.current_scope,
                                *name,
                                Some(*decl_id),
                                *visibility,
                                &mut self.ctx.diagnostics,
                            )
                        };
                        self.ctx.tables.decl_symbols.insert(*decl_id, sym_id);
                        self.ctx.tables.symbol_decls.insert(sym_id, *decl_id);
                        self.check_unsupported_lang_item(annotations, "module");
                        let mod_scope = if let Some(inner) =
                            self.ctx.symbol_table.symbols[sym_id.0 as usize].inner_scope
                        {
                            inner
                        } else {
                            let s = self.enter_scope(crate::symbol::ScopeKind::Module);
                            self.ctx.symbol_table.set_inner_scope(sym_id, s);
                            s
                        };
                        let prev_scope = self.current_scope;
                        self.current_scope = mod_scope;
                        for item_id in items {
                            let item = Item::Decl(*item_id);
                            self.declare_item(&item, DeclarationContext::Free);
                        }
                        self.current_scope = prev_scope;
                    }
                    Decl::Using { path, alias, .. } => {
                        let alias_str =
                            self.get_span_text(*alias).trim_matches('"').to_string();
                        let path_str: Vec<&str> = path
                            .iter()
                            .map(|seg| self.get_span_text(*seg).trim_matches('"'))
                            .collect();
                        let target_scope_opt = if path_str.len() == 1 {
                            self.ctx
                                .symbol_table
                                .lookup(path_str[0], self.current_scope)
                                .and_then(|s| {
                                    self.ctx.symbol_table.symbols[s.0 as usize].inner_scope
                                })
                        } else {
                            let mut curr = self
                                .ctx
                                .symbol_table
                                .lookup(path_str[0], self.current_scope)
                                .and_then(|s| {
                                    self.ctx.symbol_table.symbols[s.0 as usize].inner_scope
                                });
                            for &seg in &path_str[1..] {
                                if let Some(scope) = curr {
                                    curr = self.ctx.symbol_table.lookup_exact(seg, scope).and_then(
                                        |s| self.ctx.symbol_table.symbols[s.0 as usize].inner_scope,
                                    );
                                } else {
                                    break;
                                }
                            }
                            curr
                        };

                        if let Some(target_scope) = target_scope_opt {
                            let sym_id = self.ctx.symbol_table.declare_symbol(
                                alias_str,
                                SymbolKind::Module,
                                self.current_scope,
                                *alias,
                                Some(*decl_id),
                                Visibility::Private,
                                &mut self.ctx.diagnostics,
                            );
                            self.ctx.symbol_table.set_inner_scope(sym_id, target_scope);
                        } else {
                            self.ctx.diagnostics.push(luna_common::diagnostic::Diagnostic::error(format!("unresolved module or path `{}`", alias_str)).with_span(*alias));
                        }
                    }
                    Decl::Import { name, kind, .. } => {
                        self.resolve_import_decl(name, kind);
                    }
                    _ => {}
                }
            }
            Item::Stmt(_) => {}
        }
    }






    pub fn resolve_type(&mut self, type_id: &luna_ast::TypeId) {
        let ast_ty = &self.arena.types[type_id.0 as usize].clone();
        match ast_ty {
            luna_ast::Type::Builtin(_) | luna_ast::Type::Never | luna_ast::Type::Lifetime(_) => {}

            luna_ast::Type::Named { segments, generic_args, associated_bindings } => {
                let last_span = segments.last().copied();
                let ctxt = last_span.map(|s| s.ctxt).unwrap_or(luna_common::ids::SyntaxContext::ROOT);
                let symbol = if segments.len() == 1 {
                    let name = segments.last().map(|span| self.get_span_text(*span));
                    name.and_then(|n| {
                        self.ctx.symbol_table.lookup_with_ctxt(n, ctxt, self.current_scope)
                            .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(n, ctxt, crate::ScopeId(0)))
                    })
                } else {
                    let mut scope = self.current_scope;
                    let mut res = None;
                    for (i, seg) in segments.iter().enumerate() {
                        let seg_name = self.get_span_text(*seg);
                        let sym_id = if i == 0 {
                            self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, scope)
                                .or_else(|| self.ctx.symbol_table.lookup_with_ctxt(seg_name, seg.ctxt, crate::ScopeId(0)))
                        } else {
                            self.ctx.symbol_table.lookup_exact_with_ctxt(seg_name, seg.ctxt, scope)
                        };
                        if let Some(id) = sym_id {
                            res = Some(id);
                            if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                scope = inner;
                            } else if i < segments.len() - 1 {
                                res = None;
                                break;
                            }
                        } else {
                            res = None;
                            break;
                        }
                    }
                    res
                };
                if let Some(sym) = symbol {
                    self.ctx.tables.type_symbols.insert(*type_id, sym);
                }
                for arg in generic_args {
                    self.resolve_type(arg);
                }
                for binding in associated_bindings {
                    self.resolve_type(&binding.ty);
                }
            }

            luna_ast::Type::Reference { lifetime: _, inner, .. } => {
                self.resolve_type(inner);
            }
            luna_ast::Type::Pointer { inner, .. } => {
                self.resolve_type(inner);
            }
            luna_ast::Type::Array { element_type, size } => {
                self.resolve_type(element_type);
                self.resolve_expr(size);
            }
            luna_ast::Type::Slice { inner } => {
                self.resolve_type(inner);
            }
            luna_ast::Type::Tuple { elements } => {
                for elem in elements {
                    self.resolve_type(elem);
                }
            }
            luna_ast::Type::Function { params, return_type, .. } => {
                for param in params {
                    self.resolve_type(param);
                }
                if let Some(ret) = return_type {
                    self.resolve_type(ret);
                }
            }
            luna_ast::Type::TraitObject { trait_type } => {
                self.resolve_type(trait_type);
            }
            luna_ast::Type::Typeof { expr } => {
                self.resolve_expr(expr);
            }
            luna_ast::Type::MacroCall { .. } => {}
        }
    }

    fn resolve_decl_types(&mut self, decl_id: &luna_ast::DeclId) {
        let decl = &self.arena.decls[decl_id.0 as usize].clone();
        match decl {
            luna_ast::Decl::Param { ty, .. } => {
                if let Some(t) = ty {
                    self.resolve_type(t);
                }
            }
            luna_ast::Decl::Function { params, return_type, .. } => {
                for param_id in params {
                    self.resolve_decl_types(param_id);
                }
                if let Some(ret) = return_type {
                    self.resolve_type(ret);
                }
            }
            luna_ast::Decl::Var { type_annot, .. } => {
                if let Some(ty) = type_annot {
                    self.resolve_type(ty);
                }
            }
            luna_ast::Decl::Struct { fields, .. } => {
                for field in fields {
                    self.resolve_type(&field.ty);
                }
            }
            luna_ast::Decl::Enum { variants, .. } => {
                for variant in variants {
                    for field_id in &variant.fields {
                        self.resolve_decl_types(field_id);
                    }
                }
            }
            luna_ast::Decl::TypeAlias { aliased_type, .. } => {
                if let Some(ty) = aliased_type {
                    self.resolve_type(ty);
                }
            }
            luna_ast::Decl::Impl { self_type, trait_type, .. } => {
                self.resolve_type(self_type);
                if let Some(tt) = trait_type {
                    self.resolve_type(tt);
                }
            }
            _ => {}
        }
    }

    fn resolve_item_body(&mut self, item: &Item) {
        match item {
            Item::Decl(decl_id) => {
                let prev_scope = self.current_scope;
                if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {
                    if let Some(scope) = self.ctx.symbol_table.get_symbol(sym_id).inner_scope {
                        self.current_scope = scope;
                    }
                } else if let Some(&scope) = self.ctx.tables.decl_scopes.get(decl_id) {
                    self.current_scope = scope;
                }
                self.resolve_decl_types(decl_id);
                self.current_scope = prev_scope;
                
                let decl = &self.arena.decls[decl_id.0 as usize];
                match decl {
                    Decl::Function { body, .. } => {
                        if let Some(body_stmt) = body {
                            if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {
                                if let Some(scope) =
                                    self.ctx.symbol_table.get_symbol(sym_id).inner_scope
                                {
                                    let prev_scope = self.current_scope;
                                    self.current_scope = scope;
                                    self.resolve_stmt(body_stmt);
                                    self.current_scope = prev_scope;
                                }
                            }
                        }
                    }
                    Decl::Var { initializer, .. } => {
                        if let Some(init) = initializer {
                            self.resolve_expr(init);
                        }
                    }
                    Decl::Impl { methods, .. } => {
                        for method_id in methods {
                            self.resolve_item_body(&Item::Decl(*method_id));
                        }
                    }
                    Decl::Trait { methods, .. } => {
                        for method_id in methods {
                            self.resolve_item_body(&Item::Decl(*method_id));
                        }
                    }
                    Decl::Extern { func, .. } => {
                        self.resolve_item_body(&Item::Decl(*func));
                    }
                    Decl::Module { items, .. } => {
                        if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {
                            if let Some(inner) =
                                self.ctx.symbol_table.symbols[sym_id.0 as usize].inner_scope
                            {
                                let prev_scope = self.current_scope;
                                self.current_scope = inner;
                                for item_id in items {
                                    let item = Item::Decl(*item_id);
                                    self.resolve_item_body(&item);
                                }
                                self.current_scope = prev_scope;
                            }
                        }
                    }
                    _ => {}
                }
            }
            Item::Stmt(stmt_id) => {
                self.resolve_stmt(stmt_id);
            }
        }
    }

    fn resolve_stmt(&mut self, stmt_id: &luna_ast::StmtId) {
        let stmt = &self.arena.stmts[stmt_id.0 as usize];
        match stmt {
            Stmt::Block { body, tail_expr } => {
                self.enter_scope(crate::symbol::ScopeKind::Block);
                self.resolve_items(body);
                if let Some(expr) = tail_expr {
                    self.resolve_expr(expr);
                }
                self.exit_scope();
            }
            Stmt::Expr { expr, .. } => {
                self.resolve_expr(expr);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition);
                self.resolve_stmt(then_branch);
                if let Some(else_br) = else_branch {
                    self.resolve_stmt(else_br);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
                iterable,
                pattern,
                ..
            } => {
                self.enter_scope(crate::symbol::ScopeKind::Block);
                if let Some(item) = init {
                    self.declare_item(item, DeclarationContext::Free);
                    self.resolve_item_body(item);
                }

                if let Some(pat) = pattern {
                    self.resolve_pattern(pat, luna_ast::Visibility::Private, false);
                }

                if let Some(iter) = iterable {
                    self.resolve_expr(iter);
                }
                if let Some(c) = cond {
                    self.resolve_expr(c);
                }
                if let Some(s) = step {
                    self.resolve_expr(s);
                }
                self.resolve_stmt(body);
                self.exit_scope();
            }
            Stmt::Return { value } => {
                if let Some(val) = value {
                    self.resolve_expr(val);
                }
            }
            Stmt::Unsafe { body } => {
                self.resolve_stmt(body);
            }
            _ => {}
        }
    }

    fn resolve_pattern(
        &mut self,
        pat_id: &luna_ast::PatId,
        visibility: luna_ast::Visibility,
        is_mutable: bool,
    ) {
        match &self.arena.pats[pat_id.0 as usize] {
            Pattern::Identifier { segments } => {
                if segments.len() > 1 {
                    // It's a path, like Color::Red
                    let mut full_name = String::new();
                    for (i, seg) in segments.iter().enumerate() {
                        if i > 0 {
                            full_name.push_str("::");
                        }
                        full_name.push_str(self.get_span_text(*seg));
                    }
                    
                    let mut resolved_sym = self.ctx.symbol_table.lookup(&full_name, self.current_scope);
                    if resolved_sym.is_none() {
                        let mut current_scope = self.current_scope;
                        for (i, seg) in segments.iter().enumerate() {
                            let seg_name = self.get_span_text(*seg);
                            let sym_id = if i == 0 {
                                self.ctx.symbol_table.lookup(seg_name, current_scope)
                            } else {
                                self.ctx.symbol_table.lookup_exact(seg_name, current_scope)
                            };
                            
                            if let Some(id) = sym_id {
                                resolved_sym = Some(id);
                                if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                    current_scope = inner;
                                } else {
                                    break;
                                }
                            } else {
                                resolved_sym = None;
                                break;
                            }
                        }
                    }

                    if let Some(existing_sym_id) = resolved_sym {
                        let sym = self.ctx.symbol_table.get_symbol(existing_sym_id);
                        if matches!(sym.kind, SymbolKind::EnumVariant(_)) {
                            self.ctx.tables.pat_symbols.insert(*pat_id, existing_sym_id);
                            return;
                        }
                    }
                    // If not found, we shouldn't declare a variable with a path!
                    // It's just unresolved. We might want to emit an error, but let's leave it for now.
                } else if let Some(name) = segments.last() {
                    let name_str = self.get_span_text(*name).to_string();

                    // Check if it's an enum variant (brought into scope, though in Mellis they are usually Enum::Variant)
                    if let Some(existing_sym_id) =
                        self.ctx.symbol_table.lookup(&name_str, self.current_scope)
                    {
                        let sym = self.ctx.symbol_table.get_symbol(existing_sym_id);
                        if matches!(sym.kind, SymbolKind::EnumVariant(_)) {
                            self.ctx.tables.pat_symbols.insert(*pat_id, existing_sym_id);
                            return;
                        }
                    }

                    let sym_id = self.ctx.symbol_table.declare_symbol(
                        name_str,
                        if is_mutable {
                            SymbolKind::Variable
                        } else {
                            SymbolKind::Constant
                        },
                        self.current_scope,
                        *name,
                        None,
                        visibility,
                        &mut self.ctx.diagnostics,
                    );
                    self.ctx.tables.pat_symbols.insert(*pat_id, sym_id);
                }
            }
            Pattern::Tuple { elements, .. } => {
                for element in elements {
                    self.resolve_pattern(element, visibility, is_mutable);
                }
            }
            Pattern::Enum { path, fields } => {
                let mut full_name = String::new();
                for (i, seg) in path.iter().enumerate() {
                    if i > 0 {
                        full_name.push_str("::");
                    }
                    full_name.push_str(self.get_span_text(*seg));
                }
                
                let mut resolved_sym = self.ctx.symbol_table.lookup(&full_name, self.current_scope);
                if resolved_sym.is_none() {
                    let mut current_scope = self.current_scope;
                    for (i, seg) in path.iter().enumerate() {
                        let seg_name = self.get_span_text(*seg);
                        let sym_id = if i == 0 {
                            self.ctx.symbol_table.lookup(seg_name, current_scope)
                        } else {
                            self.ctx.symbol_table.lookup_exact(seg_name, current_scope)
                        };
                        
                        if let Some(id) = sym_id {
                            if i > 0 && !self.ctx.symbol_table.is_accessible(id, self.current_scope, self.ctx.current_provider) {
                                self.ctx.diagnostics.push(
                                    luna_common::Diagnostic::error(format!(
                                        "Symbol '{}' is private and cannot be accessed from this scope",
                                        seg_name
                                    ))
                                    .with_span(*seg),
                                );
                                resolved_sym = None;
                                break;
                            }
                            resolved_sym = Some(id);
                            if let Some(inner) = self.ctx.symbol_table.symbols[id.0 as usize].inner_scope {
                                current_scope = inner;
                            } else {
                                break;
                            }
                        } else {
                            resolved_sym = None;
                            break;
                        }
                    }
                }

                if let Some(existing_sym_id) = resolved_sym {
                    if !self.ctx.symbol_table.is_accessible(existing_sym_id, self.current_scope, self.ctx.current_provider) {
                        let span = path.last().copied().unwrap_or(luna_common::Span::default());
                        self.ctx.diagnostics.push(
                            luna_common::Diagnostic::error(format!(
                                "Symbol '{}' is private and cannot be accessed from this scope",
                                full_name
                            ))
                            .with_span(span),
                        );
                    } else {
                        let sym = self.ctx.symbol_table.get_symbol(existing_sym_id);
                        if matches!(sym.kind, SymbolKind::EnumVariant(_)) {
                            self.ctx.tables.pat_symbols.insert(*pat_id, existing_sym_id);
                        }
                    }
                }
                for element in fields {
                    self.resolve_pattern(element, visibility, is_mutable);
                }
            }
            Pattern::Struct { fields, .. } => {
                for field in fields {
                    if let Some(pattern) = field.pattern {
                        self.resolve_pattern(&pattern, visibility, is_mutable);
                    }
                }
            }
            Pattern::Literal(_) | Pattern::Wildcard => {}
        }
    }

    fn resolve_expr(&mut self, expr_id: &luna_ast::ExprId) {
        let expr = &self.arena.exprs[expr_id.0 as usize];
        match expr {
            Expr::Identifier { segments, .. } => {
                if !segments.is_empty() {
                    let name_str = segments
                        .iter()
                        .map(|seg| self.get_span_text(*seg))
                        .collect::<Vec<_>>()
                        .join("::");

                    let mut resolved_sym = self.ctx.symbol_table.lookup_with_ctxt(
                        &name_str,
                        segments[0].ctxt,
                        self.current_scope,
                    );

                    if resolved_sym.is_none() {
                        let mut current_scope = self.current_scope;

                        for (i, seg) in segments.iter().enumerate() {
                            let seg_name = self.get_span_text(*seg);

                            let sym_id = if i == 0 {
                                self.ctx.symbol_table.lookup_with_ctxt(
                                    seg_name,
                                    seg.ctxt,
                                    current_scope,
                                )
                            } else {
                                self.ctx.symbol_table.lookup_exact_with_ctxt(
                                    seg_name,
                                    seg.ctxt,
                                    current_scope,
                                )
                            };

                            if let Some(id) = sym_id {
                                if i > 0 && !self.ctx.symbol_table.is_accessible(id, self.current_scope, self.ctx.current_provider) {
                                    self.ctx.diagnostics.push(
                                        luna_common::Diagnostic::error(format!(
                                            "Symbol '{}' is private and cannot be accessed from this scope",
                                            seg_name
                                        ))
                                        .with_span(*seg),
                                    );
                                    resolved_sym = None;
                                    break;
                                }
                                resolved_sym = Some(id);
                                if let Some(inner) =
                                    self.ctx.symbol_table.symbols[id.0 as usize].inner_scope
                                {
                                    current_scope = inner;
                                } else if i < segments.len() - 1 {
                                    resolved_sym = None;
                                    break;
                                }
                            } else {
                                resolved_sym = None;
                                break;
                            }
                        }
                    }

                    if let Some(sym_id) = resolved_sym {
                        let sym = self.ctx.symbol_table.get_symbol(sym_id);
                        if !self.ctx.symbol_table.is_accessible(sym_id, self.current_scope, self.ctx.current_provider) {
                            let name_str = segments
                                .iter()
                                .map(|seg| self.get_span_text(*seg))
                                .collect::<Vec<_>>()
                                .join("::");
                            let span = Span {
                                file_id: segments[0].file_id,
                                start: segments[0].start,
                                end: segments.last().unwrap().end,
                                ctxt: segments[0].ctxt,
                            };
                            self.ctx.diagnostics.push(
                                luna_common::Diagnostic::error(format!(
                                    "Symbol '{}' is private and cannot be accessed from this scope",
                                    name_str
                                ))
                                .with_span(span),
                            );
                            return;
                        }
                        if matches!(sym.kind, crate::symbol::SymbolKind::Macro) {
                            let name_str = segments
                                .iter()
                                .map(|seg| self.get_span_text(*seg))
                                .collect::<Vec<_>>()
                                .join("::");
                            let span = Span {
                                file_id: segments[0].file_id,
                                start: segments[0].start,
                                end: segments.last().unwrap().end,
                                ctxt: segments[0].ctxt,
                            };
                            self.ctx.diagnostics.push(
                                luna_common::Diagnostic::error(format!(
                                    "macro '{}' cannot be used as a value",
                                    name_str
                                ))
                                .with_span(span),
                            );
                            return;
                        }
                        self.ctx.tables.expr_symbols.insert(*expr_id, sym_id);

                        if !self.active_lambdas.is_empty() {
                            let sym = self.ctx.symbol_table.get_symbol(sym_id);
                            if matches!(
                                sym.kind,
                                crate::symbol::SymbolKind::Variable
                                    | crate::symbol::SymbolKind::Constant
                            ) {
                                for (lambda_id, lambda_scope) in &self.active_lambdas {
                                    if self.ctx.symbol_table.is_ancestor(sym.scope, *lambda_scope)
                                        && sym.scope != *lambda_scope
                                    {
                                        let captures = self
                                            .ctx
                                            .tables
                                            .expr_captures
                                            .entry(*lambda_id)
                                            .or_default();
                                        if !captures.contains(&sym_id) {
                                            captures.push(sym_id);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        let name_str = segments
                            .iter()
                            .map(|seg| self.get_span_text(*seg))
                            .collect::<Vec<_>>()
                            .join("::");
                        let diag = luna_common::Diagnostic::error(format!(
                            "Symbol '{}' not found",
                            name_str
                        ))
                        .with_span(*segments.first().unwrap());
                        self.ctx.diagnostics.push(diag);
                    }
                }
            }
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Unary { operand, .. } => {
                self.resolve_expr(operand);
            }
            Expr::Call { callee, args, .. } => {
                self.resolve_expr(callee);
                for arg in args {
                    self.resolve_expr(&arg.value);
                }
            }
            Expr::Assign { lvalue, value, .. } => {
                self.resolve_expr(lvalue);
                self.resolve_expr(value);
            }
            Expr::MethodCall { object, args, .. } => {
                self.resolve_expr(object);
                for arg in args {
                    self.resolve_expr(&arg.value);
                }
            }
            Expr::Member { object, .. } => {
                self.resolve_expr(object);
            }
            Expr::StructInit { fields, .. } => {
                for field in fields {
                    self.resolve_expr(&field.value);
                }
            }
            Expr::Index { base, index } => {
                self.resolve_expr(base);
                self.resolve_expr(index);
            }
            Expr::TupleIndex { object, .. } => {
                self.resolve_expr(object);
            }
            Expr::Cast { expr: e, .. } => {
                self.resolve_expr(e);
            }
            Expr::Match { subject, arms, .. } => {
                self.resolve_expr(subject);
                for arm in arms {
                    self.enter_scope(crate::symbol::ScopeKind::Block);
                    self.resolve_pattern(&arm.pattern, luna_ast::Visibility::Private, true);
                    self.resolve_stmt(&arm.body);
                    self.exit_scope();
                }
            }
            Expr::Lambda {
                params,
                body,
                return_type,
                ..
            } => {
                let name = format!("__closure_{}", expr_id.0);
                let closure_sym_id = self.ctx.symbol_table.declare_symbol(
                    name,
                    SymbolKind::Function,
                    self.current_scope,
                    luna_common::ids::Span::new(luna_common::ids::FileId(0), 0, 0),
                    None,
                    luna_ast::Visibility::Private,
                    &mut self.ctx.diagnostics,
                );
                self.ctx
                    .tables
                    .expr_symbols
                    .insert(*expr_id, closure_sym_id);

                self.enter_scope(crate::symbol::ScopeKind::Function);
                self.active_lambdas.push((*expr_id, self.current_scope));
                for param_id in params {
                    if let Decl::Param {
                        name: p_name,
                        visibility: p_vis,
                        ty,
                        ..
                    } = &self.arena.decls[param_id.0 as usize]
                    {
                        let p_name_str =
                            self.get_span_text(*p_name).to_string();
                        let p_sym_id = self.ctx.symbol_table.declare_symbol(
                            p_name_str,
                            SymbolKind::Variable,
                            self.current_scope,
                            *p_name,
                            Some(*param_id),
                            *p_vis,
                            &mut self.ctx.diagnostics,
                        );
                        self.ctx.tables.decl_symbols.insert(*param_id, p_sym_id);
                        self.ctx.tables.symbol_decls.insert(p_sym_id, *param_id);
                    }
                }
                self.resolve_stmt(body);
                self.active_lambdas.pop();
                self.exit_scope();
            }
            Expr::Try { expr: e, .. } | Expr::Await { expr: e } => {
                self.resolve_expr(e);
            }
            Expr::ArrayLiteral { elements } | Expr::TupleLiteral { elements } => {
                for e in elements {
                    self.resolve_expr(e);
                }
            }
            Expr::MacroCall {
                path, name, span, ..
            } => {
                let path_strs: Vec<&str> = if !path.is_empty() {
                    path.iter()
                        .map(|seg| self.get_span_text(*seg))
                        .collect()
                } else {
                    vec![self.get_span_text(*name)]
                };
                if let Some(sym_id) = self
                    .ctx
                    .symbol_table
                    .lookup_macro(&path_strs, self.current_scope)
                {
                    self.ctx.tables.expr_symbols.insert(*expr_id, sym_id);
                } else {
                    let macro_name = path_strs.join("::");
                    self.ctx.diagnostics.push(
                        luna_common::Diagnostic::error(format!(
                            "no macro named '{}' in scope",
                            macro_name
                        ))
                        .with_span(*span),
                    );
                }
            }
            Expr::Comptime { body } => {
                self.resolve_stmt(body);
            }
            _ => {}
        }
    }

    /// Resolves lifetime expressions in a function's lifetime signature.
    /// This maps identifiers in `life_from(x)` and `outlives(x, y)` to actual parameter symbols.
    pub fn resolve_lifetime_signature(&mut self, decl_id: &luna_ast::DeclId) {
        let decl = &self.arena.decls[decl_id.0 as usize];
        let Decl::Function {
            params,
            lifetime_signature,
            ..
        } = decl
        else {
            return;
        };

        // Map parameter names to their symbol IDs
        let mut param_names: std::collections::HashMap<String, luna_common::ids::SymbolId> =
            std::collections::HashMap::new();
        for param_id in params {
            if let Decl::Param { name, .. } = &self.arena.decls[param_id.0 as usize] {
                let name_str = self.get_span_text(*name).to_string();
                if let Some(&sym_id) = self.ctx.tables.decl_symbols.get(param_id) {
                    param_names.insert(name_str, sym_id);
                }
            }
        }

        // Resolve provenance expression
        if let Some(ref provenance) = lifetime_signature.provenance {
            match provenance {
                luna_ast::LifetimeExpr::Provenance(span) => {
                    let name = self.get_span_text(*span).to_string();
                    if !param_names.contains_key(&name) {
                        self.ctx.diagnostics.push(
                            luna_common::Diagnostic::error(format!(
                                "lifetime '{}' does not refer to any parameter in scope",
                                name
                            ))
                            .with_span(*span),
                        );
                    }
                }
                luna_ast::LifetimeExpr::ProvenanceSet(idents) => {
                    for span in idents {
                        let name = self.get_span_text(*span).to_string();
                        if !param_names.contains_key(&name) {
                            self.ctx.diagnostics.push(
                                luna_common::Diagnostic::error(format!(
                                    "lifetime '{}' does not refer to any parameter in scope",
                                    name
                                ))
                                .with_span(*span),
                            );
                        }
                    }
                }
            }
        }

        // Resolve outlives constraints
        for constraint in &lifetime_signature.constraints {
            let first_name = self.get_span_text(constraint.first).to_string();
            let second_name = self.get_span_text(constraint.second).to_string();

            if !param_names.contains_key(&first_name) {
                self.ctx.diagnostics.push(
                    luna_common::Diagnostic::error(format!(
                        "lifetime '{}' does not refer to any parameter in scope",
                        first_name
                    ))
                    .with_span(constraint.first),
                );
            }
            if !param_names.contains_key(&second_name) {
                self.ctx.diagnostics.push(
                    luna_common::Diagnostic::error(format!(
                        "lifetime '{}' does not refer to any parameter in scope",
                        second_name
                    ))
                    .with_span(constraint.second),
                );
            }
        }

        // Build canonical lifetime contract if valid
        if let Ok(contract) = crate::CanonicalLifetimeContract::from_ast(
            lifetime_signature,
            params,
            self.arena,
            self.source_manager,
        ) {
            if !contract.is_empty() {
                if let Some(&fn_sym_id) = self.ctx.tables.decl_symbols.get(decl_id) {
                    self.ctx.tables.fn_lifetime_contracts.insert(fn_sym_id, contract);
                }
            }
        }
    }
}
