use crate::Parser;
use mellis_ast::{Annotation, AnnotationArg, Decl, DeclId, Item, Visibility, GenericParam, GenericParamKind};
use mellis_lexer::{Token, TokenKind};
use mellis_common::ids::Span;

impl<'a> Parser<'a> {
    pub fn parse_annotations(&mut self) -> Result<Vec<Annotation>, ()> {
        let mut annotations = Vec::new();
        while self.match_token(TokenKind::At) || self.match_token(TokenKind::AtBracket) {
            let is_bracket = self.previous().kind == TokenKind::AtBracket;
            
            if !self.match_token(TokenKind::Identifier) {
                let span = self.peek().span;
                self.error_at_current("Expected annotation name", span);
                return Err(());
            }
            let name = self.previous().span;
            
            let mut args = Vec::new();
            if self.match_token(TokenKind::LParen) {
                if !self.check(TokenKind::RParen) {
                    loop {
                        let (key, value) = if self.check(TokenKind::Identifier) && self.peek_next().kind == TokenKind::Equal {
                            let k = Some(self.advance().span);
                            self.advance(); // consume '='
                            let v = self.parse_expr()?;
                            (k, v)
                        } else {
                            (None, self.parse_expr()?)
                        };
                        args.push(AnnotationArg { key, value });
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after annotation arguments")?;
            }
            
            if is_bracket {
                self.consume(TokenKind::RBracket, "Expected ']' after annotation")?;
            }
            
            annotations.push(Annotation { name, args });
        }
        Ok(annotations)
    }

    pub fn parse_item_impl(&mut self) -> Result<Item, ()> {
        let annotations = self.parse_annotations()?;
        let mut visibility = Visibility::Internal;

        if self.match_token(TokenKind::KwExport) {
            visibility = Visibility::Public;
        }

        let is_extern = self.match_token(TokenKind::KwExtern);

        let is_function = self.check(TokenKind::KwFn)
            || self.check(TokenKind::KwAsync)
            || self.check(TokenKind::KwIntrinsic)
            || (self.check(TokenKind::KwComptime) && self.peek_next().kind == TokenKind::KwFn)
            || (self.check(TokenKind::KwUnsafe) && self.peek_next().kind == TokenKind::KwFn);

        let decl = if self.check(TokenKind::KwDec) || self.check(TokenKind::KwConst) || self.check(TokenKind::KwRw) {
            self.parse_var_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwImport) {
            self.parse_import_decl(visibility, annotations)?
        } else if is_function {
            self.parse_func_decl(visibility, annotations, is_extern)?
        } else if self.match_token(TokenKind::KwStruct) {
            self.parse_struct_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwEnum) {
            self.parse_enum_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwTrait) {
            self.parse_trait_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwImpl) {
            self.parse_impl_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwType) {
            self.parse_type_alias_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwMacro) {
            self.parse_macro_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwModule) {
            self.parse_module_decl(visibility, annotations)?
        } else if self.check(TokenKind::KwUsing) {
            // `using` does not accept visibility or annotations — it's a local alias
            if visibility == Visibility::Public {
                let span = self.peek().span;
                self.error_at_current("`using` aliases cannot be exported", span);
                return Err(());
            }
            self.advance(); // consume `using`
            self.parse_using_decl()?
        } else {
            // Not a decl, parse as statement
            if is_extern || visibility == Visibility::Public {
                let span = self.peek().span;
                self.error_at_current("Modifiers must be attached to a declaration", span);
                return Err(());
            }
            return Ok(Item::Stmt(self.parse_stmt()?));
        };

        let decl = if is_extern {
            // wrap in extern
            self.arena.alloc_decl(Decl::Extern {
                annotations: Vec::new(),
                visibility,
                func: decl,
            })
        } else {
            decl
        };

        Ok(Item::Decl(decl))
    }

    fn parse_import_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let span_start = self.previous().span.start;
        let mut kind = mellis_ast::ImportKind::External;
        let name;

        if self.match_token(TokenKind::LessThan) {
            kind = mellis_ast::ImportKind::External;
            let name_tok = self.consume(TokenKind::Identifier, "Expected external module name")?;
            name = name_tok.span;
            if self.check(TokenKind::ColonColon) {
                let span = self.peek().span;
                self.error_at_current("Import path must be a single logical module name without '::'", span);
                return Err(());
            }
            self.consume(TokenKind::GreaterThan, "Expected '>' after external module name")?;
        } else if self.match_token(TokenKind::StringLiteral) {
            kind = mellis_ast::ImportKind::Local;
            name = self.previous().span;
        } else {
            let span = self.peek().span;
            self.error_at_current("Expected '<' or string literal after 'import'", span);
            return Err(());
        }

        self.consume(TokenKind::Semi, "Expected ';' after import declaration")?;
        let span_end = self.previous().span.end;

        Ok(self.arena.alloc_decl(Decl::Import {
            annotations,
            visibility,
            kind,
            name,
        }))
    }

    fn parse_var_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let is_const = self.match_token(TokenKind::KwConst);
        let is_mutable = if is_const {
            false
        } else {
            self.consume(TokenKind::KwDec, "Expected 'dec' or 'const'")?;
            self.match_token(TokenKind::KwRw)
        };
        let pattern = Some(self.parse_pattern()?);

        let name = self.previous().span; // Fallback

        let type_annot = if self.match_token(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let initializer = if self.match_token(TokenKind::Equal) {
            Some(self.parse_expression(true)?)
        } else {
            None
        };

        self.consume(TokenKind::Semi, "Expected ';' after variable declaration")?;
        Ok(self.arena.alloc_decl(Decl::Var {
            annotations,
            visibility,
            name,
            pattern,
            type_annot,
            initializer,
            is_mutable,
            is_const,
        }))
    }

    fn parse_func_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
        _allow_empty: bool,
    ) -> Result<DeclId, ()> {
        let is_comptime = self.match_token(TokenKind::KwComptime);
        let is_async = self.match_token(TokenKind::KwAsync);
        let is_unsafe = self.match_token(TokenKind::KwUnsafe);
        let is_intrinsic = self.match_token(TokenKind::KwIntrinsic);

        self.consume(TokenKind::KwFn, "Expected 'fn'")?;
        let name = self
            .consume(TokenKind::Identifier, "Expected function name")?
            .span;

        let generic_params = self.parse_generic_params();

        self.consume(TokenKind::LParen, "Expected '(' after function name")?;
        let mut params = Vec::new();
        let mut is_variadic = false;

        if !self.check(TokenKind::RParen) {
            loop {
                if self.match_token(TokenKind::DotDotDot) {
                    is_variadic = true;
                    break;
                }

                let p_annotations = self.parse_annotations()?;

                let is_self = self.check(TokenKind::KwSelfVal);
                let p_name = if self.check(TokenKind::Identifier) || self.check(TokenKind::KwSelfVal) {
                    let span = self.peek().span;
                    self.advance();
                    span
                } else {
                    let span = self.peek().span;
                    self.error_at_current("Expected parameter name", span);
                    return Err(());
                };
                let ty = if self.match_token(TokenKind::Colon) {
                    Some(self.parse_type()?)
                } else {
                    None
                };

                params.push(self.arena.alloc_decl(Decl::Param {
                    annotations: p_annotations,
                    visibility: Visibility::Private,
                    name: p_name,
                    ty,
                    is_variadic: false,
                    is_self,
                }));

                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')'")?;

        let return_type = if self.match_token(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = if self.check(TokenKind::LBrace) {
            Some(self.parse_block_stmt()?)
        } else {
            self.consume(TokenKind::Semi, "Expected ';' or '{'")?;
            None
        };

        Ok(self.arena.alloc_decl(Decl::Function {
            annotations,
            visibility,
            name,
            generic_params,
            params,
            return_type,
            body,
            is_async,
            is_comptime,
            is_variadic,
            is_unsafe,
            is_intrinsic,
        }))
    }

    fn parse_struct_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let name = self
            .consume(TokenKind::Identifier, "Expected struct name")?
            .span;
        let generic_params = self.parse_generic_params();
        self.consume(TokenKind::LBrace, "Expected '{'")?;
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let f_name = self
                .consume(TokenKind::Identifier, "Expected field name")?
                .span;
            self.consume(TokenKind::Colon, "Expected ':'")?;
            let ty = self.parse_type()?;
            fields.push(mellis_ast::StructField {
                name: f_name,
                ty,
                visibility: Visibility::Public,
            });
            if !self.match_token(TokenKind::Semi) && !self.match_token(TokenKind::Comma) {
                if !self.check(TokenKind::RBrace) {
                    self.consume(TokenKind::Semi, "Expected ';' or ',' after struct field")?;
                }
            }
        }
        self.consume(TokenKind::RBrace, "Expected '}'")?;
        Ok(self.arena.alloc_decl(Decl::Struct {
            annotations,
            visibility,
            name,
            generic_params,
            fields,
        }))
    }

    fn parse_generic_params(&mut self) -> Vec<GenericParam> {
        let mut params = Vec::new();
        if self.match_token(TokenKind::LessThan) {
            while !self.check(TokenKind::GreaterThan) && !self.is_at_end() {
                if self.check(TokenKind::Identifier) {
                    let name = self.advance().span;
                    let mut bounds = Vec::new();
                    if self.match_token(TokenKind::Colon) {
                        // Parse trait bounds: T: Trait + Trait2
                        loop {
                            if let Ok(ty) = self.parse_type() {
                                bounds.push(ty);
                            }
                            if !self.match_token(TokenKind::Plus) {
                                break;
                            }
                        }
                    }
                    params.push(GenericParam {
                        name,
                        kind: GenericParamKind::Type,
                        bounds,
                    });
                } else if self.check(TokenKind::Lifetime) {
                    let name = self.advance().span;
                    params.push(GenericParam {
                        name,
                        kind: GenericParamKind::Lifetime,
                        bounds: Vec::new(),
                    });
                } else {
                    break;
                }
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
            let _ = self.consume(TokenKind::GreaterThan, "Expected '>' after generic params");
        }
        params
    }

    fn parse_enum_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let name = self
            .consume(TokenKind::Identifier, "Expected enum name")?
            .span;
        let generic_params = self.parse_generic_params();
        self.consume(TokenKind::LBrace, "Expected '{' for enum body")?;
        let mut variants = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let v_name = self
                .consume(TokenKind::Identifier, "Expected variant name")?
                .span;
            let mut fields = Vec::new();
            if self.match_token(TokenKind::LParen) {
                if !self.check(TokenKind::RParen) {
                    loop {
                        // Check if it's `name: Type` or just `Type`
                        let has_label = self.check(TokenKind::Identifier)
                            && self.peek_next().kind == TokenKind::Colon;
                        let p_name = if has_label {
                            let n = self.advance().span;
                            self.advance(); // consume ':'
                            n
                        } else {
                            self.peek().span // use position as fallback name
                        };
                        let ty = Some(self.parse_type()?);
                        fields.push(self.arena.alloc_decl(Decl::Param {
                            annotations: Vec::new(),
                            visibility: Visibility::Public,
                            name: p_name,
                            ty,
                            is_variadic: false,
                            is_self: false,
                        }));
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after variant fields")?;
            }
            variants.push(mellis_ast::EnumVariant {
                name: v_name,
                fields,
            });
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }
        self.consume(TokenKind::RBrace, "Expected '}'")?;
        Ok(self.arena.alloc_decl(Decl::Enum {
            annotations,
            visibility,
            name,
            generic_params,
            variants,
        }))
    }

    fn parse_trait_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let name = self
            .consume(TokenKind::Identifier, "Expected trait name")?
            .span;
        let generic_params = self.parse_generic_params();
        self.consume(TokenKind::LBrace, "Expected '{'")?;
        let mut methods = Vec::new();
        let associated_types = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            // Skip annotations/visibility for now
            let _ = self.match_token(TokenKind::KwExport);
            if self.check(TokenKind::KwFn) || self.check(TokenKind::KwUnsafe) {
                let m = self.parse_func_decl(Visibility::Public, Vec::new(), false)?;
                methods.push(m);
            } else if self.match_token(TokenKind::KwType) {
                // Skip associated type declarations for now
                let _ = self.advance(); // type name
                if self.match_token(TokenKind::Colon) {
                    // Skip bounds
                    while !self.check(TokenKind::Semi) && !self.check(TokenKind::RBrace) && !self.is_at_end() {
                        self.advance();
                    }
                }
                let _ = self.match_token(TokenKind::Semi);
            } else {
                self.advance(); // skip unexpected tokens
            }
        }
        self.consume(TokenKind::RBrace, "Expected '}'")?;
        Ok(self.arena.alloc_decl(Decl::Trait {
            annotations,
            visibility,
            name,
            generic_params,
            associated_types,
            methods,
        }))
    }

