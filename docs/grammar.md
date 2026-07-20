Notation
"..." : chuỗi kí tự cố định
| : lựa chọn (OR)
::= : Định nghĩa
* : lặp lại 0 - n lần
+ : lặp lại 1 - n lần
() : gom nhóm
? : có thể xuất hiện 1 lần hoặc không

letter     ::= "a" | "b" | ... | "z" | "A" | ... | "Z" | "_"
digit      ::= "0" | "1" | ... | "9"

IDENTIFIER ::= letter (letter | digit)*
// --- TỪ VỰNG & LITERALS (LEXER) ---
dec_digit  ::= "0" | "1" | ... | "9"
hex_digit  ::= dec_digit | "a" | ... | "f" | "A" | ... | "F"
oct_digit  ::= "0" | "1" | ... | "7"
bin_digit  ::= "0" | "1"

dec_digits ::= dec_digit (dec_digit | "_")*
hex_digits ::= hex_digit (hex_digit | "_")*
oct_digits ::= oct_digit (oct_digit | "_")*
bin_digits ::= bin_digit (bin_digit | "_")*

int_suffix   ::= "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
float_suffix ::= "f32" | "f64"

INTEGER_LITERAL ::= ( dec_digits | "0x" hex_digits | "0o" oct_digits | "0b" bin_digits ) int_suffix?
FLOAT_LITERAL   ::= dec_digits "." dec_digits ( ("e" | "E") ("+" | "-")? dec_digits )? float_suffix?

escape_seq      ::= "\" ( "n" | "r" | "t" | "\" | "0" | "'" | '"' | "x" hex_digit hex_digit | "u{" hex_digits "}" )

CHAR_LITERAL    ::= "'" ( bất_kỳ_ký_tự_nào_trừ_nháy_đơn_và_backslash | escape_seq ) "'"
STRING_LITERAL  ::= '"' ( bất_kỳ_ký_tự_nào_trừ_nháy_kép_và_backslash | escape_seq )* '"'
RAW_STRING_LITERAL ::= "r" "#"* '"' (bất_kỳ_ký_tự_nào) '"' "#"* // Số lượng # phải khớp nhau

BYTE_LITERAL    ::= "b" CHAR_LITERAL
BYTE_STRING_LITERAL ::= "b" STRING_LITERAL
type_path_segment  ::= (IDENTIFIER | KW_SELF_TYP) generic_instantiation?
type_path          ::= type_path_segment ("::" type_path_segment)*

value_path_segment ::= (IDENTIFIER | KW_SELF_VAL) generic_instantiation?
value_path         ::= value_path_segment ("::" value_path_segment)*

// --- TỪ KHÓA ---
KW_DEC     ::= "dec"
KW_FN      ::= "fn"
KW_IF      ::= "if"
KW_ELSE    ::= "else"
KW_RETURN  ::= "return"
KW_EXPORT  ::= "export"
KW_EXTERN  ::= "extern"
KW_CONST   ::= "const"
KW_MOD     ::= "mod"
KW_STRUCT  ::= "struct"
KW_ENUM    ::= "enum"
KW_RW      ::= "rw"
KW_UNSAFE  ::= "unsafe"
KW_USE     ::= "use"
KW_AS      ::= "as"
KW_TRAIT   ::= "trait"
KW_IMPL    ::= "impl"
KW_FOR     ::= "for"
KW_MATCH   ::= "match"
KW_WHILE   ::= "while"
KW_BREAK   ::= "break"
KW_CONTINUE::= "continue"
KW_IN      ::= "in"
KW_TRUE    ::= "true"
KW_FALSE   ::= "false"
KW_TYPE    ::= "type"
KW_SIZEOF  ::= "sizeof"
KW_ALIGNOF ::= "alignof"
KW_AWAIT   ::= "await"
KW_ASYNC   ::= "async"
KW_COMPTIME::= "comptime"
KW_MACRO   ::= "macro"
KW_DYN     ::= "dyn"
KW_SELF_VAL::= "self"
KW_SELF_TYP::= "Self"

