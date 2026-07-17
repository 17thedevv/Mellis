# 📋 Trạng thái Dự án — freedomLanguage (fdlang)

| Hạng mục | Giá trị |
|----------|---------|
| **Version** | `v1.0` |
| **Branch** | `main` |
| **Phase hiện tại** | **Phase 2 — Middle End (Type Checker đang nâng cấp)** |
| **Ngôn ngữ triển khai** | C++17 |
| **Build system** | CMake 3.20+ |
| **Backend** | LLVM |
| **Cập nhật lần cuối** | 2026-07-18 |

> ⚠️ **v1.0 — Đây là bản compiler sẽ dùng trong thực tế. Làm cẩn thận, không đốt cháy giai đoạn.**

---

## 🏗️ Kiến trúc Pipeline

```
Source (.fl)
    │
    ▼
[Phase 1 — FrontEnd]
    Lexer       ✅ hoàn chỉnh
    ↓
    Parser      ✅ hoàn chỉnh (Recursive Descent, v1.0 grammar)
    ↓
    AST         ✅ đầy đủ nodes (Decl, Stmt, Expr, Type, Pattern)

[Phase 2 — MiddleEnd]
    Resolver           ✅ hoàn chỉnh — Two-pass, scoped symbol table
    ↓
    Type Checker       🔄 đang nâng cấp — Constraint-Based (ConstraintGenerator → UnificationEngine → TypeResolver)
    ↓
    Borrow Checker     ❌ chưa có
    ↓
    FLIR Generator     ✅ hoàn chỉnh

[Phase 3 — BackEnd]
    LLVM IR Generator  ✅ hoàn chỉnh
    ↓
    Executable Gen     ✅ hoàn chỉnh
```

---

## 📁 Cấu trúc Thực tế (kiểm tra từ codebase)

```
fdlang/
├── include/fdlang/
│   ├── Core/
│   │   ├── FLType.h           Type system: PrimitiveType, StructType, FunctionType,
│   │   │                      InferenceVarType, UnificationTable, TypeContext
│   │   ├── Types.h            SymbolID, ScopeID, FileID, kInvalidSymbolID
│   │   ├── SourceLocation.h   { file, line, col, offset }
│   │   ├── Identifier.h       Identifier với transparent hash
│   │   └── StringInterner.h   Stub — chừa sẵn cho string interning
│   ├── AST/
│   │   ├── ASTNode.h          Base node
│   │   ├── ProgramNode.h      Top-level program
│   │   ├── DeclNode.h         FunctionDeclNode, StructDeclNode, EnumDeclNode,
│   │   │                      TraitDeclNode, ImplDeclNode, TypeAliasDeclNode, ...
│   │   ├── StmtNode.h         BlockStmtNode, IfStmtNode, WhileStmtNode, ForStmtNode,
│   │   │                      ReturnStmtNode, VarDeclNode, ExprStmtNode, ...
│   │   ├── ExprNode.h         LiteralExpr, IdentifierExpr, BinaryExpr, CallExpr,
│   │   │                      MethodCallExpr, MemberExpr, StructInitExpr, MatchExpr, ...
│   │   ├── TypeNode.h         BuiltinTypeNode, NamedTypeNode, ReferenceTypeNode,
│   │   │                      PointerTypeNode, ArrayTypeNode, FunctionTypeNode, ...
│   │   └── PatternNode.h      WildcardPattern, LiteralPattern, IdentifierPattern,
│   │                          EnumPattern, TuplePattern
│   ├── FrontEnd/
│   │   ├── Lexer.h
│   │   ├── Parser.h
│   │   ├── Token.h
│   │   └── ASTVisitor.h       ASTVisitor, TypeVisitor, PatternVisitor
│   ├── MiddleEnd/
│   │   ├── Symbol.h           Symbol, Scope, SymbolKind, ScopeKind
│   │   ├── ScopeStack.h       Traversal-time scope tracker
│   │   ├── SymbolTable.h      Arena-based symbol registry
│   │   ├── Resolver.h
│   │   ├── TypeChecker.h
│   │   ├── FLIR.h             FLIR instruction set
│   │   └── FLIRGenerator.h
│   ├── BackEnd/
│   │   ├── LLVMIRGenerator.h
│   │   └── ExecutableGenerator.h
│   └── Support/
│       └── (Diagnostic headers)
├── src/
│   ├── FrontEnd/
│   │   ├── Lexer.cpp
│   │   └── Parser.cpp
│   ├── MiddleEnd/
│   │   ├── SymbolTable.cpp
│   │   ├── Resolver.cpp
│   │   ├── TypeChecker.cpp    [🔄 đang nâng cấp — Constraint-Based architecture]
│   │   ├── FLIR.cpp
│   │   ├── FLIRGenerator.cpp
│   │   └── SemanticAnalyzer.cpp  [DEPRECATED — excluded khỏi build, chờ xóa]
│   ├── BackEnd/
│   │   ├── LLVMIRGenerator.cpp
│   │   └── ExecutableGenerator.cpp
│   ├── Runtime/
│   │   └── runtime.c
│   └── main.cpp
├── tests/
│   ├── test_lexer.cpp
│   ├── test_parser.cpp
│   ├── test_resolver.cpp       12 test cases ✅
│   ├── test_typechecker.cpp    6 test cases ✅
│   ├── test_flir_generator.cpp
│   ├── test_llvmir_generator.cpp
│   ├── test_executable_generator.cpp
│   ├── test_ast_manual.cpp
│   ├── test_diagnostic.cpp
│   └── test_sematic.cpp        [legacy — sẽ xóa cùng SemanticAnalyzer]
├── docs/
│   ├── grammar.md              ✅ v1.0 Frozen
│   ├── architecture.md
│   ├── FLIR.MD
│   ├── RoadMap.md
│   ├── VISION.MD
│   └── ast.md
└── Status.md
```

