# fdlang AST v1.0 — Đặc tả chính thức

> **Trạng thái:** Draft v2 — chờ freeze
> **Phụ thuộc:** `grammar.md` v1.0 (frozen)
> **Được dùng bởi:** Parser, Resolver, TypeChecker, BorrowChecker, FLIRGenerator
> **Cập nhật lần cuối:** 2026-07-17 (v2 — major revision)

---

## 0. Triết lý thiết kế

| Nguyên tắc | Giải thích |
|-----------|-----------|
| **AST là dữ liệu thuần túy** | Node không chứa logic. Logic nằm ở các pass. |
| **Kế thừa phản ánh ngữ nghĩa** | `IfStmt` không phải Declaration. Không ép kế thừa chỉ để nhét vào `vector`. |
| **Enum thay string cho operator** | `switch(op)` tốt hơn `if(op=="+")` ở mọi pass. |
| **Ownership rõ ràng** | Node cha sở hữu node con qua `unique_ptr`. Back-pointer là raw `*`, non-owning. |

### Annotation contract

```
Parser          -> tao node, dien SourceLocation, rawText, parsedValue
Resolver        -> dien symbolId tren IdentifierExpr, DeclNode, CallExpr, PatternNode
TypeChecker     -> dien typeId tren ExprNode, TypeNode
BorrowChecker   -> doc symbolId + typeId, KHONG viet vao AST (dung side-table rieng)
FLIRGenerator   -> doc tat ca annotations, KHONG thay doi AST
```

---

## 1. Hierarchy tong the

### 1.1 Van de voi StmtNode : DeclNode

`IfStmtNode` khong tao symbol, khong dua gi vao scope.
No KHONG PHAI declaration. Ke thua `DeclNode` chi de nhet vao
`vector<DeclNode>` vi pham Liskov Substitution Principle.

### 1.2 Giai phap — ItemNode trung gian

```
ASTNode
 |-- ItemNode                  <- "thu co the xuat hien trong mot block"
 |    |-- DeclNode             <- khai bao: tao symbol moi, dua vao scope
 |    +-- StmtNode             <- lenh: thuc thi, KHONG tao symbol
 |-- ExprNode                  <- bieu thuc: tao gia tri co kieu
 |-- TypeNode                  <- bieu dien kieu du lieu (ke thua ASTNode)
 +-- PatternNode               <- pattern trong match (ke thua ASTNode)
```

`BlockStmtNode::body` la `vector<unique_ptr<ItemNode>>` —
chua duoc ca DeclNode lan StmtNode.

### 1.3 Base classes

```cpp
class ASTNode {
public:
    SourceLocation loc;
    virtual ~ASTNode() = default;
    virtual void accept(ASTVisitor&) = 0;
};

// Thu co the xuat hien o top-level hoac ben trong block
class ItemNode : public ASTNode {};

// Khai bao: tao ten moi vao scope
class DeclNode : public ItemNode {
public:
    bool     isExported = false;
    SymbolID symbolId   = kInvalidSymbolID;   // Resolver dien
};

// Lenh: thuc thi, KHONG gioi thieu ten moi
class StmtNode : public ItemNode {};

// Bieu thuc: tao gia tri co kieu
class ExprNode : public ASTNode {
public:
    FLType inferredType = FLType::Unknown;  // TypeChecker MVP
    // TODO(TypeChecker-v2): TypeID typeId = kInvalidTypeID;
};
```

---

## 2. Operator Enums

Parser da biet token — Parser sinh enum ngay. Khong luu string.

```cpp
enum class BinaryOp : uint8_t {
    // Arithmetic
    Add, Sub, Mul, Div, Mod,
    // Comparison -> Bool
    Eq, Ne, Lt, Le, Gt, Ge,
    // Logical -> Bool
    LogicAnd, LogicOr,
    // Bitwise
    BitAnd, BitOr, BitXor, LShift, RShift,
    // Range
    Range,      // ..
    RangeInc,   // ..=
};

enum class UnaryOp : uint8_t {
    Neg,      // -x
    Not,      // !x
    BitNot,   // ~x
    Deref,    // *x
    Ref,      // &x
    RefMut,   // &rw x
    PostInc,  // x++
    PostDec,  // x--
};

enum class AssignOp : uint8_t {
    Assign,
    AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    BitAndAssign, BitOrAssign, BitXorAssign,
    LShiftAssign, RShiftAssign,
};
```

Tai sao: TypeChecker, FLIR, LLVM deu dung `switch(op)`.
Clang dung `BinaryOperatorKind` tuong tu.