// --- TOÁN TỬ VÀ KÝ HIỆU ---
PLUS       ::= "+"
MINUS      ::= "-"
STAR       ::= "*"
SLASH      ::= "/"
AMPERSAND  ::= "&"
ARROW      ::= "->"
QUESTION   ::= "?"
PLUS_PLUS  ::= "++"
MINUS_MINUS::= "--"
UNDERSCORE ::= "_"
LOGICAL_AND::= "&&"
LOGICAL_OR ::= "||"
BANG       ::= "!"
LESS_EQ    ::= "<="
GREATER_EQ ::= ">="
GENERIC_START::= "@<"
PERCENT    ::= "%"
BIT_OR     ::= "|"
BIT_XOR    ::= "^"
BIT_NOT    ::= "~"
LSHIFT     ::= "<<"
RSHIFT     ::= ">>"
DOT_DOT    ::= ".."
DOT_DOT_EQ ::= "..="
DOT_DOT_DOT::= "..."
AT_BRACKET ::= "@["
DOLLAR     ::= "$"

// Toán tử gán & so sánh
ASSIGN       ::= "="
PLUS_ASSIGN  ::= "+="
MINUS_ASSIGN ::= "-="
STAR_ASSIGN  ::= "*="
SLASH_ASSIGN ::= "/="
PERC_ASSIGN  ::= "%="
BIT_AND_ASSIGN ::= "&="
BIT_OR_ASSIGN  ::= "|="
BIT_XOR_ASSIGN ::= "^="
LSHIFT_ASSIGN  ::= "<<="
RSHIFT_ASSIGN  ::= ">>="

assign_op ::= ASSIGN | PLUS_ASSIGN | MINUS_ASSIGN | STAR_ASSIGN | SLASH_ASSIGN 
            | PERC_ASSIGN | BIT_AND_ASSIGN | BIT_OR_ASSIGN | BIT_XOR_ASSIGN 
            | LSHIFT_ASSIGN | RSHIFT_ASSIGN
EQ_EQ      ::= "=="
BANG_EQ    ::= "!="
LESS       ::= "<"
GREATER    ::= ">"

// Dấu câu cấu trúc
COLON      ::= ":"
SEMI       ::= ";"
COMMA      ::= ","
L_PAREN    ::= "("
R_PAREN    ::= ")"
L_BRACE    ::= "{"
R_BRACE    ::= "}"
L_BRACKET  ::= "["
R_BRACKET  ::= "]"
DOT        ::= "."

WHITESPACE    ::= " " | "\t" | "\n" | "\r"
LINE_COMMENT  ::= "//" (bất_kỳ_ký_tự_nào_trừ_dấu_xuống_dòng)* "\n"
BLOCK_COMMENT ::= "/*" (bất_kỳ_ký_tự_nào_bao_gồm_cả_xuống_dòng)* "*/"
COMMENT       ::= LINE_COMMENT | BLOCK_COMMENT
EOF        ::= End of File (Token đặc biệt do Lexer sinh ra)

// --- ANNOTATIONS ---
annotation  ::= AT_BRACKET IDENTIFIER ("(" argument_list ")")? "]"

// --- CẤU TRÚC CHƯƠNG TRÌNH ---
program     ::= declaration* EOF

declaration ::= use_decl
              | var_decl 
              | func_decl 
              | struct_decl
              | trait_decl
              | impl_decl
              | enum_decl
              | statement
              | extern_decl
              | mod_decl
              | type_alias_decl
              | macro_decl

use_decl      ::= "export"? KW_USE use_tree ";"

use_tree      ::= use_path (KW_AS IDENTIFIER)?
                | (use_path "::")? "*"
                | (use_path "::")? "{" use_tree_list? "}"

use_path      ::= IDENTIFIER ("::" IDENTIFIER)*

use_tree_list ::= use_tree ("," use_tree)* ","?

mod_decl      ::= annotation* "export"? KW_MOD IDENTIFIER "{" declaration* "}"

extern_decl ::= "extern" KW_FN IDENTIFIER "(" parameters? ")" ("->" type)? ";"

type_alias_decl ::= "export"? KW_TYPE IDENTIFIER generic_params? "=" type ";"

// --- GENERICS ---
generic_params ::= "<" generic_param ("," generic_param)* ">"
generic_param  ::= IDENTIFIER (":" type_path ("+" type_path)* )?
generic_instantiation ::= GENERIC_START type ("," type)* ">"

// --- ENUM ---
enum_decl ::= annotation* "export"? KW_ENUM IDENTIFIER generic_params? "{" enum_variant_list "}"
enum_variant_list ::= enum_variant ("," enum_variant)* ","?
enum_variant ::= IDENTIFIER ("(" parameters ")")?

