#include "mellis/IR/MVIR.h"
#include <iostream>

#include <fstream>

namespace fl {
void dumpMVIR(const mvir::Module& module) {
    std::ofstream out("d:/fdlang/mvir_output.txt");
    for (const auto& f : module.functions) {
        out << "Function: " << f->name.name << std::endl;
        for (const auto& b : f->blocks) {
            out << b->label.name << ":" << std::endl;
            for (const auto& inst : b->instructions) {
                out << "  " << inst->toString() << std::endl;
            }
            if (b->terminator) out << "  " << b->terminator->toString() << std::endl;
        }
    }
}
}
