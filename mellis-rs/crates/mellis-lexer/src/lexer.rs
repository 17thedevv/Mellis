use crate::token::{BuiltinKind, Token, TokenKind};
use mellis_common::ids::{FileId, Span};

pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    file_id: FileId,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str, file_id: FileId) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            file_id,
            pos: 0,
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() {
            0
        } else {
            self.bytes[self.pos]
        }
    }

    fn peek_next(&self) -> u8 {
        if self.pos + 1 >= self.bytes.len() {
            0
        } else {
            self.bytes[self.pos + 1]
        }
    }

    fn advance(&mut self) -> u8 {
        if self.is_at_end() {
            return 0;
        }
        let c = self.bytes[self.pos];
        self.pos += 1;
        c
    }

    fn match_char(&mut self, expected: u8) -> bool {
        if self.is_at_end() || self.bytes[self.pos] != expected {
            false
        } else {
            self.pos += 1;
            true
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let c = self.peek();
            match c {
                b' ' | b'\r' | b'\t' | b'\n' => {
                    self.advance();
                }
                b'/' => {
                    if self.peek_next() == b'/' {
                        while self.peek() != b'\n' && !self.is_at_end() {
                            self.advance();
                        }
                    } else if self.peek_next() == b'*' {
                        self.advance();
                        self.advance();
                        while !self.is_at_end() {
                            if self.peek() == b'*' && self.peek_next() == b'/' {
                                self.advance();
                                self.advance();
                                break;
                            }
                            self.advance();
                        }
                    } else {
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    fn make_token(&self, kind: TokenKind, start_offset: usize) -> Token {
        Token::new(
            kind,
            Span {
                file_id: self.file_id,
                start: start_offset as u32,
                end: self.pos as u32,
            },
        )
    }

    fn error_token(&self, start_offset: usize) -> Token {
        Token::new(
            TokenKind::Error,
            Span {
                file_id: self.file_id,
                start: start_offset as u32,
                end: self.pos as u32,
            },
        )
    }

    fn identifier_or_keyword(&mut self, start_offset: usize) -> Token {
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }

        let text = &self.source[start_offset..self.pos];
        let kind = match text {
            "dec" => TokenKind::KwDec,
            "const" => TokenKind::KwConst,
            "fn" => TokenKind::KwFn,
            "return" => TokenKind::KwReturn,
            "if" => TokenKind::KwIf,
            "else" => TokenKind::KwElse,
            "while" => TokenKind::KwWhile,
            "for" => TokenKind::KwFor,
            "in" => TokenKind::KwIn,
            "break" => TokenKind::KwBreak,
            "continue" => TokenKind::KwContinue,
            "mod" => TokenKind::KwMod,
            "export" => TokenKind::KwExport,
            "extern" => TokenKind::KwExtern,
            "intrinsic" => TokenKind::KwIntrinsic,
            "struct" => TokenKind::KwStruct,
            "enum" => TokenKind::KwEnum,
            "macro" => TokenKind::KwMacro,
            "trait" => TokenKind::KwTrait,
            "impl" => TokenKind::KwImpl,
            "unsafe" => TokenKind::KwUnsafe,
            "use" => TokenKind::KwUse,
            "as" => TokenKind::KwAs,
            "move" => TokenKind::KwMove,
            "match" => TokenKind::KwMatch,
            "rw" => TokenKind::KwRw,
            "mut" => TokenKind::KwMut,
            "true" => TokenKind::KwTrue,
            "false" => TokenKind::KwFalse,
            "type" => TokenKind::KwType,
            "sizeof" => TokenKind::KwSizeof,
            "alignof" => TokenKind::KwAlignof,
            "typeof" => TokenKind::KwTypeof,
            "await" => TokenKind::KwAwait,
            "async" => TokenKind::KwAsync,
            "comptime" => TokenKind::KwComptime,
            "dyn" => TokenKind::KwDyn,
            "self" => TokenKind::KwSelfVal,
            "Self" => TokenKind::KwSelfTyp,

            "i4" => TokenKind::BuiltinType(BuiltinKind::I4),
            "i8" => TokenKind::BuiltinType(BuiltinKind::I8),
            "i16" => TokenKind::BuiltinType(BuiltinKind::I16),
            "i32" => TokenKind::BuiltinType(BuiltinKind::I32),
            "i64" => TokenKind::BuiltinType(BuiltinKind::I64),
            "i128" => TokenKind::BuiltinType(BuiltinKind::I128),
            "u4" => TokenKind::BuiltinType(BuiltinKind::U4),
            "u8" => TokenKind::BuiltinType(BuiltinKind::U8),
            "u16" => TokenKind::BuiltinType(BuiltinKind::U16),
            "u32" => TokenKind::BuiltinType(BuiltinKind::U32),
            "u64" => TokenKind::BuiltinType(BuiltinKind::U64),
            "u128" => TokenKind::BuiltinType(BuiltinKind::U128),
            "f32" => TokenKind::BuiltinType(BuiltinKind::F32),
            "f64" => TokenKind::BuiltinType(BuiltinKind::F64),
            "bool" => TokenKind::BuiltinType(BuiltinKind::Bool),
            "char" => TokenKind::BuiltinType(BuiltinKind::Char),
            "str" => TokenKind::BuiltinType(BuiltinKind::Str),

            _ => TokenKind::Identifier,
        };

        self.make_token(kind, start_offset)
    }

    fn number_literal(&mut self, start_offset: usize) -> Token {
        if self.peek() == b'0' {
            let next = self.peek_next();
            if next == b'x'
                || next == b'X'
                || next == b'o'
                || next == b'O'
                || next == b'b'
                || next == b'B'
            {
                self.advance();
                self.advance();
                while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
                    self.advance();
                }
                return self.make_token(TokenKind::IntegerLiteral, start_offset);
            }
        }

        while self.peek().is_ascii_digit() || self.peek() == b'_' {
            self.advance();
        }

        let mut is_float = false;

        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            is_float = true;
            self.advance();
            while self.peek().is_ascii_digit() || self.peek() == b'_' {
                self.advance();
            }
        }

        if self.peek() == b'e' || self.peek() == b'E' {
            is_float = true;
            self.advance();
            if self.peek() == b'+' || self.peek() == b'-' {
                self.advance();
            }
            while self.peek().is_ascii_digit() || self.peek() == b'_' {
                self.advance();
            }
        }

        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.advance();
        }

        let kind = if is_float {
            TokenKind::FloatLiteral
        } else {
            TokenKind::IntegerLiteral
        };
        self.make_token(kind, start_offset)
    }

    fn char_literal(&mut self, start_offset: usize, is_byte: bool) -> Token {
        if is_byte {
            self.advance();
        }
        self.advance(); // '\''

        while self.peek() != b'\'' && !self.is_at_end() {
            if self.peek() == b'\\' {
                self.advance();
            }
            self.advance();
        }

        if self.is_at_end() {
            return self.error_token(start_offset);
        }

        self.advance(); // '\''
        let kind = if is_byte {
            TokenKind::ByteLiteral
        } else {
            TokenKind::CharLiteral
        };
        self.make_token(kind, start_offset)
    }

    fn string_literal(&mut self, start_offset: usize, is_raw: bool, is_byte: bool) -> Token {
        if is_byte {
            self.advance();
        }
        if is_raw {
            self.advance();
        }

        let mut pound_count = 0;
        if is_raw {
            while self.peek() == b'#' {
                pound_count += 1;
                self.advance();
            }
        }

        self.advance(); // '"'

        while !self.is_at_end() {
            if !is_raw && self.peek() == b'"' {
                break;
            } else if !is_raw && self.peek() == b'\\' {
                self.advance();
                self.advance();
                continue;
            } else if is_raw && self.peek() == b'"' {
                let mut match_pounds = true;
                for lookahead in 1..=pound_count {
                    if self.pos + lookahead >= self.bytes.len()
                        || self.bytes[self.pos + lookahead] != b'#'
                    {
                        match_pounds = false;
                        break;
                    }
                }
                if match_pounds {
                    break;
                }
            }
            self.advance();
        }

        if self.is_at_end() {
            return self.error_token(start_offset);
        }

        self.advance();
        if is_raw {
            for _ in 0..pound_count {
                self.advance();
            }
        }

        let mut kind = TokenKind::StringLiteral;
        if is_raw {
            kind = TokenKind::RawStringLiteral;
        }
        if is_byte && !is_raw {
            kind = TokenKind::ByteStringLiteral;
        }

        self.make_token(kind, start_offset)
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace_and_comments();

        let start_offset = self.pos;
        if self.is_at_end() {
            // Để Lexer dừng lại hẳn sau khi báo Eof lần 1, ta có thể lưu 1 cờ.
            // Nhưng hiện tại Iterator trả về None khi hết.
            return None;
        }

        let c = self.advance();

        if c.is_ascii_alphabetic() || c == b'_' {
            if c == b'r' && (self.peek() == b'"' || self.peek() == b'#') {
                self.pos -= 1;
                return Some(self.string_literal(start_offset, true, false));
            }
            if c == b'b' {
                if self.peek() == b'\'' {
                    self.pos -= 1;
                    return Some(self.char_literal(start_offset, true));
                } else if self.peek() == b'"' {
                    self.pos -= 1;
                    return Some(self.string_literal(start_offset, false, true));
                }
            }
            self.pos -= 1;
            return Some(self.identifier_or_keyword(start_offset));
        }

        if c.is_ascii_digit() {
            self.pos -= 1;
            return Some(self.number_literal(start_offset));
        }

        let kind = match c {
            b'+' => {
                if self.match_char(b'=') {
                    TokenKind::PlusAssign
                } else if self.match_char(b'+') {
                    TokenKind::PlusPlus
                } else {
                    TokenKind::Plus
                }
            }
            b'-' => {
                if self.match_char(b'=') {
                    TokenKind::MinusAssign
                } else if self.match_char(b'-') {
                    TokenKind::MinusMinus
                } else if self.match_char(b'>') {
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            b'*' => {
                if self.match_char(b'=') {
                    TokenKind::StarAssign
                } else {
                    TokenKind::Multiply
                }
            }
            b'/' => {
                if self.match_char(b'=') {
                    TokenKind::SlashAssign
                } else {
                    TokenKind::Divide
                }
            }
            b'%' => {
                if self.match_char(b'=') {
                    TokenKind::PercAssign
                } else {
                    TokenKind::Modulo
                }
            }
            b'=' => {
                if self.match_char(b'=') {
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                }
            }
            b'<' => {
                if self.match_char(b'=') {
                    TokenKind::LessThanEqual
                } else if self.match_char(b'<') {
                    if self.match_char(b'=') {
                        TokenKind::LShiftAssign
                    } else {
                        TokenKind::LShift
                    }
                } else {
                    TokenKind::LessThan
                }
            }
            b'>' => {
                if self.match_char(b'=') {
                    TokenKind::GreaterThanEqual
                } else if self.match_char(b'>') {
                    if self.match_char(b'=') {
                        TokenKind::RShiftAssign
                    } else {
                        TokenKind::RShift
                    }
                } else {
                    TokenKind::GreaterThan
                }
            }
            b'!' => {
                if self.match_char(b'=') {
                    TokenKind::NotEqual
                } else {
                    TokenKind::Bang
                }
            }
            b'&' => {
                if self.match_char(b'=') {
                    TokenKind::BitAndAssign
                } else if self.match_char(b'&') {
                    TokenKind::LogicalAnd
                } else {
                    TokenKind::BitAnd
                }
            }
            b'|' => {
                if self.match_char(b'=') {
                    TokenKind::BitOrAssign
                } else if self.match_char(b'|') {
                    TokenKind::LogicalOr
                } else {
                    TokenKind::BitOr
                }
            }
            b'^' => {
                if self.match_char(b'=') {
                    TokenKind::BitXorAssign
                } else {
                    TokenKind::BitXor
                }
            }
            b'~' => TokenKind::BitNot,
            b'?' => TokenKind::Question,
            b':' => {
                if self.match_char(b':') {
                    TokenKind::ColonColon
                } else {
                    TokenKind::Colon
                }
            }
            b'.' => {
                if self.match_char(b'.') {
                    if self.match_char(b'.') {
                        TokenKind::DotDotDot
                    } else if self.match_char(b'=') {
                        TokenKind::DotDotEq
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }
            b'@' => {
                if self.match_char(b'[') {
                    TokenKind::AtBracket
                } else if self.match_char(b'<') {
                    TokenKind::GenericStart
                } else {
                    TokenKind::At
                }
            }
            b';' => TokenKind::Semi,
            b',' => TokenKind::Comma,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b'\'' => {
                let next_c = self.peek();
                if next_c.is_ascii_alphabetic() || next_c == b'_' {
                    self.advance();
                    while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
                        self.advance();
                    }
                    if self.peek() != b'\'' {
                        return Some(self.make_token(TokenKind::Lifetime, start_offset));
                    }
                }
                self.pos = start_offset;
                return Some(self.char_literal(start_offset, false));
            }
            b'"' => {
                self.pos -= 1;
                return Some(self.string_literal(start_offset, false, false));
            }
            _ => {
                return Some(self.error_token(start_offset));
            }
        };

        Some(self.make_token(kind, start_offset))
    }
}