// --- STRUCT, TRAIT VÀ IMPL ---
struct_decl ::= annotation* "export"? KW_STRUCT IDENTIFIER generic_params? "{" struct_field* "}"

struct_field::= IDENTIFIER ":" type ";"

trait_decl  ::= annotation* "export"? KW_TRAIT IDENTIFIER generic_params? "{" trait_method* "}"

trait_method::= KW_FN IDENTIFIER "(" parameters? ")" ("->" type)? ";"

impl_decl   ::= inherent_impl | full_trait_impl

inherent_impl ::= "impl" generic_params? type_path "{" func_decl* "}"

full_trait_impl ::= "impl" generic_params? type_path "for" type_path "{" func_decl* "}"

// --- KHAI BÁO BIẾN VÀ HÀM ---
var_decl_core ::= "export"? ("dec" | "const") IDENTIFIER (":" type)? "=" expression
var_decl    ::= var_decl_core ";"

func_decl   ::= annotation* "export"? KW_COMPTIME? KW_ASYNC? KW_FN IDENTIFIER generic_params? "(" parameters? ")" ("->" type)? block_stmt

regular_param  ::= IDENTIFIER ":" type
variadic_param ::= IDENTIFIER ":" DOT_DOT_DOT type
receiver_param ::= KW_SELF_VAL ":" type

parameters     ::= receiver_param ("," regular_param)* ("," variadic_param)?
                 | receiver_param ("," variadic_param)?
                 | regular_param ("," regular_param)* ("," variadic_param)?
                 | variadic_param

type           ::= reference_type 
                 | pointer_type 
                 | array_type 
                 | tuple_type 
                 | func_type 
                 | never_type
                 | builtin_type
                 | named_type
                 | trait_object_type

builtin_type ::= BUILTIN_TYPE 

BUILTIN_TYPE ::= "int_8" | "int_16" | "int_32" | "int_64" | "int_128" 
                 | "uint_8" | "uint_16" | "uint_32" | "uint_64" | "uint_128"
                 | "float_32" | "float_64"
                 | "bool" | "char" | "string" | "void"

trait_object_type ::= KW_DYN type_path

reference_type ::= "&" KW_RW? type
pointer_type   ::= "*" KW_RW? type
array_type     ::= "[" type ("," const_expression)? "]"
tuple_type     ::= "(" (type ("," type)* ","?)? ")"
func_type      ::= KW_FN "(" (type ("," type)* ","?)? ")" "->" type
never_type     ::= BANG
named_type     ::= type_path

// --- LỆNH VÀ BIỂU THỨC ---
statement   ::= expr_stmt 
              | block_stmt 
              | if_stmt 
              | while_stmt 
              | for_stmt
              | return_stmt
              | break_stmt
              | continue_stmt
              | unsafe_stmt
              | comptime_stmt
              | macro_call_stmt
              | macro_expand_for

comptime_stmt ::= KW_COMPTIME block_stmt

for_each_stmt ::= KW_FOR "(" IDENTIFIER KW_IN expression ")" block_stmt
for_init      ::= var_decl_core | assignment
c_for_stmt    ::= "for" "(" for_init? ";" expression? ";" expression? ")" block_stmt
for_stmt      ::= for_each_stmt | c_for_stmt

expr_stmt   ::= expression ";"

block_stmt  ::= "{" declaration* "}"

if_stmt     ::= "if" expression block_stmt ("else" (block_stmt | if_stmt))?

while_stmt  ::= "while" expression block_stmt

return_stmt ::= "return" expression? ";"
break_stmt  ::= "break" ";"
continue_stmt ::= "continue" ";"

unsafe_stmt ::= "unsafe" block_stmt

expression  ::= assignment
const_expression ::= expression // Bắt buộc là hằng số tại compile-time

assignment  ::= lvalue assign_op expression 
              | range_expr

lvalue      ::= "*"* value_path ("[" expression "]" | "." IDENTIFIER)*

range_expr  ::= logical_or ((".." | "..=") logical_or?)? 
              | (".." | "..=") logical_or?

logical_or  ::= logical_and ("||" logical_and)*
logical_and ::= bitwise_or ("&&" bitwise_or)*
bitwise_or  ::= bitwise_xor ("|" bitwise_xor)*
bitwise_xor ::= bitwise_and ("^" bitwise_and)*
bitwise_and ::= equality ("&" equality)*
equality    ::= comparison (("==" | "!=") comparison)*
comparison  ::= shift (("<" | "<=" | ">" | ">=") shift)*
shift       ::= term (("<<" | ">>") term)*
term        ::= factor (("+" | "-") factor)*
factor      ::= cast (("*" | "/" | "%") cast)*
cast        ::= unary (KW_AS type)*

