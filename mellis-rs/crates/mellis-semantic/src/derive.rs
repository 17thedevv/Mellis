//! Derive macro registry and expansion system.
//!
//! This module implements a generic derive/attribute macro mechanism following
//! Rust's proc-macro model:
//!
//! - `#[derive(Trait1, Trait2)]` on structs/enums invokes registered derive macros
//! - Each derive macro receives a DeriveContext with type information
//! - Macros return generated AST items (typically impl blocks)
//! - Generated items feed back into the normal compilation pipeline

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mellis_ast::{AstArena, Decl, Item, TypeId};
use mellis_common::diagnostic::Diagnostic;
use mellis_common::ids::{FileId, Span};
use mellis_lexer::Lexer;
use mellis_parser::Parser;

/// Tracks which trait definitions have been emitted to prevent duplicates.
pub struct TraitEmitter<'arena> {
    emitted_traits: &'arena mut HashSet<String>,
}

impl<'arena> TraitEmitter<'arena> {
    pub fn new(emitted_traits: &'arena mut HashSet<String>) -> Self {
        Self { emitted_traits }
    }

    /// Emit a trait definition if it hasn't been emitted yet.
    /// Returns true if the trait was emitted, false if it was already emitted.
    pub fn ensure_trait_defined(&mut self, ctx: &mut DeriveContext, trait_name: &str, trait_def: &str) -> bool {
        if self.emitted_traits.insert(trait_name.to_string()) {
            let _ = ctx.parse_and_append_item(trait_def);
            true
        } else {
            false
        }
    }
}

/// A derive macro takes a type declaration and generates implementation items.
///
/// The macro receives a context for inspecting the type and emitting code,
/// and returns either generated items or a diagnostic error.
pub type DeriveMacro = Arc<dyn Fn(&mut DeriveContext, &DeriveInput) -> Result<Vec<Item>, Diagnostic>>;

/// Context provided to derive macros for inspecting the target type and emitting code.
pub struct DeriveContext<'arena> {
    /// The AST arena for allocating new nodes
    pub arena: &'arena mut AstArena,
    /// The source text for span resolution
    source: &'arena mut String,
    /// The file ID for diagnostics
    file_id: FileId,
    /// Span of the original derive attribute (for error reporting)
    pub derive_span: Span,
    /// Expansion counter for generating unique identifiers
    expansion_counter: &'arena mut u32,
    /// Diagnostics accumulated during expansion
    diagnostics: &'arena mut Vec<Diagnostic>,
    /// Tracks which trait definitions have been emitted
    emitted_traits: &'arena mut HashSet<String>,
    /// Extra items (like trait definitions) emitted during expansion
    pub extra_items: Vec<Item>,
}

impl<'arena> DeriveContext<'arena> {
    pub fn new(
        arena: &'arena mut AstArena,
        source: &'arena mut String,
        file_id: FileId,
        derive_span: Span,
        expansion_counter: &'arena mut u32,
        diagnostics: &'arena mut Vec<Diagnostic>,
        emitted_traits: &'arena mut HashSet<String>,
    ) -> Self {
        Self {
            arena,
            source,
            file_id,
            derive_span,
            expansion_counter,
            diagnostics,
            emitted_traits,
            extra_items: Vec::new(),
        }
    }


    /// Check if a trait definition has already been emitted.
    pub fn is_trait_emitted(&self, trait_name: &str) -> bool {
        self.emitted_traits.contains(trait_name)
    }

    /// Mark a trait as emitted.
    pub fn mark_trait_emitted(&mut self, trait_name: &str) {
        self.emitted_traits.insert(trait_name.to_string());
    }

    /// Ensure a trait definition is emitted (only emits once).
    pub fn ensure_trait_defined(&mut self, trait_name: &str, trait_def: &str) {
        if self.emitted_traits.insert(trait_name.to_string()) {
            if let Ok(items) = self.parse_and_append_item(trait_def) {
                self.extra_items.extend(items);
            }
        }
    }

    /// Get the text content of a span.
    #[allow(dead_code)]
    pub fn get_span_text(&self, span: Span) -> String {
        if (span.end as usize) <= self.source.len() && span.start <= span.end {
            self.source[span.start as usize..span.end as usize].to_string()
        } else {
            String::new()
        }
    }