---

## ✅ Đã hoàn thành

### Phase 1 — FrontEnd

| Module | Trạng thái | Ghi chú |
|--------|-----------|---------|
| **Lexer** | ✅ Hoàn chỉnh | Hỗ trợ toàn bộ Grammar v1.0: literals, operators, keywords |
| **Parser** | ✅ Hoàn chỉnh | Recursive Descent, parse đầy đủ Decl/Stmt/Expr/Type/Pattern |
| **AST** | ✅ Hoàn chỉnh | Đầy đủ node types cho grammar v1.0 |
| **ASTVisitor** | ✅ Hoàn chỉnh | ASTVisitor, TypeVisitor, PatternVisitor |

### Phase 2 — MiddleEnd

| Module | Trạng thái | Ghi chú |
|--------|-----------|---------|
| **Resolver** | ✅ Hoàn chỉnh | Two-pass: DeclarationVisitor → ResolutionVisitor |
| **SymbolTable** | ✅ Hoàn chỉnh | Arena-based, persistent, chain-walk lookup |
| **Type System (FLType.h)** | ✅ Hoàn chỉnh | PrimitiveType, StructType, EnumType, FunctionType, InferenceVarType, UnificationTable, TypeContext |
| **TypeChecker — TypePrePass** | ✅ Hoàn chỉnh | Khai báo kiểu cho fn, struct, enum trước khi resolve |
| **TypeChecker — ConstraintGenerator** | ✅ Hoàn chỉnh | Sinh Equality & Field constraints, không unify trực tiếp |
| **TypeChecker — UnificationEngine** | ✅ Hoàn chỉnh | Union-Find trên InferenceVarType, giải Field constraint qua StructDecl |
| **TypeChecker — TypeResolver** | ✅ Hoàn chỉnh | Deep-resolve InferenceVar, emit lỗi nếu còn biến chưa rõ kiểu |
| **FLIR** | ✅ Hoàn chỉnh | FLIR instruction set, specification |
| **FLIRGenerator** | ✅ Hoàn chỉnh | Lowering AST → FLIR |

### Phase 3 — BackEnd

| Module | Trạng thái | Ghi chú |
|--------|-----------|---------|
| **LLVMIRGenerator** | ✅ Hoàn chỉnh | FLIR → LLVM IR |
| **ExecutableGenerator** | ✅ Hoàn chỉnh | LLVM IR → native executable, linker abstraction |

### Specification & Documentation

| File | Trạng thái |
|------|-----------|
| `grammar.md` | ✅ v1.0 Frozen — EBNF đầy đủ, Generics, Traits, Comptime |
| `FLIR.MD` | ✅ Specification hoàn chỉnh |
| `architecture.md` | ✅ Pipeline design |
| `VISION.MD` | ✅ |

### Tests

| Test | Kết quả |
|------|---------|
| `test_lexer` | ✅ |
| `test_parser` | ✅ |
| `test_resolver` | ✅ 12/12 passed |
| `test_typechecker` | ✅ 6/6 passed |
| `test_flir_generator` | ✅ |
| `test_llvmir_generator` | ✅ |
| `test_executable_generator` | ✅ |

