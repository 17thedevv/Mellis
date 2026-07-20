#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/AST/MacroNode.h"
#include "mellis/FrontEnd/ASTVisitor.h"

namespace fl {

void ProgramNode::accept(ASTVisitor& v) { v.visit(*this); }
void VarDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void FunctionDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void ParamDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void StructDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void StructFieldNode::accept(ASTVisitor& v) { v.visit(*this); }
void EnumDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void EnumVariantNode::accept(ASTVisitor& v) { v.visit(*this); }
void TraitDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void ImplDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void ModDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void UseDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void ExternDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void TypeAliasDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void BlockStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void ExprStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void IfStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void WhileStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void ForStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void ReturnStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void BreakStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void ContinueStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void UnsafeStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void ComptimeStmtNode::accept(ASTVisitor& v) { v.visit(*this); }
void LiteralExpr::accept(ASTVisitor& v) { v.visit(*this); }
void IdentifierExpr::accept(ASTVisitor& v) { v.visit(*this); }
void BinaryExpr::accept(ASTVisitor& v) { v.visit(*this); }
void UnaryExpr::accept(ASTVisitor& v) { v.visit(*this); }
void AssignExpr::accept(ASTVisitor& v) { v.visit(*this); }
void CallExpr::accept(ASTVisitor& v) { v.visit(*this); }
void IndexExpr::accept(ASTVisitor& v) { v.visit(*this); }
void MemberExpr::accept(ASTVisitor& v) { v.visit(*this); }
void TupleIndexExpr::accept(ASTVisitor& v) { v.visit(*this); }
void CastExpr::accept(ASTVisitor& v) { v.visit(*this); }
void UnsizeCastExpr::accept(ASTVisitor& v) { v.visit(*this); }
void ArrayLiteralExpr::accept(ASTVisitor& v) { v.visit(*this); }
void TupleLiteralExpr::accept(ASTVisitor& v) { v.visit(*this); }
void StructInitExpr::accept(ASTVisitor& v) { v.visit(*this); }
void MatchExpr::accept(ASTVisitor& v) { v.visit(*this); }
void LambdaExpr::accept(ASTVisitor& v) { v.visit(*this); }
void AwaitExpr::accept(ASTVisitor& v) { v.visit(*this); }
void SizeofExpr::accept(ASTVisitor& v) { v.visit(*this); }
void AlignofExpr::accept(ASTVisitor& v) { v.visit(*this); }
void BuiltinTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void NamedTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void ReferenceTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void PointerTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void ArrayTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void TupleTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void FunctionTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void NeverTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void TraitObjectTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void WildcardPatternNode::accept(PatternVisitor& v) { v.visit(*this); }
void LiteralPatternNode::accept(PatternVisitor& v) { v.visit(*this); }
void IdentifierPatternNode::accept(PatternVisitor& v) { v.visit(*this); }
void EnumPatternNode::accept(PatternVisitor& v) { v.visit(*this); }
void TuplePatternNode::accept(PatternVisitor& v) { v.visit(*this); }
void MethodCallExpr::accept(ASTVisitor& v) { v.visit(*this); }
void PlaceholderExpr::accept(ASTVisitor& v) { v.visit(*this); }
void PlaceholderStmt::accept(ASTVisitor& v) { v.visit(*this); }
void PlaceholderTypeNode::accept(TypeVisitor& v) { v.visit(*this); }
void MacroDeclNode::accept(ASTVisitor& v) { v.visit(*this); }
void MacroCallExpr::accept(ASTVisitor& v) { v.visit(*this); }
void MacroCallStmt::accept(ASTVisitor& v) { v.visit(*this); }
void MacroExpandForStmt::accept(ASTVisitor& v) { v.visit(*this); }
} // namespace fl
