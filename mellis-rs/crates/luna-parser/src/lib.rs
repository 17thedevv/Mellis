use luna_ast::{AstArena, DeclId, ExprId, PatId, StmtId, TypeId};
use luna_common::{Diagnostic, Span};
use luna_lexer::{Lexer, Token, TokenKind};

pub mod decl;
pub mod expr;
pub mod pat;
pub mod stmt;
pub mod ty;

pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    pub source: &'a str, pub source_manager: Option<&'a luna_common::source::SourceManager>,
    pub arena: &'a mut AstArena,
    pub diagnostics: Vec<Diagnostic>,
    pub file_id: luna_common::ids::FileId,
}

impl<'a> Parser<'a> {
    pub fn new(
        lexer: Lexer<'a>,
        arena: &'a mut AstArena,
        file_id: luna_common::ids::FileId,
    ) -> Self {
        let source = lexer.source();
        let mut tokens = Vec::new();
        let mut diagnostics = Vec::new();

        for token in lexer {
            if token.kind == TokenKind::Error {
                let text = &source[token.span.start as usize..token.span.end as usize];
                if text == "#" {
                    diagnostics
                        .push(Diagnostic::error("unexpected '#'; expected '#[' for an annotation").with_span(token.span));
                } else {
                    diagnostics
                        .push(Diagnostic::error("Invalid token encountered").with_span(token.span));
                }
            } else {
                tokens.push(token);
            }
        }

        // Push an EOF token to make parsing easier
        if let Some(last) = tokens.last() {
            if last.kind != TokenKind::Eof {
                tokens.push(Token::new(
                    TokenKind::Eof,
                    Span::new(file_id, last.span.end, last.span.end),
                ));
            }
        } else {
            tokens.push(Token::new(
                TokenKind::Eof,
                Span::new(file_id, 0, 0),
            ));
        }

        Self {
            tokens,
            pos: 0,
            source_manager: None,
            source,
            arena,
            diagnostics,
            file_id,
        }
    }

    pub fn from_tokens(
        mut tokens: Vec<Token>,
        source: &'a str,
        source_manager: Option<&'a luna_common::source::SourceManager>,
        arena: &'a mut AstArena,
        file_id: luna_common::ids::FileId,
    ) -> Self {
        if let Some(last) = tokens.last() {
            if last.kind != TokenKind::Eof {
                tokens.push(Token::new(
                    TokenKind::Eof,
                    Span::new(file_id, last.span.end, last.span.end),
                ));
            }
        } else {
            tokens.push(Token::new(
                TokenKind::Eof,
                Span::new(file_id, 0, 0),
            ));
        }

        Self {
            tokens,
            pos: 0,
            source_manager,
            source,
            arena,
            diagnostics: Vec::new(),
            file_id,
        }
    }

    pub fn capture_balanced_tokens(
        &mut self,
        open_kind: TokenKind,
        close_kind: TokenKind,
    ) -> Result<Vec<Token>, ()> {
        let mut depth = 1;
        let mut captured = Vec::new();
        while self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos];
            self.pos += 1;
            if tok.kind == open_kind {
                depth += 1;
                captured.push(tok);
            } else if tok.kind == close_kind {
                depth -= 1;
                if depth == 0 {
                    return Ok(captured);
                }
                captured.push(tok);
            } else if tok.kind == TokenKind::Eof {
                let span = tok.span;
                self.error_at_current(
                    &format!("Unexpected EOF, expected closing '{:?}'", close_kind),
                    span,
                );
                return Err(());
            } else {
                captured.push(tok);
            }
        }
        Err(())
    }

    pub fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn peek(&self) -> Token {
        self.tokens[self.pos]
    }

    pub fn peek_next(&self) -> Token {
        if self.pos + 1 < self.tokens.len() {
            self.tokens[self.pos + 1]
        } else {
            self.tokens[self.tokens.len() - 1]
        }
    }

    pub fn previous(&self) -> Token {
        self.tokens[self.pos - 1]
    }

    pub fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        self.previous()
    }

    pub fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().kind == kind
        }
    }

    pub fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn consume(&mut self, kind: TokenKind, message: &str) -> Result<Token, ()> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            let span = self.peek().span;
            self.error_at_current(message, span);
            Err(())
        }
    }

    pub fn error_at_current(&mut self, message: &str, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(message).with_span(span));
    }

    pub fn parse_file(&mut self) -> Result<Vec<luna_ast::Item>, ()> {
        let mut items = Vec::new();
        while !self.is_at_end() {
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(_) => {
                    self.synchronize();
                }
            }
        }
        Ok(items)
    }

    pub fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().kind == TokenKind::Semi {
                return;
            }

            match self.peek().kind {
                TokenKind::KwFn
                | TokenKind::KwStruct
                | TokenKind::KwEnum
                | TokenKind::KwTrait
                | TokenKind::KwImpl
                | TokenKind::KwFor
                | TokenKind::KwIf
                | TokenKind::KwWhile
                | TokenKind::KwReturn => {
                    return;
                }
                _ => {}
            }

            self.advance();
        }
    }

    pub fn is_at_generic_close(&self) -> bool {
        self.check(TokenKind::GreaterThan) || self.check(TokenKind::RShift)
    }

    pub fn consume_greater_than(&mut self, err_msg: &'static str) -> Result<(), ()> {
        if self.check(TokenKind::RShift) {
            let prev_token = self.advance();
            let span1 = luna_common::ids::Span::new(prev_token.span.file_id, prev_token.span.start, prev_token.span.start + 1);
            let span2 = luna_common::ids::Span::new(prev_token.span.file_id, prev_token.span.start + 1, prev_token.span.end);
            self.tokens[self.pos - 1] = luna_lexer::Token::new(TokenKind::GreaterThan, span1);
            self.tokens.insert(self.pos, luna_lexer::Token::new(TokenKind::GreaterThan, span2));
            Ok(())
        } else {
            self.consume(TokenKind::GreaterThan, err_msg)?;
            Ok(())
        }
    }
}