unary       ::= "-" unary 
              | "*" unary
              | "!" unary
              | "~" unary
              | "&" KW_RW? unary
              | primary

postfix_op  ::= "[" expression "]" 
              | "." IDENTIFIER 
              | "?"
              | "++"
              | "--"
              | "(" argument_list? ")"
              | "." KW_AWAIT

primary     ::= base_primary postfix_op*

// Thêm match_expr vào base_primary
base_primary::= INTEGER_LITERAL 
              | FLOAT_LITERAL 
              | CHAR_LITERAL
              | STRING_LITERAL
              | RAW_STRING_LITERAL
              | BYTE_LITERAL
              | BYTE_STRING_LITERAL
              | KW_TRUE 
              | KW_FALSE 
              | KW_SELF_VAL
              | array_literal
              | tuple_literal
              | struct_literal
              | unit_literal
              | lambda_expr
              | sizeof_expr
              | alignof_expr
              | value_path 
              | match_expr
              | macro_call_expr
              | "(" expression ")"

// --- MATCH STATEMENT ---
match_expr  ::= "match" expression "{" match_arm* "}"

match_arm   ::= pattern "->" (expression | block_stmt) ","?

enum_destruct ::= "(" IDENTIFIER ("," IDENTIFIER)* ")"

pattern     ::= INTEGER_LITERAL 
              | FLOAT_LITERAL 
              | CHAR_LITERAL
              | STRING_LITERAL
              | RAW_STRING_LITERAL
              | KW_TRUE 
              | KW_FALSE 
              | "_"
              | tuple_pattern
              | unit_literal
              | type_path enum_destruct?

tuple_pattern ::= "(" pattern "," (pattern ("," pattern)* ","?)? ")"

// --- MẢNG, TUPLE VÀ GỌI HÀM ---
array_literal ::= "[" arguments? "]"

tuple_literal ::= "(" expression "," (expression ("," expression)* ","?)? ")"

unit_literal  ::= "(" ")"

// --- STRUCT LITERAL, LAMBDA, SIZEOF ---
struct_literal ::= type_path "{" (IDENTIFIER ":" expression ("," IDENTIFIER ":" expression)* ","?)? "}"
lambda_expr    ::= "|" (IDENTIFIER (":" type)? ("," IDENTIFIER (":" type)?)* ","?)? "|" ("->" type)? (expression | block_stmt)
sizeof_expr    ::= KW_SIZEOF "(" type ")"
alignof_expr   ::= KW_ALIGNOF "(" type ")"

// Hỗ trợ truyền tham số có tên (Named Arguments)
argument_list ::= argument ("," argument)*

argument    ::= (IDENTIFIER ":")? expression

// Giữ lại arguments cho array_literal
arguments   ::= expression ("," expression)*
// --- MACRO DEFINITION ---
macro_decl ::= annotation* "export"? KW_MACRO IDENTIFIER "(" macro_params? ")" block_stmt

macro_params ::= macro_param ("," macro_param)* ("," macro_variadic_param)?
               | macro_variadic_param

macro_param ::= DOLLAR IDENTIFIER ":" macro_frag_spec

macro_variadic_param ::= DOLLAR IDENTIFIER ":" macro_frag_spec DOT_DOT_DOT

macro_frag_spec ::= "expr" | "ident" | "ty" | "stmt" | "block"

// --- MACRO INVOCATION ---
macro_call_expr ::= IDENTIFIER BANG "(" macro_call_args? ")"
                  | IDENTIFIER BANG "[" macro_call_args? "]"

macro_call_stmt ::= IDENTIFIER BANG "(" macro_call_args? ")" ";"
                  | IDENTIFIER BANG "[" macro_call_args? "]" ";"
                  | IDENTIFIER BANG "{" macro_call_args? "}"

macro_call_args ::= macro_call_arg ("," macro_call_arg)* ","?

macro_call_arg  ::= expression 
                  | type 
                  | block_stmt
                  | IDENTIFIER

// --- MACRO EXPANSION LOOP ---
macro_expand_for ::= DOLLAR KW_FOR DOLLAR IDENTIFIER KW_IN DOLLAR IDENTIFIER block_stmt
