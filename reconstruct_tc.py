import re

with open('src/MiddleEnd/TypeChecker.cpp', 'r', encoding='utf-8') as f:
    content = f.read()

target = r'''        void visit(StructExpr& node) override { for (auto& field : node.fields) if (field.value) field.value->accept(*this); }'''

insertion = '''        void visit(StructExpr& node) override { for (auto& field : node.fields) if (field.value) field.value->accept(*this); }
        void visit(AlignofExpr& node) override {}
        void visit(SizeofExpr& node) override {}
        void visit(TryExpr& node) override {}
        void visit(StructInitExpr& node) override {}
        void visit(UnsizeCastExpr& node) override {}
        void visit(TupleIndexExpr& node) override {}
        void visit(MemberExpr& node) override {}
        void visit(IndexExpr& node) override {}
        void visit(IdentifierExpr& node) override {}
        void visit(LiteralExpr& node) override {}
        void visit(ComptimeStmtNode& node) override { if (node.stmt) node.stmt->accept(*this); }
        void visit(UnsafeStmtNode& node) override { if (node.body) node.body->accept(*this); }
        void visit(ContinueStmtNode& node) override {}
        void visit(BreakStmtNode& node) override {}
        void visit(IfStmtNode& node) override {}
        void visit(TypeAliasDeclNode& node) override {}
        void visit(UseDeclNode& node) override {}
        void visit(EnumVariantNode& node) override {}
        void visit(StructFieldNode& node) override {}
        void visit(ParamDeclNode& node) override {}
        void visit(VarDeclNode& node) override { if (node.initializer) node.initializer->accept(*this); }'''

if target in content:
    content = content.replace(target, insertion)
    with open('src/MiddleEnd/TypeChecker.cpp', 'w', encoding='utf-8') as f:
        f.write(content)
    print("Done")
else:
    print("Target not found")
