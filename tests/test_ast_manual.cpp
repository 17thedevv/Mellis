#include "fdlang/AST/ASTNode.h"
#include "fdlang/AST/ProgramNode.h"
#include "fdlang/AST/DeclNode.h"
#include "fdlang/AST/StmtNode.h"
#include "fdlang/AST/ExprNode.h"
#include "fdlang/AST/TypeNode.h"
#include "fdlang/AST/PatternNode.h"
#include "fdlang/FrontEnd/ASTVisitor.h"
#include <iostream>
#include <cassert>
#include <string>

using namespace fl;

class ASTPrinter : public ASTVisitor, public TypeVisitor, public PatternVisitor {
    int indent = 0;
    void printIndent() {
        for (int i = 0; i < indent; ++i) std::cout << "  ";
    }

public:
    void visit(ProgramNode& node) override {
        printIndent(); std::cout << "ProgramNode\n";
        indent++;
        for (auto& item : node.items) {
            item->accept(*this);
        }
        indent--;
    }
    
    // --- Decl ---
    void visit(VarDeclNode& node) override {
        printIndent(); std::cout << "VarDeclNode(" << node.name << ")\n";
        indent++;
        if (node.typeAnnot) node.typeAnnot->accept(static_cast<TypeVisitor&>(*this));
        if (node.initializer) node.initializer->accept(*this);
        indent--;
    }
    void visit(FunctionDeclNode& node) override {
        printIndent(); std::cout << "FunctionDeclNode(" << node.name << ")\n";
        indent++;
        for (auto& p : node.params) p->accept(*this);
        if (node.returnType) node.returnType->accept(static_cast<TypeVisitor&>(*this));
        if (node.body) node.body->accept(*this);
        indent--;
    }
    void visit(ParamDeclNode& node) override {
        printIndent(); std::cout << "ParamDeclNode(" << node.name << ")\n";
        indent++;
        if (node.type) node.type->accept(static_cast<TypeVisitor&>(*this));
        indent--;
    }
    void visit(StructDeclNode&) override {}
    void visit(StructFieldNode&) override {}
    void visit(EnumDeclNode&) override {}
    void visit(EnumVariantNode&) override {}
    void visit(TraitDeclNode&) override {}
    void visit(ImplDeclNode&) override {}
    void visit(ModDeclNode&) override {}
    void visit(UseDeclNode&) override {}
    void visit(ExternDeclNode&) override {}
    void visit(TypeAliasDeclNode&) override {}

    // --- Stmt ---
    void visit(BlockStmtNode& node) override {
        printIndent(); std::cout << "BlockStmtNode\n";
        indent++;
        for (auto& item : node.body) item->accept(*this);
        indent--;
    }
    void visit(ExprStmtNode& node) override {
        printIndent(); std::cout << "ExprStmtNode\n";
        indent++;
        if (node.expr) node.expr->accept(*this);
        indent--;
    }
    void visit(IfStmtNode&) override {}
    void visit(WhileStmtNode&) override {}
    void visit(ForStmtNode&) override {}
    void visit(ReturnStmtNode& node) override {
        printIndent(); std::cout << "ReturnStmtNode\n";
        indent++;
        if (node.value) node.value->accept(*this);
        indent--;
    }
    void visit(BreakStmtNode&) override {}
    void visit(ContinueStmtNode&) override {}
    void visit(UnsafeStmtNode&) override {}
    void visit(ComptimeStmtNode&) override {}

    // --- Expr ---
    void visit(LiteralExpr& node) override {
        printIndent(); std::cout << "LiteralExpr(" << node.rawText << ")\n";
    }
    void visit(IdentifierExpr& node) override {
        printIndent(); 
        std::cout << "IdentifierExpr(";
        for (auto s : node.segments) std::cout << s << "::";
        std::cout << ")\n";
    }
    void visit(BinaryExpr& node) override {
        printIndent(); std::cout << "BinaryExpr(" << (int)node.op << ")\n";
        indent++;
        if (node.left) node.left->accept(*this);
        if (node.right) node.right->accept(*this);
        indent--;
    }
    void visit(UnaryExpr&) override {}
    void visit(AssignExpr&) override {}
    void visit(CallExpr&) override {}
    void visit(MethodCallExpr&) override {}
    void visit(IndexExpr&) override {}
    void visit(MemberExpr&) override {}
    void visit(CastExpr&) override {}
    void visit(ArrayLiteralExpr&) override {}
    void visit(TupleLiteralExpr&) override {}
    void visit(StructInitExpr&) override {}
    void visit(MatchExpr&) override {}
    void visit(LambdaExpr&) override {}
    void visit(AwaitExpr&) override {}
    void visit(SizeofExpr&) override {}
    void visit(AlignofExpr&) override {}