    /// Generate a unique identifier for generated code.
    /// Uses a semantic expansion counter rather than string-based gensym.
    pub fn fresh_ident(&mut self, prefix: &str) -> String {
        *self.expansion_counter += 1;
        format!("{}{}", prefix, *self.expansion_counter)
    }

    /// Report a diagnostic error.
    pub fn error(&mut self, message: String, span: Option<Span>) {
        let diag = match span {
            Some(s) => Diagnostic::error(message).with_span(s),
            None => Diagnostic::error(message).with_span(self.derive_span),
        };
        self.diagnostics.push(diag);
    }

    /// Append a parsed item to the arena and return it.
    pub fn parse_and_append_item(&mut self, code: &str) -> Result<Vec<Item>, ()> {
        let offset = self.source.len() as u32;
        self.source.push('\n');
        self.source.push_str(code);

        let mut temp_arena = AstArena::new();
        let lexer = Lexer::new(code, self.file_id);
        let mut parser = Parser::new(lexer, &mut temp_arena, self.file_id);

        match parser.parse_file() {
            Ok(items) => {
                let relocator = mellis_ast::relocator::AstRelocator::new(
                    self.arena.exprs.len() as u32,
                    self.arena.stmts.len() as u32,
                    self.arena.decls.len() as u32,
                    self.arena.types.len() as u32,
                    self.arena.pats.len() as u32,
                    self.file_id,
                    offset + 1,
                );
                relocator.relocate_arena(&mut temp_arena);

                let mut result = Vec::new();
                for item in items {
                    match item {
                        Item::Decl(d) => result.push(Item::Decl(relocator.shift_decl_id(d))),
                        Item::Stmt(s) => result.push(Item::Stmt(relocator.shift_stmt_id(s))),
                    }
                }

                // Merge temp arena into main arena
                self.arena.exprs.extend(temp_arena.exprs);
                self.arena.stmts.extend(temp_arena.stmts);
                self.arena.decls.extend(temp_arena.decls);
                self.arena.types.extend(temp_arena.types);
                self.arena.pats.extend(temp_arena.pats);

                Ok(result)
            }
            Err(()) => {
                self.diagnostics.extend(parser.diagnostics);
                Err(())
            }
        }
    }

    /// Parse and emit a single impl block.
    /// Returns the decl_id of the generated impl if successful.
    pub fn emit_impl(&mut self, impl_code: &str) -> Result<(), ()> {
        self.parse_and_append_item(impl_code).map(|_| ())
    }
}

/// Input to a derive macro: the type being derived.
#[derive(Debug, Clone)]
pub struct DeriveInput {
    /// The kind of declaration (struct or enum)
    pub kind: DeriveKind,
    /// Name of the type
    pub name: String,
    /// Original span of the type name
    pub name_span: Span,
    /// Generic parameters (if any)
    pub generic_params: Vec<GenericParamInfo>,
    /// For structs: the fields
    pub fields: Vec<FieldInfo>,
    /// For enums: the variants
    pub variants: Vec<VariantInfo>,
    /// Annotations on the original type
    pub annotations: Vec<(String, Span)>,
}

#[derive(Debug, Clone)]
pub enum DeriveKind {
    Struct,
    Enum,
}

#[derive(Debug, Clone)]
pub struct GenericParamInfo {
    pub name: String,
    pub span: Span,
    pub bounds: Vec<String>, // Trait bounds as strings
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub span: Span,
    pub ty: String, // Type as string representation
}

#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    pub span: Span,
    pub fields: Vec<FieldInfo>,
}

/// Registry of derive macros.
///
/// Derive macros are registered by name and can be invoked for any struct or enum
/// that has `#[derive(MacroName, ...)]`.
pub struct DeriveRegistry {
    macros: HashMap<String, DeriveMacro>,
}

impl Default for DeriveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeriveRegistry {
    pub fn new() -> Self {
        Self {
            macros: HashMap::new(),
        }
    }

    /// Register a derive macro.
    pub fn register(&mut self, name: &str, macro_fn: DeriveMacro) {
        self.macros.insert(name.to_string(), macro_fn);
    }

    /// Check if a derive macro is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// Get a registered derive macro.
    pub fn get(&self, name: &str) -> Option<&DeriveMacro> {
        self.macros.get(name)
    }

    /// Get all registered macro names.
    pub fn registered_names(&self) -> Vec<String> {
        self.macros.keys().cloned().collect()
    }