---

## 3. Literal Storage

Luu ca hai: `rawText` (de diagnostic giu nguyen nguon) +
`value` variant (de semantic pass dung truc tiep).
Parser parse mot lan. TypeChecker, FLIR, LLVM dung luon.

```cpp
enum class LiteralKind : uint8_t {
    Integer, Float, Bool, Char,
    Str, RawStr, Byte, ByteStr,
};

// Suffix kieu — 123i32, 3.14f32, ...
// None = chua co suffix (TypeChecker se suy ra)
enum class LiteralSuffix : uint8_t {
    None,
    // Integer suffixes
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    // Float suffixes
    F32, F64,
};

class LiteralExpr : public ExprNode {
public:
    LiteralKind      kind;
    LiteralSuffix    suffix = LiteralSuffix::None;  // Parser dien neu co suffix
    std::string_view rawText;   // nguyen ban tu source — dung cho error messages

    // Parsed value — Parser dien mot lan, pass sau dung truc tiep:
    std::variant<
        uint64_t,        // kind == Integer (unsigned parse)
        int64_t,         // kind == Integer (signed parse)
        double,          // kind == Float
        char32_t,        // kind == Char (Unicode code point)
        std::string_view // kind == Str / RawStr / Byte / ByteStr (slice vao source)
    > value;

    void accept(ASTVisitor& v) override;
};
```

Tai sao variant thay union:
- Union khong biet active member — UB neu doc sai.
- `std::variant` type-safe, `std::visit` an toan.
- Parser parse mot lan. TypeChecker / FLIR dung `std::get<>` truc tiep.

---

## 4. Declarations (DeclNode)

### 4.1 ProgramNode

```cpp
class ProgramNode : public ASTNode {
public:
    std::vector<std::unique_ptr<DeclNode>> decls;
    void accept(ASTVisitor& v) override;
};
```

### 4.2 VarDeclNode

```cpp
class VarDeclNode : public DeclNode {
public:
    std::string_view              name;
    std::unique_ptr<TypeNode>     typeAnnot;    // nullptr = type inference
    std::unique_ptr<ExprNode>     initializer;  // nullptr = dec x; (no init)
    bool                          isMutable;    // true=dec, false=const
    void accept(ASTVisitor& v) override;
};
```

### 4.3 FunctionDeclNode

```cpp
class FunctionDeclNode : public DeclNode {
public:
    std::string_view                             name;
    std::vector<GenericParamNode>                genericParams;
    std::vector<std::unique_ptr<ParamDeclNode>>  params;
    std::unique_ptr<TypeNode>                    returnType;  // nullptr = void/infer
    std::unique_ptr<BlockStmtNode>               body;        // nullptr = abstract/extern
    bool isAsync    = false;
    bool isComptime = false;
    void accept(ASTVisitor& v) override;
};
```

### 4.4 ParamDeclNode

```cpp
class ParamDeclNode : public DeclNode {
public:
    std::string_view          name;
    std::unique_ptr<TypeNode> type;
    bool isVariadic = false;
    bool isSelf     = false;
    void accept(ASTVisitor& v) override;
};
```

### 4.5 StructDeclNode

```cpp
class StructFieldNode : public ASTNode {
public:
    std::string_view          name;
    std::unique_ptr<TypeNode> type;
    void accept(ASTVisitor& v) override;
};

class StructDeclNode : public DeclNode {
public:
    std::string_view                              name;
    std::vector<GenericParamNode>                 genericParams;
    std::vector<std::unique_ptr<StructFieldNode>> fields;
    void accept(ASTVisitor& v) override;
};
```

### 4.6 EnumDeclNode

```cpp
class EnumVariantNode : public ASTNode {
public:
    std::string_view                             name;
    std::vector<std::unique_ptr<ParamDeclNode>>  fields;
    void accept(ASTVisitor& v) override;
};

class EnumDeclNode : public DeclNode {
public:
    std::string_view                               name;
    std::vector<GenericParamNode>                  genericParams;
    std::vector<std::unique_ptr<EnumVariantNode>>  variants;
    void accept(ASTVisitor& v) override;
};
```

### 4.7 TraitDeclNode

```cpp
class TraitDeclNode : public DeclNode {
public:
    std::string_view                               name;
    std::vector<GenericParamNode>                  genericParams;
    std::vector<std::unique_ptr<FunctionDeclNode>> methods;  // body=nullptr = abstract
    void accept(ASTVisitor& v) override;
};
```