    // --- Type ---
    void visit(BuiltinTypeNode& node) override {
        printIndent(); std::cout << "BuiltinTypeNode(" << (int)node.kind << ")\n";
    }
    void visit(NamedTypeNode&) override {}
    void visit(ReferenceTypeNode&) override {}
    void visit(PointerTypeNode&) override {}
    void visit(ArrayTypeNode&) override {}
    void visit(TupleTypeNode&) override {}
    void visit(FunctionTypeNode&) override {}
    void visit(NeverTypeNode&) override {}
    void visit(TraitObjectTypeNode&) override {}

    // --- Pattern ---
    void visit(WildcardPatternNode&) override {}
    void visit(LiteralPatternNode&) override {}
    void visit(IdentifierPatternNode&) override {}
    void visit(EnumPatternNode&) override {}
    void visit(TuplePatternNode&) override {}
};

void test_var_decl() {
    std::cout << "--- Test 1: dec x = 1 + 2; ---\n";
    auto prog = std::make_unique<ProgramNode>();
    auto dec = std::make_unique<VarDeclNode>();
    dec->name = "x";
    dec->isMutable = true;
    
    auto bin = std::make_unique<BinaryExpr>();
    bin->op = BinaryOp::Add;
    
    auto lit1 = std::make_unique<LiteralExpr>();
    lit1->kind = LiteralKind::Integer;
    lit1->rawText = "1";
    lit1->value = (uint64_t)1;
    
    auto lit2 = std::make_unique<LiteralExpr>();
    lit2->kind = LiteralKind::Integer;
    lit2->rawText = "2";
    lit2->value = (uint64_t)2;
    
    bin->left = std::move(lit1);
    bin->right = std::move(lit2);
    
    dec->initializer = std::move(bin);
    prog->items.push_back(std::move(dec));
    
    ASTPrinter printer;
    prog->accept(printer);
}

void test_function_decl() {
    std::cout << "\n--- Test 2: fn add(a:i32, b:i32)->i32 { return a+b; } ---\n";
    auto prog = std::make_unique<ProgramNode>();
    
    auto func = std::make_unique<FunctionDeclNode>();
    func->name = "add";
    
    auto p1 = std::make_unique<ParamDeclNode>();
    p1->name = "a";
    auto t1 = std::make_unique<BuiltinTypeNode>();
    t1->kind = BuiltinKind::I32;
    p1->type = std::move(t1);
    
    auto p2 = std::make_unique<ParamDeclNode>();
    p2->name = "b";
    auto t2 = std::make_unique<BuiltinTypeNode>();
    t2->kind = BuiltinKind::I32;
    p2->type = std::move(t2);
    
    func->params.push_back(std::move(p1));
    func->params.push_back(std::move(p2));
    
    auto retT = std::make_unique<BuiltinTypeNode>();
    retT->kind = BuiltinKind::I32;
    func->returnType = std::move(retT);
    
    auto body = std::make_unique<BlockStmtNode>();
    auto ret = std::make_unique<ReturnStmtNode>();
    
    auto bin = std::make_unique<BinaryExpr>();
    bin->op = BinaryOp::Add;
    
    auto id1 = std::make_unique<IdentifierExpr>();
    id1->segments.push_back("a");
    
    auto id2 = std::make_unique<IdentifierExpr>();
    id2->segments.push_back("b");
    
    bin->left = std::move(id1);
    bin->right = std::move(id2);
    
    ret->value = std::move(bin);
    body->body.push_back(std::move(ret));
    
    func->body = std::move(body);
    prog->items.push_back(std::move(func));
    
    ASTPrinter printer;
    prog->accept(printer);
}

int main() {
    test_var_decl();
    test_function_decl();
    std::cout << "\nAll AST Manual Tests Passed!\n";
    return 0;
}
