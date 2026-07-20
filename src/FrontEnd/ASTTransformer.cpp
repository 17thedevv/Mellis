#include "mellis/FrontEnd/ASTTransformer.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/PatternNode.h"

namespace fl {
void ASTTransformer::visit(VarDeclNode& node) {
    if (node.typeAnnot) node.typeAnnot = transformType(std::move(node.typeAnnot));
    if (node.initializer) node.initializer = transformExpr(std::move(node.initializer));
}

void ASTTransformer::visit(ParamDeclNode& node) {
    if (node.type) node.type = transformType(std::move(node.type));
}

std::vector<std::unique_ptr<ItemNode>> ASTTransformer::transformItemList(std::vector<std::unique_ptr<ItemNode>> list) {
    std::vector<std::unique_ptr<ItemNode>> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (item) {
            auto transformed = transformNode(std::move(item));
            if (transformed) {
                newList.push_back(std::unique_ptr<ItemNode>(static_cast<ItemNode*>(transformed.release())));
            }
        }
    }
    return newList;
}

void ASTTransformer::visit(FunctionDeclNode& node) {
    if (node.returnType) node.returnType = transformType(std::move(node.returnType));
    if (node.body) node.body = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.body)).release()));
    for (auto& item : node.params) {
        if (item) item = std::unique_ptr<ParamDeclNode>(static_cast<ParamDeclNode*>(transformNode(std::move(item)).release()));
    }
    for (auto& gp : node.genericParams) {
        for (auto& bound : gp.bounds) {
            if (bound) bound = transformType(std::move(bound));
        }
    }
}

void ASTTransformer::visit(StructFieldNode& node) {
    if (node.type) node.type = transformType(std::move(node.type));
}

void ASTTransformer::visit(StructDeclNode& node) {
    for (auto& item : node.fields) {
        if (item) item = std::unique_ptr<StructFieldNode>(static_cast<StructFieldNode*>(transformNode(std::move(item)).release()));
    }
    for (auto& gp : node.genericParams) {
        for (auto& bound : gp.bounds) {
            if (bound) bound = transformType(std::move(bound));
        }
    }
}

void ASTTransformer::visit(EnumVariantNode& node) {
    for (auto& item : node.fields) {
        if (item) item = std::unique_ptr<ParamDeclNode>(static_cast<ParamDeclNode*>(transformNode(std::move(item)).release()));
    }
}

void ASTTransformer::visit(EnumDeclNode& node) {
    for (auto& item : node.variants) {
        if (item) item = std::unique_ptr<EnumVariantNode>(static_cast<EnumVariantNode*>(transformNode(std::move(item)).release()));
    }
}

void ASTTransformer::visit(TraitDeclNode& node) {
    for (auto& item : node.methods) {
        if (item) item = std::unique_ptr<FunctionDeclNode>(static_cast<FunctionDeclNode*>(transformNode(std::move(item)).release()));
    }
    for (auto& gp : node.genericParams) {
        for (auto& bound : gp.bounds) {
            if (bound) bound = transformType(std::move(bound));
        }
    }
}

void ASTTransformer::visit(ImplDeclNode& node) {
    if (node.selfType) node.selfType = transformType(std::move(node.selfType));
    if (node.traitType) node.traitType = transformType(std::move(node.traitType));
    for (auto& item : node.methods) {
        if (item) item = std::unique_ptr<FunctionDeclNode>(static_cast<FunctionDeclNode*>(transformNode(std::move(item)).release()));
    }
    for (auto& gp : node.genericParams) {
        for (auto& bound : gp.bounds) {
            if (bound) bound = transformType(std::move(bound));
        }
    }
}

void ASTTransformer::visit(ModDeclNode& node) {
    for (auto& item : node.decls) {
        if (item) item = transformDecl(std::move(item));
    }
}

void ASTTransformer::visit(UseDeclNode& node) {
}

void ASTTransformer::visit(ExternDeclNode& node) {
    if (node.func) node.func = std::unique_ptr<FunctionDeclNode>(static_cast<FunctionDeclNode*>(transformNode(std::move(node.func)).release()));
}

