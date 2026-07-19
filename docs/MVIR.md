// ==========================================
// MELLIS VIRTUAL IR (MVIR) - SPECIFICATION v2.0
// Độc lập kiến trúc - Hướng ngữ nghĩa (Semantic-Driven)
// ==========================================

// --- 1. TỪ VỰNG (LEXICAL) ---
letter      ::= "a" | ... | "z" | "A" | ... | "Z" | "_"
digit       ::= "0" | ... | "9"

global_id   ::= "@" letter (letter | digit)*
local_id    ::= "%" (letter | digit)+
label_id    ::= letter (letter | digit)*
meta_id     ::= "!" letter (letter | digit)*         // VD: !dbg, !alias
type_id     ::= "%" letter (letter | digit | "_" | "<" | ">" | ",")*

number      ::= "-"? digit+ ("." digit+)? (("e" | "E") ("+" | "-")? digit+)?
boolean     ::= "true" | "false"
string_lit  ::= '"' (bất_kỳ_ký_tự_nào_trừ_nháy_kép_hoặc_escape)* '"'

array_lit   ::= "[" (operand ("," operand)*)? "]"
struct_lit  ::= "{" (operand ("," operand)*)? "}"
operand     ::= local_id | global_id | number | boolean | string_lit
              | array_lit | struct_lit | "null" | "undef" | "zeroinit"

// --- 2. HỆ THỐNG KIỂU (TYPE SYSTEM) ---
type           ::= primitive_type | vector_type | pointer_type 
                 | array_type | type_id | func_type

primitive_type ::= "i8" | "i16" | "i32" | "i64" | "f32" | "f64" | "bool" | "void"
vector_type    ::= "<" number "x" primitive_type ">" // SIMD
pointer_type   ::= "*" type
array_type     ::= "[" number "x" type "]"
func_type      ::= "fn" "(" (type ("," type)*)? ")" "->" type

// --- 3. ATTRIBUTES & CẤU TRÚC (STRUCTURE) ---
call_conv   ::= "cdecl" | "stdcall" | "fastcall" | "sysv" | "win64"
func_attr   ::= "inline" | "cold" | "hot" | "pure" | "readonly" | "noreturn"
attr_list   ::= "[" func_attr ("," func_attr)* "]"

module      ::= type_decl* global_decl* extern_decl* func_decl* EOF

type_decl   ::= "type" type_id "=" ("struct" "{" (type ("," type)*)? "}" | "opaque")
global_decl ::= global_id "=" "global" type operand
extern_decl ::= "extern" call_conv? "func" global_id "(" extern_params? ")" "->" type attr_list?
extern_params ::= params ("," "...")? | "..."

func_decl   ::= "func" global_id "(" params? ")" "->" type attr_list? "{" basic_block+ "}"
params      ::= type local_id ("," type local_id)*
basic_block ::= label_id ":" (instruction)* terminator

// --- 4. TẬP LỆNH (INSTRUCTIONS) ---
instruction ::= inst_core (meta_id number)*

// 4.1. Nhóm Bộ nhớ (Semantic Memory) - Đã gỡ bỏ alloca và gep
mem_inst    ::= local_id "=" "local" type                     // Khai báo biến cục bộ (Stack/Reg)
              | local_id "=" "load" type operand
              | "store" type operand "," operand
              | local_id "=" "field" type operand "," number  // Truy cập trường của Struct
              | local_id "=" "index" type operand "," operand // Truy cập phần tử Array

// 4.2. Nhóm Bất biến (Aggregate / Vector)
agg_inst    ::= local_id "=" "extract" type operand "," number
              | local_id "=" "insert" type operand "," operand "," number

// 4.3. Nhóm Nguyên tử (Atomics) - Có Memory Ordering chuẩn
mem_order   ::= "relaxed" | "acquire" | "release" | "seqcst"
atomic_inst ::= local_id "=" "atomic_load" mem_order type operand
              | "atomic_store" mem_order type operand "," operand
              | local_id "=" "cmpxchg" mem_order type operand "," operand "," operand
              | "fence" mem_order

// 4.4. Nhóm Tính toán (ALU)
alu_inst    ::= local_id "=" alu_op type operand "," operand
              | local_id "=" "select" operand "," operand "," operand
alu_op      ::= "add" | "sub" | "mul" | "div" | "rem"
              | "eq" | "ne" | "lt" | "le" | "gt" | "ge"
              | "and" | "or" | "xor" | "shl" | "shr"

// 4.5. Nhóm Ép kiểu (Explicit Casts)
cast_inst   ::= local_id "=" cast_op operand "to" type
cast_op     ::= "trunc" | "zext" | "sext" | "fptosi" | "sitofp" 
              | "ptrtoint" | "inttoptr" | "bitcast"

// 4.6. Nhóm Gọi hàm & Intrinsics
call_inst      ::= (local_id "=")? "call" type operand "(" args? ")"
args           ::= operand ("," operand)*
intrinsic_inst ::= "intrinsic" "memcpy" "(" operand "," operand "," operand ")"
                 | "intrinsic" "memset" "(" operand "," operand "," operand ")"

// --- 5. LỆNH KẾT THÚC KHỐI (TERMINATORS) ---
terminator  ::= term_core (meta_id number)*
term_core   ::= "jump" label_id
              | "branch" operand "," label_id "," label_id
              | "switch" operand "{" (number ":" label_id)* "default" ":" label_id "}"
              | "ret" operand?
              | "unreachable"``