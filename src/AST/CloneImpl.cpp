#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/TypeNode.h"
#include "mellis/AST/StmtNode.h"

#include "mellis/AST/StmtNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/AST/MacroNode.h"

namespace fl {

static std::vector<AnnotationNode> cloneAnnotations(const std::vector<AnnotationNode>& src) {
    std::vector<AnnotationNode> dst;
    for (const auto& a : src) {
        AnnotationNode an;
        an.name = a.name;
        an.line = a.line;
        an.column = a.column;
        for (const auto& arg : a.args) {
            AnnotationArg newArg;
            newArg.key = arg.key;
            if (arg.value) {
                newArg.value = arg.value->cloneAs<ExprNode>();
            }
            an.args.push_back(std::move(newArg));
        }
        dst.push_back(std::move(an));
    }
    return dst;
}

// ==========================================
// DeclNode Clones
// ==========================================

ASTNode* VarDeclNode::cloneImpl() const {
    auto copy = new VarDeclNode();
    copy->loc = this->loc;
    copy->annotations = cloneAnnotations(this->annotations);
    copy->isExported = this->isExported;
    copy->name = this->name;
    copy->isMutable = this->isMutable;
    if (this->typeAnnot) {
        copy->typeAnnot = this->typeAnnot->cloneAs<TypeNode>();
    }
    if (this->initializer) {
        copy->initializer = this->initializer->cloneAs<ExprNode>();
    }
    return copy;
}

ASTNode* ParamDeclNode::cloneImpl() const {
    auto copy = new ParamDeclNode();
    copy->loc = this->loc;
    copy->annotations = cloneAnnotations(this->annotations);
    copy->isExported = this->isExported;
    copy->name = this->name;
    copy->isVariadic = this->isVariadic;
    copy->isSelf = this->isSelf;
    if (this->type) {
        copy->type = this->type->cloneAs<TypeNode>();
    }
    return copy;
}

ASTNode* FunctionDeclNode::cloneImpl() const {
    auto copy = new FunctionDeclNode();
    copy->loc = this->loc;
    copy->annotations = cloneAnnotations(this->annotations);
    copy->isExported = this->isExported;
    copy->name = this->name;
    copy->isAsync = this->isAsync;
    copy->isComptime = this->isComptime;
    copy->isVariadic = this->isVariadic;
    
    // Note: genericParams are usually dropped during monomorphization,
    // but if we clone them verbatim, we do it here:
    for (const auto& gp : this->genericParams) {
        GenericParamNode gpCopy;
        gpCopy.loc = gp.loc;
        gpCopy.name = gp.name;
        gpCopy.symbolId = gp.symbolId;
        for (const auto& b : gp.bounds) {
            gpCopy.bounds.push_back(b->cloneAs<TypeNode>());
        }
        copy->genericParams.push_back(std::move(gpCopy));
    }
    
    for (const auto& p : this->params) {
        copy->params.push_back(p->cloneAs<ParamDeclNode>());
    }
    if (this->returnType) {
        copy->returnType = this->returnType->cloneAs<TypeNode>();
    }
    if (this->body) {
        copy->body = this->body->cloneAs<BlockStmtNode>();
    }
    return copy;
}

ASTNode* StructFieldNode::cloneImpl() const {
    auto copy = new StructFieldNode();
    copy->loc = this->loc;
    copy->name = this->name;
    if (this->type) {
        copy->type = this->type->cloneAs<TypeNode>();
    }
    return copy;
}

ASTNode* StructDeclNode::cloneImpl() const {
    auto copy = new StructDeclNode();
    copy->loc = this->loc;
    copy->annotations = cloneAnnotations(this->annotations);
    copy->isExported = this->isExported;
    copy->name = this->name;
    
    for (const auto& gp : this->genericParams) {
        GenericParamNode gpCopy;
        gpCopy.loc = gp.loc;
        gpCopy.name = gp.name;
        gpCopy.symbolId = gp.symbolId;
        for (const auto& b : gp.bounds) {
            gpCopy.bounds.push_back(b->cloneAs<TypeNode>());
        }
        copy->genericParams.push_back(std::move(gpCopy));
    }
    
    for (const auto& f : this->fields) {
        copy->fields.push_back(f->cloneAs<StructFieldNode>());
    }
    return copy;
}

// ==========================================
// StmtNode Clones
// ==========================================

ASTNode* BlockStmtNode::cloneImpl() const {
    auto copy = new BlockStmtNode();
    copy->loc = this->loc;
    copy->bodyScopeId = kInvalidSymbolID; // RESET so Resolver creates a new scope!
    for (const auto& s : this->body) {
        copy->body.push_back(s->cloneAs<ItemNode>());
    }
    if (this->tailExpr) {
        copy->tailExpr = this->tailExpr->cloneAs<ExprNode>();
    }
    copy->inferredType = this->inferredType;
    return copy;
}

ASTNode* ExprStmtNode::cloneImpl() const {
    auto copy = new ExprStmtNode();
    copy->loc = this->loc;
    if (this->expr) {
        copy->expr = this->expr->cloneAs<ExprNode>();
    }
    copy->hasSemicolon = this->hasSemicolon;
    return copy;
}

ASTNode* ReturnStmtNode::cloneImpl() const {
    auto copy = new ReturnStmtNode();
    copy->loc = this->loc;
    if (this->value) {
        copy->value = this->value->cloneAs<ExprNode>();
    }
    return copy;
}

// ==========================================
// ExprNode Clones
// ==========================================

ASTNode* IdentifierExpr::cloneImpl() const {
    auto copy = new IdentifierExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    
    copy->segments = this->segments;
    for (const auto& arg : this->genericArgs) {
        copy->genericArgs.push_back(arg->cloneAs<TypeNode>());
    }
    return copy;
}

ASTNode* StructInitExpr::cloneImpl() const {
    auto copy = new StructInitExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    
    copy->path = this->path;
    copy->structId = this->structId;
    for (const auto& arg : this->genericArgs) {
        copy->genericArgs.push_back(arg->cloneAs<TypeNode>());
    }
    for (const auto& f : this->fields) {
        FieldInitNode fin;
        fin.loc = f.loc;
        fin.name = f.name;
        fin.value = f.value->cloneAs<ExprNode>();
        copy->fields.push_back(std::move(fin));
    }
    return copy;
}

ASTNode* CallExpr::cloneImpl() const {
    auto copy = new CallExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    
    if (this->callee) {
        copy->callee = this->callee->cloneAs<ExprNode>();
    }
    for (const auto& arg : this->genericArgs) {
        copy->genericArgs.push_back(arg->cloneAs<TypeNode>());
    }
    for (const auto& a : this->args) {
        CallArgNode ca;
        ca.loc = a.loc;
        ca.label = a.label;
        ca.value = a.value->cloneAs<ExprNode>();
        copy->args.push_back(std::move(ca));
    }
    return copy;
}

ASTNode* MethodCallExpr::cloneImpl() const {
    auto copy = new MethodCallExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    
    copy->methodName = this->methodName;
    if (this->object) {
        copy->object = this->object->cloneAs<ExprNode>();
    }
    for (const auto& arg : this->genericArgs) {
        copy->genericArgs.push_back(arg->cloneAs<TypeNode>());
    }
    for (const auto& a : this->args) {
        CallArgNode ca;
        ca.loc = a.loc;
        ca.label = a.label;
        ca.value = a.value->cloneAs<ExprNode>();
        copy->args.push_back(std::move(ca));
    }
    return copy;
}

// ==========================================
// TypeNode Clones
// ==========================================

ASTNode* BuiltinTypeNode::cloneImpl() const {
    auto copy = new BuiltinTypeNode();
    copy->loc = this->loc;
    copy->kind = this->kind;
    return copy;
}

ASTNode* NeverTypeNode::cloneImpl() const {
    auto copy = new NeverTypeNode();
    copy->loc = this->loc;
    return copy;
}

ASTNode* TraitObjectTypeNode::cloneImpl() const {
    auto copy = new TraitObjectTypeNode();
    copy->loc = this->loc;
    if (this->trait) copy->trait = this->trait->cloneAs<TypeNode>();
    return copy;
}

ASTNode* NamedTypeNode::cloneImpl() const {
    auto copy = new NamedTypeNode();
    copy->loc = this->loc;
    copy->segments = this->segments;
    copy->symbolId = this->symbolId;
    for (const auto& arg : this->genericArgs) {
        copy->genericArgs.push_back(arg->cloneAs<TypeNode>());
    }
    return copy;
}

ASTNode* ReferenceTypeNode::cloneImpl() const {
    auto copy = new ReferenceTypeNode();
    copy->loc = this->loc;
    copy->isMutable = this->isMutable;
    if (this->inner) {
        copy->inner = this->inner->cloneAs<TypeNode>();
    }
    return copy;
}

ASTNode* PointerTypeNode::cloneImpl() const {
    auto copy = new PointerTypeNode();
    copy->loc = this->loc;
    copy->isMutable = this->isMutable;
    if (this->inner) {
        copy->inner = this->inner->cloneAs<TypeNode>();
    }
    return copy;
}

ASTNode* ArrayTypeNode::cloneImpl() const {
    auto copy = new ArrayTypeNode();
    copy->loc = this->loc;
    if (this->elementType) copy->elementType = this->elementType->cloneAs<TypeNode>();
    if (this->size) copy->size = this->size->cloneAs<ExprNode>();
    copy->resolvedSize = this->resolvedSize;
    return copy;
}

ASTNode* TupleTypeNode::cloneImpl() const {
    auto copy = new TupleTypeNode();
    copy->loc = this->loc;
    for (const auto& e : this->elements) {
        copy->elements.push_back(e->cloneAs<TypeNode>());
    }
    return copy;
}

ASTNode* FunctionTypeNode::cloneImpl() const {
    auto copy = new FunctionTypeNode();
    copy->loc = this->loc;
    for (const auto& p : this->params) {
        copy->params.push_back(p->cloneAs<TypeNode>());
    }
    if (this->returnType) {
        copy->returnType = this->returnType->cloneAs<TypeNode>();
    }
    return copy;
}
ASTNode* TraitDeclNode::cloneImpl() const {
    auto copy = new TraitDeclNode();
    copy->loc = this->loc;
    copy->annotations = cloneAnnotations(this->annotations);
    copy->isExported = this->isExported;
    copy->name = this->name;
    
    for (const auto& gp : this->genericParams) {
        GenericParamNode gpCopy;
        gpCopy.loc = gp.loc;
        gpCopy.name = gp.name;
        gpCopy.symbolId = gp.symbolId;
        for (const auto& b : gp.bounds) {
            gpCopy.bounds.push_back(b->cloneAs<TypeNode>());
        }
        copy->genericParams.push_back(std::move(gpCopy));
    }
    
    for (const auto& m : this->methods) {
        copy->methods.push_back(m->cloneAs<FunctionDeclNode>());
    }
    return copy;
}

ASTNode* ImplDeclNode::cloneImpl() const {
    auto copy = new ImplDeclNode();
    copy->loc = this->loc;
    copy->annotations = cloneAnnotations(this->annotations);
    copy->isExported = this->isExported;
    
    for (const auto& gp : this->genericParams) {
        GenericParamNode gpCopy;
        gpCopy.loc = gp.loc;
        gpCopy.name = gp.name;
        gpCopy.symbolId = gp.symbolId;
        for (const auto& b : gp.bounds) {
            gpCopy.bounds.push_back(b->cloneAs<TypeNode>());
        }
        copy->genericParams.push_back(std::move(gpCopy));
    }
    
    if (this->selfType) {
        copy->selfType = this->selfType->cloneAs<TypeNode>();
    }
    
    if (this->traitType) {
        copy->traitType = this->traitType->cloneAs<TypeNode>();
    }
    
    for (const auto& m : this->methods) {
        copy->methods.push_back(m->cloneAs<FunctionDeclNode>());
    }
    return copy;
}


// --- GENERATED CLONES ---

ASTNode* LiteralExpr::cloneImpl() const {
    auto copy = new LiteralExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    copy->kind = this->kind;
    copy->suffix = this->suffix;
    copy->rawText = this->rawText;
    copy->value = this->value;
    return copy;
}

ASTNode* BinaryExpr::cloneImpl() const {
    auto copy = new BinaryExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    copy->op = this->op;
    if (this->left) copy->left = this->left->cloneAs<ExprNode>();
    if (this->right) copy->right = this->right->cloneAs<ExprNode>();
    return copy;
}

ASTNode* UnaryExpr::cloneImpl() const {
    auto copy = new UnaryExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    copy->op = this->op;
    if (this->operand) copy->operand = this->operand->cloneAs<ExprNode>();
    return copy;
}

ASTNode* AssignExpr::cloneImpl() const {
    auto copy = new AssignExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    copy->op = this->op;
    if (this->lvalue) copy->lvalue = this->lvalue->cloneAs<ExprNode>();
    if (this->value) copy->value = this->value->cloneAs<ExprNode>();
    return copy;
}

ASTNode* IndexExpr::cloneImpl() const {
    auto copy = new IndexExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    if (this->base) copy->base = this->base->cloneAs<ExprNode>();
    if (this->index) copy->index = this->index->cloneAs<ExprNode>();
    return copy;
}

ASTNode* MemberExpr::cloneImpl() const {
    auto copy = new MemberExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    if (this->object) copy->object = this->object->cloneAs<ExprNode>();
    copy->member = this->member;
    copy->memberId = this->memberId;
    copy->resolvedFieldIndex = this->resolvedFieldIndex;
    return copy;
}

ASTNode* CastExpr::cloneImpl() const {
    auto copy = new CastExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    if (this->expr) copy->expr = this->expr->cloneAs<ExprNode>();
    if (this->targetType) copy->targetType = this->targetType->cloneAs<TypeNode>();
    return copy;
}

ASTNode* ArrayLiteralExpr::cloneImpl() const {
    auto copy = new ArrayLiteralExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    for (const auto& e : this->elements) {
        copy->elements.push_back(e->cloneAs<ExprNode>());
    }
    return copy;
}

ASTNode* TupleLiteralExpr::cloneImpl() const {
    auto copy = new TupleLiteralExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    for (const auto& e : this->elements) {
        copy->elements.push_back(e->cloneAs<ExprNode>());
    }
    return copy;
}

ASTNode* MatchExpr::cloneImpl() const {
    auto copy = new MatchExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    if (this->subject) copy->subject = this->subject->cloneAs<ExprNode>();
    return copy;
}

ASTNode* LambdaExpr::cloneImpl() const {
    auto copy = new LambdaExpr();
    copy->loc = this->loc;
    copy->inferredType = this->inferredType;
    copy->valueCategory = this->valueCategory;
    copy->isConstant = this->isConstant;
    return copy;
}

ASTNode* AwaitExpr::cloneImpl() const { 
    auto copy = new AwaitExpr();
    copy->loc = this->loc;
    if (this->expr) copy->expr = this->expr->cloneAs<ExprNode>();
    return copy;
}
ASTNode* SizeofExpr::cloneImpl() const { 
    auto copy = new SizeofExpr();
    copy->loc = this->loc;
    if (this->targetType) copy->targetType = this->targetType->cloneAs<TypeNode>();
    return copy;
}
ASTNode* AlignofExpr::cloneImpl() const { 
    auto copy = new AlignofExpr();
    copy->loc = this->loc;
    if (this->targetType) copy->targetType = this->targetType->cloneAs<TypeNode>();
    return copy;
}

ASTNode* IfStmtNode::cloneImpl() const {
    auto copy = new IfStmtNode();
    copy->loc = this->loc;
    if (this->condition) copy->condition = this->condition->cloneAs<ExprNode>();
    if (this->thenBranch) copy->thenBranch = this->thenBranch->cloneAs<BlockStmtNode>();
    if (this->elseBranch) copy->elseBranch = this->elseBranch->cloneAs<StmtNode>();
    return copy;
}

ASTNode* WhileStmtNode::cloneImpl() const {
    auto copy = new WhileStmtNode();
    copy->loc = this->loc;
    if (this->condition) copy->condition = this->condition->cloneAs<ExprNode>();
    if (this->body) copy->body = this->body->cloneAs<BlockStmtNode>();
    return copy;
}

ASTNode* ForStmtNode::cloneImpl() const {
    auto copy = new ForStmtNode();
    copy->loc = this->loc;
    copy->kind = this->kind;
    copy->bindingName = this->bindingName;
    copy->bindingId = this->bindingId;
    if (this->iterable) copy->iterable = this->iterable->cloneAs<ExprNode>();
    if (this->init) copy->init = this->init->cloneAs<ItemNode>();
    if (this->cond) copy->cond = this->cond->cloneAs<ExprNode>();
    if (this->step) copy->step = this->step->cloneAs<ExprNode>();
    if (this->body) copy->body = this->body->cloneAs<BlockStmtNode>();
    copy->bodyScopeId = kInvalidSymbolID;
    return copy;
}

ASTNode* BreakStmtNode::cloneImpl() const { 
    auto copy = new BreakStmtNode();
    copy->loc = this->loc;
    return copy;
}
ASTNode* ContinueStmtNode::cloneImpl() const { 
    auto copy = new ContinueStmtNode();
    copy->loc = this->loc;
    return copy;
}
ASTNode* UnsafeStmtNode::cloneImpl() const {
    auto copy = new UnsafeStmtNode();
    copy->loc = this->loc;
    if (this->body) copy->body = this->body->cloneAs<BlockStmtNode>();
    return copy;
}
ASTNode* ComptimeStmtNode::cloneImpl() const {
    auto copy = new ComptimeStmtNode();
    copy->loc = this->loc;
    if (this->body) copy->body = this->body->cloneAs<BlockStmtNode>();
    return copy;
}


// Macro & Placeholder Clones
// ==========================================
ASTNode* PlaceholderExpr::cloneImpl() const {
    auto copy = new PlaceholderExpr();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->data = this->data;
    return copy;
}
ASTNode* PlaceholderStmt::cloneImpl() const {
    auto copy = new PlaceholderStmt();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->data = this->data;
    return copy;
}
ASTNode* PlaceholderTypeNode::cloneImpl() const {
    auto copy = new PlaceholderTypeNode();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->data = this->data;
    return copy;
}
ASTNode* MacroDeclNode::cloneImpl() const {
    auto copy = new MacroDeclNode();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->name = this->name;
    copy->params = this->params;
    if (this->body) copy->body = this->body->cloneAs<BlockStmtNode>();
    return copy;
}
ASTNode* MacroCallExpr::cloneImpl() const {
    auto copy = new MacroCallExpr();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->name = this->name;
    copy->resolvedMacroId = this->resolvedMacroId;
    for (const auto& a : this->args) {
        MacroCallArgNode clonedArg;
        if (a.node) clonedArg.node = std::unique_ptr<ASTNode>(a.node->cloneImpl());
        copy->args.push_back(std::move(clonedArg));
    }
    return copy;
}
ASTNode* MacroCallStmt::cloneImpl() const {
    auto copy = new MacroCallStmt();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->name = this->name;
    copy->resolvedMacroId = this->resolvedMacroId;
    for (const auto& a : this->args) {
        MacroCallArgNode clonedArg;
        if (a.node) clonedArg.node = std::unique_ptr<ASTNode>(a.node->cloneImpl());
        copy->args.push_back(std::move(clonedArg));
    }
    return copy;
}
ASTNode* MacroExpandForStmt::cloneImpl() const {
    auto copy = new MacroExpandForStmt();
    copy->loc = this->loc;
    copy->expansionID = this->expansionID;
    copy->iterName = this->iterName;
    copy->listName = this->listName;
    if (this->body) copy->body = this->body->cloneAs<BlockStmtNode>();
    return copy;
}
} // namespace fl