void ASTTransformer::visit(TypeAliasDeclNode& node) {
    if (node.aliasedType) node.aliasedType = transformType(std::move(node.aliasedType));
}

void ASTTransformer::visit(LiteralExpr& node) {
}

void ASTTransformer::visit(IdentifierExpr& node) {
    for (auto& item : node.genericArgs) {
        if (item) item = transformType(std::move(item));
    }
}

void ASTTransformer::visit(BinaryExpr& node) {
    if (node.left) node.left = transformExpr(std::move(node.left));
    if (node.right) node.right = transformExpr(std::move(node.right));
}

void ASTTransformer::visit(UnaryExpr& node) {
    if (node.operand) node.operand = transformExpr(std::move(node.operand));
}

void ASTTransformer::visit(AssignExpr& node) {
    if (node.lvalue) node.lvalue = transformExpr(std::move(node.lvalue));
    if (node.value) node.value = transformExpr(std::move(node.value));
}

void ASTTransformer::visit(CallExpr& node) {
    if (node.callee) node.callee = transformExpr(std::move(node.callee));
    for (auto& item : node.genericArgs) {
        if (item) item = transformType(std::move(item));
    }
}

void ASTTransformer::visit(MethodCallExpr& node) {
    if (node.object) node.object = transformExpr(std::move(node.object));
    for (auto& item : node.genericArgs) {
        if (item) item = transformType(std::move(item));
    }
}

void ASTTransformer::visit(IndexExpr& node) {
    if (node.base) node.base = transformExpr(std::move(node.base));
    if (node.index) node.index = transformExpr(std::move(node.index));
}

void ASTTransformer::visit(MemberExpr& node) {
    if (node.object) node.object = transformExpr(std::move(node.object));
}

void ASTTransformer::visit(TupleIndexExpr& node) {
    if (node.object) node.object = transformExpr(std::move(node.object));
}

void ASTTransformer::visit(CastExpr& node) {
    if (node.expr) node.expr = transformExpr(std::move(node.expr));
    if (node.targetType) node.targetType = transformType(std::move(node.targetType));
}

void ASTTransformer::visit(UnsizeCastExpr& node) {
    if (node.expr) node.expr = transformExpr(std::move(node.expr));
}

void ASTTransformer::visit(ArrayLiteralExpr& node) {
    node.elements = transformExprList(std::move(node.elements));
}

void ASTTransformer::visit(TupleLiteralExpr& node) {
    node.elements = transformExprList(std::move(node.elements));
}

void ASTTransformer::visit(StructInitExpr& node) {
    for (auto& item : node.genericArgs) {
        if (item) item = transformType(std::move(item));
    }
    for (auto& field : node.fields) {
        if (field.value) field.value = transformExpr(std::move(field.value));
    }
}

void ASTTransformer::visit(MatchExpr& node) {
    if (node.subject) node.subject = transformExpr(std::move(node.subject));
    for (auto& arm : node.arms) {
        if (arm.pattern) arm.pattern = transformPattern(std::move(arm.pattern));
        if (arm.body) arm.body = transformStmt(std::move(arm.body));
    }
}

void ASTTransformer::visit(PlaceholderExpr& node) {
}

void ASTTransformer::visit(LambdaExpr& node) {
    if (node.returnType) node.returnType = transformType(std::move(node.returnType));
    if (node.body) node.body = transformStmt(std::move(node.body));
    for (auto& item : node.params) {
        if (item) item = std::unique_ptr<ParamDeclNode>(static_cast<ParamDeclNode*>(transformNode(std::move(item)).release()));
    }
}

void ASTTransformer::visit(AwaitExpr& node) {
    if (node.expr) node.expr = transformExpr(std::move(node.expr));
}

void ASTTransformer::visit(SizeofExpr& node) {
    if (node.targetType) node.targetType = transformType(std::move(node.targetType));
}

void ASTTransformer::visit(AlignofExpr& node) {
    if (node.targetType) node.targetType = transformType(std::move(node.targetType));
}

