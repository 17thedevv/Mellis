use crate::Parser;
use luna_ast::{ForKind, Item, Stmt, StmtId};
use luna_lexer::TokenKind;

impl<'a> Parser<'a> {
    pub fn parse_stmt(&mut self) -> Result<StmtId, ()> {
        let stmt = if self.match_token(TokenKind::KwReturn) {
            self.parse_return_stmt()?
        } else if self.match_token(TokenKind::KwBreak) {
            self.parse_break_stmt()?
        } else if self.match_token(TokenKind::KwContinue) {
            self.parse_continue_stmt()?
        } else if self.check(TokenKind::LBrace) {
            self.parse_block_stmt()?
        } else if self.check(TokenKind::KwIf) {
            self.parse_if_stmt()?
        } else if self.check(TokenKind::KwWhile) {
            self.parse_while_stmt()?
        } else if self.check(TokenKind::KwFor) {
            self.parse_for_stmt()?
        } else if self.match_token(TokenKind::KwUnsafe) {
            self.parse_unsafe_stmt()?
        } else if self.match_token(TokenKind::KwComptime) {
            self.parse_comptime_stmt()?
        } else {
            self.parse_expression_stmt()?
        };
        Ok(stmt)
    }

    pub fn parse_block_stmt(&mut self) -> Result<StmtId, ()> {
        self.consume(TokenKind::LBrace, "Expected '{' to start block")?;
        let mut body = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.parse_item()?);
        }

        let mut tail_expr = None;
        if let Some(Item::Stmt(last_stmt_id)) = body.last() {
            if let Stmt::Expr {
                expr,
                has_semicolon: false,
            } = &self.arena.stmts[last_stmt_id.0 as usize]
            {
                tail_expr = Some(*expr);
                body.pop();
            }
        }

        self.consume(TokenKind::RBrace, "Expected '}' to end block")?;
        Ok(self.arena.alloc_stmt(Stmt::Block { body, tail_expr }))
    }

    fn parse_expression_stmt(&mut self) -> Result<StmtId, ()> {
        let expr = self.parse_expression(true)?;
        if self.check(TokenKind::RBrace) {
            return Ok(self.arena.alloc_stmt(Stmt::Expr {
                expr,
                has_semicolon: false,
            }));
        }
        if let luna_ast::Expr::MacroCall { delimiter: luna_ast::MacroDelimiter::Brace, .. } = &self.arena.exprs[expr.0 as usize] {
            let has_semi = self.match_token(TokenKind::Semi);
            return Ok(self.arena.alloc_stmt(Stmt::Expr {
                expr,
                has_semicolon: has_semi,
            }));
        }
        self.consume(TokenKind::Semi, "Expected ';' after expression")?;
        Ok(self.arena.alloc_stmt(Stmt::Expr {
            expr,
            has_semicolon: true,
        }))
    }

    fn parse_if_stmt(&mut self) -> Result<StmtId, ()> {
        self.consume(TokenKind::KwIf, "Expected 'if'")?;
        let condition = self.parse_expression(false)?;
        let then_branch = self.parse_block_stmt()?;
        let else_branch = if self.match_token(TokenKind::KwElse) {
            if self.check(TokenKind::KwIf) {
                Some(self.parse_if_stmt()?)
            } else {
                Some(self.parse_block_stmt()?)
            }
        } else {
            None
        };
        Ok(self.arena.alloc_stmt(Stmt::If {
            condition,
            then_branch,
            else_branch,
        }))
    }

    fn parse_while_stmt(&mut self) -> Result<StmtId, ()> {
        self.consume(TokenKind::KwWhile, "Expected 'while'")?;
        let condition = self.parse_expression(false)?;
        let body = self.parse_block_stmt()?;
        Ok(self.arena.alloc_stmt(Stmt::While {
            label: None,
            condition,
            body,
        }))
    }

    fn is_cstyle_for(&self) -> bool {
        if self.pos >= self.tokens.len() || self.tokens[self.pos].kind != TokenKind::LParen {
            return false;
        }
        let mut p = self.pos;
        let mut depth = 0;
        while p < self.tokens.len() {
            let k = self.tokens[p].kind;
            if k == TokenKind::LParen { depth += 1; }
            else if k == TokenKind::RParen {
                depth -= 1;
                if depth == 0 { break; }
            }
            else if k == TokenKind::Semi && depth == 1 {
                return true;
            }
            p += 1;
        }
        false
    }

    fn parse_for_stmt(&mut self) -> Result<StmtId, ()> {
        self.consume(TokenKind::KwFor, "Expected 'for'")?;

        if self.match_token(TokenKind::At) {
            self.error_at_current(
                "Macro for loops are not fully implemented",
                self.previous().span,
            );
            return Err(());
        }

        let mut kind = ForKind::ForEach;
        let mut init = None;
        let mut cond = None;
        let mut step = None;
        let mut pattern = None;
        let mut iterable = None;

        if self.is_cstyle_for() {
            self.consume(TokenKind::LParen, "Expected '('")?;
            kind = ForKind::CStyle;
            if !self.check(TokenKind::Semi) {
                init = Some(self.parse_item()?);
            } else {
                self.consume(TokenKind::Semi, "Expected ';' after empty init")?;
            }
            if !self.check(TokenKind::Semi) {
                cond = Some(self.parse_expression(true)?);
            }
            self.consume(TokenKind::Semi, "Expected ';' after condition")?;
            if !self.check(TokenKind::RParen) {
                step = Some(self.parse_expression(true)?);
            }
            self.consume(TokenKind::RParen, "Expected ')' after step")?;
        } else {
            pattern = Some(self.parse_pattern()?);
            self.consume(TokenKind::KwIn, "Expected 'in' after loop pattern")?;
            iterable = Some(self.parse_expression(true)?);
        }

        let body = self.parse_block_stmt()?;
        Ok(self.arena.alloc_stmt(Stmt::For {
            kind,
            label: None,
            pattern,
            iterable,
            init,
            cond,
            step,
            body,
        }))
    }

    fn parse_return_stmt(&mut self) -> Result<StmtId, ()> {
        let value = if !self.check(TokenKind::Semi) {
            Some(self.parse_expression(true)?)
        } else {
            None
        };
        self.consume(TokenKind::Semi, "Expected ';' after return value")?;
        Ok(self.arena.alloc_stmt(Stmt::Return { value }))
    }

    fn parse_break_stmt(&mut self) -> Result<StmtId, ()> {
        let label = if self.check(TokenKind::Lifetime) {
            Some(self.advance().span)
        } else {
            None
        };
        self.consume(TokenKind::Semi, "Expected ';' after break")?;
        Ok(self.arena.alloc_stmt(Stmt::Break { label }))
    }

    fn parse_continue_stmt(&mut self) -> Result<StmtId, ()> {
        let label = if self.check(TokenKind::Lifetime) {
            Some(self.advance().span)
        } else {
            None
        };
        self.consume(TokenKind::Semi, "Expected ';' after continue")?;
        Ok(self.arena.alloc_stmt(Stmt::Continue { label }))
    }

    fn parse_unsafe_stmt(&mut self) -> Result<StmtId, ()> {
        let body = self.parse_block_stmt()?;
        Ok(self.arena.alloc_stmt(Stmt::Unsafe { body }))
    }

    fn parse_comptime_stmt(&mut self) -> Result<StmtId, ()> {
        let body = self.parse_block_stmt()?;
        Ok(self.arena.alloc_stmt(Stmt::Comptime { body }))
    }

    pub fn parse_item(&mut self) -> Result<Item, ()> {
        // Implementation delegates to parse_decl or parse_stmt
        // We'll call parse_decl. If parse_decl returns something, we wrap it in Item::Decl.
        // Wait, parse_decl can return a DeclId, or fallback to parsing a statement.
        // Let's implement this logic in decl.rs, and just call it here:
        self.parse_item_impl()
    }
}
