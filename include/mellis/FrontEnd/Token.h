#pragma once
#include <cstdint>
#include <string_view>
#include "mellis/AST/TypeNode.h" // Để lấy BuiltinKind

namespace fl {
    enum class TokenType {
        END_OF_FILE,
        ERROR,
        
        // --- Toán tử và Ký hiệu ---
        PLUS,                // +
        MINUS,               // -
        MULTIPLY,            // *
        DIVIDE,              // /
        MODULO,              // %
        EQUAL,               // =
        LESS_THAN,           // <
        GREATER_THAN,        // >

        EQUAL_EQUAL,         // ==
        LESS_THAN_EQUAL,     // <=
        GREATER_THAN_EQUAL,  // >=
        NOT_EQUAL,           // !=

        PLUS_ASSIGN,         // +=
        MINUS_ASSIGN,        // -=
        STAR_ASSIGN,         // *=
        SLASH_ASSIGN,        // /=
        PERC_ASSIGN,         // %=
        BIT_AND_ASSIGN,      // &=
        BIT_OR_ASSIGN,       // |=
        BIT_XOR_ASSIGN,      // ^=
        LSHIFT_ASSIGN,       // <<=
        RSHIFT_ASSIGN,       // >>=

        LOGICAL_AND,         // &&
        LOGICAL_OR,          // ||
        BANG,                // !
        
        BIT_AND,             // &
        BIT_OR,              // |
        BIT_XOR,             // ^
        BIT_NOT,             // ~
        LSHIFT,              // <<
        RSHIFT,              // >>

        ARROW,               // ->
        PLUS_PLUS,           // ++
        MINUS_MINUS,         // --
        DOT_DOT,             // ..
        DOT_DOT_EQ,          // ..=
        DOT_DOT_DOT,         // ...
        AT_BRACKET,          // @[
        AT,                  // @
        GENERIC_START,       // @<
        QUESTION,            // ?

        // --- Các dấu câu cấu trúc ---
        COLON,               // :
        COLON_COLON,         // ::
        SEMI,                // ;
        COMMA,               // ,
        DOT,                 // .
        L_PAREN,             // (
        R_PAREN,             // )
        L_BRACE,             // {
        R_BRACE,             // }
        L_BRACKET,           // [
        R_BRACKET,           // ]

        // --- Literals ---
        IDENTIFIER,
        INTEGER_LITERAL,
        FLOAT_LITERAL,
        CHAR_LITERAL,
        STRING_LITERAL,
        RAW_STRING_LITERAL,
        BYTE_LITERAL,
        BYTE_STRING_LITERAL,

        // --- Kiểu dữ liệu nguyên thủy ---
        BUILTIN_TYPE,

        // --- Keywords ---
        KW_DEC,
        KW_CONST,
        KW_FN,
        KW_RETURN,
        KW_IF,
        KW_ELSE,
        KW_WHILE,
        KW_FOR,
        KW_IN,
        KW_BREAK,
        KW_CONTINUE,
        KW_MOD,
        KW_EXPORT,
        KW_EXTERN,
        KW_STRUCT,
        KW_ENUM,
        KW_MACRO,
        KW_TRAIT,
        KW_DYN,
        KW_IMPL,
        KW_UNSAFE,
        KW_USE,
        KW_AS,
        KW_MATCH,
        KW_RW,
        KW_TRUE,
        KW_FALSE,
        KW_TYPE,
        KW_SIZEOF,
        KW_ALIGNOF,
        KW_AWAIT,
        KW_ASYNC,
        KW_COMPTIME,
        KW_SELF_VAL, // self
        KW_SELF_TYP, // Self
        
        KW_PRINT, // Giữ lại cho testing hiện tại
    };

    class Token {
    public:
        TokenType type;
        std::string_view text;
        uint32_t byteOffset; // byte offset in the source code
        uint32_t line = 0;   // 1-indexed source line (0 = not tracked)
        uint32_t col  = 0;   // 1-indexed source column (0 = not tracked)
        
        // Payload bổ sung cho Token đặc biệt
        union {
            BuiltinKind builtinKind;
        };
        
        Token() : type(TokenType::ERROR), text(""), byteOffset(0), line(0), col(0), builtinKind(BuiltinKind::Void) {}
    };
}