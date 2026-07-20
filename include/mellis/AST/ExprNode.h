#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/Core/Types.h"
#include "mellis/Core/FLType.h"
#include "mellis/AST/PlaceholderData.h"
#include <vector>
#include <memory>
#include <string_view>
#include <variant>

namespace fl {

class TypeNode;
class ParamDeclNode;
class StmtNode;
class PatternNode;

enum class ValueCategory : uint8_t {
    RValue,
    LValue,
};

class ExprNode : public ASTNode {
public:
    const Type* inferredType = nullptr;
    SymbolID resolvedSymbol = kInvalidSymbolID;
    ValueCategory valueCategory = ValueCategory::RValue;
    bool isConstant = false;
};

enum class BinaryOp : uint8_t {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    LogicAnd, LogicOr,
    BitAnd, BitOr, BitXor, LShift, RShift,
    Range, RangeInc,
};

enum class UnaryOp : uint8_t {
    Neg, Not, BitNot, Deref, Ref, RefMut, PostInc, PostDec,
};

enum class AssignOp : uint8_t {
    Assign, AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    BitAndAssign, BitOrAssign, BitXorAssign, LShiftAssign, RShiftAssign,
};

enum class LiteralKind : uint8_t {
    Integer, Float, Bool, Char, Str, RawStr, Byte, ByteStr,
};

enum class LiteralSuffix : uint8_t {
    None, I8, I16, I32, I64, I128, U8, U16, U32, U64, U128, F32, F64,
};

class LiteralExpr : public ExprNode {
public:
    LiteralKind      kind;
    LiteralSuffix    suffix = LiteralSuffix::None;
    std::string_view rawText;
    
    std::variant<
        uint64_t,
        int64_t,
        double,
        char32_t,
        std::string_view
    > value;

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class IdentifierExpr : public ExprNode {
public:
    std::vector<std::string_view> segments;
    std::vector<std::unique_ptr<TypeNode>> genericArgs;
    SymbolID symbolId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class BinaryExpr : public ExprNode {
public:
    BinaryOp                  op;
    std::unique_ptr<ExprNode> left;
    std::unique_ptr<ExprNode> right;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class UnaryExpr : public ExprNode {
public:
    UnaryOp                   op;
    std::unique_ptr<ExprNode> operand;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class AssignExpr : public ExprNode {
public:
    AssignOp                  op;
    std::unique_ptr<ExprNode> lvalue;
    std::unique_ptr<ExprNode> value;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

struct CallArgNode {
    SourceLocation            loc;
    std::string_view          label;
    std::unique_ptr<ExprNode> value;
};

class CallExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode>              callee;
    std::vector<std::unique_ptr<TypeNode>> genericArgs;
    std::vector<const Type*>               inferredGenericArgs;
    std::vector<CallArgNode>               args;
    SymbolID resolvedFn = kInvalidSymbolID;
    bool isClosureCall = false; // Set by TypeChecker if calling a closure struct
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class MethodCallExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode>              object;
    std::string_view                       methodName;
    std::vector<std::unique_ptr<TypeNode>> genericArgs;
    std::vector<const Type*>               inferredGenericArgs;
    std::vector<CallArgNode>               args;
    SymbolID resolvedFn = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class IndexExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> base;
    std::unique_ptr<ExprNode> index;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class MemberExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> object;
    std::string_view          member;
    SymbolID memberId = kInvalidSymbolID;
    size_t resolvedFieldIndex = (size_t)-1;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class TupleIndexExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> object;
    uint32_t                  index; // t.0 → 0, t.1 → 1
    void accept(ASTVisitor& v) override;
};

class CastExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> expr;
    std::unique_ptr<TypeNode> targetType;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

// Inserted by TypeChecker for `&T -> &dyn Trait` coercion
class UnsizeCastExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> expr;
    const Type*               targetTypePtr = nullptr; // Since it's inserted in MiddleEnd
    void accept(ASTVisitor& v) override;
};

class ArrayLiteralExpr : public ExprNode {
public:
    std::vector<std::unique_ptr<ExprNode>> elements;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class TupleLiteralExpr : public ExprNode {
public:
    std::vector<std::unique_ptr<ExprNode>> elements;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

struct FieldInitNode {
    SourceLocation            loc;
    std::string_view          name;
    std::unique_ptr<ExprNode> value;
};

class StructInitExpr : public ExprNode {
public:
    std::vector<std::string_view>          path;
    std::vector<std::unique_ptr<TypeNode>> genericArgs;
    SymbolID                               structId = kInvalidSymbolID;
    std::vector<FieldInitNode>             fields;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

struct MatchArmNode {
    SourceLocation               loc;
    std::unique_ptr<PatternNode> pattern;
    std::unique_ptr<StmtNode>    body;
};

class MatchExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode>  subject;
    std::vector<MatchArmNode>  arms;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class PlaceholderExpr : public ExprNode {
public:
    PlaceholderData data;

    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class LambdaExpr : public ExprNode {
public:
    std::vector<std::unique_ptr<ParamDeclNode>> params;
    std::unique_ptr<TypeNode>                   returnType;
    std::unique_ptr<StmtNode>                   body;
    std::vector<SymbolID>                       capturedSymbols;
    SymbolID                                    generatedStructId = kInvalidSymbolID;
    SymbolID                                    generatedFuncId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class AwaitExpr : public ExprNode {
public:
    std::unique_ptr<ExprNode> expr;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class SizeofExpr : public ExprNode {
public:
    std::unique_ptr<TypeNode> targetType;
    const Type* evaluatedTargetType = nullptr;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class AlignofExpr : public ExprNode {
public:
    std::unique_ptr<TypeNode> targetType;
    const Type* evaluatedTargetType = nullptr;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

} // namespace fl