    /// Create a registry pre-populated with built-in standard library derives.
    pub fn with_std_derives() -> Self {
        let mut registry = Self::new();
        registry.register_builtin_derives();
        registry
    }

    /// Register the standard library derive macros.
    /// These implementations are part of the standard library, not the compiler itself.
    fn register_builtin_derives(&mut self) {
        // Register Debug derive
        let debug = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            // First, emit the trait definition if not already emitted
            ctx.ensure_trait_defined(
                "Debug",
                "trait Debug { fn fmt(self: &Self) -> void; }"
            );

            // Generate Debug trait impl - use concrete type instead of Self
            let code = format!(
                "impl Debug for {} {{ fn fmt(self: &{}) -> void {{}} }}",
                type_name, type_name
            );

            ctx.parse_and_append_item(&code)
                .map_err(|_| Diagnostic::error(format!("failed to parse generated Debug for `{}`", type_name)).with_span(ctx.derive_span))
        });
        self.register("Debug", debug);

        // Register Clone derive
        let clone = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            // First, emit the trait definition if not already emitted
            ctx.ensure_trait_defined(
                "Clone",
                "trait Clone { fn clone(self: &Self) -> Self; }"
            );

            let code = match &input.kind {
                DeriveKind::Struct => {
                    if input.fields.is_empty() {
                        format!(
                            "impl Clone for {} {{ fn clone(self: &{}) -> {} {{ return {} {{}}; }} }}",
                            type_name, type_name, type_name, type_name
                        )
                    } else {
                        let field_copies: Vec<String> = input.fields.iter()
                            .map(|f| format!("{}: self.{}.clone()", f.name, f.name))
                            .collect();
                        format!(
                            "impl Clone for {} {{ fn clone(self: &{}) -> {} {{ return {} {{ {} }}; }} }}",
                            type_name,
                            type_name,
                            type_name,
                            type_name,
                            field_copies.join(", ")
                        )
                    }
                }
                DeriveKind::Enum => {
                    let mut match_arms = Vec::new();
                    for variant in &input.variants {
                        if variant.fields.is_empty() {
                            match_arms.push(format!("{}::{} -> {{ {}::{} }}", type_name, variant.name, type_name, variant.name));
                        } else {
                            let binders: Vec<String> = variant.fields.iter().enumerate().map(|(i, _)| format!("v{}", i)).collect();
                            let cloners: Vec<String> = binders.iter().map(|b| format!("{}.clone()", b)).collect();
                            // In Mellis, enum variants with payload are written like Variant(v0, v1)
                            match_arms.push(format!("{}::{}({}) -> {{ {}::{}({}) }}", 
                                type_name, variant.name, binders.join(", "),
                                type_name, variant.name, cloners.join(", ")));
                        }
                    }
                    format!(
                        "impl Clone for {} {{ fn clone(self: &{}) -> {} {{ match *self {{ {} }} }} }}",
                        type_name, type_name, type_name, match_arms.join(" ")
                    )
                }
            };


            ctx.parse_and_append_item(&code)
                .map_err(|_| Diagnostic::error(format!("failed to parse generated Clone for `{}`", type_name)).with_span(ctx.derive_span))
        });
        self.register("Clone", clone);

        // Register PartialEq derive
        let partial_eq = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            // First, emit the trait definition if not already emitted
            ctx.ensure_trait_defined(
                "PartialEq",
                "trait PartialEq { fn eq(self: &Self, other: &Self) -> bool; }"
            );

            let code = match &input.kind {
                DeriveKind::Struct => {
                    if input.fields.is_empty() {
                        format!(
                            "impl PartialEq for {} {{ fn eq(self: &{}, other: &{}) -> bool {{ true }} }}",
                            type_name, type_name, type_name
                        )
                    } else {
                        let comparisons: Vec<String> = input.fields.iter()
                            .map(|f| format!("self.{}.eq(&other.{})", f.name, f.name))
                            .collect();
                        let cond = comparisons.join(" && ");
                        format!(
                            "impl PartialEq for {} {{ fn eq(self: &{}, other: &{}) -> bool {{ {} }} }}",
                            type_name, type_name, type_name, cond
                        )
                    }
                }
                DeriveKind::Enum => {
                    let mut match_arms = Vec::new();
                    for variant in &input.variants {
                        if variant.fields.is_empty() {
                            match_arms.push(format!("{}::{} -> {{ match *other {{ {}::{} -> {{ true }} _ -> {{ false }} }} }}", 
                                type_name, variant.name, type_name, variant.name));
                        } else {
                            let self_binders: Vec<String> = variant.fields.iter().enumerate().map(|(i, _)| format!("s{}", i)).collect();
                            let other_binders: Vec<String> = variant.fields.iter().enumerate().map(|(i, _)| format!("o{}", i)).collect();
                            let comparisons: Vec<String> = self_binders.iter().zip(other_binders.iter())
                                .map(|(s, o)| format!("{}.eq(&{})", s, o))
                                .collect();
                            let cond = comparisons.join(" && ");
                            match_arms.push(format!("{}::{}({}) -> {{ match *other {{ {}::{}({}) -> {{ {} }} _ -> {{ false }} }} }}", 
                                type_name, variant.name, self_binders.join(", "),
                                type_name, variant.name, other_binders.join(", "),
                                cond));
                        }
                    }
                    format!(
                        "impl PartialEq for {} {{ fn eq(self: &{}, other: &{}) -> bool {{ match *self {{ {} _ -> {{ false }} }} }} }}",
                        type_name, type_name, type_name, match_arms.join(" ")
                    )
                }
            };

            ctx.parse_and_append_item(&code)
                .map_err(|_| Diagnostic::error(format!("failed to parse generated PartialEq for `{}`", type_name)).with_span(ctx.derive_span))
        });
        self.register("PartialEq", partial_eq);

        // Register Eq derive
        let eq = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            // First, emit the trait definition if not already emitted
            ctx.ensure_trait_defined(
                "Eq",
                "trait Eq {}"
            );

            let code = format!("impl Eq for {} {{}}", type_name);
            ctx.parse_and_append_item(&code)
                .map_err(|_| Diagnostic::error(format!("failed to parse generated Eq for `{}`", type_name)).with_span(ctx.derive_span))
        });
        self.register("Eq", eq);

        // Register Default derive
        let default = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            // First, emit the trait definition if not already emitted
            ctx.ensure_trait_defined(
                "Default",
                "trait Default { fn default() -> Self; }"
            );

            let code = match &input.kind {
                DeriveKind::Struct => {
                    if input.fields.is_empty() {
                        format!(
                            "impl Default for {} {{ fn default() -> {} {{ return {} {{}}; }} }}",
                            type_name, type_name, type_name
                        )
                    } else {
                        // Assume fields implement Default and use Default::default()
                        // Mellis might not have trait resolution for `Default::default()` on arbitrary types yet without type annotations,
                        // Wait, Default::default() requires a type parameter if the compiler can't infer it, or we can use the field type!
                        // Let's use `Default::default()` and hope inference works, or we must extract the field type.
                        // For v1, the previous stub just used `0`. We should generate `Default::default()`.
                        let field_inits: Vec<String> = input.fields.iter()
                            .map(|f| format!("{}: {}", f.name, default_value_for_type(&f.ty)))
                            .collect();
                        format!(
                            "impl Default for {} {{ fn default() -> {} {{ return {} {{ {} }}; }} }}",
                            type_name,
                            type_name,
                            type_name,
                            field_inits.join(", ")
                        )
                    }
                }
                DeriveKind::Enum => {
                    if let Some(first_variant) = input.variants.first() {
                        if first_variant.fields.is_empty() {
                            format!(
                                "impl Default for {} {{ fn default() -> {} {{ return {}::{}; }} }}",
                                type_name, type_name, type_name, first_variant.name
                            )
                        } else {
                            let field_defaults: Vec<String> = first_variant.fields.iter()
                                .map(|f| default_value_for_type(&f.ty))
                                .collect();
                            format!(
                                "impl Default for {} {{ fn default() -> {} {{ return {}::{}({}); }} }}",
                                type_name,
                                type_name,
                                type_name,
                                first_variant.name,
                                field_defaults.join(", ")
                            )
                        }
                    } else {
                        ctx.error(
                            format!("cannot derive Default for empty enum `{}`", type_name),
                            Some(input.name_span)
                        );
                        return Err(Diagnostic::error(format!("cannot derive Default for empty enum `{}`", type_name)));
                    }
                }
            };

            ctx.parse_and_append_item(&code)
                .map_err(|_| Diagnostic::error(format!("failed to parse generated Default for `{}`", type_name)).with_span(ctx.derive_span))
        });
        self.register("Default", default);
    }
}

