import re

with open('src/MiddleEnd/TypeChecker.cpp', 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Fix captures loop in ConstraintGenerator
cap_target = r'''            for (auto capId : node.captures) {
                const Type* capTy = typeTable[capId];'''
cap_repl = r'''            for (auto capRef : node.captures) {
                auto capId = capRef.symbolId;
                const Type* capTy = typeTable[capId];'''
content = content.replace(cap_target, cap_repl)

# 2. Fix PatternResolverVisitor abstract class error
pat_res_target = r'''    class PatternResolverVisitor : public PatternVisitor {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        std::vector<const Type*>& typeTable;
        const Type* expectedType;

    public:
        PatternResolverVisitor(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, std::vector<const Type*>& typeTable, const Type* expected)
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), expectedType(expected) {}
            
        void visit(WildcardPatternNode& node) override {}
        
        void visit(LiteralPatternNode& node) override {}
        
        void visit(IdentifierPatternNode& node) override {'''

pat_res_repl = r'''    class PatternResolverVisitor : public PatternVisitor {
        SymbolTable& table;
        DiagnosticEngine& diag;
        TypeContext& ctx;
        std::vector<const Type*>& typeTable;
        const Type* expectedType;

    public:
        PatternResolverVisitor(SymbolTable& table, DiagnosticEngine& diag, TypeContext& ctx, std::vector<const Type*>& typeTable, const Type* expected)
            : table(table), diag(diag), ctx(ctx), typeTable(typeTable), expectedType(expected) {}
            
        void visit(StructPatternNode& node) override {}
        void visit(WildcardPatternNode& node) override {}
        
        void visit(LiteralPatternNode& node) override {}
        
        void visit(IdentifierPatternNode& node) override {'''

content = content.replace(pat_res_target, pat_res_repl)

with open('src/MiddleEnd/TypeChecker.cpp', 'w', encoding='utf-8') as f:
    f.write(content)

print("Done")
