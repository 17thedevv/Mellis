# Kiến Trúc Drop Implementation (Flattened & Minimized MVIR)

## Tổng Quan

Sau quá trình điều tra và audit, đội ngũ phát triển Mellis quyết định thay đổi chiến lược cài đặt cơ chế `Drop` tại tầng MVIR và Backend. 

**Vấn đề:** Ban đầu, `Drop` được cài đặt dưới dạng một Opaque Instruction (`Instruction::Drop`) với kỳ vọng rằng LLVM Backend (hoặc một Drop Glue Generator) sẽ nhận một pointer tới struct và tự động sinh mã đệ quy để gọi `drop` trên từng field bên trong. Tuy nhiên, cách tiếp cận này đi ngược lại nguyên lý kiến trúc của Mellis: "Mọi semantic của chương trình phải được thể hiện tường minh tại MVIR". Ngoài ra, nó còn dẫn tới hiện tượng DCE (Dead Code Elimination) vô tình xóa mất lệnh `Drop` vì không nhận diện được side-effect ngầm.

**Giải pháp:** Áp dụng mô hình **Flattened Drop**. MVIR sẽ sinh tường minh toàn bộ cây Drop, trích xuất từng field bằng `Extract`, cấp phát con trỏ trung gian bằng `Alloca`, và gọi `Call` trực tiếp tới hàm `drop`. Bằng cách này, Backend không cần biết gì về Drop Glue và DCE không bao giờ vô tình tối ưu hóa nhầm.

## 1. Mapping Hàm Drop (`semantic_tables.rs` & `typechecker.rs`)

Để MVIR Generator có thể gọi trực tiếp hàm `drop`, bộ Typechecker cần gán chính xác `SymbolId` của struct với `SymbolId` của hàm `drop` của struct đó. 

- Thuộc tính `drop_impls` trong bảng `SemanticTables` được chuyển từ `HashSet<SymbolId>` thành `HashMap<SymbolId, SymbolId>`.
- Quá trình duyệt AST Pass 3 (Check Drop Impls) giờ đây sẽ quét `Decl::Impl` cho trait `Drop` và trích xuất hàm mang tên `drop`, sau đó mapping `Struct SymbolId -> Drop Function SymbolId`.

## 2. Loại bỏ Opaque Instruction (`mvir.rs`)

Loại bỏ hoàn toàn biến thể `Instruction::Drop` khỏi hệ thống lệnh MVIR. Các modules bị ảnh hưởng đã được dọn dẹp sạch sẽ:
- `mellis-mvir/src/mvir.rs` & `printer.rs`
- `mellis-borrowck/src/move_analysis.rs` & `borrow_analysis.rs`
- `mellis-optimizer/src/verifier.rs` & `passes/dce.rs`
- `mellis-mlib/src/writer.rs`
- `mellis-backend/src/llvm_codegen.rs`

## 3. Explicit Flattened Drop (`generator.rs`)

Hàm đệ quy `emit_flattened_drop` được thiết kế lại để thực hiện 2 nhánh rõ ràng:

1. **Primitive / Không cần Drop:** Kết thúc đệ quy.
2. **Struct có explicit Drop:**
   - Sử dụng lệnh `Instruction::Borrow { is_rw: true, base: ptr_op }` để lấy mutable reference của con trỏ struct.
   - Sử dụng lệnh `Instruction::Call` với `callee` trỏ thẳng tới hàm `drop` (dựa trên `SymbolId` tra cứu từ `drop_impls`).
3. **Struct tự động đệ quy:**
   - Dùng `Instruction::Load` nạp struct lên.
   - Lặp qua các fields theo thứ tự LIFO (để đảm bảo thứ tự phá hủy từ cuối lên đầu).
   - Dùng `Instruction::Extract` trích xuất các field cần drop.
   - Cấp phát tạm bằng `Instruction::Alloca`, sau đó dùng `Instruction::Store` ghi giá trị trích xuất vào.
   - Tiếp tục gọi `emit_flattened_drop` đệ quy lên con trỏ alloca tạm này.

## 4. Backend Support cho Borrow (`llvm_codegen.rs`)

Trước đó, lệnh `Instruction::Borrow` chỉ được stub bằng giá trị `const_zero()`. Trong bản cập nhật này, Backend đã triển khai `Borrow` dưới dạng "Zero-cost Abstraction":
- Việc gọi `generate_operand` lên một `Alloca Value` sẽ trả về chính con trỏ `Alloca` đó (do cơ chế map giá trị). Do đó, hàm xử lý `Borrow` chỉ cần truyền thẳng kết quả `llvm_ptr` thu được từ `generate_operand`. 

## 5. Bảo Vệ bởi DCE Tripwire

Do toàn bộ luồng gọi drop giờ là explicit `Instruction::Call`, DCE tự động bỏ qua tối ưu hóa nhờ bảo thủ coi mọi lệnh `Call` đều có side-effect. Để đề phòng cho tương lai (nếu xây dựng tính năng Purity Analysis), một "Tripwire" Invariant đã được ghi chú vào file `dce.rs`.

**Mẫu LLVM IR Sinh Ra Từ Cơ Chế Flattened Drop:**
```llvm
  %v12 = load { { i32 }, i32 }, ptr %v10, align 4
  %struct_field = extractvalue { { i32 }, i32 } %v12, 0
  %v14 = alloca { i32 }, align 8
  store { i32 } %struct_field, ptr %v14, align 4
  call void (...) @drop(ptr %v14)
```

## Kết Luận

Kiến trúc mới không chỉ loại trừ vĩnh viễn bug Drop bị xóa nhầm mà còn đơn giản hóa backend, duy trì vững chắc nguyên tắc cốt lõi: MVIR là Source of Truth tối thượng của chuỗi Compiler.
