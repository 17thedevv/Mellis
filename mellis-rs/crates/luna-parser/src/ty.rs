use crate::Parser;
use luna_ast::{AssociatedBinding, Type, TypeId};
use luna_lexer::{BuiltinKind, TokenKind};

impl<'a> Parser<'a> {
    pub fn parse_type(&mut self) -> Result<TypeId, ()> {
        if let TokenKind::BuiltinType(k) = self.peek().kind {
            self.advance();
            return Ok(self.arena.alloc_type(Type::Builtin(k)));
        }

        if self.match_token(TokenKind::Lifetime) {
            let span = self.previous().span;
            return Ok(self.arena.alloc_type(Type::Lifetime(span)));
        }

        if self.match_token(TokenKind::Bang) {
            return Ok(self.arena.alloc_type(Type::Never));
        }

        if self.match_token(TokenKind::BitAnd) {
            let lifetime = if self.check(TokenKind::Lifetime) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let is_mutable =
                self.match_token(TokenKind::KwRw);
            let inner = self.parse_type()?;
            return Ok(self.arena.alloc_type(Type::Reference {
                is_mutable,
                lifetime,
                inner,
            }));
        }

        if self.match_token(TokenKind::Multiply) {
            let is_mutable =
                self.match_token(TokenKind::KwRw);
            let inner = self.parse_type()?;
            return Ok(self.arena.alloc_type(Type::Pointer { is_mutable, inner }));
        }

        if self.match_token(TokenKind::LBracket) {
            let element_type = self.parse_type()?;
            if self.match_token(TokenKind::RBracket) {
                return Ok(self.arena.alloc_type(Type::Slice { inner: element_type }));
            }
            if !self.match_token(TokenKind::Semi) {
                let span = self.peek().span;
                self.error_at_current("Expected ';' in array type", span);
                return Err(());
            }
            let size = self.parse_expression(true)?;
            self.consume(TokenKind::RBracket, "Expected ']' after array size")?;
            return Ok(self.arena.alloc_type(Type::Array { element_type, size }));
        }

        if self.match_token(TokenKind::LParen) {
            if self.match_token(TokenKind::RParen) {
                return Ok(self.arena.alloc_type(Type::Tuple {
                    elements: Vec::new(),
                }));
            }
            let mut elements = Vec::new();
            loop {
                elements.push(self.parse_type()?);
                if !self.match_token(TokenKind::Comma) || self.check(TokenKind::RParen) {
                    break;
                }
            }
            self.consume(TokenKind::RParen, "Expected ')' after tuple type elements")?;
            return Ok(self.arena.alloc_type(Type::Tuple { elements }));
        }

        let is_unsafe_fn = self.match_token(TokenKind::KwUnsafe);
        if is_unsafe_fn || self.match_token(TokenKind::KwFn) {
            if is_unsafe_fn {
                self.consume(TokenKind::KwFn, "Expected 'fn' after 'unsafe' in function type")?;
            }
            self.consume(
                TokenKind::LParen,
                "Expected '(' for function type parameters",
            )?;
            let mut params = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    params.push(self.parse_type()?);
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
            return Ok(self.arena.alloc_type(Type::Function {
                params,
                return_type,
                is_unsafe: is_unsafe_fn,
            }));
        }

        if self.check(TokenKind::Identifier) || self.check(TokenKind::KwSelfTyp) || matches!(self.peek().kind, TokenKind::BuiltinType(_)) {
            let mut segments = Vec::new();
            let mut generic_args = Vec::new();
            let mut associated_bindings = Vec::new();

            loop {
                if self.match_token(TokenKind::KwSelfTyp) {
                    segments.push(self.previous().span);
                } else if self.check(TokenKind::Identifier) || matches!(self.peek().kind, TokenKind::BuiltinType(_)) {
                    segments.push(self.advance().span);
                } else {
                    let span = self.peek().span;
                    self.error_at_current("Expected identifier in named type", span);
                    return Err(());
                }

                if self.match_token(TokenKind::Bang) {
                    let name_span = *segments.last().unwrap();
                    let (delimiter, close_kind) = if self.match_token(TokenKind::LParen) {
                        (luna_ast::MacroDelimiter::Paren, TokenKind::RParen)
                    } else if self.match_token(TokenKind::LBracket) {
                        (luna_ast::MacroDelimiter::Bracket, TokenKind::RBracket)
                    } else if self.match_token(TokenKind::LBrace) {
                        (luna_ast::MacroDelimiter::Brace, TokenKind::RBrace)
                    } else {
                        let span = self.peek().span;
                        self.error_at_current("Expected '(', '[', or '{' after macro '!'", span);
                        return Err(());
                    };

                    let start_pos = self.pos;
                    let mut args = Vec::new();
                    while !self.check(close_kind) && !self.is_at_end() {
                        args.push(self.parse_token_tree()?);
                    }
                    let raw_tokens = self.tokens[start_pos..self.pos].to_vec();
                    let end_tok = self.consume(close_kind, &format!("Expected closing {:?}", close_kind))?;
                    let full_span = luna_common::Span::new(
                        segments[0].file_id,
                        segments[0].start,
                        end_tok.span.end,
                    ).with_ctxt(segments[0].ctxt);

                    return Ok(self.arena.alloc_type(Type::MacroCall {
                        name: name_span,
                        path: segments,
                        delimiter,
                        args,
                        raw_tokens,
                        span: full_span,
                    }));
                }

                if self.match_token(TokenKind::LessThan) {
                    if !self.check(TokenKind::GreaterThan) {
                        loop {
                            if self.check(TokenKind::Identifier) && self.peek_next().kind == TokenKind::Equal {
                                let name = self.advance().span;
                                self.consume(TokenKind::Equal, "Expected '=' in associated type binding")?;
                                let ty = self.parse_type()?;
                                associated_bindings.push(AssociatedBinding { name, ty });
                            } else {
                                generic_args.push(self.parse_type()?);
                            }
                            if !self.match_token(TokenKind::Comma)
                                || self.check(TokenKind::GreaterThan)
                            {
                                break;
                            }
                        }
                    }
                    self.consume(
                        TokenKind::GreaterThan,
                        "Expected '>' after type generic arguments",
                    )?;
                }

                if !self.match_token(TokenKind::ColonColon) {
                    break;
                }
            }

            return Ok(self.arena.alloc_type(Type::Named {
                segments,
                generic_args,
                associated_bindings,
            }));
        }

        if self.match_token(TokenKind::KwTypeof) {
            self.consume(TokenKind::LParen, "Expected '(' after typeof")?;
            let expr = self.parse_expression(true)?;
            self.consume(TokenKind::RParen, "Expected ')' after typeof expression")?;
            return Ok(self.arena.alloc_type(Type::Typeof { expr }));
        }

        if self.match_token(TokenKind::KwDyn) {
            let trait_type = self.parse_type()?;
            return Ok(self.arena.alloc_type(Type::TraitObject { trait_type }));
        }

        let span = self.peek().span;
        self.error_at_current("Expected type", span);
        Err(())
    }
}
