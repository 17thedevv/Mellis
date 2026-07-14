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

KW_LET    ::= "let"
KW_FN     ::= "fn"
KW_IF     ::= "if"
KW_ELSE   ::= "else"
KW_RETURN ::= "return"
KW_EXPORT ::= "export"
KW_EXTERN ::= "extern"
KW_CONST ::= "const"

// Toán tử số học
PLUS      ::= "+"
MINUS     ::= "-"
STAR      ::= "*"
SLASH     ::= "/"

// Toán tử gán & so sánh
ASSIGN    ::= "="
EQ_EQ     ::= "=="
BANG_EQ   ::= "!="
LESS      ::= "<"
GREATER   ::= ">"

// Dấu câu cấu trúc
COLON     ::= ":"
SEMI      ::= ";"
COMMA     ::= ","
L_PAREN   ::= "("
R_PAREN   ::= ")"
L_BRACE   ::= "{"
R_BRACE   ::= "}"


WHITESPACE ::= " " | "\t" | "\n" | "\r"
COMMENT    ::= "//" (bất_kỳ_ký_tự_nào_trừ_dấu_xuống_dòng)* "\n"


program     ::= declaration* EOF

declaration ::= var_decl 
              | func_decl 
              | statement
              | extern_decl

extern_decl ::= "extern" "fn" IDENTIFIER "(" parameters? ")" ("->" type)? ";"

var_decl ::= "export"? ("let" | "const") IDENTIFIER (":" type)? "=" expression ";"

func_decl   ::= "export"? "fn" IDENTIFIER "(" parameters? ")" ("->" type)? block_stmt

parameters  ::= IDENTIFIER ":" type ("," IDENTIFIER ":" type)*

type        ::= IDENTIFIER

statement   ::= expr_stmt 
              | block_stmt 
              | if_stmt 
              | while_stmt 
              | return_stmt
              | break_stmt
              | continue_stmt


expr_stmt   ::= expression ";"

block_stmt  ::= "{" declaration* "}"

if_stmt     ::= "if" expression block_stmt ("else" (block_stmt | if_stmt))?

while_stmt  ::= "while" expression block_stmt

return_stmt ::= "return" expression? ";"

// 1. Phép gán (Ưu tiên thấp nhất)
// Hỗ trợ: a = b = 5 hoặc a = 5
expression  ::= assignment

assignment  ::= IDENTIFIER "=" expression 
              | equality

// 2. So sánh bằng (==, !=)
equality    ::= comparison (("==" | "!=") comparison)*

// 3. So sánh lớn bé (<, >)
comparison  ::= term (("<" | ">") term)*

// 4. Cộng trừ (+, -)
term        ::= factor (("+" | "-") factor)*

// 5. Nhân chia (*, /)
factor      ::= unary (("*" | "/") unary)*

// 6. Toán tử một ngôi (-x, không có dấu ! vì MVP chưa hỗ trợ logic NOT)
unary       ::= "-" unary 
              | primary

// 7. Mảnh nhỏ nhất & Lời gọi hàm (Ưu tiên cao nhất)
primary     ::= NUMBER 
              | STRING 
              | "true" 
              | "false" 
              | IDENTIFIER 
              | call
              | "(" expression ")"

call        ::= IDENTIFIER "(" arguments? ")"

arguments   ::= expression ("," expression)*