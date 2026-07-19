#include <iostream>
#include <string>
#include "mellis/Core/CompilerSession.h"

using namespace fl;

int main(int argc, char* argv[]) {
    std::cout << "[MELLIS COMPILER]" << std::endl;
    if (argc < 2) {
        std::cerr << "mellis: error: Cach su dung: mellis <file_path> [-v|--verbose]\n";
        return 1;
    }

    std::string filepath = argv[1];
    bool verbose = false;
    for (int i = 2; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "-v" || arg == "--verbose") {
            verbose = true;
        }
    }

    CompilerSession session;
    bool success = session.compile(filepath, verbose);

    if (!success) {
        return 65; // data format error
    }

    return 0;
}
