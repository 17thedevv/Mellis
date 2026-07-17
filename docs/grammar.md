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
NUMBER     ::= digit+ ("." digit+)?
STRING     ::= '"' (bất_kỳ_ký_tự_nào_trừ_dấu_ngoặc_kép)* '"'
path_segment ::= IDENTIFIER generic_instantiation?
path         ::= path_segment ("::" path_segment)*

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

// Toán tử gán & so sánh
ASSIGN     ::= "="
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

WHITESPACE ::= " " | "\t" | "\n" | "\r"
COMMENT    ::= "//" (bất_kỳ_ký_tự_nào_trừ_dấu_xuống_dòng)* "\n"
EOF        ::= End of File (Token đặc biệt do Lexer sinh ra)

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

use_decl      ::= "export"? KW_USE use_tree ";"

use_tree      ::= use_path (KW_AS IDENTIFIER)?
                | (use_path "::")? "*"
                | (use_path "::")? "{" use_tree_list? "}"

use_path      ::= IDENTIFIER ("::" IDENTIFIER)*

use_tree_list ::= use_tree ("," use_tree)* ","?

mod_decl      ::= "export"? "mod" IDENTIFIER "{" declaration* "}"

extern_decl ::= "extern" "fn" IDENTIFIER "(" parameters? ")" ("->" type)? ";"

// --- GENERICS ---
generic_params ::= "<" generic_param ("," generic_param)* ">"
generic_param  ::= IDENTIFIER (":" path ("+" path)* )?
generic_instantiation ::= GENERIC_START type ("," type)* ">"

// --- ENUM ---
enum_decl ::= "export"? "enum" IDENTIFIER generic_params? "{" enum_variant_list "}"
enum_variant_list ::= enum_variant ("," enum_variant)* ","?
enum_variant ::= IDENTIFIER ("(" parameters ")")?

// --- STRUCT, TRAIT VÀ IMPL ---
struct_decl ::= "export"? "struct" IDENTIFIER generic_params? "{" struct_field* "}"

struct_field::= IDENTIFIER ":" type ";"

trait_decl  ::= "export"? "trait" IDENTIFIER generic_params? "{" trait_method* "}"

trait_method::= "fn" IDENTIFIER "(" parameters? ")" ("->" type)? ";"

impl_decl   ::= inherent_impl | full_trait_impl

inherent_impl ::= "impl" generic_params? path "{" func_decl* "}"

full_trait_impl ::= "impl" generic_params? path "for" path "{" func_decl* "}"

// --- KHAI BÁO BIẾN VÀ HÀM ---
var_decl_core ::= "export"? ("dec" | "const") IDENTIFIER (":" type)? "=" expression
var_decl    ::= var_decl_core ";"

func_decl   ::= "export"? "fn" IDENTIFIER generic_params? "(" parameters? ")" ("->" type)? block_stmt

parameters  ::= IDENTIFIER ":" type ("," IDENTIFIER ":" type)*

type           ::= reference_type | pointer_type | array_type | named_type
reference_type ::= "&" KW_RW? type
pointer_type   ::= "*" KW_RW? type
array_type     ::= "[" type ("," const_expression)? "]"
named_type     ::= path

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

assignment  ::= lvalue "=" expression 
              | logical_or

lvalue      ::= "*"* path ("[" expression "]" | "." IDENTIFIER)*

logical_or  ::= logical_and ("||" logical_and)*
logical_and ::= equality ("&&" equality)*

equality    ::= comparison (("==" | "!=") comparison)*

comparison  ::= term (("<" | "<=" | ">" | ">=") term)*

term        ::= factor (("+" | "-") factor)*

factor      ::= unary (("*" | "/") unary)*

unary       ::= "-" unary 
              | "*" unary
              | "!" unary
              | "&" KW_RW? unary
              | primary

postfix_op  ::= "[" expression "]" 
              | "." IDENTIFIER 
              | "?"
              | "++"
              | "--"
              | "(" argument_list? ")"

primary     ::= base_primary postfix_op*

// Thêm match_expr vào base_primary
base_primary::= NUMBER 
              | STRING 
              | KW_TRUE 
              | KW_FALSE 
              | array_literal
              | path 
              | match_expr
              | "(" expression ")"

// --- MATCH STATEMENT ---
match_expr  ::= "match" expression "{" match_arm* "}"

match_arm   ::= pattern "->" (expression | block_stmt) ","?

enum_destruct ::= "(" IDENTIFIER ("," IDENTIFIER)* ")"

pattern     ::= NUMBER 
              | STRING 
              | KW_TRUE 
              | KW_FALSE 
              | "_"
              | path enum_destruct?

// --- MẢNG VÀ GỌI HÀM ---
array_literal ::= "[" arguments? "]"

// Hỗ trợ truyền tham số có tên (Named Arguments)
argument_list ::= argument ("," argument)*

argument    ::= (IDENTIFIER ":")? expression

// Giữ lại arguments cho array_literal
arguments   ::= expression ("," expression)*