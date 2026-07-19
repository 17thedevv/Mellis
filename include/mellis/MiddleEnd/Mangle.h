#pragma once
#include <string>
#include <vector>
#include <string_view>

namespace fl {

class Type;
class SymbolTable;

namespace Mangle {

/// Mangles a concrete type into a stable string identifier.
/// E.g., Vec<int_32> -> Vec_int_32
std::string typeToString(const Type* type, const SymbolTable& symTable);

/// Mangles a function signature into a specialized name.
/// E.g., push(Vec<int_32>, int_32) -> push_Vec_int_32_int_32
std::string mangleFunction(std::string_view baseName, const std::vector<const Type*>& genericArgs, const SymbolTable& symTable);

/// Mangles a struct or enum into a specialized name.
std::string mangleStruct(std::string_view baseName, const std::vector<const Type*>& genericArgs, const SymbolTable& symTable);

} // namespace Mangle
} // namespace fl