void ASTTransformer::visit(PlaceholderStmt& node) {
}

void ASTTransformer::visit(BlockStmtNode& node) {
    node.body = transformItemList(std::move(node.body));
}

void ASTTransformer::visit(ExprStmtNode& node) {
    if (node.expr) node.expr = transformExpr(std::move(node.expr));
}

void ASTTransformer::visit(IfStmtNode& node) {
    if (node.condition) node.condition = transformExpr(std::move(node.condition));
    if (node.thenBranch) node.thenBranch = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.thenBranch)).release()));
    if (node.elseBranch) node.elseBranch = transformStmt(std::move(node.elseBranch));
}

void ASTTransformer::visit(WhileStmtNode& node) {
    if (node.condition) node.condition = transformExpr(std::move(node.condition));
    if (node.body) node.body = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.body)).release()));
}

void ASTTransformer::visit(ForStmtNode& node) {
    if (node.iterable) node.iterable = transformExpr(std::move(node.iterable));
    if (node.init) node.init = std::unique_ptr<ItemNode>(static_cast<ItemNode*>(transformNode(std::move(node.init)).release()));
    if (node.cond) node.cond = transformExpr(std::move(node.cond));
    if (node.step) node.step = transformExpr(std::move(node.step));
    if (node.body) node.body = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.body)).release()));
}

void ASTTransformer::visit(ReturnStmtNode& node) {
    if (node.value) node.value = transformExpr(std::move(node.value));
}

void ASTTransformer::visit(BreakStmtNode& node) {
}

void ASTTransformer::visit(ContinueStmtNode& node) {
}

void ASTTransformer::visit(UnsafeStmtNode& node) {
    if (node.body) node.body = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.body)).release()));
}

void ASTTransformer::visit(ComptimeStmtNode& node) {
    if (node.body) node.body = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.body)).release()));
}

void ASTTransformer::visit(BuiltinTypeNode& node) {
}

void ASTTransformer::visit(NamedTypeNode& node) {
    for (auto& item : node.genericArgs) {
        if (item) item = transformType(std::move(item));
    }
}

void ASTTransformer::visit(ReferenceTypeNode& node) {
    if (node.inner) node.inner = transformType(std::move(node.inner));
}

void ASTTransformer::visit(PointerTypeNode& node) {
    if (node.inner) node.inner = transformType(std::move(node.inner));
}

void ASTTransformer::visit(ArrayTypeNode& node) {
    if (node.elementType) node.elementType = transformType(std::move(node.elementType));
    if (node.size) node.size = transformExpr(std::move(node.size));
}

void ASTTransformer::visit(TupleTypeNode& node) {
    for (auto& item : node.elements) {
        if (item) item = transformType(std::move(item));
    }
}

void ASTTransformer::visit(FunctionTypeNode& node) {
    if (node.returnType) node.returnType = transformType(std::move(node.returnType));
    for (auto& item : node.params) {
        if (item) item = transformType(std::move(item));
    }
}

void ASTTransformer::visit(NeverTypeNode& node) {
}

void ASTTransformer::visit(TraitObjectTypeNode& node) {
    if (node.trait) node.trait = transformType(std::move(node.trait));
}

void ASTTransformer::visit(PlaceholderTypeNode& node) {
}

void ASTTransformer::visit(WildcardPatternNode& node) {
}

void ASTTransformer::visit(LiteralPatternNode& node) {
    if (node.lit) node.lit = std::unique_ptr<LiteralExpr>(static_cast<LiteralExpr*>(transformNode(std::move(node.lit)).release()));
}

void ASTTransformer::visit(IdentifierPatternNode& node) {
}

void ASTTransformer::visit(EnumPatternNode& node) {
    for (auto& item : node.fields) {
        if (item) item = transformPattern(std::move(item));
    }
}

void ASTTransformer::visit(TuplePatternNode& node) {
    for (auto& item : node.elements) {
        if (item) item = transformPattern(std::move(item));
    }
}

