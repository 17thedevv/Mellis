//! Attribute processing for the Mellis compiler.
//!
//! This module handles:
//! - `#[derive(...)]` - attribute-driven derive macro expansion
//! - `#[repr(...)]` - compiler-defined representation hints
//! - `#[test]` - compiler-defined test marker
//! - `#[inline]`, `#[no_mangle]`, `#[link(...)]` - compiler ABI/codegen hints
//! - Other attributes are validated and passed through
//!
//! The derive system uses a registry-based approach where derive macros are
//! registered by name and invoked generically. See `derive.rs` for details.

use std::collections::HashSet;
use std::sync::Arc;
use luna_ast::{AstArena, Decl, DeclId, Expr, Item};
use luna_common::diagnostic::Diagnostic;
use luna_common::ids::{FileId, Span};

use crate::derive::{DeriveInput, DeriveRegistry, DeriveContext, extract_struct_input, extract_enum_input};

/// Compiler-defined attributes that affect codegen/ABI but don't use the derive system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerAttr {
    Repr,
    Test,
    Inline,
    NoMangle,
    Link,
    Lang,
    SyncNoescape,
}

/// The set of known compiler-defined attribute names.
const COMPILER_ATTRS: &[&str] = &["repr", "test", "inline", "no_mangle", "link", "lang", "sync_noescape"];

/// Attribute processor that handles both compiler-defined attributes and derive expansion.
///
/// This processor maintains separation between:
/// - Compiler-defined attributes (repr, test, inline, no_mangle, link) that are validated here
/// - User-defined derives that are dispatched to the DeriveRegistry
pub struct AttributeProcessor<'a> {
    arena: &'a mut AstArena,
    source_manager: &'a mut luna_common::source::SourceManager,
    file_id: FileId,
    diagnostics: Vec<Diagnostic>,
    /// Registry of available derive macros
    derive_registry: DeriveRegistry,
    /// Tracks which traits have been emitted to prevent duplicates
    emitted_traits: HashSet<String>,
    /// Counter for expansion hygiene
    expansion_counter: u32,
}

impl<'a> AttributeProcessor<'a> {
    /// Create a new AttributeProcessor with standard library derives pre-registered.
    pub fn new(arena: &'a mut AstArena, source_manager: &'a mut luna_common::source::SourceManager, file_id: FileId) -> Self {
        Self {
            arena,
            source_manager,
            file_id,
            diagnostics: Vec::new(),
            derive_registry: DeriveRegistry::with_std_derives(),
            emitted_traits: HashSet::new(),
            expansion_counter: 0,
        }
    }

    /// Create a new AttributeProcessor with a custom derive registry.
    ///
    /// This allows tests or external tools to provide their own derive implementations.
    pub fn with_registry(
        arena: &'a mut AstArena,
        source_manager: &'a mut luna_common::source::SourceManager,
        file_id: FileId,
        registry: DeriveRegistry,
    ) -> Self {
        Self {
            arena,
            source_manager,
            file_id,
            diagnostics: Vec::new(),
            derive_registry: registry,
            emitted_traits: HashSet::new(),
            expansion_counter: 0,
        }
    }

    /// Access the underlying derive registry (for testing or external registration).
    pub fn derive_registry_mut(&mut self) -> &mut DeriveRegistry {
        &mut self.derive_registry
    }

    /// Access the underlying derive registry (immutable reference).
    pub fn derive_registry(&self) -> &DeriveRegistry {
        &self.derive_registry
    }

    /// Process all items, expanding derives and validating compiler attributes.
    pub fn process_items(&mut self, items: Vec<Item>) -> Result<Vec<Item>, Vec<Diagnostic>> {
        let mut generated_items = Vec::new();
        let mut out_items = Vec::new();

        for item in &items {
            out_items.push(item.clone());
            if let Item::Decl(decl_id) = item {
                let decl = self.arena.decls[decl_id.0 as usize].clone();
                let extra = self.process_decl(*decl_id, &decl);
                generated_items.extend(extra);
            }
        }

        out_items.extend(generated_items);

        if self.diagnostics.is_empty() {
            Ok(out_items)
        } else {
            Err(self.diagnostics.clone())
        }
    }

