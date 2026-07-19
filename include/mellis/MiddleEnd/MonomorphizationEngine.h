#pragma once
#include <unordered_map>
#include <unordered_set>
#include <string>
#include <vector>
#include <memory>
#include "mellis/Core/FLType.h"
#include "mellis/MiddleEnd/SymbolTable.h"
#include "mellis/MiddleEnd/Resolver.h"
#include "mellis/MiddleEnd/TypeChecker.h"
#include "mellis/AST/DeclNode.h"
#include "mellis/Support/Diagnostic.h"

namespace fl {

class MonomorphizationEngine {
public:
    MonomorphizationEngine(SymbolTable& symTable, Resolver& resolver, TypeChecker& typeChecker, DiagnosticEngine& diag)
        : symTable(symTable), resolver(resolver), typeChecker(typeChecker), diag(diag) {}

    /// Requests a specialized version of a generic function.
    /// Returns the SymbolID of the specialized function.
    /// Throws if it detects infinite generic recursion.
    SymbolID requestSpecialization(
        const FunctionDeclNode* genericTemplate,
        const std::vector<const Type*>& genericArgs,
        SourceLocation loc
    );

    /// Requests a specialized version of a generic struct.
    SymbolID requestStructSpecialization(
        const StructDeclNode* genericTemplate,
        const std::vector<const Type*>& genericArgs,
        SourceLocation loc
    );

    /// Requests a specialized version of a generic enum.
    SymbolID requestEnumSpecialization(
        const EnumDeclNode* genericTemplate,
        const std::vector<const Type*>& genericArgs,
        SourceLocation loc
    );

    /// Extracts the specialized ASTs generated so far.
    std::vector<std::unique_ptr<DeclNode>> takeSpecializedASTs() {
        return std::move(specializedASTs);
    }

private:
    SymbolTable& symTable;
    Resolver& resolver;
    TypeChecker& typeChecker;
    DiagnosticEngine& diag;
    
    // Instantiation depth for infinite recursion prevention
    int currentDepth = 0;
    static constexpr int kMaxDepth = 64;

    // Cache of specialized function mangled names -> SymbolID
    std::unordered_map<std::string, SymbolID> specializedRegistry;
    
    // Tracks mangled names of functions currently being specialized to prevent infinite recursion
    std::unordered_set<std::string> inProgress;

    // Stores the specialized ASTs (the Engine owns them, or they can be injected into the ProgramNode)
    std::vector<std::unique_ptr<DeclNode>> specializedASTs;
};

} // namespace fl
