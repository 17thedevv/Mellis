#pragma once
#include "mellis/AST/ASTNode.h"
#include "mellis/Core/Types.h"
#include <vector>
#include <memory>
#include <string_view>
#include <string>
#include "mellis/AST/ExprNode.h"

namespace fl {

class TypeNode;
class ExprNode;
class BlockStmtNode;
class FunctionDeclNode;
class PatternNode;

struct AnnotationArg {
    std::string key;                 // Tùy chọn (VD: "name" trong name="c")
    std::unique_ptr<ExprNode> value; // Giá trị (VD: "c")
};

class AnnotationNode {
public:
    std::string name;                // VD: "link", "repr"
    std::vector<AnnotationArg> args; 
    
    size_t line = 0;
    size_t column = 0;
};

class DeclNode : public ItemNode {
public:
    std::vector<AnnotationNode> annotations;
    Visibility visibility = Visibility::Internal;
    SymbolID symbolId   = kInvalidSymbolID;
};

class VarDeclNode : public DeclNode {
public:
    std::string_view              name;
    std::unique_ptr<PatternNode>  pattern;
    std::unique_ptr<TypeNode>     typeAnnot;
    std::unique_ptr<ExprNode>     initializer;
    bool                          isMutable;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class ParamDeclNode : public DeclNode {
public:
    std::string_view          name;
    std::unique_ptr<TypeNode> type;
    bool isVariadic = false;
    bool isSelf     = false;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

enum class GenericParamKind : uint8_t {
    Type,
    Lifetime,
    Const
};

struct GenericParamNode {
    SourceLocation                         loc;
    std::string_view                       name;
    GenericParamKind                       kind = GenericParamKind::Type;
    std::vector<std::unique_ptr<TypeNode>> bounds; // For Lifetimes, bounds can represent 'a : 'b
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
    bool isVariadic = false;
    bool isUnsafe   = false;
    bool isIntrinsic = false;
    IntrinsicKind intrinsicKind = IntrinsicKind::None;
    bool hasBody = true;  // false: intrinsic forward decl, extern fn, trait method
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class StructFieldNode : public ASTNode {
public:
    std::string_view          name;
    std::unique_ptr<TypeNode> type;
    SymbolID                  symbolId = kInvalidSymbolID;
    Visibility                visibility = Visibility::Private;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class StructDeclNode : public DeclNode {
public:
    std::string_view                              name;
    std::vector<GenericParamNode>                 genericParams;
    std::vector<std::unique_ptr<StructFieldNode>> fields;
    ScopeID                                       bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class EnumVariantNode : public ASTNode {
public:
    std::string_view                             name;
    std::vector<std::unique_ptr<ParamDeclNode>>  fields;
    SymbolID                                     symbolId = kInvalidSymbolID;
    ScopeID                                      bodyScopeId = kInvalidScopeID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class EnumDeclNode : public DeclNode {
public:
    std::string_view                               name;
    std::vector<GenericParamNode>                  genericParams;
    std::vector<std::unique_ptr<EnumVariantNode>>  variants;
    ScopeID                                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class TypeAliasDeclNode;

class TraitDeclNode : public DeclNode {
public:
    std::string_view                               name;
    std::vector<GenericParamNode>                  genericParams;
    std::vector<std::unique_ptr<TypeAliasDeclNode>> associatedTypes;
    std::vector<std::unique_ptr<FunctionDeclNode>> methods;
    ScopeID                                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class ImplDeclNode : public DeclNode {
public:
    std::vector<GenericParamNode>                  genericParams;
    std::unique_ptr<TypeNode>                      selfType;
    std::unique_ptr<TypeNode>                      traitType;
    std::vector<std::unique_ptr<TypeAliasDeclNode>> associatedTypes;
    std::vector<std::unique_ptr<FunctionDeclNode>> methods;
    ScopeID                                        bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

class ModDeclNode : public DeclNode {
public:
    std::string_view                       name;
    std::vector<std::unique_ptr<DeclNode>> decls;
    ScopeID                                bodyScopeId = kInvalidSymbolID;
    bool                                   isOutlined = false;
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
    bool isResolvedExternal = false;
    void accept(ASTVisitor& v) override;
};

class ExternDeclNode : public DeclNode {
public:
    std::unique_ptr<FunctionDeclNode> func;
    void accept(ASTVisitor& v) override;
};

class TypeAliasDeclNode : public DeclNode {
public:
    std::string_view                       name;
    std::vector<GenericParamNode>          genericParams;
    std::vector<std::unique_ptr<TypeNode>> bounds;
    std::unique_ptr<TypeNode>              aliasedType;
    ScopeID                                bodyScopeId = kInvalidSymbolID;
    void accept(ASTVisitor& v) override;
    ASTNode* cloneImpl() const override;
};

} // namespace fl
