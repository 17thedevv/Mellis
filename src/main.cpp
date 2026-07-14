#include <iostream>
#include "llvm/Support/raw_ostream.h"

int main() {
    // Nếu dòng này in ra được, nghĩa là fdlang đã kết nối thành công với LLVM!
    llvm::outs() << "[System] fdlang compiler is successfully linked with LLVM!\n";
    return 0;
}