### 4.8 ImplDeclNode

```cpp
class ImplDeclNode : public DeclNode {
public:
    std::vector<GenericParamNode>                  genericParams;
    std::unique_ptr<TypeNode>                      selfType;
    std::unique_ptr<TypeNode>                      traitType;  // nullptr = inherent
    std::vector<std::unique_ptr<FunctionDeclNode>> methods;
    void accept(ASTVisitor& v) override;
};
```

### 4.9 ModDeclNode

```cpp
class ModDeclNode : public DeclNode {
public:
    std::string_view                       name;
    std::vector<std::unique_ptr<DeclNode>> decls;
    void accept(ASTVisitor& v) override;
};
```

### 4.10 UseDeclNode

```cpp
struct UseTreeNode {
    SourceLocation                loc;
    std::vector<std::string_view> segments;
    std::string_view              alias;
    bool                          isGlob;
    std::vector<UseTreeNode>      children;
};

class UseDeclNode : public DeclNode {
public:
    UseTreeNode tree;
    void accept(ASTVisitor& v) override;
};
```

### 4.11 ExternDeclNode

```cpp
class ExternDeclNode : public DeclNode {
public:
    std::unique_ptr<FunctionDeclNode> func;  // body luon nullptr
    void accept(ASTVisitor& v) override;
};
```

### 4.12 TypeAliasDeclNode

```cpp
class TypeAliasDeclNode : public DeclNode {
public:
    std::string_view              name;
    std::vector<GenericParamNode> genericParams;
    std::unique_ptr<TypeNode>     aliasedType;
    void accept(ASTVisitor& v) override;
};
```

### 4.13 GenericParamNode (struct helper)

```cpp
struct GenericParamNode {
    SourceLocation                         loc;
    std::string_view                       name;
    std::vector<std::unique_ptr<TypeNode>> bounds;
    SymbolID symbolId = kInvalidSymbolID;
};
```

---

## 5. Statements (StmtNode : ItemNode)

`StmtNode : public ItemNode` — KHONG ke thua `DeclNode`.

### 5.1 BlockStmtNode

```cpp
class BlockStmtNode : public StmtNode {
public:
    // Chua ca DeclNode lan StmtNode thong qua ItemNode.
    std::vector<std::unique_ptr<ItemNode>> body;
    void accept(ASTVisitor& v) override;
};
```

### 5.2 ExprStmtNode

```cpp
class ExprStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode> expr;
    void accept(ASTVisitor& v) override;
};
```

### 5.3 IfStmtNode

```cpp
class IfStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode>      condition;
    std::unique_ptr<BlockStmtNode> thenBranch;
    std::unique_ptr<StmtNode>      elseBranch;  // nullptr | BlockStmt | IfStmt
    void accept(ASTVisitor& v) override;
};
```

### 5.4 WhileStmtNode

```cpp
class WhileStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode>      condition;
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};
```

### 5.5 ForStmtNode

```cpp
enum class ForKind : uint8_t { ForEach, CStyle };

class ForStmtNode : public StmtNode {
public:
    ForKind kind;
    // ForEach: for (x in expr)
    std::string_view               bindingName;
    SymbolID                       bindingId = kInvalidSymbolID;
    std::unique_ptr<ExprNode>      iterable;
    // CStyle: for (init; cond; step)
    std::unique_ptr<ItemNode>      init;    // VarDeclNode hoac ExprStmtNode
    std::unique_ptr<ExprNode>      cond;
    std::unique_ptr<ExprNode>      step;
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};
```

### 5.6 ReturnStmtNode

```cpp
class ReturnStmtNode : public StmtNode {
public:
    std::unique_ptr<ExprNode> value;  // nullptr = "return;" void
    void accept(ASTVisitor& v) override;
};
```

### 5.7 Control flow & Special

```cpp
class BreakStmtNode    : public StmtNode { void accept(ASTVisitor& v) override; };
class ContinueStmtNode : public StmtNode { void accept(ASTVisitor& v) override; };

class UnsafeStmtNode : public StmtNode {
public:
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};

class ComptimeStmtNode : public StmtNode {
public:
    std::unique_ptr<BlockStmtNode> body;
    void accept(ASTVisitor& v) override;
};
```

---

## 6. Expressions (ExprNode)

### 6.1 LiteralExpr — xem Section 3

### 6.2 IdentifierExpr

```cpp
class IdentifierExpr : public ExprNode {
public:
    std::vector<std::string_view> segments;
    SymbolID symbolId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};
```

