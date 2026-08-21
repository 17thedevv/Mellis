#pragma once

#include "mellis/Core/FLType.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/MiddleEnd/TraitSolver.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include <vector>
#include <string>
#include <unordered_map>

namespace fl {

struct MethodInfo {
    SymbolID id;
    bool isTraitMethod;
    SymbolID traitId;
    const FunctionType* type;
    const DeclNode* implNode = nullptr;
};

struct MethodCandidate {
    SymbolID traitId; // kInvalidSymbolID if inherent
    const ImplDeclNode* inherentImplNode; // nullptr if trait method
    SymbolID methodId; 
    const FunctionType* methodType; 
};

class MethodResolver {
    std::unordered_map<std::string, std::vector<MethodCandidate>> candidates;
public:
    const std::unordered_map<std::string, std::vector<MethodCandidate>>& getCandidates() const { return candidates; }
    void addTraitMethod(const std::string& name, SymbolID traitId, SymbolID methodId, const FunctionType* type);
    void addInherentMethod(const std::string& name, const ImplDeclNode* implNode, SymbolID methodId, const FunctionType* type);
    
    bool probe(const Type* receiverType, const std::string& name, MethodInfo& outMethod, TraitSolver& solver, TypeContext& ctx, SymbolTable& table, const std::vector<const Type*>& typeTable, ModuleID callerModuleID = 0, DiagnosticEngine* diag = nullptr, SourceLocation callLoc = {});
};

} // namespace fl