---

## ❌ Chưa triển khai / Cần làm tiếp

### Phase 2 — MiddleEnd (còn thiếu)

| Tính năng | Mức độ ưu tiên | Ghi chú |
|-----------|---------------|---------|
| **TypeChecker — CallExpr, MethodCallExpr** | 🔴 Cao | Hiện đang `{}` empty — chưa generate constraints cho function calls |
| **TypeChecker — Generics** | 🔴 Cao | Generic parameter constraints chưa được sinh |
| **TypeChecker — Trait Resolution** | 🔴 Cao | ImplMap có sẵn nhưng chưa kết nối vào solver |
| **TypeChecker — IfStmt, WhileStmt** | 🟡 Trung bình | Condition check chưa có |
| **TypeChecker — Pattern Matching** | 🟡 Trung bình | MatchExpr, EnumPattern chưa được handle |
| **Initialization Check** | 🟡 Trung bình | Cần tích hợp vào TypeChecker, thay thế SemanticAnalyzer cũ |
| **Borrow Checker** | 🔴 Cao | Chưa có — là blocker trước khi gọi là memory-safe |

---

## 🔧 Vấn đề kỹ thuật cần chú ý

| # | Vấn đề | Mức độ |
|---|--------|--------|
| 1 | **`SourceLocation` chưa có line/col thực** — Lexer chưa điền, mọi `loc.line == 0`. Lỗi diagnostic hiện không chỉ đúng vị trí. | 🔴 Cao |
| 2 | **`SemanticAnalyzer.cpp` deprecated** — Excluded khỏi build nhưng chưa xóa khỏi repo. | 🟡 Trung bình |
| 3 | **`README.MD`** — Cần viết hướng dẫn build và sử dụng cho v1.0. | 🟡 Trung bình |
| 4 | **Unused Variables** — TypeChecker chưa báo lỗi biến không dùng → FLIRGenerator sinh `%id = alloca void`. | 🟡 Trung bình |
| 5 | **LLVM Build Mode Mismatch** — LLVM Release (`/MD`) vs fdlang Debug (`/MDd`) → link error `_ITERATOR_DEBUG_LEVEL`. Tạm thời build fdlang ở Release. | 🟡 Trung bình |
| 6 | **Transparent lookup C++17/MSVC** — `unordered_map::find(string_view)` cần explicit `Identifier key(name)`. Fix dài hạn: nâng C++20. | 🟢 Thấp |

---

## 📊 Tiến độ thực tế

```
Phase 1 — Lexer                 ██████████  100% ✅
Phase 1 — Parser                ██████████  100% ✅
Phase 1 — AST                   ██████████  100% ✅

Phase 2 — Resolver              ██████████  100% ✅
Phase 2 — Type System (FLType)  ██████████  100% ✅
Phase 2 — TypeChecker Core      ████████░░   80% 🔄
Phase 2 — TypeChecker Generics  ██░░░░░░░░   20% 🔄
Phase 2 — TypeChecker Traits    ██░░░░░░░░   20% 🔄
Phase 2 — Borrow Checker        ░░░░░░░░░░    0% ❌

Phase 2 — FLIR Generator        ██████████  100% ✅
Phase 3 — LLVM IR Generator     ██████████  100% ✅
Phase 3 — Executable Gen        ██████████  100% ✅

Diagnostic Engine               ███████░░░   70% (line/col chưa có)
Tests                           ██████████  100% ✅
Documentation                   ████████░░   80%
────────────────────────────────────────────────────
Tổng thể v1.0                   ██████░░░░  ~60%
```

---

## 🗺️ Thứ tự làm tiếp (theo đúng lộ trình)

```
[Đang làm]  TypeChecker — hoàn thiện nốt các node còn thiếu
                CallExpr, MethodCallExpr, IfStmt, WhileStmt
                MatchExpr + Pattern binding
                Generic parameter constraints
                Trait method resolution qua ImplMap
    ↓
[Tiếp theo] Initialization Check
                Tích hợp vào TypeChecker, xóa SemanticAnalyzer.cpp
    ↓
[Sau đó]    Borrow Checker
                Ownership, borrowing, lifetime cơ bản
    ↓
[Dài hạn]   Module system (mod, export, extern)
            StringInterner upgrade
            Diagnostic Engine — line/col chính xác
```