### 6.3 BinaryExpr

```cpp
class BinaryExpr : public ExprNode {
public:
    BinaryOp                  op;
    std::unique_ptr<ExprNode> left;
    std::unique_ptr<ExprNode> right;
    void accept(ASTVisitor& v) override;
};
```

### 6.4 UnaryExpr

```cpp
class UnaryExpr : public ExprNode {
public:
    UnaryOp                   op;
    std::unique_ptr<ExprNode> operand;
    void accept(ASTVisitor& v) override;
};
```

### 6.5 AssignExpr

```cpp
class AssignExpr : public ExprNode {
public:
    AssignOp                  op;
    std::unique_ptr<ExprNode> lvalue;
    std::unique_ptr<ExprNode> value;
    void accept(ASTVisitor& v) override;
};
```

### 6.6 CallExpr

```cpp
struct CallArgNode {
    SourceLocation            loc;
    std::string_view          label;   // named arg — empty neu positional
    std::unique_ptr<ExprNode> value;
};

class CallExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode>              callee;
    std::vector<std::unique_ptr<TypeNode>> genericArgs;
    std::vector<CallArgNode>               args;
    SymbolID resolvedFn = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};
```

### 6.7 IndexExpr, MemberExpr, CastExpr

```cpp
class IndexExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> base;
    std::unique_ptr<ExprNode> index;
    void accept(ASTVisitor& v) override;
};

class MemberExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> object;
    std::string_view          member;
    SymbolID memberId = kInvalidSymbolID;   // Resolver dien — a.x khong lookup lai
    void accept(ASTVisitor& v) override;
};

class CastExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> expr;
    std::unique_ptr<TypeNode> targetType;
    void accept(ASTVisitor& v) override;
};
```

### 6.8 ArrayLiteralExpr, TupleLiteralExpr

```cpp
class ArrayLiteralExpr : public ExprNode {
public:
    std::vector<std::unique_ptr<ExprNode>> elements;
    void accept(ASTVisitor& v) override;
};

class TupleLiteralExpr : public ExprNode {
public:
    std::vector<std::unique_ptr<ExprNode>> elements;
    void accept(ASTVisitor& v) override;
};
```

### 6.9 StructInitExpr

```cpp
struct FieldInitNode {
    SourceLocation            loc;
    std::string_view          name;
    std::unique_ptr<ExprNode> value;
};

class StructInitExpr : public ExprNode {
public:
    std::vector<std::string_view> path;
    SymbolID                      structId = kInvalidSymbolID;  // Resolver dien
    std::vector<FieldInitNode>    fields;
    void accept(ASTVisitor& v) override;
};
```

### 6.10 MatchExpr

```cpp
struct MatchArmNode {
    SourceLocation               loc;
    std::unique_ptr<PatternNode> pattern;

    // Grammar: pattern -> (expression | block_stmt)
    // Neu expression: Parser wrap trong ExprStmtNode.
    // body luon la StmtNode — khong bao gio ExprNode truc tiep.
    std::unique_ptr<StmtNode>    body;
};

class MatchExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode>  subject;
    std::vector<MatchArmNode>  arms;
    void accept(ASTVisitor& v) override;
};
```

### 6.11 LambdaExpr, AwaitExpr, SizeofExpr, AlignofExpr

```cpp
class LambdaExpr : public ExprNode {
public:
    std::vector<std::unique_ptr<ParamDeclNode>> params;
    std::unique_ptr<TypeNode>                   returnType;
    std::unique_ptr<StmtNode>                   body;  // ExprStmt hoac BlockStmt
    void accept(ASTVisitor& v) override;
};

class AwaitExpr   : public ExprNode {
public: std::unique_ptr<ExprNode> expr;
    void accept(ASTVisitor& v) override;
};
class SizeofExpr  : public ExprNode {
public: std::unique_ptr<TypeNode> targetType;
    void accept(ASTVisitor& v) override;
};
class AlignofExpr : public ExprNode {
public: std::unique_ptr<TypeNode> targetType;
    void accept(ASTVisitor& v) override;
};
```

---

## 7. Types (TypeNode : ASTNode)

Tai sao TypeNode ke thua ASTNode:
- TypeChecker duyet `Vec@<Unknown>` -> generic args -> nested types -> can `accept()`.
- BorrowChecker doc reference/pointer types.
- Diagnostic can `loc` de chi vi tri type sai.

```cpp
class TypeNode : public ASTNode {
    // loc va accept() ke thua tu ASTNode
    // accept() nhan TypeVisitor& — xem Section 9
};
```

