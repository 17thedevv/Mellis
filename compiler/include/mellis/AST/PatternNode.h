#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/Core/Types.h"
#include <vector>
#include <memory>
#include <string_view>

namespace fl {

class LiteralExpr;
class PatternVisitor;

enum class PatternBindingAction {
    Unknown,
    Copy,
    Move,
    Borrow,
    BorrowMut,
    Discard
};

class PatternNode : public ASTNode {
public:
    const Type* inferredType = nullptr;
    PatternBindingAction action = PatternBindingAction::Unknown;
    virtual void accept(PatternVisitor& v) = 0;
};

class WildcardPatternNode : public PatternNode {
public:
    void accept(PatternVisitor& v) override;
    void accept(ASTVisitor& v) override { }
    ASTNode* cloneImpl() const override;
};

class LiteralPatternNode : public PatternNode {
public:
    std::unique_ptr<LiteralExpr> lit;
    void accept(PatternVisitor& v) override;
    void accept(ASTVisitor& v) override { }
    ASTNode* cloneImpl() const override;
};

class IdentifierPatternNode : public PatternNode {
public:
    std::vector<std::string_view> segments;
    SymbolID                      symbolId = kInvalidSymbolID;
    void accept(PatternVisitor& v) override;
    void accept(ASTVisitor& v) override { }
    ASTNode* cloneImpl() const override;
};

class EnumPatternNode : public PatternNode {
public:
    std::vector<std::string_view>           path;
    std::vector<std::unique_ptr<PatternNode>> fields;
    SymbolID                                variantSymbolId = kInvalidSymbolID;
    void accept(PatternVisitor& v) override;
    void accept(ASTVisitor& v) override { }
    ASTNode* cloneImpl() const override;
};

class TuplePatternNode : public PatternNode {
public:
    std::vector<std::unique_ptr<PatternNode>> elements;
    bool hasRest = false;
    void accept(PatternVisitor& v) override;
    void accept(ASTVisitor& v) override { }
    ASTNode* cloneImpl() const override;
};

class StructPatternField {
public:
    std::string_view name;
    std::unique_ptr<PatternNode> pattern; // Optional
};

class StructPatternNode : public PatternNode {
public:
    std::vector<std::string_view> path;
    std::vector<StructPatternField> fields;
    bool hasRest = false;
    SymbolID structSymbolId = kInvalidSymbolID;
    void accept(PatternVisitor& v) override;
    void accept(ASTVisitor& v) override { }
    ASTNode* cloneImpl() const override;
};

} // namespace fl