    /// Get text from a span.
    fn get_span_text(&self, span: Span) -> &str {
        if (span.end as usize) <= self.source_manager.get_file(span.file_id).unwrap().source.len() && span.start <= span.end {
            &self.source_manager.get_file(span.file_id).unwrap().source[span.start as usize..span.end as usize]
        } else {
            ""
        }
    }

    /// Process a declaration's annotations.
    fn process_decl(&mut self, decl_id: DeclId, decl: &Decl) -> Vec<Item> {
        let mut generated = Vec::new();

        // Extract annotations from any decl variant
        let annotations = match decl {
            Decl::Struct { annotations, .. } => annotations,
            Decl::Enum { annotations, .. } => annotations,
            Decl::Function { annotations, .. } => annotations,
            Decl::Var { annotations, .. } => annotations,
            Decl::Trait { annotations, .. } => annotations,
            Decl::Impl { annotations, .. } => annotations,
            Decl::Module { annotations, .. } => annotations,
            Decl::Extern { annotations, .. } => annotations,
            Decl::TypeAlias { annotations, .. } => annotations,
            Decl::Macro { annotations, .. } => annotations,
            _ => return generated,
        };

        for annot in annotations {
            let attr_name = self.get_span_text(annot.name);

            match attr_name {
                "derive" => {
                    // Derive is handled by the generic derive system
                    let extra = self.handle_derive(decl_id, decl, annot);
                    generated.extend(extra);
                }
                "repr" => {
                    self.handle_repr(decl, annot);
                }
                "test" => {
                    self.handle_test(decl, annot);
                }
                "inline" | "no_mangle" | "link" | "lang" | "sync_noescape" => {
                    // Valid compiler directives - validated but not processed
                    self.validate_compiler_attr(decl, annot);
                }
                _ => {
                    // Unknown attribute - report error
                    // Note: In a more extensible system, this could be configurable
                    // to allow user-defined attributes that are processed elsewhere.
                    // For now, we report unknown attributes as errors.
                    self.diagnostics.push(
                        Diagnostic::error(format!("unknown attribute `{}`", attr_name))
                            .with_span(annot.name),
                    );
                }
            }
        }

        // If Enum, also validate variant annotations
        if let Decl::Enum { variants, .. } = decl {
            for variant in variants {
                for annot in &variant.annotations {
                    let attr_name = self.get_span_text(annot.name);
                    if attr_name == "lang" {
                        // valid on variant in compiler/core contexts
                    } else if !COMPILER_ATTRS.contains(&attr_name) && attr_name != "derive" {
                        self.diagnostics.push(
                            Diagnostic::error(format!("unknown attribute `{}`", attr_name))
                                .with_span(annot.name),
                        );
                    } else {
                        self.diagnostics.push(
                            Diagnostic::error(format!("`#[{}]` cannot be applied to enum variants", attr_name))
                                .with_span(annot.name),
                        );
                    }
                }
            }
        }

        // If Function, also validate parameter annotations
        if let Decl::Function { params, .. } = decl {
            for param_id in params {
                if let Decl::Param { annotations, .. } = &self.arena.decls[param_id.0 as usize] {
                    for annot in annotations {
                        let attr_name = self.get_span_text(annot.name);
                        if attr_name == "sync_noescape" {
                            // valid on parameter
                        } else if !COMPILER_ATTRS.contains(&attr_name) && attr_name != "derive" {
                            self.diagnostics.push(
                                Diagnostic::error(format!("unknown attribute `{}`", attr_name))
                                    .with_span(annot.name),
                            );
                        } else {
                            self.diagnostics.push(
                                Diagnostic::error(format!("`#[{}]` cannot be applied to parameters", attr_name))
                                    .with_span(annot.name),
                            );
                        }
                    }
                }
            }
        }

        // If Extern, also validate parameter annotations of extern func
        if let Decl::Extern { func, .. } = decl {
            if let Decl::Function { params, .. } = &self.arena.decls[func.0 as usize] {
                for param_id in params {
                    if let Decl::Param { annotations, .. } = &self.arena.decls[param_id.0 as usize] {
                        for annot in annotations {
                            let attr_name = self.get_span_text(annot.name);
                            if attr_name == "sync_noescape" {
                                // valid on parameter
                            } else if !COMPILER_ATTRS.contains(&attr_name) && attr_name != "derive" {
                                self.diagnostics.push(
                                    Diagnostic::error(format!("unknown attribute `{}`", attr_name))
                                        .with_span(annot.name),
                                );
                            } else {
                                self.diagnostics.push(
                                    Diagnostic::error(format!("`#[{}]` cannot be applied to parameters", attr_name))
                                        .with_span(annot.name),
                                );
                            }
                        }
                    }
                }
            }
        }

        generated
    }

