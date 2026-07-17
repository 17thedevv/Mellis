#pragma once
#include "fdlang/AST/ASTNode.h"
#include "fdlang/Core/Types.h"
#include <vector>
#include <memory>
#include <string_view>

namespace fl {

class TypeNode;
class ExprNode;
class BlockStmtNode;
class FunctionDeclNode;

class DeclNode : public ItemNode {
public:
    bool     isExported = false;
    SymbolID symbolId   = kInvalidSymbolID;
};

class VarDeclNode : public DeclNode {
public:
    std::string_view              name;
    std::unique_ptr<TypeNode>     typeAnnot;
    std::unique_ptr<ExprNode>     initializer;
    bool                          isMutable;
    void accept(ASTVisitor& v) override;
};

class ParamDeclNode : public DeclNode {
public:
    std::string_view          name;
    std::unique_ptr<TypeNode> type;
    bool isVariadic = false;
    bool isSelf     = false;
    void accept(ASTVisitor& v) override;
};

struct GenericParamNode {
    SourceLocation                         loc;
    std::string_view                       name;
    std::vector<std::unique_ptr<TypeNode>> bounds;
    SymbolID symbolId = kInvalidSymbolID;
};

class FunctionDeclNode : public DeclNode {
public:
    std::string_view                             name;
    std::vector<GenericParamNode>                genericParams;
    std::vector<std::unique_ptr<ParamDeclNode>>  params;
    std::unique_ptr<TypeNode>                    returnType;
    std::unique_ptr<BlockStmtNode>               body;
    ScopeID                                      bodyScopeId = kInvalidSymbolID;
    bool isAsync    = false;
    bool isComptime = false;
    void accept(ASTVisitor& v) override;
};

class StructFieldNode : public ASTNode {
public:
    std::string_view          name;
    std::unique_ptr<TypeNode> type;
    SymbolID                  symbolId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class StructDeclNode : public DeclNode {
public:
    std::string_view                              name;
    std::vector<GenericParamNode>                 genericParams;
    std::vector<std::unique_ptr<StructFieldNode>> fields;
    ScopeID                                       bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class EnumVariantNode : public ASTNode {
public:
    std::string_view                             name;
    std::vector<std::unique_ptr<ParamDeclNode>>  fields;
    SymbolID                                     symbolId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class EnumDeclNode : public DeclNode {
public:
    std::string_view                               name;
    std::vector<GenericParamNode>                  genericParams;
    std::vector<std::unique_ptr<EnumVariantNode>>  variants;
    ScopeID                                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class TraitDeclNode : public DeclNode {
public:
    std::string_view                               name;
    std::vector<GenericParamNode>                  genericParams;
    std::vector<std::unique_ptr<FunctionDeclNode>> methods;
    ScopeID                                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class ImplDeclNode : public DeclNode {
public:
    std::vector<GenericParamNode>                  genericParams;
    std::unique_ptr<TypeNode>                      selfType;
    std::unique_ptr<TypeNode>                      traitType;
    std::vector<std::unique_ptr<FunctionDeclNode>> methods;
    ScopeID                                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

class ModDeclNode : public DeclNode {
public:
    std::string_view                       name;
    std::vector<std::unique_ptr<DeclNode>> decls;
    ScopeID                                bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
};

struct UseTreeNode {
    SourceLocation                loc;
    std::vector<std::string_view> segments;
    std::string_view              alias;
    bool                          isGlob = false;
    std::vector<UseTreeNode>      children;
};

class UseDeclNode : public DeclNode {
public:
    UseTreeNode tree;
    void accept(ASTVisitor& v) override;
};

class ExternDeclNode : public DeclNode {
public:
    std::unique_ptr<FunctionDeclNode> func;
    void accept(ASTVisitor& v) override;
};

class TypeAliasDeclNode : public DeclNode {
public:
    std::string_view              name;
    std::vector<GenericParamNode> genericParams;
    std::unique_ptr<TypeNode>     aliasedType;
    void accept(ASTVisitor& v) override;
};

} // namespace fl
