#include "mellis/MiddleEnd/MatchAnalyzer.h"
#include "mellis/AST/ExprNode.h"
#include "mellis/AST/StmtNode.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/AST/ProgramNode.h"
#include "mellis/AST/PatternNode.h"
#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/Semantic/DecisionTree.h"
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

struct PatRow {
    std::vector<PatternNode*> cols;
    size_t armIndex;
    std::vector<std::pair<SymbolID, std::string>> bindings;
};
using PatMatrix = std::vector<PatRow>;

PatMatrix filterMatrix(const PatMatrix& matrix, const Constructor& ctor, const std::string& headPlace) {
    PatMatrix result;
    for (const auto& row : matrix) {
        if (row.cols.empty()) continue;
        PatternNode* head = row.cols[0];
        auto dec = deconstruct(head);
        
        if (dec.ctor.kind == CtorKind::Wildcard) {
            PatRow newRow;
            newRow.armIndex = row.armIndex;
            newRow.bindings = row.bindings;
            if (auto* idPat = dynamic_cast<IdentifierPatternNode*>(head)) {
                newRow.bindings.push_back({idPat->symbolId, headPlace});
            }
            for (size_t i = 0; i < ctor.arity; ++i) newRow.cols.push_back(nullptr);
            for (size_t i = 1; i < row.cols.size(); ++i) newRow.cols.push_back(row.cols[i]);
            result.push_back(newRow);
        } else if (dec.ctor == ctor) {
            PatRow newRow;
            newRow.armIndex = row.armIndex;
            newRow.bindings = row.bindings;
            newRow.cols = dec.fields;
            for (size_t i = 1; i < row.cols.size(); ++i) newRow.cols.push_back(row.cols[i]);
            result.push_back(newRow);
        }
    }
    return result;
}