    /// Validate that a compiler attribute is applied to an appropriate declaration.
    fn validate_compiler_attr(&mut self, decl: &Decl, annot: &luna_ast::Annotation) {
        let attr_name = self.get_span_text(annot.name);

        match attr_name {
            "repr" => {
                match decl {
                    Decl::Struct { .. } | Decl::Enum { .. } => {}
                    _ => {
                        self.diagnostics.push(
                            Diagnostic::error("`#[repr]` can only be applied to structs or enums")
                                .with_span(annot.name),
                        );
                    }
                }
            }
            "test" => {
                match decl {
                    Decl::Function { .. } => {}
                    _ => {
                        self.diagnostics.push(
                            Diagnostic::error("`#[test]` can only be applied to functions")
                                .with_span(annot.name),
                        );
                    }
                }
            }
            "inline" => {
                match decl {
                    Decl::Function { .. } => {}
                    _ => {
                        self.diagnostics.push(
                            Diagnostic::error("`#[inline]` can only be applied to functions")
                                .with_span(annot.name),
                        );
                    }
                }
            }
            "no_mangle" => {
                match decl {
                    Decl::Function { .. } => {}
                    _ => {
                        self.diagnostics.push(
                            Diagnostic::error("`#[no_mangle]` can only be applied to functions")
                                .with_span(annot.name),
                        );
                    }
                }
            }
            "link" => {
                // link can be applied to functions (for FFI) or extern blocks
                match decl {
                    Decl::Function { .. } | Decl::Extern { .. } => {}
                    _ => {
                        self.diagnostics.push(
                            Diagnostic::error("`#[link]` can only be applied to functions or extern blocks")
                                .with_span(annot.name),
                        );
                    }
                }
            }
            "lang" => {
                // lang can be applied to various items (traits, functions, structs, etc)
                // in trusted compiler/core contexts. Handled in Resolver.
            }
            "sync_noescape" => {
                self.diagnostics.push(
                    Diagnostic::error("`#[sync_noescape]` can only be applied to parameters")
                        .with_span(annot.name),
                );
            }
            _ => {}
        }
    }