fn default_value_for_type(ty: &str) -> String {
    match ty {
        "bool" => "false".to_string(),
        "i8" | "i16" | "i32" | "i64" | "isize" | "int" => "0".to_string(),
        "u8" | "u16" | "u32" | "u64" | "usize" | "uint" => "0".to_string(),
        "f32" | "f64" | "float" => "0.0".to_string(),
        "char" => "'\\0'".to_string(),
        "void" => "{}".to_string(),
        _ => format!("{}::default()", ty),
    }
}

fn format_type(arena: &AstArena, source: &str, ty_id: TypeId) -> String {
    let ty = &arena.types[ty_id.0 as usize];
    match ty {
        mellis_ast::Type::Builtin(bk) => format!("{:?}", bk).to_lowercase(),
        mellis_ast::Type::Named { segments, .. } => {
            segments.iter().map(|s| get_span_text(source, *s)).collect::<Vec<_>>().join("::")
        }
        mellis_ast::Type::Reference { inner, .. } => {
            format!("&{}", format_type(arena, source, *inner))
        }
        _ => "Unknown".to_string() // Fallback
    }
}

/// Extract DeriveInput from a struct declaration.
pub fn extract_struct_input(arena: &AstArena, source: &str, decl: &Decl) -> Option<DeriveInput> {
    match decl {
        Decl::Struct { name, fields, generic_params, annotations, .. } => {
            let name_str = get_span_text(source, *name);

            let gen_params = generic_params.iter().map(|p| {
                GenericParamInfo {
                    name: get_span_text(source, p.name),
                    span: p.name,
                    bounds: Vec::new(),
                }
            }).collect();

            let field_infos: Vec<FieldInfo> = fields.iter().map(|f| {
                FieldInfo {
                    name: get_span_text(source, f.name),
                    span: f.name,
                    ty: format_type(arena, source, f.ty),
                }
            }).collect();

            let annots: Vec<(String, Span)> = annotations.iter().map(|a| {
                (get_span_text(source, a.name), a.name)
            }).collect();

            Some(DeriveInput {
                kind: DeriveKind::Struct,
                name: name_str,
                name_span: *name,
                generic_params: gen_params,
                fields: field_infos,
                variants: Vec::new(),
                annotations: annots,
            })
        }
        _ => None,
    }
}