| Subclass | Nguồn (Grammar) | Fields chính | Ghi chú |
|----------|-----------------|-------------|--------|
| `BuiltinTypeNode` | `builtin_type` (e.g., `int_32`, `bool`) | `kind: BuiltinKind` | Không cần Resolver. Compiler nhận ra ngay. |
| `NamedTypeNode` | `named_type` (e.g., `Vec@<T>`, `MyStruct`) | `segments`, `genericArgs`, `symbolId` | User-defined types. Resolver điền `symbolId`. |
| `ReferenceTypeNode` | `&T`, `&rw T` | `isMutable`, `inner` | |
| `PointerTypeNode` | `*T`, `*rw T` | `isMutable`, `inner` | |
| `ArrayTypeNode` | `[T, N]` | `elementType`, `size: ExprNode?` | |
| `TupleTypeNode` | `(i32, bool)` | `elements` | |
| `FunctionTypeNode` | `fn(i32) -> i32` | `params`, `returnType` | |
| `NeverTypeNode` | `!` | (none) | |
| `TraitObjectTypeNode` | `dyn Drawable` | `trait` | |

```cpp
// BuiltinKind — khong can Symbol, khong can Resolver lookup
enum class BuiltinKind : uint8_t {
    // Signed integers
    I8, I16, I32, I64, I128,
    // Unsigned integers
    U8, U16, U32, U64, U128,
    // Floats
    F32, F64,
    // Other primitives
    Bool, Char, Str,
    // Special
    Void,   // kieu tra ve void (tuong duong unit () trong Rust)
};

class BuiltinTypeNode : public TypeNode {
public:
    BuiltinKind kind;
    void accept(ASTVisitor& v) override;  // tham chieu vao TypeVisitor
};
```

Tai sao tach `BuiltinTypeNode` khoi `NamedTypeNode`:
- Builtin type khong can Resolver. `i32` la `i32` — khong lookup Symbol.
- TypeChecker nhan biet ngay qua `kind` enum, khong so sanh string.
- FLIR/LLVM map truc tiep `BuiltinKind::I32` -> `i32` LLVM type.

---

## 8. Patterns (PatternNode : ASTNode)

Tai sao PatternNode ke thua ASTNode:
- Resolver phai traverse `Color::Red(a, b)` de declare `a`, `b` vao scope.
- TypeChecker kiem tra type cua binding.
- BorrowChecker kiem tra move vs borrow.

```cpp
class PatternNode : public ASTNode {
    // loc va accept() ke thua tu ASTNode
};
```

| Subclass | Nguon | Fields chinh |
|----------|-------|-------------|
| `WildcardPatternNode` | `_` | (none) |
| `LiteralPatternNode` | `42`, `true` | `lit: unique_ptr<LiteralExpr>` |
| `IdentifierPatternNode` | `x`, `Foo::Bar` | `segments`, `symbolId` |
| `EnumPatternNode` | `Color::Red(x, y)` | `path`, `fields: vector<PatternNode>` |
| `TuplePatternNode` | `(a, b)` | `elements: vector<PatternNode>` |

---

## 9. Visitor API

Ba visitor doc lap theo ba node family:

