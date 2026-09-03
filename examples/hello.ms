// 1. Sửa cú pháp FFI (bỏ chữ fn nếu Parser của bạn không yêu cầu, 
// hoặc giữ 'extern fn' nếu EBNF của bạn quy định vậy - dưới đây tôi viết chuẩn FDLang nhất)
extern fn printf(format: str, ...) -> int_32; 

// 2. Struct
struct Point {
    x: int_32;
    y: int_32;
}

// 3. Trait
trait Printable {
    // Shorthand self (FDLang will parse it as self: Self)
    fn display(self); 
}

// 4. Impl
impl Printable for Point {
    // Impl cũng có thể dùng shorthand self
    fn display(self) {
        printf("Point(x: %d, y: %d)\n", self.x, self.y); 
    }
}

// 5. Hàm Generic bị ràng buộc
fn display_item<T: Printable>(item: T) {
    // Compiler phải link được hàm display() của Point vào đây!
    item.display(); 
}

extern fn scanf(format: str, ...) -> int_32;

fn main() -> int_32 {
    dec x: int_32;
    printf("Please enter a number: ");
    dec r1 = scanf("%d", &x);
    if r1 == 1 {
        dec pt: Point;
        pt.x = x;
        pt.y = 20;
        display_item<Point>(pt);
        return x;
    }
    return r1;
}