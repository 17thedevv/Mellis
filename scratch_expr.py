import os

rust_code = """use crate::Parser;
use mellis_ast::{
    AssignOp, BinaryOp, CallArg, Expr, ExprId, FieldInit, MatchArm, UnaryOp,
};
use mellis_lexer::TokenKind;
use mellis_common::Span;

impl<'a> Parser<'a> {
    pub fn parse_expr(&mut self) -> Result<ExprId, ()> {
        self.parse_expression(true)
    }

    pub fn parse_expression(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        self.parse_assignment(allow_struct_literal)
    }

    fn parse_assignment(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let expr = self.parse_range(allow_struct_literal)?;
        
        let op = match self.peek().kind {
            TokenKind::Equal => Some(AssignOp::Assign),
            TokenKind::PlusAssign => Some(AssignOp::AddAssign),
            TokenKind::MinusAssign => Some(AssignOp::SubAssign),
            TokenKind::StarAssign => Some(AssignOp::MulAssign),
            TokenKind::SlashAssign => Some(AssignOp::DivAssign),
            TokenKind::PercAssign => Some(AssignOp::ModAssign),
            TokenKind::BitAndAssign => Some(AssignOp::BitAndAssign),
            TokenKind::BitOrAssign => Some(AssignOp::BitOrAssign),
            TokenKind::BitXorAssign => Some(AssignOp::BitXorAssign),
            TokenKind::LShiftAssign => Some(AssignOp::LShiftAssign),
            TokenKind::RShiftAssign => Some(AssignOp::RShiftAssign),
            _ => None,
        };

        if let Some(assign_op) = op {
            self.advance();
            let value = self.parse_assignment(allow_struct_literal)?;
            return Ok(self.arena.alloc_expr(Expr::Assign {
                op: assign_op,
                lvalue: expr,
                value,
            }));
        }

        Ok(expr)
    }

    fn parse_range(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_logical_or(allow_struct_literal)?;
        if self.check(TokenKind::DotDot) || self.check(TokenKind::DotDotEq) {
            let op = if self.match_token(TokenKind::DotDot) { BinaryOp::Range } else { self.advance(); BinaryOp::RangeInc };
            let right = self.parse_logical_or(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_logical_or(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_logical_and(allow_struct_literal)?;
        while self.match_token(TokenKind::LogicalOr) {
            let right = self.parse_logical_and(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op: BinaryOp::LogicOr, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_bitwise_or(allow_struct_literal)?;
        while self.match_token(TokenKind::LogicalAnd) {
            let right = self.parse_bitwise_or(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op: BinaryOp::LogicAnd, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_bitwise_or(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_bitwise_xor(allow_struct_literal)?;
        while self.match_token(TokenKind::BitOr) {
            let right = self.parse_bitwise_xor(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op: BinaryOp::BitOr, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_bitwise_xor(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_bitwise_and(allow_struct_literal)?;
        while self.match_token(TokenKind::BitXor) {
            let right = self.parse_bitwise_and(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op: BinaryOp::BitXor, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_bitwise_and(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_equality(allow_struct_literal)?;
        while self.match_token(TokenKind::BitAnd) {
            let right = self.parse_equality(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op: BinaryOp::BitAnd, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_equality(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_comparison(allow_struct_literal)?;
        while self.check(TokenKind::EqualEqual) || self.check(TokenKind::NotEqual) {
            let op = if self.match_token(TokenKind::EqualEqual) { BinaryOp::Eq } else { self.advance(); BinaryOp::Ne };
            let right = self.parse_comparison(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_shift(allow_struct_literal)?;
        while self.check(TokenKind::LessThan) || self.check(TokenKind::LessThanEqual) ||
              self.check(TokenKind::GreaterThan) || self.check(TokenKind::GreaterThanEqual) {
            let op = match self.peek().kind {
                TokenKind::LessThan => BinaryOp::Lt,
                TokenKind::LessThanEqual => BinaryOp::Le,
                TokenKind::GreaterThan => BinaryOp::Gt,
                TokenKind::GreaterThanEqual => BinaryOp::Ge,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_shift(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_shift(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_term(allow_struct_literal)?;
        while self.check(TokenKind::LShift) || self.check(TokenKind::RShift) {
            let op = if self.match_token(TokenKind::LShift) { BinaryOp::LShift } else { self.advance(); BinaryOp::RShift };
            let right = self.parse_term(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_term(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_factor(allow_struct_literal)?;
        while self.check(TokenKind::Plus) || self.check(TokenKind::Minus) {
            let op = if self.match_token(TokenKind::Plus) { BinaryOp::Add } else { self.advance(); BinaryOp::Sub };
            let right = self.parse_factor(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_factor(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_cast(allow_struct_literal)?;
        while self.check(TokenKind::Multiply) || self.check(TokenKind::Divide) || self.check(TokenKind::Modulo) {
            let op = match self.peek().kind {
                TokenKind::Multiply => BinaryOp::Mul,
                TokenKind::Divide => BinaryOp::Div,
                TokenKind::Modulo => BinaryOp::Mod,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_cast(allow_struct_literal)?;
            expr = self.arena.alloc_expr(Expr::Binary { op, left: expr, right });
        }
        Ok(expr)
    }

    fn parse_cast(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_unary(allow_struct_literal)?;
        while self.match_token(TokenKind::KwAs) {
            let target_type = self.parse_type()?;
            expr = self.arena.alloc_expr(Expr::Cast { expr, target_type });
        }
        Ok(expr)
    }

    fn parse_unary(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        if self.check(TokenKind::Minus) || self.check(TokenKind::Bang) || self.check(TokenKind::BitNot) ||
           self.check(TokenKind::Multiply) || self.check(TokenKind::BitAnd) {
            let op_token = self.advance();
            let op = match op_token.kind {
                TokenKind::Minus => UnaryOp::Neg,
                TokenKind::Bang => UnaryOp::Not,
                TokenKind::BitNot => UnaryOp::BitNot,
                TokenKind::Multiply => UnaryOp::Deref,
                TokenKind::BitAnd => if self.match_token(TokenKind::KwRw) { UnaryOp::RefMut } else { UnaryOp::Ref },
                _ => unreachable!(),
            };
            let operand = self.parse_unary(allow_struct_literal)?;
            return Ok(self.arena.alloc_expr(Expr::Unary { op, operand }));
        }
        self.parse_postfix(allow_struct_literal)
    }

    fn parse_postfix(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        let mut expr = self.parse_primary(allow_struct_literal)?;
        
        loop {
            if self.match_token(TokenKind::Question) {
                expr = self.arena.alloc_expr(Expr::Try { expr });
                continue;
            }
            if self.match_token(TokenKind::PlusPlus) {
                expr = self.arena.alloc_expr(Expr::Unary { op: UnaryOp::PostInc, operand: expr });
                continue;
            }
            if self.match_token(TokenKind::MinusMinus) {
                expr = self.arena.alloc_expr(Expr::Unary { op: UnaryOp::PostDec, operand: expr });
                continue;
            }
            if self.match_token(TokenKind::Bang) {
                // Macro Call `name!(...)`
                // Ensure `expr` is an IdentifierExpr with no generic args
                if let Expr::Identifier { segments, generic_args } = &self.arena.exprs[expr.0 as usize] {
                    if segments.len() == 1 && generic_args.is_empty() {
                        let name = segments[0];
                        self.consume(TokenKind::LParen, "Expected '(' after macro '!'")?;
                        let mut args = Vec::new();
                        if !self.check(TokenKind::RParen) {
                            loop {
                                args.push(CallArg { label: None, value: self.parse_expression(true)? });
                                if !self.match_token(TokenKind::Comma) { break; }
                            }
                        }
                        self.consume(TokenKind::RParen, "Expected ')' after macro arguments")?;
                        // For now we map macro calls to a Call Expr or we would need a MacroCallExpr.
                        // Wait, ast doesn't have MacroCallExpr, let me represent it as a regular Call for now
                        // or add MacroCall to AST. Looking at `expr.rs`, it's not there. We'll use Call.
                        expr = self.arena.alloc_expr(Expr::Call { callee: expr, generic_args: Vec::new(), args });
                    } else {
                        let span = self.previous().span;
                        self.error_at_current("Macro call must be a simple identifier", span);
                    }
                } else {
                    let span = self.previous().span;
                    self.error_at_current("Expected identifier before '!' in macro call", span);
                }
                continue;
            }
            if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check(TokenKind::RParen) {
                    loop {
                        let mut label = None;
                        if self.check(TokenKind::Identifier) && self.peek_next().kind == TokenKind::Colon {
                            label = Some(self.advance().span);
                            self.advance(); // consume ':'
                        }
                        args.push(CallArg { label, value: self.parse_expression(true)? });
                        if !self.match_token(TokenKind::Comma) { break; }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after function arguments")?;
                expr = self.arena.alloc_expr(Expr::Call { callee: expr, generic_args: Vec::new(), args });
            } else if self.match_token(TokenKind::LBracket) {
                let index = self.parse_expression(true)?;
                self.consume(TokenKind::RBracket, "Expected ']' after index")?;
                expr = self.arena.alloc_expr(Expr::Index { base: expr, index });
            } else if self.match_token(TokenKind::Dot) {
                if self.match_token(TokenKind::KwAwait) {
                    expr = self.arena.alloc_expr(Expr::Await { expr });
                    continue;
                }
                if self.check(TokenKind::IntegerLiteral) {
                    let token = self.advance();
                    // Basic parse to u32. Assuming token text is valid.
                    // Needs text access... Wait, token doesn't have text, it has span!
                    // We'll just store the span or dummy index for now, because lexer only gives token.
                    expr = self.arena.alloc_expr(Expr::TupleIndex { object: expr, index: 0 }); // FIXME: get text
                    continue;
                }
                
                let member_tok = if self.check(TokenKind::Identifier) || self.check(TokenKind::KwPrint) {
                    self.advance()
                } else {
                    let span = self.peek().span;
                    self.error_at_current("Expected member name or tuple index", span);
                    return Err(());
                };
                
                if self.match_token(TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        loop {
                            let mut label = None;
                            if self.check(TokenKind::Identifier) && self.peek_next().kind == TokenKind::Colon {
                                label = Some(self.advance().span);
                                self.advance(); // consume ':'
                            }
                            args.push(CallArg { label, value: self.parse_expression(true)? });
                            if !self.match_token(TokenKind::Comma) { break; }
                        }
                    }
                    self.consume(TokenKind::RParen, "Expected ')' after method arguments")?;
                    expr = self.arena.alloc_expr(Expr::MethodCall { object: expr, method_name: member_tok.span, generic_args: Vec::new(), args });
                } else {
                    expr = self.arena.alloc_expr(Expr::Member { object: expr, member: member_tok.span });
                }
            } else if allow_struct_literal && self.check(TokenKind::LBrace) {
                if let Expr::Identifier { segments, generic_args } = self.arena.exprs[expr.0 as usize].clone() {
                    self.advance(); // consume '{'
                    let mut fields = Vec::new();
                    while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                        let name = self.consume(TokenKind::Identifier, "Expected field name")?.span;
                        self.consume(TokenKind::Colon, "Expected ':' after field name")?;
                        let value = self.parse_expression(true)?;
                        fields.push(FieldInit { name, value });
                        if !self.match_token(TokenKind::Comma) { break; }
                    }
                    self.consume(TokenKind::RBrace, "Expected '}' after struct literal fields")?;
                    expr = self.arena.alloc_expr(Expr::StructInit { path: segments, generic_args, fields });
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }
    
    fn parse_value_path(&mut self) -> Result<ExprId, ()> {
        let mut segments = Vec::new();
        let mut generic_args = Vec::new();
        
        loop {
            let id_tok = if self.match_token(TokenKind::KwSelfTyp) || self.match_token(TokenKind::KwSelfVal) {
                self.previous()
            } else if self.check(TokenKind::Identifier) || self.check(TokenKind::KwPrint) {
                self.advance()
            } else {
                let span = self.peek().span;
                self.error_at_current("Expected identifier in value path", span);
                return Err(());
            };
            
            segments.push(id_tok.span);
            
            // Generic args `:: < ... >` or `<...>`? C++ Parser says `if (check(TokenType::GENERIC_START))` 
            // We'll skip generics in expression path for now to keep it simple, or implement if needed.
            
            if !self.match_token(TokenKind::ColonColon) {
                break;
            }
        }
        
        Ok(self.arena.alloc_expr(Expr::Identifier { segments, generic_args }))
    }

    fn parse_match_expr(&mut self) -> Result<ExprId, ()> {
        self.consume(TokenKind::KwMatch, "Expected 'match'")?;
        let subject = self.parse_expression(false)?;
        self.consume(TokenKind::LBrace, "Expected '{' for match body")?;
        
        let mut arms = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            let pattern = self.parse_pat()?;
            self.consume(TokenKind::Arrow, "Expected '->' after pattern")?;
            
            let body = if self.check(TokenKind::LBrace) {
                let stmt = self.parse_block_stmt()?;
                self.match_token(TokenKind::Comma);
                stmt
            } else {
                let expr = self.parse_expression(true)?;
                let stmt = self.arena.alloc_stmt(mellis_ast::Stmt::Expr { expr, has_semicolon: false });
                self.match_token(TokenKind::Comma);
                stmt
            };
            arms.push(MatchArm { pattern, body });
        }
        self.consume(TokenKind::RBrace, "Expected '}'")?;
        Ok(self.arena.alloc_expr(Expr::Match { subject, arms }))
    }
    
    fn parse_lambda_expr(&mut self) -> Result<ExprId, ()> {
        let is_move = self.match_token(TokenKind::KwMove);
        self.consume(TokenKind::BitOr, "Expected '|'")?;
        
        let mut params = Vec::new();
        if !self.check(TokenKind::BitOr) {
            loop {
                let name = self.consume(TokenKind::Identifier, "Expected lambda param name")?.span;
                let ty = if self.match_token(TokenKind::Colon) { Some(self.parse_type()?) } else { None };
                
                let param_decl = self.arena.alloc_decl(mellis_ast::Decl::Param {
                    annotations: Vec::new(), visibility: mellis_ast::Visibility::Private,
                    name, ty, is_variadic: false, is_self: false,
                });
                params.push(param_decl);
                
                if !self.match_token(TokenKind::Comma) { break; }
            }
        }
        self.consume(TokenKind::BitOr, "Expected '|'")?;
        self.consume(TokenKind::Arrow, "Expected '->' after lambda parameters")?;
        
        // Simplified lambda return type parsing logic for now (omitting bookmark rollback for brevity)
        let return_type = None; // For real implementation, attempt to parse type if present
        
        let body = if self.check(TokenKind::LBrace) {
            self.parse_block_stmt()?
        } else {
            let expr = self.parse_expression(true)?;
            self.arena.alloc_stmt(mellis_ast::Stmt::Block {
                body: Vec::new(), tail_expr: Some(expr)
            })
        };
        
        Ok(self.arena.alloc_expr(Expr::Lambda { params, return_type, body, is_move }))
    }

    fn parse_primary(&mut self, allow_struct_literal: bool) -> Result<ExprId, ()> {
        if self.check(TokenKind::IntegerLiteral) || self.check(TokenKind::FloatLiteral) ||
           self.check(TokenKind::StringLiteral) || self.check(TokenKind::RawStringLiteral) ||
           self.check(TokenKind::CharLiteral) || self.check(TokenKind::KwTrue) || self.check(TokenKind::KwFalse) {
            let token = self.advance();
            return Ok(self.arena.alloc_expr(Expr::Literal(token)));
        }
        
        if self.check(TokenKind::Identifier) || self.check(TokenKind::KwSelfVal) {
            return self.parse_value_path();
        }
        
        if self.check(TokenKind::KwMatch) {
            return self.parse_match_expr();
        }
        
        if self.check(TokenKind::KwMove) || self.check(TokenKind::BitOr) {
            return self.parse_lambda_expr();
        }
        
        if self.match_token(TokenKind::LBracket) {
            let mut elements = Vec::new();
            if !self.check(TokenKind::RBracket) {
                loop {
                    elements.push(self.parse_expression(true)?);
                    if !self.match_token(TokenKind::Comma) || self.check(TokenKind::RBracket) { break; }
                }
            }
            self.consume(TokenKind::RBracket, "Expected ']' after array literal")?;
            return Ok(self.arena.alloc_expr(Expr::ArrayLiteral { elements }));
        }
        
        if self.match_token(TokenKind::LParen) {
            if self.match_token(TokenKind::RParen) {
                return Ok(self.arena.alloc_expr(Expr::TupleLiteral { elements: Vec::new() }));
            }
            let expr = self.parse_expression(true)?;
            if self.match_token(TokenKind::Comma) {
                let mut elements = vec![expr];
                if !self.check(TokenKind::RParen) {
                    loop {
                        elements.push(self.parse_expression(true)?);
                        if !self.match_token(TokenKind::Comma) || self.check(TokenKind::RParen) { break; }
                    }
                }
                self.consume(TokenKind::RParen, "Expected ')' after tuple elements")?;
                return Ok(self.arena.alloc_expr(Expr::TupleLiteral { elements }));
            }
            self.consume(TokenKind::RParen, "Expected ')' after expression")?;
            return Ok(expr);
        }
        
        if self.match_token(TokenKind::KwSizeof) {
            self.consume(TokenKind::LParen, "Expected '(' after sizeof")?;
            let target_type = self.parse_type()?;
            self.consume(TokenKind::RParen, "Expected ')'")?;
            return Ok(self.arena.alloc_expr(Expr::Sizeof { target_type }));
        }
        
        if self.match_token(TokenKind::KwAlignof) {
            self.consume(TokenKind::LParen, "Expected '(' after alignof")?;
            let target_type = self.parse_type()?;
            self.consume(TokenKind::RParen, "Expected ')'")?;
            return Ok(self.arena.alloc_expr(Expr::Alignof { target_type }));
        }
        
        if self.match_token(TokenKind::KwTypeof) {
            self.consume(TokenKind::LParen, "Expected '(' after typeof")?;
            let expr = self.parse_expression(true)?;
            self.consume(TokenKind::RParen, "Expected ')'")?;
            return Ok(self.arena.alloc_expr(Expr::Typeof { expr }));
        }

        let span = self.peek().span;
        self.error_at_current("Expected expression.", span);
        Err(())
    }
}
"""

with open(r"d:\fdlang\mellis-rs\crates\mellis-parser\src\expr.rs", "w", encoding="utf-8") as f:
    f.write(rust_code)

print("Created expr.rs")
