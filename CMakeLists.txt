# Yêu cầu phiên bản CMake tối thiểu
cmake_minimum_required(VERSION 3.20)

# Khai báo tên dự án và ngôn ngữ sử dụng
project(fdlang VERSION 1.0.0 LANGUAGES CXX)

# Ép buộc sử dụng C++17 (Cực kỳ quan trọng để dùng std::unique_ptr, std::variant)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ==========================================
# PHẦN 1: TÌM VÀ LIÊN KẾT LLVM
# ==========================================
# Tìm gói LLVM đã cài trên máy tính
find_package(LLVM REQUIRED CONFIG)

# Nạp các cờ (flags) và đường dẫn include của LLVM
include_directories(${LLVM_INCLUDE_DIRS})
separate_arguments(LLVM_DEFINITIONS_LIST NATIVE_COMMAND ${LLVM_DEFINITIONS})
add_definitions(${LLVM_DEFINITIONS_LIST})

# Yêu cầu LLVM cung cấp các module cốt lõi (Core, MC, JIT, v.v.)
llvm_map_components_to_libnames(llvm_libs Core ExecutionEngine Interpreter Support Native)

# ==========================================
# PHẦN 2: CẤU HÌNH DỰ ÁN FDLANG
# ==========================================
# Trỏ đường dẫn tới thư mục chứa các file .h của chúng ta
include_directories(include)

# Tự động gom tất cả các file .cpp trong thư mục src và thư mục con
file(GLOB_RECURSE SRC_FILES "src/*.cpp")

# Tạo file thực thi tên là 'fdlang' từ đống source code trên
add_executable(fdlang ${SRC_FILES})

# Liên kết mã nguồn của ta với các thư viện LLVM đã tìm thấy ở Phần 1
target_link_libraries(fdlang PRIVATE ${llvm_libs})