void ASTTransformer::visit(MacroDeclNode& node) {
    // DO NOT transform the body of a MacroDeclNode, because its body is a template!
    // Expanding macros inside a template before instantiation is wrong, as placeholders are unbound.
}

void ASTTransformer::visit(MacroCallExpr& node) {
    node.args = transformMacroCallArgList(std::move(node.args));
}

void ASTTransformer::visit(MacroCallStmt& node) {
    node.args = transformMacroCallArgList(std::move(node.args));
}

void ASTTransformer::visit(MacroExpandForStmt& node) {
    if (node.body) node.body = std::unique_ptr<BlockStmtNode>(static_cast<BlockStmtNode*>(transformStmt(std::move(node.body)).release()));
}


std::unique_ptr<ASTNode> ASTTransformer::transformNode(std::unique_ptr<ASTNode> node) {
    if (!node) return nullptr;
    result_ = nullptr;
    node->accept(*this);
    if (result_) {
        return std::move(result_);
    }
    return node;
}

std::unique_ptr<DeclNode> ASTTransformer::transformDecl(std::unique_ptr<DeclNode> node) {
    if (!node) return nullptr;
    result_ = nullptr;
    node->accept(*this);
    if (result_) {
        return std::unique_ptr<DeclNode>(static_cast<DeclNode*>(result_.release()));
    }
    return node;
}

std::unique_ptr<ExprNode> ASTTransformer::transformExpr(std::unique_ptr<ExprNode> node) {
    if (!node) return nullptr;
    result_ = nullptr;
    node->accept(*this);
    if (result_) {
        return std::unique_ptr<ExprNode>(static_cast<ExprNode*>(result_.release()));
    }
    return node;
}

std::unique_ptr<StmtNode> ASTTransformer::transformStmt(std::unique_ptr<StmtNode> node) {
    if (!node) return nullptr;
    result_ = nullptr;
    node->accept(*this);
    if (result_) {
        return std::unique_ptr<StmtNode>(static_cast<StmtNode*>(result_.release()));
    }
    return node;
}

std::unique_ptr<TypeNode> ASTTransformer::transformType(std::unique_ptr<TypeNode> node) {
    if (!node) return nullptr;
    result_ = nullptr;
    node->accept(*this);
    if (result_) {
        return std::unique_ptr<TypeNode>(static_cast<TypeNode*>(result_.release()));
    }
    return node;
}

std::unique_ptr<PatternNode> ASTTransformer::transformPattern(std::unique_ptr<PatternNode> node) {
    if (!node) return nullptr;
    result_ = nullptr;
    node->accept(*this);
    if (result_) {
        return std::unique_ptr<PatternNode>(static_cast<PatternNode*>(result_.release()));
    }
    return node;
}

void ASTTransformer::visit(ProgramNode& node) {
    node.items = transformItemList(std::move(node.items));
}
} // namespace fl

namespace fl {
std::vector<std::unique_ptr<ExprNode>> ASTTransformer::transformExprList(std::vector<std::unique_ptr<ExprNode>> list) {
    std::vector<std::unique_ptr<ExprNode>> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (!item) continue;
        auto transformed = transformExpr(std::move(item));
        if (transformed) {
            newList.push_back(std::move(transformed));
        }
    }
    return newList;
}

std::vector<std::unique_ptr<StmtNode>> ASTTransformer::transformStmtList(std::vector<std::unique_ptr<StmtNode>> list) {
    std::vector<std::unique_ptr<StmtNode>> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (!item) continue;
        auto transformed = transformStmt(std::move(item));
        if (transformed) {
            newList.push_back(std::move(transformed));
        }
    }
    return newList;
}

std::vector<MacroCallArgNode> ASTTransformer::transformMacroCallArgList(std::vector<MacroCallArgNode> list) {
    std::vector<MacroCallArgNode> newList;
    newList.reserve(list.size());
    for (auto& item : list) {
        if (item.node) {
            item.node = transformNode(std::move(item.node));
            if (item.node) newList.push_back(std::move(item));
        }
    }
    return newList;
}

}
