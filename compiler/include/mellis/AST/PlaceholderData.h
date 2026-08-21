#pragma once
#include <string>
#include "mellis/Core/SourceLocation.h"

namespace fl {

enum class MacroFragSpec {
    Expr,
    Ident,
    Ty,
    Stmt,
    Block,
    Item,
    Pat,
    Unknown // For placeholders used before resolving frag spec
};

struct PlaceholderData {
    std::string name;
    MacroFragSpec fragment = MacroFragSpec::Unknown;
    bool isVariadic = false;
    SourceLocation loc;
};

} // namespace fl
