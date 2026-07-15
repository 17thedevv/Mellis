#include <iostream>
#include <fstream>
#include <string>
#include <string_view>
#include <iomanip>
#include <cstdlib>
#include "fdlang/Frontend/Lexer.h"
#include "fdlang/Frontend/Parser.h"

using namespace fl;

std::string readFile(const std::string& filepath) {
    std::ifstream file(filepath, std::ios::binary | std::ios::ate);
    if (!file.is_open()) {
        std::cerr << "Loi: Khong the mo file '" << filepath << "'\n";
        std::exit(74);
    }
    std::streamsize size = file.tellg();
    std::string buffer(size, '\0');
    file.seekg(0, std::ios::beg);
    if (file.read(buffer.data(), size)) return buffer;
    std::cerr << "Loi: Khong the doc noi dung file '" << filepath << "'\n";
    std::exit(74);
}

int main(int argc, char* argv[]) {
    if (argc != 2) {
        std::cerr << "Cach su dung: fdlang <file_path>\n";
        return 64;
    }

    std::string filepath = argv[1];
    std::string sourceCode = readFile(filepath);

    std::cout << "[FDLANG COMPILER - MVP PHASE]\n";
    std::cout << "Compiling: " << filepath << "\n";
    std::cout << "-----------------------------------\n";

    // 1. Khởi tạo Lexer
    Lexer lexer(sourceCode);

    // 2. Nhồi Lexer vào Parser
    Parser parser(lexer);

    // 3. Ra lệnh Parse để xây dựng cây AST
    std::cout << "[1] Bat dau phan tich cu phap...\n";
    auto ast = parser.parse();

    // 4. Kiểm tra kết quả (Kiểm thử MVP)
    std::cout << "[2] Parse thanh cong! Da tim thay " << ast->statements.size() << " cau lenh.\n";
    std::cout << "[3] Kiem tra chi tiet cay AST (Cac Node trong RAM):\n";

    // Lặp qua các câu lệnh và ép kiểu (dynamic_cast) để in ra xem nó có hiểu đúng không
    for (size_t i = 0; i < ast->statements.size(); ++i) {
        auto* stmt = ast->statements[i].get();
        
        if (auto* varDecl = dynamic_cast<VarDeclStmt*>(stmt)) {
            std::cout << "  -> Cau lenh " << i + 1 << ": Khai bao bien\n";
            std::cout << "       - Ten bien : " << varDecl->varName << "\n";
            
            if (auto* numExpr = dynamic_cast<NumberExpr*>(varDecl->initializer.get())) {
                std::cout << "       - Gia tri  : " << numExpr->value << "\n";
            }
        }
    }

    std::cout << "-----------------------------------\n";
    std::cout << "Build hoan tat. Khong co loi cu phap.\n";

    return 0;
}