/// Extract DeriveInput from an enum declaration.
pub fn extract_enum_input(arena: &AstArena, source: &str, decl: &Decl) -> Option<DeriveInput> {
    match decl {
        Decl::Enum { name, variants, generic_params, annotations, .. } => {
            let name_str = get_span_text(source, *name);

            let gen_params = generic_params.iter().map(|p| {
                GenericParamInfo {
                    name: get_span_text(source, p.name),
                    span: p.name,
                    bounds: Vec::new(),
                }
            }).collect();

            let variant_infos: Vec<VariantInfo> = variants.iter().map(|v| {
                let field_infos: Vec<FieldInfo> = v.fields.iter().map(|f| {
                    if let Decl::Param { name: p_name, ty, .. } = &arena.decls[f.0 as usize] {
                        FieldInfo {
                            name: get_span_text(source, *p_name),
                            span: *p_name,
                            ty: ty.map(|t| format_type(arena, source, t)).unwrap_or("Unknown".to_string()),
                        }
                    } else {
                        FieldInfo {
                            name: "".to_string(),
                            span: v.name,
                            ty: "unknown".to_string(),
                        }
                    }
                }).collect();
                VariantInfo {
                    name: get_span_text(source, v.name),
                    span: v.name,
                    fields: field_infos,
                }
            }).collect();

            let annots: Vec<(String, Span)> = annotations.iter().map(|a| {
                (get_span_text(source, a.name), a.name)
            }).collect();

            Some(DeriveInput {
                kind: DeriveKind::Enum,
                name: name_str,
                name_span: *name,
                generic_params: gen_params,
                fields: Vec::new(),
                variants: variant_infos,
                annotations: annots,
            })
        }
        _ => None,
    }
}

fn get_span_text(source: &str, span: Span) -> String {
    if (span.end as usize) <= source.len() && span.start <= span.end {
        source[span.start as usize..span.end as usize].to_string()
    } else {
        String::new()
    }
}