std::unique_ptr<DecisionNode> compileMatrix(
    const PatMatrix& matrix, 
    const std::vector<const Type*>& colTypes, 
    const std::vector<std::string>& places,
    SymbolTable& table, 
    TypeChecker& typeChecker, 
    std::vector<std::string>& missingPatterns) 
{
    if (colTypes.empty()) {
        if (!matrix.empty()) return std::make_unique<SuccessNode>(matrix[0].armIndex, matrix[0].bindings);
        return std::make_unique<FailureNode>();
    }
    if (matrix.empty()) {
        missingPatterns.push_back("_");
        return std::make_unique<FailureNode>();
    }
    
    TypeContext& ctx = typeChecker.getContext();
    const Type* headTy = colTypes[0];
    const Type* resolvedTy = ctx.unificationTable.deepResolve(headTy, ctx);
    
    std::string headPlace = places[0];
    
    std::vector<const Type*> tailTypes;
    for (size_t i = 1; i < colTypes.size(); ++i) tailTypes.push_back(colTypes[i]);
    std::vector<std::string> tailPlaces;
    for (size_t i = 1; i < places.size(); ++i) tailPlaces.push_back(places[i]);
    
    if (auto* enumTy = dynamic_cast<const EnumType*>(resolvedTy)) {
        std::cerr << "[DEBUG MatchAnalyzer] resolvedTy is EnumType. enumSymbolId=" << enumTy->enumSymbolId << std::endl;
        auto sym = table.getSymbol(enumTy->enumSymbolId);
        if (sym.kind == SymbolKind::Enum && sym.decl) {
            auto* enumDecl = static_cast<EnumDeclNode*>(sym.decl);
            std::vector<Constructor> allCtors;
            for (auto& var : enumDecl->variants) {
                allCtors.push_back(Constructor{CtorKind::EnumVariant, "", var->symbolId, var->fields.size()});
            }
            
            auto switchNode = std::make_unique<SwitchTagNode>();
            switchNode->placeStr = headPlace;
            bool allExhaustive = true;

            for (size_t i = 0; i < allCtors.size(); ++i) {
                const auto& ctor = allCtors[i];
                PatMatrix filtered = filterMatrix(matrix, ctor, headPlace);
                
                std::vector<const Type*> newColTypes;
                std::vector<std::string> newPlaces;
                
                auto extractNode = std::make_unique<ExtractNode>();
                extractNode->placeStr = headPlace;
                extractNode->variantId = ctor.variantId;

                for (size_t fIdx = 0; fIdx < enumDecl->variants[i]->fields.size(); ++fIdx) {
                    auto& fDecl = enumDecl->variants[i]->fields[fIdx];
                    newColTypes.push_back(typeChecker.typeOf(fDecl->symbolId));
                    std::string newPlace = headPlace + "_v" + std::to_string(ctor.variantId) + "_f" + std::to_string(fIdx);
                    newPlaces.push_back(newPlace);
                    extractNode->bindNames.push_back(newPlace);
                }
                newColTypes.insert(newColTypes.end(), tailTypes.begin(), tailTypes.end());
                newPlaces.insert(newPlaces.end(), tailPlaces.begin(), tailPlaces.end());
                
                std::vector<std::string> subMissing;
                auto childNode = compileMatrix(filtered, newColTypes, newPlaces, table, typeChecker, subMissing);
                
                if (!subMissing.empty()) {
                    allExhaustive = false;
                    for (auto& m : subMissing) {
                        std::string prefix = std::string(enumDecl->variants[i]->name);
                        if (ctor.arity > 0) {
                            if (m == "_") prefix += "(_)"; 
                            else prefix += "(" + m + ")";
                        }
                        missingPatterns.push_back(prefix);
                    }
                }
                extractNode->next = std::move(childNode);
                switchNode->cases.push_back({ctor.variantId, std::move(extractNode)});
            }
            if (!allExhaustive) switchNode->fallback = std::make_unique<FailureNode>();
            return switchNode;
        }
    } else if (auto* tupTy = dynamic_cast<const TupleType*>(resolvedTy)) {
        Constructor ctor{CtorKind::Tuple, "", kInvalidSymbolID, tupTy->elements.size()};
        PatMatrix filtered = filterMatrix(matrix, ctor, headPlace);
        
        std::vector<const Type*> newColTypes;
        std::vector<std::string> newPlaces;
        
        auto extractNode = std::make_unique<ExtractNode>();
        extractNode->placeStr = headPlace;
        extractNode->variantId = kInvalidSymbolID; // Khong phai Enum

        for (size_t i = 0; i < tupTy->elements.size(); ++i) {
            newColTypes.push_back(tupTy->elements[i]);
            std::string newPlace = headPlace + "_tup" + std::to_string(i);
            newPlaces.push_back(newPlace);
            extractNode->bindNames.push_back(newPlace);
        }
        newColTypes.insert(newColTypes.end(), tailTypes.begin(), tailTypes.end());
        newPlaces.insert(newPlaces.end(), tailPlaces.begin(), tailPlaces.end());
        
        std::vector<std::string> subMissing;
        auto childNode = compileMatrix(filtered, newColTypes, newPlaces, table, typeChecker, subMissing);
        if (!subMissing.empty()) {
            for (auto& m : subMissing) missingPatterns.push_back("(" + m + ")");
        }
        extractNode->next = std::move(childNode);
        return extractNode;
    }
    
    // Primitive types (requires wildcard)
    PatMatrix filtered;
    for (const auto& row : matrix) {
        if (row.cols.empty()) continue;
        auto dec = deconstruct(row.cols[0]);
        if (dec.ctor.kind == CtorKind::Wildcard) {
            PatRow newRow;
            newRow.armIndex = row.armIndex;
            newRow.bindings = row.bindings;
            if (auto* idPat = dynamic_cast<IdentifierPatternNode*>(row.cols[0])) {
                newRow.bindings.push_back({idPat->symbolId, headPlace});
            }
            for (size_t i = 1; i < row.cols.size(); ++i) newRow.cols.push_back(row.cols[i]);
            filtered.push_back(newRow);
        }
    }
    
    std::vector<std::string> subMissing;
    auto childNode = compileMatrix(filtered, tailTypes, tailPlaces, table, typeChecker, subMissing);
    
    auto switchNode = std::make_unique<SwitchLitNode>();
    switchNode->placeStr = headPlace;
    
    // Tạm thời coi Primitive type fallback vào nhánh con (do chưa hỗ trợ đầy đủ literal matching ở pattern resolver v1, nếu có sẽ thêm SwitchLitNode->cases sau).
    switchNode->fallback = std::move(childNode);
    
    if (!subMissing.empty()) {
        for (auto& m : subMissing) missingPatterns.push_back("_");
    }
    return switchNode;
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
    for (size_t i = 0; i < node.arms.size(); ++i) {
        auto& arm = node.arms[i];
        if (arm.pattern) {
            PatRow row;
            row.armIndex = i;
            row.cols.push_back(arm.pattern.get());
            matrix.push_back(row);
        }
        if (arm.body) arm.body->accept(*this);
    }

    if (node.subject->inferredType) {
        std::vector<const Type*> colTypes = { node.subject->inferredType };
        std::vector<std::string> places = { "subject" };
        std::vector<std::string> missingPatterns;
        
        node.decisionTree = compileMatrix(matrix, colTypes, places, table, typeChecker, missingPatterns);
        
        if (!missingPatterns.empty()) {
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
