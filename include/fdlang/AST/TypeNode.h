#pragma once
#include "fdlang/AST/ASTNode.h"
#include "fdlang/Core/Types.h"
#include <vector>
#include <memory>
#include <string_view>

namespace fl {

class ExprNode;
class TypeVisitor;

enum class BuiltinKind : uint8_t {
    I4, I8, I16, I32, I64, I128,
    U4, U8, U16, U32, U64, U128,
    F32, F64,
    Bool, Char, Str,
    Void,
};

class TypeNode : public ASTNode {
public:
    virtual void accept(TypeVisitor& v) = 0;
};

class BuiltinTypeNode : public TypeNode {
public:
    BuiltinKind kind;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class NamedTypeNode : public TypeNode {
public:
    std::vector<std::string_view>          segments;
    std::vector<std::unique_ptr<TypeNode>> genericArgs;
    SymbolID                               symbolId = kInvalidSymbolID;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class ReferenceTypeNode : public TypeNode {
public:
    bool                      isMutable;
    std::unique_ptr<TypeNode> inner;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class PointerTypeNode : public TypeNode {
public:
    bool                      isMutable;
    std::unique_ptr<TypeNode> inner;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class ArrayTypeNode : public TypeNode {
public:
    std::unique_ptr<TypeNode> elementType;
    std::unique_ptr<ExprNode> size;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class TupleTypeNode : public TypeNode {
public:
    std::vector<std::unique_ptr<TypeNode>> elements;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class FunctionTypeNode : public TypeNode {
public:
    std::vector<std::unique_ptr<TypeNode>> params;
    std::unique_ptr<TypeNode>              returnType;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class NeverTypeNode : public TypeNode {
public:
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

class TraitObjectTypeNode : public TypeNode {
public:
    std::unique_ptr<TypeNode> trait;
    void accept(TypeVisitor& v) override;
    void accept(ASTVisitor& v) override { }
};

} // namespace fl
