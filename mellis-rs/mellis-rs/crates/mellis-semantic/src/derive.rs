//! Derive macro registry and expansion system.
//!
//! This module implements a generic derive/attribute macro mechanism following
//! Rust's proc-macro model:
//!
//! - `#[derive(Trait1, Trait2)]` on structs/enums invokes registered derive macros
//! - Each derive macro receives a DeriveContext with type information
//! - Macros return generated AST items (typically impl blocks)
//! - Generated items feed back into the normal compilation pipeline

use std::collections::HashMap;
use std::sync::Arc;

use mellis_ast::{AstArena, Decl, DeclId, Item, StructField, TypeId};
use mellis_common::diagnostic::Diagnostic;
use mellis_common::ids::{FileId, Span};
use mellis_lexer::Token;
use mellis_parser::Parser;
use mellis_lexer::Lexer;

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
    source: &'arena str,
    /// The file ID for diagnostics
    file_id: FileId,
    /// Span of the original derive attribute (for error reporting)
    pub derive_span: Span,
    /// Expansion counter for generating unique identifiers
    expansion_counter: &'arena mut u32,
    /// Diagnostics accumulated during expansion
    diagnostics: &'arena mut Vec<Diagnostic>,
}

impl<'arena> DeriveContext<'arena> {
    pub fn new(
        arena: &'arena mut AstArena,
        source: &'arena str,
        file_id: FileId,
        derive_span: Span,
        expansion_counter: &'arena mut u32,
        diagnostics: &'arena mut Vec<Diagnostic>,
    ) -> Self {
        Self {
            arena,
            source,
            file_id,
            derive_span,
            expansion_counter,
            diagnostics,
        }
    }

    /// Get the text content of a span.
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

            // Generate Debug trait impl
            let code = format!(
                "impl Debug for {} {{ fn fmt(self: &Self) -> void {{}} }}",
                type_name
            );

            // Note: In a fully AST-based system, we'd construct the impl directly.
            // For now, we use the parse_and_append mechanism as a bridge.
            let _ = ctx.parse_and_append_item(&code);

