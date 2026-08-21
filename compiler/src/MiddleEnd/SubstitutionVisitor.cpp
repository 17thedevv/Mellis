#include "mellis/MiddleEnd/SubstitutionVisitor.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/PatternNode.h"
#include <iostream>

namespace fl {

std::unique_ptr<TypeNode> SubstitutionVisitor::substituteType(std::unique_ptr<TypeNode> type) {
    if (!type) return nullptr;

    if (auto* named = dynamic_cast<NamedTypeNode*>(type.get())) {
        if (named->segments.size() == 2) {
            // E.g. T::Item -> check if we have a substitution for it
            std::string key = GenericSubstitution::projectionKey(std::string(named->segments[0]), std::string(named->segments[1]));
            if (genericSubstitution.associatedProjections.count(key)) {
                return genericSubstitution.associatedProjections.at(key)->cloneAs<TypeNode>();
            }
        }
        if (!named->segments.empty()) {
            std::string baseName = std::string(named->segments[0]);
            if (baseName == "T") std::cerr << "[DEBUG] SubstitutionVisitor: checking type 'T'\n";
            if (genericSubstitution.typeSubstitutions.count(baseName)) {
                std::cerr << "[DEBUG] SubstitutionVisitor: Substituted '" << baseName << "' with its replacement.\n";
                auto* subNode = genericSubstitution.typeSubstitutions.at(baseName).get();
                if (named->segments.size() == 1) {
                    return subNode->cloneAs<TypeNode>();
                } else if (auto* subNamed = dynamic_cast<NamedTypeNode*>(subNode)) {
                    auto newNamed = subNamed->cloneAs<NamedTypeNode>();
                    for (size_t i = 1; i < named->segments.size(); ++i) {
                        newNamed->segments.push_back(named->segments[i]);
                    }
                    return newNamed;
                }
            } else {
                if (baseName == "T") std::cerr << "[DEBUG] SubstitutionVisitor: NO substitution found for 'T'.\n";
                std::cerr << "[DEBUG] SubstitutionVisitor: NO substitution found for '" << baseName << "'.\n";
            }
        }
        for (auto& arg : named->genericArgs) {
            arg = substituteType(std::move(arg));
        }
    } else if (auto* ptr = dynamic_cast<PointerTypeNode*>(type.get())) {
        ptr->inner = substituteType(std::move(ptr->inner));
    } else if (auto* ref = dynamic_cast<ReferenceTypeNode*>(type.get())) {
        if (ref->lifetime && !ref->lifetime->name.empty()) {
            std::string lfName = std::string(ref->lifetime->name);
            if (genericSubstitution.lifetimeSubstitutions.count(lfName)) {
                ref->lifetime = genericSubstitution.lifetimeSubstitutions.at(lfName)->cloneAs<LifetimeNode>();
            }
        }
        ref->inner = substituteType(std::move(ref->inner));
    } else if (auto* arr = dynamic_cast<ArrayTypeNode*>(type.get())) {
        arr->elementType = substituteType(std::move(arr->elementType));
        if (arr->size) arr->size->accept(*this);
    } else if (auto* tup = dynamic_cast<TupleTypeNode*>(type.get())) {
        for (auto& elem : tup->elements) {
            elem = substituteType(std::move(elem));
        }
    } else if (auto* func = dynamic_cast<FunctionTypeNode*>(type.get())) {
        for (auto& param : func->params) {
            param = substituteType(std::move(param));
        }
        func->returnType = substituteType(std::move(func->returnType));
    } else {
        std::cerr << "[DEBUG] SubstitutionVisitor: Skipping node type " << typeid(*type).name() << "\n";
    }

    return type;
}

// ==========================================
// Decl
// ==========================================

void SubstitutionVisitor::visit(VarDeclNode& n) {
    n.typeAnnot = substituteType(std::move(n.typeAnnot));
    if (n.initializer) n.initializer->accept(*this);
}
void SubstitutionVisitor::visit(FunctionDeclNode& n) {
    for (auto& p : n.params) p->accept(*this);
    n.returnType = substituteType(std::move(n.returnType));
    if (n.body) n.body->accept(*this);
}
void SubstitutionVisitor::visit(ParamDeclNode& n) {
    n.type = substituteType(std::move(n.type));
}
void SubstitutionVisitor::visit(StructDeclNode& n) {
    for (auto& f : n.fields) f->accept(*this);
}
void SubstitutionVisitor::visit(StructFieldNode& n) {
    n.type = substituteType(std::move(n.type));
}
void SubstitutionVisitor::visit(EnumDeclNode& n) {
    for (auto& v : n.variants) v->accept(*this);
}
void SubstitutionVisitor::visit(EnumVariantNode& n) {
    for (auto& f : n.fields) f->accept(*this);
}

void SubstitutionVisitor::visit(ImplDeclNode& n) {
    n.selfType = substituteType(std::move(n.selfType));
    n.traitType = substituteType(std::move(n.traitType));
    n.genericParams.clear(); // A specialized impl is no longer generic!
    for (auto& at : n.associatedTypes) at->accept(*this);
    for (auto& m : n.methods) m->accept(*this);
}

void SubstitutionVisitor::visit(TraitDeclNode& n) {
    for (auto& at : n.associatedTypes) at->accept(*this);
    for (auto& m : n.methods) m->accept(*this);
}

void SubstitutionVisitor::visit(TypeAliasDeclNode& n) {
    n.aliasedType = substituteType(std::move(n.aliasedType));
}

// ==========================================
// Stmt
// ==========================================

void SubstitutionVisitor::visit(BlockStmtNode& n) {
    std::cerr << "[DEBUG] SubstitutionVisitor: visiting BlockStmtNode with " << n.body.size() << " stmts.\n";
    for (auto& s : n.body) s->accept(*this);
    if (n.tailExpr) n.tailExpr->accept(*this);
}
void SubstitutionVisitor::visit(ExprStmtNode& n) {
    if (n.expr) n.expr->accept(*this);
}
void SubstitutionVisitor::visit(IfStmtNode& n) {
    if (n.condition) n.condition->accept(*this);
    if (n.thenBranch) n.thenBranch->accept(*this);
    if (n.elseBranch) n.elseBranch->accept(*this);
}
void SubstitutionVisitor::visit(WhileStmtNode& n) {
    if (n.condition) n.condition->accept(*this);
    if (n.body) n.body->accept(*this);
}
void SubstitutionVisitor::visit(ForStmtNode& n) {
    if (n.iterable) n.iterable->accept(*this);
    if (n.body) n.body->accept(*this);
}
void SubstitutionVisitor::visit(ReturnStmtNode& n) {
    if (n.value) n.value->accept(*this);
}
void SubstitutionVisitor::visit(UnsafeStmtNode& n) {
    if (n.body) n.body->accept(*this);
}
void SubstitutionVisitor::visit(ComptimeStmtNode& n) {
    if (n.body) n.body->accept(*this);
}

// ==========================================
// Expr
// ==========================================

void SubstitutionVisitor::visit(IdentifierExpr& n) {
    n.resolvedSymbol = kInvalidSymbolID;
    for (auto& arg : n.genericArgs) {
        arg = substituteType(std::move(arg));
    }
}
void SubstitutionVisitor::visit(BinaryExpr& n) {
    if (n.left) n.left->accept(*this);
    if (n.right) n.right->accept(*this);
}
void SubstitutionVisitor::visit(UnaryExpr& n) {
    if (n.operand) n.operand->accept(*this);
}
void SubstitutionVisitor::visit(AssignExpr& n) {
    if (n.lvalue) n.lvalue->accept(*this);
    if (n.value) n.value->accept(*this);
}
void SubstitutionVisitor::visit(CallExpr& n) {
    if (n.callee) n.callee->accept(*this);
    for (auto& arg : n.genericArgs) {
        arg = substituteType(std::move(arg));
    }
    for (auto& a : n.args) {
        if (a.value) a.value->accept(*this);
    }
}
void SubstitutionVisitor::visit(MethodCallExpr& n) {
    if (n.object) n.object->accept(*this);
    for (auto& arg : n.genericArgs) {
        arg = substituteType(std::move(arg));
    }
    for (auto& a : n.args) {
        if (a.value) a.value->accept(*this);
    }
}
void SubstitutionVisitor::visit(IndexExpr& n) {
    if (n.base) n.base->accept(*this);
    if (n.index) n.index->accept(*this);
}
void SubstitutionVisitor::visit(MemberExpr& n) {
    if (n.object) n.object->accept(*this);
}
void SubstitutionVisitor::visit(TupleIndexExpr& n) {
    if (n.object) n.object->accept(*this);
}
void SubstitutionVisitor::visit(CastExpr& n) {
    std::cerr << "[DEBUG] SubstitutionVisitor: visiting CastExpr\n";
    if (n.expr) n.expr->accept(*this);
    n.targetType = substituteType(std::move(n.targetType));
}
void SubstitutionVisitor::visit(UnsizeCastExpr& n) {
    if (n.expr) n.expr->accept(*this);
}
void SubstitutionVisitor::visit(ArrayLiteralExpr& n) {
    for (auto& e : n.elements) e->accept(*this);
}
void SubstitutionVisitor::visit(TupleLiteralExpr& n) {
    for (auto& e : n.elements) e->accept(*this);
}
void SubstitutionVisitor::visit(StructInitExpr& n) {
    std::cerr << "[DEBUG] SubstitutionVisitor: visiting StructInitExpr with " << n.genericArgs.size() << " genericArgs\n";
    for (auto& arg : n.genericArgs) {
        arg = substituteType(std::move(arg));
    }
    for (auto& f : n.fields) {
        if (f.value) f.value->accept(*this);
    }
}
void SubstitutionVisitor::visit(MatchExpr& n) {
      if (n.subject) n.subject->accept(*this);
      for (auto& arm : n.arms) {
          if (arm.body) arm.body->accept(*this);
      }
  }
void SubstitutionVisitor::visit(LambdaExpr& n) {
    for (auto& p : n.params) p->accept(*this);
    n.returnType = substituteType(std::move(n.returnType));
    if (n.body) n.body->accept(*this);
}
void SubstitutionVisitor::visit(TryExpr& n) {
    if (n.expr) n.expr->accept(*this);
}
void SubstitutionVisitor::visit(AwaitExpr& n) {
    if (n.expr) n.expr->accept(*this);
}
void SubstitutionVisitor::visit(SizeofExpr& n) {
    n.targetType = substituteType(std::move(n.targetType));
}
void SubstitutionVisitor::visit(AlignofExpr& n) {
    n.targetType = substituteType(std::move(n.targetType));
}
void SubstitutionVisitor::visit(TypeofExpr& n) {
    if (n.expr) n.expr->accept(*this);
}

} // namespace fl
