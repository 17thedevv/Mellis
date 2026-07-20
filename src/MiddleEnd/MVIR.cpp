#include "mellis/MiddleEnd/MVIR.h"
#include <sstream>

namespace fl {
namespace mvir {

// =============================================================================
// 1. Types
// =============================================================================

std::string formatType(const Type* type) {
    if (!type) return "void";
    if (type->getKind() == TypeKind::Primitive) {
        auto prim = static_cast<const PrimitiveType*>(type);
        if (prim->builtinKind == BuiltinKind::I32) return "i32";
        if (prim->builtinKind == BuiltinKind::Bool) return "bool";
        if (prim->builtinKind == BuiltinKind::Void) return "void";
    }
    return type->toString();
}

// =============================================================================
// 2. Operands
// =============================================================================

std::string toString(const Operand& op) {
    return std::visit([](auto&& arg) -> std::string {
        return arg.toString();
    }, op);
}

// =============================================================================
// 3. Instructions
// =============================================================================

std::string LocalInst::toString() const {
    return dest.toString() + " = local " + formatType(type);
}

std::string LoadInst::toString() const {
    return dest.toString() + " = load " + formatType(type) + " " + mvir::toString(ptr);
}

std::string StoreInst::toString() const {
    return "store " + formatType(type) + " " + mvir::toString(value) + ", " + mvir::toString(ptr);
}

std::string IndexInst::toString() const {
    return dest.toString() + " = index " + formatType(type) + " " + mvir::toString(base) + ", " + mvir::toString(index);
}

std::string FieldInst::toString() const {
    return dest.toString() + " = field " + formatType(type) + " " + mvir::toString(base) + ", " + std::to_string(index);
}


std::string BorrowInst::toString() const {
    return dest.toString() + " = borrow " + (isMutable ? "mut" : "shared") + " " + mvir::toString(base);
}

std::string CastInst::toString() const {
    return dest.toString() + " = cast " + mvir::toString(value) + " to " + formatType(targetType);
}

std::string DropInst::toString() const {
    return "drop " + mvir::toString(value) + " : " + formatType(type);
}

std::string SizeofInst::toString() const {
    return dest.toString() + " = sizeof " + formatType(targetType);
}

std::string AlignofInst::toString() const {
    return dest.toString() + " = alignof " + formatType(targetType);
}

std::string formatAluOp(AluOp op) {
    switch (op) {
        case AluOp::Add: return "add";
        case AluOp::Sub: return "sub";
        case AluOp::Mul: return "mul";
        case AluOp::Div: return "div";
        case AluOp::Eq:  return "eq";
        case AluOp::Ne:  return "ne";
        case AluOp::Lt:  return "lt";
        case AluOp::Le:  return "le";
        case AluOp::Gt:  return "gt";
        case AluOp::Ge:  return "ge";
    }
    return "unknown_alu_op";
}

std::string AluInst::toString() const {
    return dest.toString() + " = " + formatAluOp(op) + " " +
           mvir::toString(left) + ", " + mvir::toString(right);
}

std::string formatUnaryOp(UnaryOp op) {
    switch (op) {
        case UnaryOp::Negate: return "neg";
        case UnaryOp::BitNot: return "not";
    }
    return "unknown_unary_op";
}

std::string UnaryInst::toString() const {
    return dest.toString() + " = " + formatUnaryOp(op) + " " + mvir::toString(operand);
}

std::string ExtractInst::toString() const {
    return dest.name + " = extract " + mvir::toString(base) + ", var " + std::to_string(variantIndex) + ", fld " + std::to_string(fieldIndex);
}

std::string TagInst::toString() const {
    return dest.toString() + " = tag " + mvir::toString(base);
}

std::string VariantInst::toString() const {
    std::string res = dest.toString() + " = variant " + enumType->toString() + ", " + std::to_string(variantIndex);
    if (!args.empty()) {
        res += " (";
        for (size_t i = 0; i < args.size(); ++i) {
            res += mvir::toString(args[i]);
            if (i + 1 < args.size()) res += ", ";
        }
        res += ")";
    }
    return res;
}
std::string MakeTraitObjectInst::toString() const {
    std::string res = dest.toString() + " = make_trait_object " + mvir::toString(value) + " from " + formatType(concreteType) + " to " + formatType(targetType) + " vtable: [";
    for (size_t i = 0; i < vtableMethods.size(); ++i) {
        res += std::to_string(vtableMethods[i]);
        if (i + 1 < vtableMethods.size()) res += ", ";
    }
    res += "]";
    return res;
}

std::string CallInst::toString() const {
    std::string res;
    if (dest) {
        res += dest->toString() + " = ";
    }
    res += "call " + fl::mvir::toString(func) + "(";
    for (size_t i = 0; i < args.size(); ++i) {
        res += mvir::toString(args[i]);
        if (i < args.size() - 1) {
            res += ", ";
        }
    }
    res += ")";
    return res;
}

std::string VirtualCallInst::toString() const {
    std::string res;
    if (dest) {
        res += dest->toString() + " = ";
    }
    res += "vcall " + mvir::toString(receiver) + " [" + std::to_string(methodIndex) + "] (";
    for (size_t i = 0; i < args.size(); ++i) {
        res += mvir::toString(args[i]);
        if (i < args.size() - 1) {
            res += ", ";
        }
    }
    res += ")";
    return res;
}

std::string AwaitInst::toString() const {
    return dest.toString() + " = await " + mvir::toString(futureVal);
}

// =============================================================================
// 4. Terminators
// =============================================================================

std::string JumpTerm::toString() const {
    return "jump " + target.toString();
}

std::string BranchTerm::toString() const {
    return "branch " + mvir::toString(condition) + ", " +
           trueTarget.toString() + ", " + falseTarget.toString();
}

std::string RetTerm::toString() const {
    if (value) {
        return "ret " + mvir::toString(*value);
    }
    return "ret";
}

std::string SwitchTerm::toString() const {
    std::string res = "switch " + mvir::toString(condition) + " { ";
    for (const auto& c : cases) {
        res += c.first.toString() + ": " + c.second.toString() + ", ";
    }
    res += "default: " + defaultTarget.toString() + " }";
    return res;
}

// =============================================================================
// 5. Structure
// =============================================================================

std::string BasicBlock::toString() const {
    std::string res = label.toString() + ":\n";
    for (const auto& inst : instructions) {
        res += "  " + inst->toString() + "\n";
    }
    if (terminator) {
        res += "  " + terminator->toString() + "\n";
    }
    return res;
}

std::string Param::toString() const {
    return formatType(type) + " " + id.toString();
}

std::string Function::toString() const {
    std::string res = "func " + name.toString() + "(";
    for (size_t i = 0; i < params.size(); ++i) {
        res += params[i].toString();
        if (i < params.size() - 1) {
            res += ", ";
        }
    }
    res += ") -> " + formatType(returnType) + " {\n";
    for (const auto& bb : blocks) {
        res += bb->toString();
    }
    res += "}\n";
    return res;
}

std::string ExternFunction::toString() const {
    std::string res = "extern func " + name.toString() + "(";
    for (size_t i = 0; i < paramTypes.size(); ++i) {
        res += formatType(paramTypes[i]);
        if (i < paramTypes.size() - 1) res += ", ";
    }
    res += ") -> " + formatType(returnType);
    return res;
}

std::string TypeDecl::toString() const {
    std::string res = "type " + name + " = struct {";
    for (size_t i = 0; i < fields.size(); ++i) {
        res += formatType(fields[i]);
        if (i < fields.size() - 1) res += ", ";
    }
    res += "}";
    return res;
}

std::string GlobalDecl::toString() const {
    return name.toString() + " = global " + formatType(type) + " " + stringLiteral;
}

std::string Module::toString() const {
    std::string res;
    for (const auto& t : typeDecls) {
        res += t.toString() + "\n";
    }
    if (!typeDecls.empty()) res += "\n";
    for (const auto& g : globalDecls) {
        res += g.toString() + "\n";
    }
    if (!globalDecls.empty()) res += "\n";
    for (const auto& e : externFunctions) {
        res += e.toString() + "\n";
    }
    if (!externFunctions.empty()) res += "\n";
    for (const auto& func : functions) {
        res += func->toString() + "\n";
    }
    return res;
}

} // namespace mvir
} // namespace fl
