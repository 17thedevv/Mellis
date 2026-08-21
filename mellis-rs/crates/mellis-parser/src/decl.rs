use crate::Parser;
use mellis_ast::{Annotation, Decl, DeclId, Item, Visibility, GenericParam, GenericParamKind};
use mellis_lexer::TokenKind;

impl<'a> Parser<'a> {
    pub fn parse_item_impl(&mut self) -> Result<Item, ()> {
        // annotations
        let annotations = Vec::new(); // TODO: parse annotations
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

        let decl = if self.check(TokenKind::KwDec) || self.check(TokenKind::KwConst) {
            self.parse_var_decl(visibility, annotations)?
        } else if self.match_token(TokenKind::KwUse) {
            self.parse_use_decl(visibility, annotations)?
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

    fn parse_use_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let tree = self.parse_use_tree()?;
        self.consume(TokenKind::Semi, "Expected ';' after use declaration")?;
        Ok(self.arena.alloc_decl(Decl::Use {
            annotations,
            visibility,
            tree,
        }))
    }

    fn parse_use_tree(&mut self) -> Result<mellis_ast::UseTree, ()> {
        let mut segments = Vec::new();
        let mut is_glob = false;
        let mut children = Vec::new();
        let mut alias = None;

        loop {
            if self.match_token(TokenKind::Multiply) {
                is_glob = true;
                break;
            } else if self.match_token(TokenKind::LBrace) {
                if !self.check(TokenKind::RBrace) {
                    loop {
                        children.push(self.parse_use_tree()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RBrace, "Expected '}'")?;
                break;
            } else {
                let name = self.consume(TokenKind::Identifier, "Expected module or item name")?;
                segments.push(name.span);

                if self.match_token(TokenKind::KwAs) {
                    let alias_token = self.consume(TokenKind::Identifier, "Expected alias name")?;
                    alias = Some(alias_token.span);
                    break;
                }

                if !self.match_token(TokenKind::ColonColon) {
                    break;
                }
            }
        }

        Ok(mellis_ast::UseTree {
            segments,
            alias,
            is_glob,
            children,
        })
    }

    fn parse_var_decl(
        &mut self,
        visibility: Visibility,
        annotations: Vec<Annotation>,
    ) -> Result<DeclId, ()> {
        let is_mutable = if self.match_token(TokenKind::KwConst) {
            false
        } else {
            self.consume(TokenKind::KwDec, "Expected 'dec' or 'const'")?;
            true
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

        // generic params skipped for basic impl
        let generic_params = Vec::new();

        self.consume(TokenKind::LParen, "Expected '(' after function name")?;
        let mut params = Vec::new();
        let mut is_variadic = false;

        if !self.check(TokenKind::RParen) {
            loop {
                if self.match_token(TokenKind::DotDotDot) {
                    is_variadic = true;
                    break;
                }

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
                    annotations: Vec::new(),
                    visibility: Visibility::Private,
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
        let generic_params = Vec::new();
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
                    // Skip lifetimes for now
                    self.advance();
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
}
