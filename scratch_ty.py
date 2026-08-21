import os

rust_code = """use crate::Parser;
use mellis_ast::{Type, TypeId, AssociatedBinding};
use mellis_lexer::{TokenKind, BuiltinKind};

impl<'a> Parser<'a> {
    pub fn parse_type(&mut self) -> Result<TypeId, ()> {
        if self.check(TokenKind::BuiltinType) {
            let tok = self.advance();
            // Since we don't have direct access to BuiltinKind from Token (lexing detail), 
            // we assume Token exposes it or we parse from span.
            // For now, let's use a dummy BuiltinKind::I32 as a placeholder
            // In a real implementation we'd map string -> BuiltinKind or store it in Token.
            return Ok(self.arena.alloc_type(Type::Builtin(BuiltinKind::I32))); 
        }

        if self.match_token(TokenKind::Lifetime) {
            let span = self.previous().span;
            return Ok(self.arena.alloc_type(Type::Lifetime(span)));
        }

        if self.match_token(TokenKind::Bang) {
            return Ok(self.arena.alloc_type(Type::Never));
        }

        if self.match_token(TokenKind::BitAnd) {
            let is_mutable = self.match_token(TokenKind::KwMut) || self.match_token(TokenKind::KwRw);
            let lifetime = if self.check(TokenKind::Lifetime) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let inner = self.parse_type()?;
            return Ok(self.arena.alloc_type(Type::Reference { is_mutable, lifetime, inner }));
        }

        if self.match_token(TokenKind::Multiply) {
            let is_mutable = self.match_token(TokenKind::KwMut) || self.match_token(TokenKind::KwRw);
            let inner = self.parse_type()?;
            return Ok(self.arena.alloc_type(Type::Pointer { is_mutable, inner }));
        }

        if self.match_token(TokenKind::LBracket) {
            let element_type = self.parse_type()?;
            self.consume(TokenKind::Semi, "Expected ';' in array type")?;
            let size = self.parse_expression(true)?;
            self.consume(TokenKind::RBracket, "Expected ']' after array size")?;
            return Ok(self.arena.alloc_type(Type::Array { element_type, size }));
        }

        if self.match_token(TokenKind::LParen) {
            if self.match_token(TokenKind::RParen) {
                return Ok(self.arena.alloc_type(Type::Tuple { elements: Vec::new() }));
            }
            let mut elements = Vec::new();
            loop {
                elements.push(self.parse_type()?);
                if !self.match_token(TokenKind::Comma) || self.check(TokenKind::RParen) { break; }
            }
            self.consume(TokenKind::RParen, "Expected ')' after tuple type elements")?;
            return Ok(self.arena.alloc_type(Type::Tuple { elements }));
        }
        
        if self.match_token(TokenKind::KwFn) {
            self.consume(TokenKind::LParen, "Expected '(' for function type parameters")?;
            let mut params = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    params.push(self.parse_type()?);
                    if !self.match_token(TokenKind::Comma) { break; }
                }
            }
            self.consume(TokenKind::RParen, "Expected ')'")?;
            let return_type = if self.match_token(TokenKind::Arrow) { Some(self.parse_type()?) } else { None };
            return Ok(self.arena.alloc_type(Type::Function { params, return_type, is_unsafe: false }));
        }
        
        if self.check(TokenKind::Identifier) || self.check(TokenKind::KwSelfTyp) {
            let mut segments = Vec::new();
            let mut generic_args = Vec::new();
            let mut associated_bindings = Vec::new();
            
            loop {
                if self.match_token(TokenKind::KwSelfTyp) {
                    segments.push(self.previous().span);
                } else if self.check(TokenKind::Identifier) {
                    segments.push(self.advance().span);
                } else {
                    let span = self.peek().span;
                    self.error_at_current("Expected identifier in named type", span);
                    return Err(());
                }
                
                if self.match_token(TokenKind::LessThan) {
                    if !self.check(TokenKind::GreaterThan) {
                        loop {
                            // Binding? e.g. Item = Type
                            // Let's assume standard generic arg for now
                            generic_args.push(self.parse_type()?);
                            if !self.match_token(TokenKind::Comma) || self.check(TokenKind::GreaterThan) { break; }
                        }
                    }
                    self.consume(TokenKind::GreaterThan, "Expected '>' after generic arguments")?;
                }
                
                if !self.match_token(TokenKind::ColonColon) { break; }
            }
            
            return Ok(self.arena.alloc_type(Type::Named { segments, generic_args, associated_bindings }));
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
"""

with open(r"d:\fdlang\mellis-rs\crates\mellis-parser\src\ty.rs", "w", encoding="utf-8") as f:
    f.write(rust_code)

print("Created ty.rs")
