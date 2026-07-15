# Yêu cầu phiên bản CMake tối thiểu
cmake_minimum_required(VERSION 3.20)

# Khai báo tên dự án và ngôn ngữ sử dụng
project(fdlang VERSION 1.0.0 LANGUAGES CXX)

# Ép buộc sử dụng C++17 (Cực kỳ quan trọng để dùng std::unique_ptr, std::variant)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ==========================================
# PHẦN 1: TÌM VÀ CẤU HÌNH LLVM
# ==========================================
# Thả CMake vào thư mục gốc của LLVM-DEV để nó tự tìm
list(APPEND CMAKE_PREFIX_PATH "D:/Programs/LLVM-DEV")
find_package(LLVM REQUIRED CONFIG)

# Bơm các thông số của LLVM vào bộ nhớ của CMake
include_directories(${LLVM_INCLUDE_DIRS})
link_directories(${LLVM_LIBRARY_DIRS})      # Trỏ đường dẫn đến các file .lib

link_directories("C:/Program Files/Microsoft Visual Studio/18/Community/DIA SDK/lib/amd64")  # Trỏ đường dẫn đến các file .exe
add_definitions(${LLVM_DEFINITIONS})

# Yêu cầu LLVM tự động tìm tên file .lib chuẩn cho các module cần thiết
llvm_map_components_to_libnames(LLVM_LIBS Core ExecutionEngine Interpreter Support native)

# ==========================================
# PHẦN 2: CẤU HÌNH DỰ ÁN FDLANG
# ==========================================
# Trỏ đường dẫn tới thư mục chứa các file .h của chúng ta
include_directories(include)

# Tự động gom tất cả các file .cpp trong thư mục src và thư mục con
file(GLOB_RECURSE SRC_FILES "src/*.cpp")

# BƯỚC QUAN TRỌNG: Tạo file thực thi tên là 'fdlang' trước
add_executable(fdlang ${SRC_FILES})

# BƯỚC CUỐI CÙNG: Liên kết mã nguồn của ta với các thư viện LLVM đã chuẩn bị ở Phần 1
target_link_libraries(fdlang PRIVATE ${LLVM_LIBS})