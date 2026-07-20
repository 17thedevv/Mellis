#pragma once
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/FrontEnd/MacroRegistry.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class MacroValidator : public ASTVisitor, public PatternVisitor, public TypeVisitor {
public:
    MacroValidator(MacroRegistry& registry, DiagnosticEngine& diag)
        : registry_(registry), diag_(diag) {}

    void validate(ASTNode& node) {
        node.accept(*this);
    }
    TypeVisitor& typeVisitor() { return *this; }
    void visit(ProgramNode& node) override;
    void visit(VarDeclNode& node) override;
    void visit(ParamDeclNode& node) override;
    void visit(FunctionDeclNode& node) override;
    void visit(StructFieldNode& node) override;
    void visit(StructDeclNode& node) override;
    void visit(EnumVariantNode& node) override;
    void visit(EnumDeclNode& node) override;
    void visit(TraitDeclNode& node) override;
    void visit(ImplDeclNode& node) override;
    void visit(ModDeclNode& node) override;
    void visit(UseDeclNode& node) override;
    void visit(ExternDeclNode& node) override;
    void visit(TypeAliasDeclNode& node) override;
    void visit(LiteralExpr& node) override;
    void visit(IdentifierExpr& node) override;
    void visit(BinaryExpr& node) override;
    void visit(UnaryExpr& node) override;
    void visit(AssignExpr& node) override;
    void visit(CallExpr& node) override;
    void visit(MethodCallExpr& node) override;
    void visit(IndexExpr& node) override;
    void visit(MemberExpr& node) override;
    void visit(TupleIndexExpr& node) override;
    void visit(CastExpr& node) override;
    void visit(UnsizeCastExpr& node) override;
    void visit(ArrayLiteralExpr& node) override;
    void visit(TupleLiteralExpr& node) override;
    void visit(StructInitExpr& node) override;
    void visit(MatchExpr& node) override;
    void visit(PlaceholderExpr& node) override;
    void visit(LambdaExpr& node) override;
    void visit(AwaitExpr& node) override;
    void visit(SizeofExpr& node) override;
    void visit(AlignofExpr& node) override;
    void visit(PlaceholderStmt& node) override;
    void visit(BlockStmtNode& node) override;
    void visit(ExprStmtNode& node) override;
    void visit(IfStmtNode& node) override;
    void visit(WhileStmtNode& node) override;
    void visit(ForStmtNode& node) override;
    void visit(ReturnStmtNode& node) override;
    void visit(BreakStmtNode& node) override;
    void visit(ContinueStmtNode& node) override;
    void visit(UnsafeStmtNode& node) override;
    void visit(ComptimeStmtNode& node) override;
    void visit(BuiltinTypeNode& node) override;
    void visit(NamedTypeNode& node) override;
    void visit(ReferenceTypeNode& node) override;
    void visit(PointerTypeNode& node) override;
    void visit(ArrayTypeNode& node) override;
    void visit(TupleTypeNode& node) override;
    void visit(FunctionTypeNode& node) override;
    void visit(NeverTypeNode& node) override;
    void visit(TraitObjectTypeNode& node) override;
    void visit(PlaceholderTypeNode& node) override;
    void visit(WildcardPatternNode& node) override;
    void visit(LiteralPatternNode& node) override;
    void visit(IdentifierPatternNode& node) override;
    void visit(EnumPatternNode& node) override;
    void visit(TuplePatternNode& node) override;
    void visit(MacroDeclNode& node) override;
    void visit(MacroCallExpr& node) override;
    void visit(MacroCallStmt& node) override;
    void visit(MacroExpandForStmt& node) override;

private:
    void validateCall(MacroID macroId, const SourceLocation& loc, const std::vector<MacroCallArgNode>& args);
    void validateFragment(const MacroCallArgNode& arg, MacroFragSpec expected, const SourceLocation& loc);

    MacroRegistry& registry_;
    DiagnosticEngine& diag_;
};

} // namespace fl
