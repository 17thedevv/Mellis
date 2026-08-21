use crate::Parser;
use mellis_ast::{ForKind, Item, Stmt, StmtId};
use mellis_lexer::TokenKind;

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

    fn parse_for_stmt(&mut self) -> Result<StmtId, ()> {
        self.consume(TokenKind::KwFor, "Expected 'for'")?;

        if self.match_token(TokenKind::At) {
            // Macro-expanded for loop handling (omitted or stubbed for basic parsing)
            self.error_at_current(
                "Macro for loops are not fully implemented",
                self.previous().span,
            );
            return Err(());
        }

        self.consume(TokenKind::LParen, "Expected '(' after 'for'")?;

        let kind;
        let mut init = None;
        let mut cond = None;
        let mut step = None;
        let mut pattern = None;
        let mut iterable = None;
        let mut binding_name = None;

        if self.check(TokenKind::KwDec)
            || self.check(TokenKind::KwConst)
            || self.check(TokenKind::Semi)
        {
            kind = ForKind::CStyle;
            if !self.check(TokenKind::Semi) {
                init = Some(self.parse_item()?);
            } else {
                self.advance();
            }
            if !self.check(TokenKind::Semi) {
                cond = Some(self.parse_expression(false)?);
            }
            self.consume(TokenKind::Semi, "Expected ';' after for condition")?;

            if !self.check(TokenKind::RParen) {
                step = Some(self.parse_expression(false)?);
            }
            self.consume(TokenKind::RParen, "Expected ')' after for update")?;
        } else {
            kind = ForKind::ForEach;
            pattern = Some(self.parse_pattern()?);
            // Binding name fallback
            // To properly fallback we would need to inspect pattern. Doing dummy for now.
            binding_name = Some(self.previous().span);

            self.consume(TokenKind::KwIn, "Expected 'in' after loop pattern")?;
            iterable = Some(self.parse_expression(true)?);
            self.consume(TokenKind::RParen, "Expected ')' after for condition")?;
        }

        let body = self.parse_block_stmt()?;
        Ok(self.arena.alloc_stmt(Stmt::For {
            kind,
            label: None,
            binding_name,
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