            Ok(Vec::new())
        });
        self.register("Debug", debug);

        // Register Clone derive
        let clone = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            let code = match &input.kind {
                DeriveKind::Struct => {
                    if input.fields.is_empty() {
                        format!(
                            "impl Clone for {} {{ fn clone(self: &Self) -> Self {{ return {} {{}}; }} }}",
                            type_name, type_name
                        )
                    } else {
                        let field_copies: Vec<String> = input.fields.iter()
                            .map(|f| format!("self.{}", f.name))
                            .collect();
                        format!(
                            "impl Clone for {} {{ fn clone(self: &Self) -> Self {{ return {} {{ {} }}; }} }}",
                            type_name,
                            type_name,
                            field_copies.join(", ")
                        )
                    }
                }
                DeriveKind::Enum => {
                    format!(
                        "impl Clone for {} {{ fn clone(self: &Self) -> Self {{ return *self; }} }}",
                        type_name
                    )
                }
            };

            let _ = ctx.parse_and_append_item(&code);
            Ok(Vec::new())
        });
        self.register("Clone", clone);

        // Register PartialEq derive
        let partial_eq = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            let code = match &input.kind {
                DeriveKind::Struct => {
                    if input.fields.is_empty() {
                        format!(
                            "impl PartialEq for {} {{ fn eq(self: &Self, other: &Self) -> bool {{ return true; }} }}",
                            type_name
                        )
                    } else {
                        let comparisons: Vec<String> = input.fields.iter()
                            .map(|f| format!("self.{} == other.{}", f.name, f.name))
                            .collect();
                        let cond = comparisons.join(" && ");
                        format!(
                            "impl PartialEq for {} {{ fn eq(self: &Self, other: &Self) -> bool {{ return {}; }} }}",
                            type_name, cond
                        )
                    }
                }
                DeriveKind::Enum => {
                    // For enums, generate a proper match-based comparison
                    // For now, fall back to true as a placeholder for proper enum comparison
                    format!(
                        "impl PartialEq for {} {{ fn eq(self: &Self, other: &Self) -> bool {{ return true; }} }}",
                        type_name
                    )
                }
            };

            let _ = ctx.parse_and_append_item(&code);
            Ok(Vec::new())
        });
        self.register("PartialEq", partial_eq);

        // Register Eq derive
        let eq = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            let code = format!("impl Eq for {} {{}}", type_name);
            let _ = ctx.parse_and_append_item(&code);
            Ok(Vec::new())
        });
        self.register("Eq", eq);

        // Register Default derive
        let default = Arc::new(|ctx: &mut DeriveContext, input: &DeriveInput| -> Result<Vec<Item>, Diagnostic> {
            let type_name = &input.name;

            let code = match &input.kind {
                DeriveKind::Struct => {
                    if input.fields.is_empty() {
                        format!(
                            "impl Default for {} {{ fn default() -> Self {{ return {} {{}}; }} }}",
                            type_name, type_name
                        )
                    } else {
                        let field_inits: Vec<String> = input.fields.iter()
                            .map(|f| format!("{}: 0", f.name))
                            .collect();
                        format!(
                            "impl Default for {} {{ fn default() -> Self {{ return {} {{ {} }}; }} }}",
                            type_name,
                            type_name,
                            field_inits.join(", ")
                        )
                    }
                }
                DeriveKind::Enum => {
                    // For enums, use the first variant
                    if let Some(first_variant) = input.variants.first() {
                        if first_variant.fields.is_empty() {
                            format!(
                                "impl Default for {} {{ fn default() -> Self {{ return Self::{}; }} }}",
                                type_name, first_variant.name
                            )
                        } else {
                            let field_defaults: Vec<String> = first_variant.fields.iter()
                                .map(|_| "0".to_string())
                                .collect();
                            format!(
                                "impl Default for {} {{ fn default() -> Self {{ return Self::{} {{ {} }}; }} }}",
                                type_name,
                                first_variant.name,
                                field_defaults.join(", ")
                            )
                        }
                    } else {
                        // No variants - error
                        ctx.error(
                            format!("cannot derive Default for empty enum `{}`", type_name),
                            Some(input.name_span)
                        );
                        return Err(Diagnostic::error(format!("cannot derive Default for empty enum `{}`", type_name)));
                    }
                }
            };

            let _ = ctx.parse_and_append_item(&code);
            Ok(Vec::new())
        });
        self.register("Default", default);
    }
}

/// Extract DeriveInput from a struct declaration.
pub fn extract_struct_input(source: &str, decl: &Decl) -> Option<DeriveInput> {
    match decl {
        Decl::Struct { name, fields, generic_params, annotations, .. } => {
            let name_str = get_span_text(source, *name);

            let gen_params = generic_params.iter().map(|p| {
                let bounds: Vec<String> = p.bounds.iter().map(|_| "".to_string()).collect();
                GenericParamInfo {
                    name: get_span_text(source, p.name),
                    span: p.name,
                    bounds,
                }
            }).collect();

            let field_infos: Vec<FieldInfo> = fields.iter().map(|f| {
                FieldInfo {
                    name: get_span_text(source, f.name),
                    span: f.name,
                    ty: format!("Type({})", f.ty.0),
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
                let bounds: Vec<String> = p.bounds.iter().map(|_| "".to_string()).collect();
                GenericParamInfo {
                    name: get_span_text(source, p.name),
                    span: p.name,
                    bounds,
                }
            }).collect();

            let variant_infos: Vec<VariantInfo> = variants.iter().map(|v| {
                let field_infos: Vec<FieldInfo> = v.fields.iter().map(|f| {
                    if let Decl::Param { name: p_name, ty, .. } = &arena.decls[f.0 as usize] {
                        FieldInfo {
                            name: get_span_text(source, *p_name),
                            span: *p_name,
                            ty: format!("Type({})", ty.map(|t| t.0).unwrap_or(0)),
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
