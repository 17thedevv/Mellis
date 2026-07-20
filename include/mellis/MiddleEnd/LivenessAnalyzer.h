#pragma once

#include <unordered_map>
#include <unordered_set>
#include <string>
#include "mellis/MiddleEnd/MVIR.h"

namespace fl {

struct LivenessInfo {
    // Map từ Tên Biến (Reference) sang tập các Instruction pointers mà biến đó còn sống (LiveOut)
    std::unordered_map<std::string, std::unordered_set<const mvir::Instruction*>> liveInstructions;
    
    // LiveOut cho từng block (phục vụ DropInsertion tại terminator)
    std::unordered_map<const mvir::BasicBlock*, std::unordered_set<std::string>> blockLiveOut;
};

class LivenessAnalyzer {
public:
    static LivenessInfo computeLiveness(const mvir::Function& func);
};

} // namespace fl
