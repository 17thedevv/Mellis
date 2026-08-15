#pragma once

#include "mellis/IR/MVIR.h"
#include "mellis/MiddleEnd/Semantic/MoveAnalyzer.h"
#include "mellis/MiddleEnd/Semantic/InitializationAnalyzer.h"

namespace fl {

class DropElaborator {
    mvir::Module* module_;

public:
    explicit DropElaborator(mvir::Module* module) : module_(module) {}

    void run();

private:
    void elaborateFunction(mvir::Function& func);
};

} // namespace fl