```cpp
// ASTVisitor — main visitor, dung boi tat ca pass
class ASTVisitor {
public:
    virtual ~ASTVisitor() = default;
    virtual void visit(ProgramNode&)       = 0;
    // Decl
    virtual void visit(VarDeclNode&)       = 0;
    virtual void visit(FunctionDeclNode&)  = 0;
    virtual void visit(ParamDeclNode&)     = 0;
    virtual void visit(StructDeclNode&)    = 0;
    virtual void visit(StructFieldNode&)   = 0;
    virtual void visit(EnumDeclNode&)      = 0;
    virtual void visit(EnumVariantNode&)   = 0;
    virtual void visit(TraitDeclNode&)     = 0;
    virtual void visit(ImplDeclNode&)      = 0;
    virtual void visit(ModDeclNode&)       = 0;
    virtual void visit(UseDeclNode&)       = 0;
    virtual void visit(ExternDeclNode&)    = 0;
    virtual void visit(TypeAliasDeclNode&) = 0;
    // Stmt
    virtual void visit(BlockStmtNode&)     = 0;
    virtual void visit(ExprStmtNode&)      = 0;
    virtual void visit(IfStmtNode&)        = 0;
    virtual void visit(WhileStmtNode&)     = 0;
    virtual void visit(ForStmtNode&)       = 0;
    virtual void visit(ReturnStmtNode&)    = 0;
    virtual void visit(BreakStmtNode&)     = 0;
    virtual void visit(ContinueStmtNode&)  = 0;
    virtual void visit(UnsafeStmtNode&)    = 0;
    virtual void visit(ComptimeStmtNode&)  = 0;
    // Expr
    virtual void visit(LiteralExpr&)       = 0;
    virtual void visit(IdentifierExpr&)    = 0;
    virtual void visit(BinaryExpr&)        = 0;
    virtual void visit(UnaryExpr&)         = 0;
    virtual void visit(AssignExpr&)        = 0;
    virtual void visit(CallExpr&)          = 0;
    virtual void visit(IndexExpr&)         = 0;
    virtual void visit(MemberExpr&)        = 0;
    virtual void visit(CastExpr&)          = 0;
    virtual void visit(ArrayLiteralExpr&)  = 0;
    virtual void visit(TupleLiteralExpr&)  = 0;
    virtual void visit(StructInitExpr&)    = 0;
    virtual void visit(MatchExpr&)         = 0;
    virtual void visit(LambdaExpr&)        = 0;
    virtual void visit(AwaitExpr&)         = 0;
    virtual void visit(SizeofExpr&)        = 0;
    virtual void visit(AlignofExpr&)       = 0;
};

// TypeVisitor — duyet TypeNode tree
// Dung boi: TypeChecker, BorrowChecker
class TypeVisitor {
public:
    virtual ~TypeVisitor() = default;
    virtual void visit(BuiltinTypeNode&)     = 0;   // i32, u64, bool... — khong can Resolver
    virtual void visit(NamedTypeNode&)       = 0;   // user-defined types
    virtual void visit(ReferenceTypeNode&)   = 0;
    virtual void visit(PointerTypeNode&)     = 0;
    virtual void visit(ArrayTypeNode&)       = 0;
    virtual void visit(TupleTypeNode&)       = 0;
    virtual void visit(FunctionTypeNode&)    = 0;
    virtual void visit(NeverTypeNode&)       = 0;
    virtual void visit(TraitObjectTypeNode&) = 0;
};

// PatternVisitor — duyet PatternNode tree
// Dung boi: Resolver, TypeChecker, BorrowChecker
class PatternVisitor {
public:
    virtual ~PatternVisitor() = default;
    virtual void visit(WildcardPatternNode&)   = 0;
    virtual void visit(LiteralPatternNode&)    = 0;
    virtual void visit(IdentifierPatternNode&) = 0;
    virtual void visit(EnumPatternNode&)       = 0;
    virtual void visit(TuplePatternNode&)      = 0;
};
```

---

## 10. Tom tat thay doi tu Draft v1

| Diem | v1 | v2 |
|------|----|----|
| Hierarchy | `StmtNode : DeclNode` (vi pham LSP) | `ItemNode` trung gian; `StmtNode : ItemNode` |
| `BlockStmtNode::body` | `vector<DeclNode>` | `vector<ItemNode>` |
| `TypeNode` | Khong co `accept()` | `TypeNode : ASTNode` + `TypeVisitor` |
| `PatternNode` | Khong co `accept()` | `PatternNode : ASTNode` + `PatternVisitor` |
| Operator | `string_view op` | `BinaryOp`, `UnaryOp`, `AssignOp` enum |
| Literal | Chi `rawText` | `rawText` + `parsedValue` union |
| `MatchArm::body` | `unique_ptr<ExprNode>` | `unique_ptr<StmtNode>` |

---

## 11. Thu tu trien khai

```
[1] Freeze ast.md (file nay)
    |
[2] AST headers C++
    include/fdlang/AST/
        ASTNode.h       — ASTNode, ItemNode, DeclNode, StmtNode, ExprNode
        DeclNode.h      — tat ca DeclNode subclasses + GenericParamNode
        StmtNode.h      — tat ca StmtNode subclasses
        ExprNode.h      — tat ca ExprNode subclasses + LiteralExpr + enums
        TypeNode.h      — tat ca TypeNode subclasses
        PatternNode.h   — tat ca PatternNode subclasses
    include/fdlang/FrontEnd/
        ASTVisitor.h    — ASTVisitor + TypeVisitor + PatternVisitor
    |
[3] Parser v1.0
    |
[4] Resolver v1.0
    |
[5] TypeChecker v1.0
    |
[6] BorrowChecker Phase 1
    |
[7] FLIRGenerator v1.0
```
