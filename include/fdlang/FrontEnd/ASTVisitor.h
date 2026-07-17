#pragma once
#include "fdlang/AST/ASTNode.h"

namespace fl {

// Forward declarations
class ProgramNode;
class VarDeclNode;
class FunctionDeclNode;
class ParamDeclNode;
class StructDeclNode;
class StructFieldNode;
class EnumDeclNode;
class EnumVariantNode;
class TraitDeclNode;
class ImplDeclNode;
class ModDeclNode;
class UseDeclNode;
class ExternDeclNode;
class TypeAliasDeclNode;

class BlockStmtNode;
class ExprStmtNode;
class IfStmtNode;
class WhileStmtNode;
class ForStmtNode;
class ReturnStmtNode;
class BreakStmtNode;
class ContinueStmtNode;
class UnsafeStmtNode;
class ComptimeStmtNode;

class LiteralExpr;
class IdentifierExpr;
class BinaryExpr;
class UnaryExpr;
class AssignExpr;
class CallExpr;
class MethodCallExpr;
class IndexExpr;
class MemberExpr;
class CastExpr;
class ArrayLiteralExpr;
class TupleLiteralExpr;
class StructInitExpr;
class MatchExpr;
class LambdaExpr;
class AwaitExpr;
class SizeofExpr;
class AlignofExpr;

class BuiltinTypeNode;
class NamedTypeNode;
class ReferenceTypeNode;
class PointerTypeNode;
class ArrayTypeNode;
class TupleTypeNode;
class FunctionTypeNode;
class NeverTypeNode;
class TraitObjectTypeNode;

class WildcardPatternNode;
class LiteralPatternNode;
class IdentifierPatternNode;
class EnumPatternNode;
class TuplePatternNode;

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
    virtual void visit(MethodCallExpr&)    = 0;
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

class TypeVisitor {
public:
    virtual ~TypeVisitor() = default;
    virtual void visit(BuiltinTypeNode&)     = 0;
    virtual void visit(NamedTypeNode&)       = 0;
    virtual void visit(ReferenceTypeNode&)   = 0;
    virtual void visit(PointerTypeNode&)     = 0;
    virtual void visit(ArrayTypeNode&)       = 0;
    virtual void visit(TupleTypeNode&)       = 0;
    virtual void visit(FunctionTypeNode&)    = 0;
    virtual void visit(NeverTypeNode&)       = 0;
    virtual void visit(TraitObjectTypeNode&) = 0;
};

class PatternVisitor {
public:
    virtual ~PatternVisitor() = default;
    virtual void visit(WildcardPatternNode&)   = 0;
    virtual void visit(LiteralPatternNode&)    = 0;
    virtual void visit(IdentifierPatternNode&) = 0;
    virtual void visit(EnumPatternNode&)       = 0;
    virtual void visit(TuplePatternNode&)      = 0;
};

} // namespace fl