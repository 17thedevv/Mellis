use mellis_common::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKind {
    I4,
    I8,
    I16,
    I32,
    I64,
    I128,
    U4,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Bool,
    Char,
    Str,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Eof,
    Error,

    // Operators
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    Equal,
    LessThan,
    GreaterThan,
    EqualEqual,
    LessThanEqual,
    GreaterThanEqual,
    NotEqual,

    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    LShiftAssign,
    RShiftAssign,

    LogicalAnd,
    LogicalOr,
    Bang,
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    LShift,
    RShift,

    Arrow,
    PlusPlus,
    MinusMinus,
    DotDot,
    DotDotEq,
    DotDotDot,
    AtBracket,
    At,
    GenericStart,
    Question,

    // Punctuation
    Colon,
    ColonColon,
    Semi,
    Comma,
    Dot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // Literals & Identifiers
    Identifier,
    Lifetime,
    IntegerLiteral,
    FloatLiteral,
    CharLiteral,
    StringLiteral,
    RawStringLiteral,
    ByteLiteral,
    ByteStringLiteral,

    BuiltinType(BuiltinKind),

    // Keywords
    KwDec,
    KwConst,
    KwFn,
    KwReturn,
    KwIf,
    KwElse,
    KwWhile,
    KwFor,
    KwIn,
    KwBreak,
    KwContinue,
    KwMod,
    KwExport,
    KwExtern,
    KwIntrinsic,
    KwStruct,
    KwEnum,
    KwMacro,
    KwTrait,
    KwDyn,
    KwImpl,
    KwUnsafe,
    KwUse,
    KwAs,
    KwMove,
    KwMatch,
    KwRw,
    KwMut,
    KwTrue,
    KwFalse,
    KwType,
    KwSizeof,
    KwAlignof,
    KwTypeof,
    KwAwait,
    KwAsync,
    KwComptime,
    KwPrint,
    KwSelfVal,
    KwSelfTyp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
