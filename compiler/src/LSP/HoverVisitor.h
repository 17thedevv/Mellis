#pragma once
#include "mellis/FrontEnd/ASTVisitor.h"
#include "mellis/AST/ASTNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/PatternNode.h"

namespace fl {
class HoverVisitor : public ASTVisitor, public PatternVisitor, public TypeVisitor {
public:
    uint32_t targetOffset;
    ASTNode* bestNode = nullptr;
    uint32_t bestRange = 0xFFFFFFFF;
    
    HoverVisitor(uint32_t offset) : targetOffset(offset) {}
    
    void checkNode(ASTNode& node) {
        if (targetOffset >= node.loc.offset && targetOffset <= node.endLoc.offset) {
            uint32_t range = node.endLoc.offset - node.loc.offset;
            if (range <= bestRange) {
                bestRange = range;
                bestNode = &node;
            }
        }
    }

    // We only traverse the AST branches that MIGHT contain the cursor.
    // If a node doesn't contain the cursor, we don't visit its children.
    bool shouldVisit(ASTNode& node) {
        if (targetOffset >= node.loc.offset && targetOffset <= node.endLoc.offset) {
            checkNode(node);
            return true;
        }
        return false;
    }

    void visit(ProgramNode& node) override {
        if (shouldVisit(node)) {
            for (auto& item : node.items) item->accept(*this);
        }
    }
    void visit(ModDeclNode& node) override {
        if (shouldVisit(node)) {
            for (auto& decl : node.decls) decl->accept(*this);
        }
    }
    void visit(FunctionDeclNode& node) override {
        if (shouldVisit(node)) {
            for (auto& p : node.params) p->accept(*this);
            if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
            if (node.body) node.body->accept(*this);
        }
    }
    void visit(BlockStmtNode& node) override {
        if (shouldVisit(node)) {
            for (auto& s : node.body) s->accept(*this);
        }
    }
    void visit(ExprStmtNode& node) override {
        if (shouldVisit(node)) {
            if (node.expr) node.expr->accept(*this);
        }
    }
    void visit(AssignExpr& node) override {
        if (shouldVisit(node)) {
            if (node.lvalue) node.lvalue->accept(*this);
            if (node.value) node.value->accept(*this);
        }
    }
    void visit(CallExpr& node) override {
        if (shouldVisit(node)) {
            if (node.callee) node.callee->accept(*this);
            for (auto& arg : node.args) arg.value->accept(*this);
        }
    }
    void visit(MethodCallExpr& node) override {
        if (shouldVisit(node)) {
            if (node.object) node.object->accept(*this);
            for (auto& arg : node.args) arg.value->accept(*this);
        }
    }
    void visit(MemberExpr& node) override {
        if (shouldVisit(node)) {
            if (node.object) node.object->accept(*this);
        }
    }
    void visit(IndexExpr& node) override {
        if (shouldVisit(node)) {
            if (node.base) node.base->accept(*this);
            if (node.index) node.index->accept(*this);
        }
    }
    void visit(BinaryExpr& node) override {
        if (shouldVisit(node)) {
            if (node.left) node.left->accept(*this);
            if (node.right) node.right->accept(*this);
        }
    }
    void visit(UnaryExpr& node) override {
        if (shouldVisit(node)) {
            if (node.operand) node.operand->accept(*this);
        }
    }
    void visit(IfStmtNode& node) override {
        if (shouldVisit(node)) {
            if (node.condition) node.condition->accept(*this);
            if (node.thenBranch) node.thenBranch->accept(*this);
            if (node.elseBranch) node.elseBranch->accept(*this);
        }
    }
    void visit(WhileStmtNode& node) override {
        if (shouldVisit(node)) {
            if (node.condition) node.condition->accept(*this);
            if (node.body) node.body->accept(*this);
        }
    }
    void visit(ForStmtNode& node) override {
        if (shouldVisit(node)) {
            if (node.iterable) node.iterable->accept(*this);
            if (node.body) node.body->accept(*this);
        }
    }
    void visit(ReturnStmtNode& node) override {
        if (shouldVisit(node)) {
            if (node.value) node.value->accept(*this);
        }
    }
    void visit(VarDeclNode& node) override {
        if (shouldVisit(node)) {
            if (node.typeAnnot) node.typeAnnot->accept(static_cast<TypeVisitor&>(*this));
            if (node.initializer) node.initializer->accept(*this);
        }
    }
    void visit(StructDeclNode& node) override {
        if (shouldVisit(node)) {
            for (auto& f : node.fields) f->accept(*this);
        }
    }
    void visit(StructFieldNode& node) override {
        if (shouldVisit(node)) {
            if (node.type) node.type->accept(static_cast<TypeVisitor&>(*this));
        }
    }
    void visit(ImplDeclNode& node) override {
        if (shouldVisit(node)) {
            for (auto& m : node.methods) m->accept(*this);
        }
    }
    void visit(TraitDeclNode& node) override {
        if (shouldVisit(node)) {
            for (auto& m : node.methods) m->accept(*this);
        }
    }
    void visit(StructInitExpr& node) override {
        if (shouldVisit(node)) {
            for (auto& f : node.fields) f.value->accept(*this);
        }
    }
    
