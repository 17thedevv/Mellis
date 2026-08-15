#include "mellis/MiddleEnd/MatchAnalyzer.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/Core/FLType.h"
#include <unordered_set>
#include <iostream>
#include <string>

namespace fl {

namespace {

enum class CtorKind { Wildcard, Literal, Tuple, EnumVariant };

struct Constructor {
    CtorKind kind;
    std::string literalText;
    SymbolID variantId = kInvalidSymbolID;
    size_t arity = 0;

    bool operator==(const Constructor& o) const {
        if (kind != o.kind) return false;
        if (kind == CtorKind::Literal) return literalText == o.literalText;
        if (kind == CtorKind::EnumVariant) return variantId == o.variantId;
        if (kind == CtorKind::Tuple) return arity == o.arity;
        return true;
    }
};

struct Deconstructed {
    Constructor ctor;
    std::vector<PatternNode*> fields;
};

Deconstructed deconstruct(PatternNode* pat) {
    if (!pat) return { Constructor{CtorKind::Wildcard}, {} };
    if (dynamic_cast<WildcardPatternNode*>(pat) || dynamic_cast<IdentifierPatternNode*>(pat)) {
        return { Constructor{CtorKind::Wildcard}, {} };
    }
    if (auto* lit = dynamic_cast<LiteralPatternNode*>(pat)) {
        return { Constructor{CtorKind::Literal, std::string(lit->lit->rawText)}, {} };
    }
    if (auto* tup = dynamic_cast<TuplePatternNode*>(pat)) {
        std::vector<PatternNode*> fields;
        for (auto& e : tup->elements) fields.push_back(e.get());
        return { Constructor{CtorKind::Tuple, "", kInvalidSymbolID, fields.size()}, fields };
    }
    if (auto* enm = dynamic_cast<EnumPatternNode*>(pat)) {
        std::vector<PatternNode*> fields;
        for (auto& f : enm->fields) fields.push_back(f.get());
        return { Constructor{CtorKind::EnumVariant, "", enm->variantSymbolId, fields.size()}, fields };
    }
    return { Constructor{CtorKind::Wildcard}, {} };
}

using PatRow = std::vector<PatternNode*>;
using PatMatrix = std::vector<PatRow>;

PatMatrix filterMatrix(const PatMatrix& matrix, const Constructor& ctor) {
    PatMatrix result;
    for (const auto& row : matrix) {
        if (row.empty()) continue;
        PatternNode* head = row[0];
        auto dec = deconstruct(head);
        
        if (dec.ctor.kind == CtorKind::Wildcard) {
            PatRow newRow;
            for (size_t i = 0; i < ctor.arity; ++i) newRow.push_back(nullptr);
            for (size_t i = 1; i < row.size(); ++i) newRow.push_back(row[i]);
            result.push_back(newRow);
        } else if (dec.ctor == ctor) {
            PatRow newRow = dec.fields;
            for (size_t i = 1; i < row.size(); ++i) newRow.push_back(row[i]);
            result.push_back(newRow);
        }
    }
    return result;
}

bool isExhaustive(const PatMatrix& matrix, const std::vector<const Type*>& colTypes, SymbolTable& table, TypeChecker& typeChecker, std::vector<std::string>& missingPatterns) {
    if (colTypes.empty()) return !matrix.empty();
    if (matrix.empty()) {
        missingPatterns.push_back("_");
        return false;
    }
    
    TypeContext& ctx = typeChecker.getContext();
    const Type* headTy = colTypes[0];
    const Type* resolvedTy = ctx.unificationTable.deepResolve(headTy, ctx);
    
    std::vector<const Type*> tailTypes;
    for (size_t i = 1; i < colTypes.size(); ++i) tailTypes.push_back(colTypes[i]);
    
    if (auto* enumTy = dynamic_cast<const EnumType*>(resolvedTy)) {
        auto sym = table.getSymbol(enumTy->enumSymbolId);
        if (sym.kind == SymbolKind::Enum && sym.decl) {
            auto* enumDecl = static_cast<EnumDeclNode*>(sym.decl);
            std::vector<Constructor> allCtors;
            for (auto& var : enumDecl->variants) {
                allCtors.push_back(Constructor{CtorKind::EnumVariant, "", var->symbolId, var->fields.size()});
            }
            
            bool allExhaustive = true;
            for (size_t i = 0; i < allCtors.size(); ++i) {
                const auto& ctor = allCtors[i];
                PatMatrix filtered = filterMatrix(matrix, ctor);
                
                std::vector<const Type*> newColTypes;
                for (auto& fDecl : enumDecl->variants[i]->fields) {
                    newColTypes.push_back(typeChecker.typeOf(fDecl->symbolId));
                }
                newColTypes.insert(newColTypes.end(), tailTypes.begin(), tailTypes.end());
                
                std::vector<std::string> subMissing;
                if (!isExhaustive(filtered, newColTypes, table, typeChecker, subMissing)) {
                    allExhaustive = false;
                    for (auto& m : subMissing) {
                        std::string prefix = std::string(enumDecl->variants[i]->name);
                        if (ctor.arity > 0) {
                            if (m == "_") prefix += "(_)"; // simplified format
                            else prefix += "(" + m + ")";
                        }
                        missingPatterns.push_back(prefix);
                    }
                }
            }
            return allExhaustive;
        }
    } else if (auto* tupTy = dynamic_cast<const TupleType*>(resolvedTy)) {
        Constructor ctor{CtorKind::Tuple, "", kInvalidSymbolID, tupTy->elements.size()};
        PatMatrix filtered = filterMatrix(matrix, ctor);
        
        std::vector<const Type*> newColTypes;
        for (auto* eTy : tupTy->elements) newColTypes.push_back(eTy);
        newColTypes.insert(newColTypes.end(), tailTypes.begin(), tailTypes.end());
        
        std::vector<std::string> subMissing;
        if (!isExhaustive(filtered, newColTypes, table, typeChecker, subMissing)) {
            for (auto& m : subMissing) missingPatterns.push_back("(" + m + ")");
            return false;
        }
        return true;
    }
    
    // Primitive types (requires wildcard)
    PatMatrix filtered;
    for (const auto& row : matrix) {
        if (row.empty()) continue;
        auto dec = deconstruct(row[0]);
        if (dec.ctor.kind == CtorKind::Wildcard) {
            PatRow newRow;
            for (size_t i = 1; i < row.size(); ++i) newRow.push_back(row[i]);
            filtered.push_back(newRow);
        }
    }
    
    std::vector<std::string> subMissing;
    if (!isExhaustive(filtered, tailTypes, table, typeChecker, subMissing)) {
        for (auto& m : subMissing) missingPatterns.push_back("_");
        return false;
    }
    
    return true;
}

} // namespace

MatchAnalyzer::MatchAnalyzer(SymbolTable& table, TypeChecker& typeChecker, DiagnosticEngine& diag)
    : table(table), typeChecker(typeChecker), diag(diag) {}

bool MatchAnalyzer::analyze(ASTNode* root) {
    if (!root) return false;
    hasError_ = false;
    root->accept(*this);
    return !hasError_;
}

void MatchAnalyzer::visit(ProgramNode& node) { for (auto& item : node.items) item->accept(*this); }
void MatchAnalyzer::visit(ModDeclNode& node) { for (auto& d : node.decls) d->accept(*this); }
void MatchAnalyzer::visit(VarDeclNode& node) { if (node.initializer) node.initializer->accept(*this); }
void MatchAnalyzer::visit(FunctionDeclNode& node) { if (node.body) node.body->accept(*this); }
void MatchAnalyzer::visit(ImplDeclNode& node) { for (auto& m : node.methods) m->accept(*this); }

// Statements
void MatchAnalyzer::visit(BlockStmtNode& node) { for (auto& s : node.body) s->accept(*this); }
void MatchAnalyzer::visit(ExprStmtNode& node) { node.expr->accept(*this); }
void MatchAnalyzer::visit(IfStmtNode& node) {
    node.condition->accept(*this);
    node.thenBranch->accept(*this);
    if (node.elseBranch) node.elseBranch->accept(*this);
}
void MatchAnalyzer::visit(WhileStmtNode& node) {
    node.condition->accept(*this);
    node.body->accept(*this);
}
void MatchAnalyzer::visit(ForStmtNode& node) {
    if (node.init) node.init->accept(*this);
    if (node.cond) node.cond->accept(*this);
    if (node.step) node.step->accept(*this);
    if (node.iterable) node.iterable->accept(*this);
    node.body->accept(*this);
}
void MatchAnalyzer::visit(ReturnStmtNode& node) { if (node.value) node.value->accept(*this); }
void MatchAnalyzer::visit(UnsafeStmtNode& node) { if (node.body) node.body->accept(*this); }
void MatchAnalyzer::visit(ComptimeStmtNode& node) { if (node.body) node.body->accept(*this); }

// Expressions
void MatchAnalyzer::visit(MatchExpr& node) {
    node.subject->accept(*this);

    PatMatrix matrix;
    for (auto& arm : node.arms) {
        if (arm.pattern) {
            matrix.push_back({arm.pattern.get()});
        }
        if (arm.body) arm.body->accept(*this);
    }

    if (node.subject->inferredType) {
        std::vector<const Type*> colTypes = { node.subject->inferredType };
        std::vector<std::string> missingPatterns;
        if (!isExhaustive(matrix, colTypes, table, typeChecker, missingPatterns)) {
            std::string missingStr = "";
            for (size_t i = 0; i < missingPatterns.size(); ++i) {
                missingStr += "`" + missingPatterns[i] + "`";
                if (i < missingPatterns.size() - 1) missingStr += ", ";
            }
            diag.error(node.loc, "non-exhaustive patterns: " + missingStr + " not covered");
            hasError_ = true;
        }
    }
}

void MatchAnalyzer::visit(BinaryExpr& node) { node.left->accept(*this); node.right->accept(*this); }
void MatchAnalyzer::visit(UnaryExpr& node) { node.operand->accept(*this); }
void MatchAnalyzer::visit(AssignExpr& node) { node.lvalue->accept(*this); node.value->accept(*this); }
void MatchAnalyzer::visit(CallExpr& node) {
    node.callee->accept(*this);
    for (auto& arg : node.args) if (arg.value) arg.value->accept(*this);
}
void MatchAnalyzer::visit(MethodCallExpr& node) {
    node.object->accept(*this);
    for (auto& arg : node.args) if (arg.value) arg.value->accept(*this);
}
void MatchAnalyzer::visit(IndexExpr& node) { node.base->accept(*this); node.index->accept(*this); }
void MatchAnalyzer::visit(MemberExpr& node) { node.object->accept(*this); }
void MatchAnalyzer::visit(TupleIndexExpr& node) { node.object->accept(*this); }
void MatchAnalyzer::visit(CastExpr& node) { node.expr->accept(*this); }
void MatchAnalyzer::visit(UnsizeCastExpr& node) { node.expr->accept(*this); }
void MatchAnalyzer::visit(ArrayLiteralExpr& node) { for (auto& e : node.elements) e->accept(*this); }
void MatchAnalyzer::visit(TupleLiteralExpr& node) { for (auto& e : node.elements) e->accept(*this); }
void MatchAnalyzer::visit(StructInitExpr& node) { for (auto& f : node.fields) f.value->accept(*this); }
void MatchAnalyzer::visit(LambdaExpr& node) { if (node.body) node.body->accept(*this); }
void MatchAnalyzer::visit(TryExpr& node) { node.expr->accept(*this); }
void MatchAnalyzer::visit(AwaitExpr& node) { node.expr->accept(*this); }

} // namespace fl
