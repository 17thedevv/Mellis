import os

rust_code = """use crate::Parser;
use mellis_ast::{Decl, DeclId, Visibility, Item, Annotation};
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
        
        let is_function = self.check(TokenKind::KwFn) || self.check(TokenKind::KwAsync) || self.check(TokenKind::KwIntrinsic) || 
                          (self.check(TokenKind::KwComptime) && self.peek_next().kind == TokenKind::KwFn) ||
                          (self.check(TokenKind::KwUnsafe) && self.peek_next().kind == TokenKind::KwFn);
                          
        let decl = if self.check(TokenKind::KwDec) || self.check(TokenKind::KwConst) {
            self.parse_var_decl(visibility, annotations)?
        } else if is_function {
            self.parse_func_decl(visibility, annotations, is_extern)?
        } else if self.match_token(TokenKind::KwStruct) {
            self.parse_struct_decl(visibility, annotations)?
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
            self.arena.alloc_decl(Decl::Extern { annotations: Vec::new(), visibility, func: decl })
        } else {
            decl
        };
        
        Ok(Item::Decl(decl))
    }

    fn parse_var_decl(&mut self, visibility: Visibility, annotations: Vec<Annotation>) -> Result<DeclId, ()> {
        let is_mutable = if self.match_token(TokenKind::KwConst) { false } else { self.consume(TokenKind::KwDec, "Expected 'dec' or 'const'")?; true };
        let pattern = Some(self.parse_pattern()?);
        
        let name = self.previous().span; // Fallback
        
        let type_annot = if self.match_token(TokenKind::Colon) { Some(self.parse_type()?) } else { None };
        let initializer = if self.match_token(TokenKind::Equal) { Some(self.parse_expression(true)?) } else { None };
        
        self.consume(TokenKind::Semi, "Expected ';' after variable declaration")?;
        Ok(self.arena.alloc_decl(Decl::Var { annotations, visibility, name, pattern, type_annot, initializer, is_mutable }))
    }

    fn parse_func_decl(&mut self, visibility: Visibility, annotations: Vec<Annotation>, _allow_empty: bool) -> Result<DeclId, ()> {
        let is_comptime = self.match_token(TokenKind::KwComptime);
        let is_async = self.match_token(TokenKind::KwAsync);
        let is_unsafe = self.match_token(TokenKind::KwUnsafe);
        let is_intrinsic = self.match_token(TokenKind::KwIntrinsic);
        
        self.consume(TokenKind::KwFn, "Expected 'fn'")?;
        let name = self.consume(TokenKind::Identifier, "Expected function name")?.span;
        
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
                
                let p_name = self.consume(TokenKind::Identifier, "Expected parameter name")?.span;
                self.consume(TokenKind::Colon, "Expected ':' after parameter name")?;
                let ty = Some(self.parse_type()?);
                
                params.push(self.arena.alloc_decl(Decl::Param {
                    annotations: Vec::new(), visibility: Visibility::Private, name: p_name, ty, is_variadic: false, is_self: false
                }));
                
                if !self.match_token(TokenKind::Comma) { break; }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')'")?;
        
        let return_type = if self.match_token(TokenKind::Arrow) { Some(self.parse_type()?) } else { None };
        
        let body = if self.check(TokenKind::LBrace) {
            Some(self.parse_block_stmt()?)
        } else {
            self.consume(TokenKind::Semi, "Expected ';' or '{'")?;
            None
        };
        
        Ok(self.arena.alloc_decl(Decl::Function {
            annotations, visibility, name, generic_params, params, return_type, body, is_async, is_comptime, is_variadic, is_unsafe, is_intrinsic
        }))
    }

    fn parse_struct_decl(&mut self, visibility: Visibility, annotations: Vec<Annotation>) -> Result<DeclId, ()> {
        let name = self.consume(TokenKind::Identifier, "Expected struct name")?.span;
        let generic_params = Vec::new();
        self.consume(TokenKind::LBrace, "Expected '{'")?;
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let f_name = self.consume(TokenKind::Identifier, "Expected field name")?.span;
            self.consume(TokenKind::Colon, "Expected ':'")?;
            let ty = self.parse_type()?;
            fields.push(mellis_ast::StructField { name: f_name, ty, visibility: Visibility::Public });
            if !self.match_token(TokenKind::Comma) { break; }
        }
        self.consume(TokenKind::RBrace, "Expected '}'")?;
        Ok(self.arena.alloc_decl(Decl::Struct { annotations, visibility, name, generic_params, fields }))
    }
}
"""

with open(r"d:\fdlang\mellis-rs\crates\mellis-parser\src\decl.rs", "w", encoding="utf-8") as f:
    f.write(rust_code)

print("Created decl.rs")
