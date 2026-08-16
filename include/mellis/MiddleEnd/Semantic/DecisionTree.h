#pragma once
#include <vector>
#include <memory>
#include <string>
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/IR/MVIR.h"

namespace fl {

enum class DecisionKind {
    Success,
    Failure,
    SwitchTag,
    SwitchLit,
    Extract
};

struct DecisionNode {
    DecisionKind kind;
    virtual ~DecisionNode() = default;
    DecisionNode(DecisionKind k) : kind(k) {}
};

struct SuccessNode : public DecisionNode {
    size_t armIndex;
    std::vector<std::pair<SymbolID, std::string>> bindings; // symbolId -> placeStr
    SuccessNode(size_t arm, std::vector<std::pair<SymbolID, std::string>> b = {}) 
        : DecisionNode(DecisionKind::Success), armIndex(arm), bindings(std::move(b)) {}
};

struct FailureNode : public DecisionNode {
    FailureNode() : DecisionNode(DecisionKind::Failure) {}
};

struct SwitchTagNode : public DecisionNode {
    std::string placeStr;
    std::vector<std::pair<SymbolID, std::unique_ptr<DecisionNode>>> cases;
    std::unique_ptr<DecisionNode> fallback;
    
    SwitchTagNode() : DecisionNode(DecisionKind::SwitchTag) {}
};

struct SwitchLitNode : public DecisionNode {
    std::string placeStr;
    std::vector<std::pair<std::string, std::unique_ptr<DecisionNode>>> cases;
    std::unique_ptr<DecisionNode> fallback;

    SwitchLitNode() : DecisionNode(DecisionKind::SwitchLit) {}
};

struct ExtractNode : public DecisionNode {
    std::string placeStr;
    SymbolID variantId;
    std::vector<std::string> bindNames;
    std::unique_ptr<DecisionNode> next;

    ExtractNode() : DecisionNode(DecisionKind::Extract) {}
};

} // namespace fl
