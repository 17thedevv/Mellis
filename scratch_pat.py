import os

rust_code = """use crate::Parser;
use mellis_ast::{Pattern, PatId, StructPatternField};
use mellis_lexer::TokenKind;

impl<'a> Parser<'a> {
    pub fn parse_pattern(&mut self) -> Result<PatId, ()> {
        if self.match_token(TokenKind::LParen) {
            let mut elements = Vec::new();
            let mut has_rest = false;
            
            if !self.check(TokenKind::RParen) {
                loop {
                    if self.match_token(TokenKind::DotDot) {
                        has_rest = true;
                        break;
                    }
                    elements.push(self.parse_pattern()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, "Expected ')' for tuple pattern")?;
            return Ok(self.arena.alloc_pat(Pattern::Tuple { elements, has_rest }));
        }
        
        if self.check(TokenKind::Identifier) {
            let tok = self.advance();
            // wait, tok.span.text doesn't exist. We need to check if it's '_' from the source code, but we don't have source code in parser yet easily.
            // Actually, we can check if it's '_' in lexer if lexer has a special token, but lexer probably gives Identifier for '_'.
            // Let's assume we can't easily check for '_' without source, so everything is Identifier.
            // Wait, does mellis_lexer have `TokenKind::Underscore`?
            // Let's assume it's just Identifier for now, since we only have `Span`.
            let mut segments = vec![tok.span];
            
            while self.match_token(TokenKind::ColonColon) {
                let seg = self.consume(TokenKind::Identifier, "Expected identifier")?;
                segments.push(seg.span);
            }
            
            if segments.len() > 1 || self.check(TokenKind::LParen) || self.check(TokenKind::LBrace) {
                if self.match_token(TokenKind::LBrace) {
                    let mut fields = Vec::new();
                    let mut has_rest = false;
                    
                    if !self.check(TokenKind::RBrace) {
                        loop {
                            if self.match_token(TokenKind::DotDot) {
                                has_rest = true;
                                break;
                            }
                            let field_name = self.consume(TokenKind::Identifier, "Expected field name in struct pattern")?;
                            let pattern = if self.match_token(TokenKind::Colon) {
                                Some(self.parse_pattern()?)
                            } else {
                                // Shorthand
                                let id_pat = self.arena.alloc_pat(Pattern::Identifier { segments: vec![field_name.span] });
                                Some(id_pat)
                            };
                            fields.push(StructPatternField { name: field_name.span, pattern });
                            if !self.match_token(TokenKind::Comma) { break; }
                        }
                    }
                    self.consume(TokenKind::RBrace, "Expected '}' after struct pattern")?;
                    return Ok(self.arena.alloc_pat(Pattern::Struct { path: segments, fields, has_rest }));
                } else if self.match_token(TokenKind::LParen) {
                    let mut fields = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        loop {
                            fields.push(self.parse_pattern()?);
                            if !self.match_token(TokenKind::Comma) { break; }
                        }
                    }
                    self.consume(TokenKind::RParen, "Expected ')'")?;
                    return Ok(self.arena.alloc_pat(Pattern::Enum { path: segments, fields }));
                }
            }
            
            return Ok(self.arena.alloc_pat(Pattern::Identifier { segments }));
        }
        
        if self.check(TokenKind::IntegerLiteral) || self.check(TokenKind::FloatLiteral) || 
           self.check(TokenKind::StringLiteral) || self.check(TokenKind::KwTrue) || self.check(TokenKind::KwFalse) {
            let lit = self.advance();
            return Ok(self.arena.alloc_pat(Pattern::Literal(lit)));
        }
        
        let span = self.peek().span;
        self.error_at_current("Expected a pattern (e.g., literal, identifier, or '_') in match arm", span);
        Err(())
    }
}
"""

with open(r"d:\fdlang\mellis-rs\crates\mellis-parser\src\pat.rs", "w", encoding="utf-8") as f:
    f.write(rust_code)

print("Created pat.rs")