    /// Handle #[repr(...)] attribute - compiler-defined, validation only.
    fn handle_repr(&mut self, decl: &Decl, annot: &luna_ast::Annotation) {
        match decl {
            Decl::Struct { .. } | Decl::Enum { .. } => {}
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("`#[repr]` can only be applied to structs or enums")
                        .with_span(annot.name),
                );
                return;
            }
        }

        for arg in &annot.args {
            let arg_text = self.get_expr_identifier_text(arg.value);
            match arg_text.as_str() {
                "C" | "transparent" | "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" => {
                    // Valid repr
                }
                _ => {
                    let span = self.get_expr_span(arg.value).unwrap_or(annot.name);
                    self.diagnostics.push(
                        Diagnostic::error(format!("invalid repr argument `{}`", arg_text))
                            .with_span(span),
                    );
                }
            }
        }
    }

    /// Handle #[test] attribute - compiler-defined, validation only.
    fn handle_test(&mut self, decl: &Decl, annot: &luna_ast::Annotation) {
        match decl {
            Decl::Function { .. } => {}
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("`#[test]` can only be applied to functions")
                        .with_span(annot.name),
                );
            }
        }
    }

    /// Handle #[derive(...)] attribute using the generic derive system.
    ///
    /// This dispatches to the DeriveRegistry rather than hardcoding derive implementations.
    fn handle_derive(&mut self, _decl_id: DeclId, decl: &Decl, annot: &luna_ast::Annotation) -> Vec<Item> {
        // Only structs and enums can derive
        let derive_input = match decl {
            Decl::Struct { .. } => extract_struct_input(self.arena, &self.source_manager.get_file(self.file_id).unwrap().source, decl),
            Decl::Enum { .. } => extract_enum_input(self.arena, &self.source_manager.get_file(self.file_id).unwrap().source, decl),
            _ => {
                self.diagnostics.push(
                    Diagnostic::error("`#[derive]` can only be applied to structs or enums")
                        .with_span(annot.name),
                );
                return Vec::new();
            }
        };

        let Some(input) = derive_input else {
            return Vec::new();
        };

        let mut generated = Vec::new();

        for arg in &annot.args {
            let trait_name = self.get_expr_identifier_text(arg.value);

            // Check if the derive macro is registered
            if !self.derive_registry.is_registered(&trait_name) {
                let span = self.get_expr_span(arg.value).unwrap_or(annot.name);
                let available = self.derive_registry.registered_names();
                let msg = if available.is_empty() {
                    format!("unknown derive trait `{}`", trait_name)
                } else {
                    format!(
                        "unknown derive trait `{}`. Available derives: {}",
                        trait_name,
                        available.join(", ")
                    )
                };
                self.diagnostics.push(Diagnostic::error(msg).with_span(span));
                continue;
            }

            // Get the derive macro from the registry
            let Some(derive_fn) = self.derive_registry.get(&trait_name) else {
                continue;
            };

            // Create the derive context
            let mut ctx = DeriveContext::new(
                self.arena,
                self.source_manager,
                self.file_id,
                annot.name,
                &mut self.expansion_counter,
                &mut self.diagnostics,
                &mut self.emitted_traits,
            );

            // Invoke the derive macro
            match derive_fn(&mut ctx, &input) {
                Ok(items) => {
                    generated.extend(ctx.extra_items);
                    generated.extend(items);
                }
                Err(diag) => {
                    self.diagnostics.push(diag);
                }
            }
        }

        generated
    }

    /// Extract identifier text from an expression (for attribute arguments).
    fn get_expr_identifier_text(&self, expr_id: luna_ast::ExprId) -> String {
        match &self.arena.exprs[expr_id.0 as usize] {
            Expr::Identifier { segments, .. } => {
                if let Some(first) = segments.first() {
                    self.get_span_text(*first).to_string()
                } else {
                    String::new()
                }
            }
            Expr::Literal(tok, _) => {
                self.get_span_text(tok.span).to_string()
            }
            _ => String::new(),
        }
    }

    /// Get the span for an expression.
    fn get_expr_span(&self, expr_id: luna_ast::ExprId) -> Option<Span> {
        match &self.arena.exprs[expr_id.0 as usize] {
            Expr::Identifier { segments, .. } => segments.first().copied(),
            Expr::Literal(tok, _) => Some(tok.span),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_registry_has_std_derives() {
        let registry = DeriveRegistry::with_std_derives();

        assert!(registry.is_registered("Debug"));
        assert!(registry.is_registered("Clone"));
        assert!(registry.is_registered("PartialEq"));
        assert!(registry.is_registered("Eq"));
        assert!(registry.is_registered("Default"));
    }

    #[test]
    fn test_compiler_attrs_are_separate() {
        // Compiler attributes should not be in the derive registry
        let registry = DeriveRegistry::with_std_derives();

        assert!(!registry.is_registered("repr"));
        assert!(!registry.is_registered("test"));
        assert!(!registry.is_registered("inline"));
        assert!(!registry.is_registered("no_mangle"));
        assert!(!registry.is_registered("link"));
    }

    #[test]
    fn test_can_register_custom_derive() {
        let mut registry = DeriveRegistry::new();

        assert!(!registry.is_registered("CustomTrait"));

        registry.register("CustomTrait", Arc::new(|_ctx, _input| {
            Ok(Vec::new())
        }));

        assert!(registry.is_registered("CustomTrait"));
    }
}