    fn parse_impl_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let generic_params = self.parse_generic_params();
        let first_type = self.parse_type()?;
        let (self_type, trait_type) = if self.match_token(TokenKind::KwFor) {
            let actual_self_type = self.parse_type()?;
            (actual_self_type, Some(first_type))
        } else {
            (first_type, None)
        };
        self.consume(TokenKind::LBrace, "Expected '{'")?;
        let mut methods = Vec::new();
        let associated_types = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let _ = self.match_token(TokenKind::KwExport);
            if self.check(TokenKind::KwFn) || self.check(TokenKind::KwUnsafe) || self.check(TokenKind::KwIntrinsic) {
                let m = self.parse_func_decl(Visibility::Public, Vec::new(), false)?;
                methods.push(m);
            } else if self.match_token(TokenKind::KwType) {
                // Skip associated type impl
                while !self.check(TokenKind::Semi) && !self.check(TokenKind::RBrace) && !self.is_at_end() {
                    self.advance();
                }
                let _ = self.match_token(TokenKind::Semi);
            } else {
                self.advance();
            }
        }
        self.consume(TokenKind::RBrace, "Expected '}'")?;
        Ok(self.arena.alloc_decl(Decl::Impl {
            annotations,
            visibility,
            generic_params,
            self_type,
            trait_type,
            associated_types,
            methods,
        }))
    }

    fn parse_module_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        if !self.match_token(TokenKind::Identifier) {
            let span = self.peek().span;
            self.error_at_current("Expected module name", span);
            return Err(());
        }
        let name = self.previous().span;

        self.consume(TokenKind::LBrace, "Expected '{' before module body")?;
        let mut items = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_item_impl() {
                Ok(Item::Decl(d)) => items.push(d),
                Ok(Item::Stmt(_)) => {
                    self.error_at_current("Modules can only contain declarations, not statements", self.previous().span);
                    return Err(());
                }
                Err(_) => return Err(()),
            }
        }
        self.consume(TokenKind::RBrace, "Expected '}' after module body")?;

        Ok(self.arena.alloc_decl(Decl::Module {
            annotations,
            visibility,
            name,
            items,
        }))
    }

    fn parse_type_alias_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let name_token = self.consume(TokenKind::Identifier, "Expected alias name")?;
        
        let generic_params = if self.check(TokenKind::LessThan) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };
        
        self.consume(TokenKind::Equal, "Expected '=' in type alias")?;
        
        let aliased_type = Some(self.parse_type()?);
        
        self.consume(TokenKind::Semi, "Expected ';' after type alias")?;
        
        let decl = Decl::TypeAlias {
            annotations,
            visibility,
            name: name_token.span,
            generic_params,
            bounds: Vec::new(),
            aliased_type,
        };
        
        Ok(self.arena.alloc_decl(decl))
    }

    /// Parse `using <module_path> as <alias>;`
    ///
    /// Grammar: `using_decl ::= "using" module_path "as" IDENTIFIER ";"`
    /// where `module_path ::= IDENTIFIER ("::" IDENTIFIER)*`
    ///
    /// Rejects:
    /// - `using as x;` (empty path)
    /// - `using ::foo as x;` (leading `::`)
    /// - `using foo:: as x;` (trailing `::`)
    /// - `using foo::bar;` (missing `as`)
    fn parse_using_decl(&mut self) -> Result<DeclId, ()> {
        let start_span = self.previous().span; // span of `using` keyword

        // Must start with an identifier (reject `using as x;` and `using ::foo as x;`)
        if !self.check(TokenKind::Identifier) {
            let span = self.peek().span;
            self.error_at_current("Expected namespace path after `using`", span);
            return Err(());
        }

        // Parse module_path: IDENTIFIER (:: IDENTIFIER)*
        let mut path = Vec::new();
        let first = self.consume(TokenKind::Identifier, "Expected identifier in using path")?;
        path.push(first.span);

        while self.match_token(TokenKind::ColonColon) {
            if !self.check(TokenKind::Identifier) {
                let span = self.peek().span;
                self.error_at_current("Expected identifier after `::` in using path", span);
                return Err(());
            }
            let seg = self.consume(TokenKind::Identifier, "Expected identifier")?;
            path.push(seg.span);
        }

        // Path must have at least one segment (already guaranteed above)

        // Must have `as`
        if !self.match_token(TokenKind::KwAs) {
            let span = self.peek().span;
            self.error_at_current("`using` requires `as <alias>` — bare `using path;` is not allowed", span);
            return Err(());
        }

        // Parse alias identifier
        let alias_token = self.consume(TokenKind::Identifier, "Expected alias identifier after `as`")?;

        // Semicolon
        self.consume(TokenKind::Semi, "Expected `;` after using declaration")?;

        let end_span = self.previous().span;
        let full_span = mellis_common::Span::new(
            start_span.file_id,
            start_span.start,
            end_span.end,
        ).with_ctxt(start_span.ctxt);

        Ok(self.arena.alloc_decl(Decl::Using {
            path,
            alias: alias_token.span,
            span: full_span,
        }))
    }

    fn parse_macro_fragment(&mut self) -> Result<mellis_ast::FragmentKind, ()> {
        if !self.check(TokenKind::Identifier) {
            let span = self.peek().span;
            self.error_at_current("Expected fragment specifier (`expr`, `ident`, `ty`, `stmt`, `block`, `item`)", span);
            return Err(());
        }
        let tok = self.advance();
        let text = &self.source[tok.span.start as usize..tok.span.end as usize];
        match text {
            "expr" => Ok(mellis_ast::FragmentKind::Expr),
            "ident" => Ok(mellis_ast::FragmentKind::Ident),
            "ty" => Ok(mellis_ast::FragmentKind::Ty),
            "stmt" => Ok(mellis_ast::FragmentKind::Stmt),
            "block" => Ok(mellis_ast::FragmentKind::Block),
            "item" => Ok(mellis_ast::FragmentKind::Item),
            _ => {
                self.error_at_current(&format!("Unknown macro fragment specifier '{}'", text), tok.span);
                Err(())
            }
        }
    }

    fn parse_matcher_element(&mut self) -> Result<mellis_ast::MatcherElement, ()> {
        if self.match_token(TokenKind::At) || self.match_token(TokenKind::Dollar) {
            let at_span = self.previous().span;
            let name_tok = self.consume(TokenKind::Identifier, "Expected identifier after '@' in macro metavariable")?;
            self.consume(TokenKind::Colon, "Expected ':' after metavariable name")?;
            let fragment = self.parse_macro_fragment()?;
            let span = Span::new(
                at_span.file_id,
                at_span.start,
                self.previous().span.end,
            ).with_ctxt(at_span.ctxt);
            Ok(mellis_ast::MatcherElement::MetaVar {
                name: name_tok.span,
                fragment,
                span,
            })
        } else if self.check(TokenKind::LParen) || self.check(TokenKind::LBracket) || self.check(TokenKind::LBrace) {
            let (delimiter, close_kind) = match self.peek().kind {
                TokenKind::LParen => (mellis_ast::MacroDelimiter::Paren, TokenKind::RParen),
                TokenKind::LBracket => (mellis_ast::MacroDelimiter::Bracket, TokenKind::RBracket),
                TokenKind::LBrace => (mellis_ast::MacroDelimiter::Brace, TokenKind::RBrace),
                _ => unreachable!(),
            };
            let start_span = self.advance().span;
            let mut elements = Vec::new();
            while !self.check(close_kind) && !self.is_at_end() {
                elements.push(self.parse_matcher_element()?);
            }
            let end_tok = self.consume(close_kind, &format!("Expected closing {:?}", close_kind))?;
            let span = Span::new(
                start_span.file_id,
                start_span.start,
                end_tok.span.end,
            ).with_ctxt(start_span.ctxt);
            Ok(mellis_ast::MatcherElement::Group {
                delimiter,
                elements,
                span,
            })
        } else {
            let tok = self.advance();
            Ok(mellis_ast::MatcherElement::Leaf { token: tok })
        }
    }

    fn parse_macro_pattern(&mut self) -> Result<mellis_ast::MacroPattern, ()> {
        let (delimiter, close_kind) = if self.match_token(TokenKind::LParen) {
            (mellis_ast::MacroDelimiter::Paren, TokenKind::RParen)
        } else if self.match_token(TokenKind::LBracket) {
            (mellis_ast::MacroDelimiter::Bracket, TokenKind::RBracket)
        } else if self.match_token(TokenKind::LBrace) {
            (mellis_ast::MacroDelimiter::Brace, TokenKind::RBrace)
        } else {
            let span = self.peek().span;
            self.error_at_current("Expected '(', '[', or '{' before macro rule pattern", span);
            return Err(());
        };

        let start_span = self.previous().span;
        let mut elements = Vec::new();
        while !self.check(close_kind) && !self.is_at_end() {
            elements.push(self.parse_matcher_element()?);
        }
        let end_tok = self.consume(close_kind, &format!("Expected closing {:?}", close_kind))?;
        let span = Span::new(
            start_span.file_id,
            start_span.start,
            end_tok.span.end,
        ).with_ctxt(start_span.ctxt);
        Ok(mellis_ast::MacroPattern {
            delimiter,
            elements,
            span,
        })
    }

    fn parse_transcriber_element(&mut self) -> Result<mellis_ast::TranscriberElement, ()> {
        if self.match_token(TokenKind::At) || self.match_token(TokenKind::Dollar) {
            let at_span = self.previous().span;
            let name_tok = self.consume(TokenKind::Identifier, "Expected identifier after '@' in macro transcriber")?;
            let span = Span {
                file_id: at_span.file_id,
                start: at_span.start,
                end: name_tok.span.end,
                ctxt: at_span.ctxt,
            };
            Ok(mellis_ast::TranscriberElement::MetaVar {
                name: name_tok.span,
                span,
            })
        } else if self.check(TokenKind::LParen) || self.check(TokenKind::LBracket) || self.check(TokenKind::LBrace) {
            let (delimiter, close_kind) = match self.peek().kind {
                TokenKind::LParen => (mellis_ast::MacroDelimiter::Paren, TokenKind::RParen),
                TokenKind::LBracket => (mellis_ast::MacroDelimiter::Bracket, TokenKind::RBracket),
                TokenKind::LBrace => (mellis_ast::MacroDelimiter::Brace, TokenKind::RBrace),
                _ => unreachable!(),
            };
            let start_span = self.advance().span;
            let mut elements = Vec::new();
            while !self.check(close_kind) && !self.is_at_end() {
                elements.push(self.parse_transcriber_element()?);
            }
            let end_tok = self.consume(close_kind, &format!("Expected closing {:?}", close_kind))?;
            let span = Span {
                file_id: start_span.file_id,
                start: start_span.start,
                end: end_tok.span.end,
                ctxt: start_span.ctxt,
            };
            Ok(mellis_ast::TranscriberElement::Group {
                delimiter,
                elements,
                span,
            })
        } else {
            let tok = self.advance();
            Ok(mellis_ast::TranscriberElement::Leaf { token: tok })
        }
    }

    fn parse_macro_transcriber(&mut self) -> Result<mellis_ast::MacroTranscriber, ()> {
        let (delimiter, close_kind) = if self.match_token(TokenKind::LBrace) {
            (mellis_ast::MacroDelimiter::Brace, TokenKind::RBrace)
        } else if self.match_token(TokenKind::LParen) {
            (mellis_ast::MacroDelimiter::Paren, TokenKind::RParen)
        } else if self.match_token(TokenKind::LBracket) {
            (mellis_ast::MacroDelimiter::Bracket, TokenKind::RBracket)
        } else {
            let span = self.peek().span;
            self.error_at_current("Expected '{' before macro rule template", span);
            return Err(());
        };

        let start_span = self.previous().span;
        let mut elements = Vec::new();
        while !self.check(close_kind) && !self.is_at_end() {
            elements.push(self.parse_transcriber_element()?);
        }
        let end_tok = self.consume(close_kind, &format!("Expected closing {:?}", close_kind))?;
        let span = Span {
            file_id: start_span.file_id,
            start: start_span.start,
            end: end_tok.span.end,
            ctxt: start_span.ctxt,
        };
        Ok(mellis_ast::MacroTranscriber {
            delimiter,
            elements,
            span,
        })
    }

    fn collect_matchers(elements: &[mellis_ast::MatcherElement], out: &mut Vec<mellis_ast::MacroMatcher>) {
        for elem in elements {
            match elem {
                mellis_ast::MatcherElement::MetaVar { name, fragment, .. } => {
                    out.push(mellis_ast::MacroMatcher {
                        name: *name,
                        fragment: *fragment,
                        separator: None,
                        repetition: None,
                    });
                }
                mellis_ast::MatcherElement::Group { elements, .. } => {
                    Self::collect_matchers(elements, out);
                }
                mellis_ast::MatcherElement::Repetition { elements, separator, kind, .. } => {
                    for inner in elements {
                        if let mellis_ast::MatcherElement::MetaVar { name, fragment, .. } = inner {
                            out.push(mellis_ast::MacroMatcher {
                                name: *name,
                                fragment: *fragment,
                                separator: *separator,
                                repetition: Some(*kind),
                            });
                        }
                    }
                }
                mellis_ast::MatcherElement::Leaf { .. } => {}
            }
        }
    }

    fn collect_template_tokens(elements: &[mellis_ast::TranscriberElement], out: &mut Vec<Token>) {
        for elem in elements {
            match elem {
                mellis_ast::TranscriberElement::MetaVar { name, .. } => {
                    out.push(Token::new(TokenKind::At, *name));
                    out.push(Token::new(TokenKind::Identifier, *name));
                }
                mellis_ast::TranscriberElement::Group { delimiter, elements, span } => {
                    let (open, close) = match delimiter {
                        mellis_ast::MacroDelimiter::Paren => (TokenKind::LParen, TokenKind::RParen),
                        mellis_ast::MacroDelimiter::Bracket => (TokenKind::LBracket, TokenKind::RBracket),
                        mellis_ast::MacroDelimiter::Brace => (TokenKind::LBrace, TokenKind::RBrace),
                    };
                    let mut start_span = *span;
                    start_span.end = start_span.start + 1;
                    let mut end_span = *span;
                    end_span.start = end_span.end.saturating_sub(1);
                    out.push(Token::new(open, start_span));
                    Self::collect_template_tokens(elements, out);
                    out.push(Token::new(close, end_span));
                }
                mellis_ast::TranscriberElement::Leaf { token } => {
                    out.push(*token);
                }
                mellis_ast::TranscriberElement::Repetition { elements, .. } => {
                    Self::collect_template_tokens(elements, out);
                }
            }
        }
    }

    fn parse_macro_rule(&mut self) -> Result<mellis_ast::MacroRule, ()> {
        let pattern = self.parse_macro_pattern()?;
        
        if !self.match_token(TokenKind::FatArrow) && !self.match_token(TokenKind::Arrow) {
            let span = self.peek().span;
            self.error_at_current("Expected '=>' after macro rule pattern", span);
            return Err(());
        }

        let transcriber = self.parse_macro_transcriber()?;
        let span = Span {
            file_id: pattern.span.file_id,
            start: pattern.span.start,
            end: transcriber.span.end,
            ctxt: pattern.span.ctxt,
        };

        let mut matchers = Vec::new();
        Self::collect_matchers(&pattern.elements, &mut matchers);
        let mut template_tokens = Vec::new();
        Self::collect_template_tokens(&transcriber.elements, &mut template_tokens);

        Ok(mellis_ast::MacroRule {
            pattern,
            transcriber,
            matchers,
            template_tokens,
            span,
        })
    }

    fn parse_macro_decl(&mut self, visibility: Visibility, annotations: Vec<Annotation>) -> Result<DeclId, ()> {
        let name = self.consume(TokenKind::Identifier, "Expected macro name")?.span;
        let mut rules = Vec::new();
        
        if self.match_token(TokenKind::LBrace) {
            // Multi-rule macro: macro name { (pattern) => { body } ... }
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                rules.push(self.parse_macro_rule()?);
                // Allow optional comma or semicolon between rules
                let _ = self.match_token(TokenKind::Comma) || self.match_token(TokenKind::Semi);
            }
            self.consume(TokenKind::RBrace, "Expected '}' after macro rules")?;
        } else if self.check(TokenKind::LParen) {
            // Shorthand function-like: macro name(pattern) { body }
            let pattern = self.parse_macro_pattern()?;
            if self.match_token(TokenKind::FatArrow) || self.match_token(TokenKind::Arrow) {
                // optional arrow
            }
            let transcriber = self.parse_macro_transcriber()?;
            let span = Span {
                file_id: pattern.span.file_id,
                start: pattern.span.start,
                end: transcriber.span.end,
                ctxt: pattern.span.ctxt,
            };
            let mut matchers = Vec::new();
            Self::collect_matchers(&pattern.elements, &mut matchers);
            let mut template_tokens = Vec::new();
            Self::collect_template_tokens(&transcriber.elements, &mut template_tokens);
            rules.push(mellis_ast::MacroRule {
                pattern,
                transcriber,
                matchers,
                template_tokens,
                span,
            });
        } else {
            let span = self.peek().span;
            self.error_at_current("Expected '{' after macro name", span);
            return Err(());
        }
        
        Ok(self.arena.alloc_decl(Decl::Macro {
            annotations,
            visibility,
            name,
            rules,
        }))
    }
}