    // Leaf nodes
    void visit(IdentifierExpr& node) override { shouldVisit(node); }
    void visit(LiteralExpr& node) override { shouldVisit(node); }
    void visit(TupleIndexExpr& node) override { shouldVisit(node); }
    void visit(BreakStmtNode& node) override { shouldVisit(node); }
    void visit(ContinueStmtNode& node) override { shouldVisit(node); }

    // Fallback for everything else
    void visit(ParamDeclNode& node) override { shouldVisit(node); }
    void visit(EnumDeclNode& node) override { shouldVisit(node); }
    void visit(EnumVariantNode& node) override { shouldVisit(node); }
    void visit(UseDeclNode& node) override { shouldVisit(node); }
    void visit(ExternDeclNode& node) override { shouldVisit(node); }
    void visit(TypeAliasDeclNode& node) override { shouldVisit(node); }
    void visit(UnsafeStmtNode& node) override { shouldVisit(node); }
    void visit(ComptimeStmtNode& node) override { shouldVisit(node); }
    void visit(CastExpr& node) override { shouldVisit(node); }
    void visit(UnsizeCastExpr& node) override { shouldVisit(node); }
    void visit(ArrayLiteralExpr& node) override { shouldVisit(node); }
    void visit(TupleLiteralExpr& node) override { shouldVisit(node); }
    void visit(MatchExpr& node) override { shouldVisit(node); }
    void visit(LambdaExpr& node) override { shouldVisit(node); }
    void visit(TryExpr& node) override { shouldVisit(node); }
    void visit(AwaitExpr& node) override { shouldVisit(node); }
    void visit(SizeofExpr& node) override { shouldVisit(node); }
    void visit(AlignofExpr& node) override { shouldVisit(node); }
    
    // Type visitor stubs
    void visit(BuiltinTypeNode& node) override { shouldVisit(node); }
    void visit(LifetimeNode& node) override { shouldVisit(node); }
    void visit(NamedTypeNode& node) override { shouldVisit(node); }
    void visit(ReferenceTypeNode& node) override { shouldVisit(node); }
    void visit(PointerTypeNode& node) override { shouldVisit(node); }
    void visit(ArrayTypeNode& node) override { shouldVisit(node); }
    void visit(TupleTypeNode& node) override { shouldVisit(node); }
    void visit(FunctionTypeNode& node) override { shouldVisit(node); }
    void visit(NeverTypeNode& node) override { shouldVisit(node); }
    void visit(TraitObjectTypeNode& node) override { shouldVisit(node); }
    
    // Pattern visitor stubs
    void visit(WildcardPatternNode& node) override { shouldVisit(node); }
    void visit(LiteralPatternNode& node) override { shouldVisit(node); }
    void visit(IdentifierPatternNode& node) override { shouldVisit(node); }
    void visit(EnumPatternNode& node) override { shouldVisit(node); }
    void visit(TuplePatternNode& node) override { shouldVisit(node); }
    void visit(StructPatternNode& node) override { shouldVisit(node); }
};
}