#include <iostream>
#include <string>
#include "mellis/Core/CompilerSession.h"
#include "mellis/LSP/LSPServer.h"

using namespace fl;

void printUsage() {
    std::cout << "mellis: error: Cach su dung: mellis <file_path.ms> [-v|--verbose] [-lib] [--lsp] [--version]\n";
}

int main(int argc, char* argv[]) {
    bool runLsp = false;
    bool verbose = false;
    bool emitLib = false;
    int optLevel = 0;
    std::string filepath = "";

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "-h" || arg == "--help") {
            printUsage();
            return 0;
        } else if (arg == "--version") {
            std::cout << "mellis version 0.1.0\n";
            return 0;
        } else if (arg == "--lsp") {
            runLsp = true;
        } else if (arg == "-v" || arg == "--verbose") {
            verbose = true;
        } else if (arg == "-lib") {
            emitLib = true;
        } else if (arg == "-O1") {
            optLevel = 1;
        } else if (filepath.empty() && arg[0] != '-') {
            filepath = arg;
        }
    }

    if (runLsp) {
        return fl::lsp::runServer();
    }

    if (filepath.empty()) {
        std::cerr << "mellis: error: Cach su dung: mellis <file_path.ms> [-v|--verbose] [-lib] [--lsp] [--version]\n";
        return 1;
    }

    // std::cout << "[MELLIS COMPILER]" << std::endl;

    CompilerSession session;
    bool success = session.compile(filepath, verbose, optLevel, emitLib);

    if (!success) {
        return 65; // data format error
    }

    return 0;
}
