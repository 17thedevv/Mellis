# Ngôn ngữ Mellis - Tài liệu tham khảo

Mellis (FDLang) là ngôn ngữ lập trình hệ thống với borrow checker, generics, và LLVM backend.

---

## Mục lục

1. [Kiểu dữ liệu cơ bản](#1-kiểu-dữ-liệu-cơ-bản)
2. [Khai báo biến](#2-khai-báo-biến)
3. [Hàm](#3-hàm)
4. [Struct](#4-struct)
5. [Enum](#5-enum)
6. [Traits](#6-traits)
7. [Generics](#7-generics)
8. [Điều khiển luồng](#8-điều-khiển-luồng)
9. [Pattern Matching](#9-pattern-matching)
10. [Borrowing & Tham chiếu](#10-borrowing--tham-chiếu)
11. [Modules](#11-modules)
12. [Comptime](#12-comptime)
13. [Macros](#13-macros)
14. [Attributes & Derive](#14-attributes--derive)
15. [Async/Await](#15-asyncawait)
16. [Unsafe & Pointers](#16-unsafe--pointers)
17. [Closures](#17-closures)
18. [Literals](#18-literals)

---

## 1. Kiểu dữ liệu cơ bản

```mellis
// Số nguyên có dấu
i8, i16, i32, i64, i128, isize

// Số nguyên không dấu
u8, u16, u32, u64, u128, usize

// Số thực
f32, f64

// Boolean, ký tự, chuỗi
bool, char, str

// Void (hàm không trả về)
void

// Pointer
*T        // immutable pointer
*rw T     // mutable pointer

// Reference
&T        // immutable reference
&rw T     // mutable reference
```

---

## 2. Khai báo biến

```mellis
// Immutable (mặc định)
dec x: i32 = 10;
dec y = 20;           // type inference

// Mutable
dec mut z: i32 = 30;
z = 40;               // OK

// Const (compile-time constant)
const PI: f64 = 3.14159;

// rw (alias cho mutable)
rw counter: i32 = 0;
counter = counter + 1;
```

**Quy tắc:**
- `dec` - biến immutable
- `dec mut` hoặc `rw` - biến mutable
- `const` - hằng số compile-time

---

## 3. Hàm

```mellis
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

fn greet(name: str) {
    // no return needed for void
}

fn main() -> i32 {
    dec result = add(1, 2);
    return result;
}
```

**Method syntax:**
```mellis
struct Point {
    x: i32;
    y: i32;
}

impl Point {
    // Receiver as explicit first parameter
    fn add(self: Point, other: Point) -> Point {
        return Point {
            x: self.x + other.x,
            y: self.y + other.y
        };
    }
}

// Gọi method
dec p1 = Point { x: 1, y: 2 };
dec p2 = Point { x: 3, y: 4 };
dec p3 = p1.add(p2);
```

---

## 4. Struct

```mellis
// Named Struct
struct Point {
    x: i32;
    y: i32;
}
dec p = Point { x: 10, y: 20 };

// Tuple Struct
struct Color(i32, i32, i32);
dec c = Color(255, 0, 0);

// Unit Struct
struct Empty;
dec e = Empty;

// Generic Struct
struct Box<T> {
    value: T;
}
dec b = Box<i32> { value: 42 };

// Default initialization
dec v: Vector3;
v.x = 10;
v.y = 20;
v.z = 30;
```

---

## 5. Enum

```mellis
enum Color {
    Red,
    Green,
    Blue,
}

// Enum với data (tuple variants)
enum Shape {
    Circle(f32),
    Rectangle(f32, f32),
    Point { x: f32, y: f32 },
}

dec shape = Shape::Circle(1.0);

// Enum với repr
#[repr(u8)]
enum EntityState {
    Idle,
    Moving(f32, f32),
    Dead,
}
```

---

## 6. Traits

```mellis
trait Display {
    fn fmt(self: &Self);
}

struct Point {
    x: i32;
    y: i32;
}

impl Display for Point {
    fn fmt(self: &Point) {
        // ...
    }
}

// Trait bound
fn print_it<T: Display>(item: &T) {
    item.fmt();
}

// Trait với generic
trait Process {
    fn process(self) -> i32;
}

struct Data { val: i32 }

impl Process for Data {
    fn process(self) -> i32 {
        self.val
    }
}

fn run_process<T: Process>(item: T) -> i32 {
    item.process()
}
```

---

## 7. Generics

```mellis
struct Vec<T> {
    data: T;
}

struct Map<K, V> {
    key: K;
    value: V;
}

fn identity<T>(x: T) -> T {
    return x;
}

fn main() {
    dec v: Vec<i32> = Vec<i32> { data: 42 };
    dec val = identity(42);
    dec b = identity(true);
}
```

**Generic với trait bounds:**
```mellis
struct Container<T: Clone> {
    item: T,
}
```

---

## 8. Điều khiển luồng

### if/else
```mellis
if x > 10 {
    print("Greater\n");
} else if x == 10 {
    print("Equal\n");
} else {
    print("Less\n");
}
```

### while
```mellis
dec mut count = 0;
while count < 5 {
    count = count + 1;
}
```

### break/continue
```mellis
dec mut i = 0;
while i < 10 {
    i = i + 1;
    if i == 5 {
        continue;
    }
    if i > 8 {
        break;
    }
    print("i = {}\n", i);
}
```

---

## 9. Pattern Matching

```mellis
fn calc_match(x: i32) -> i32 {
    dec res: i32 = 0;
    match x {
        1 -> { res = 10; }
        2 -> { res = 20; }
        _ -> { res = 100; }
    };
    return res;
}
```

**Destructuring:**
```mellis
match shape {
    Shape::Circle(r) -> r * r * 3.14,
    Shape::Rectangle(w, h) -> w * h,
    Shape::Point { x, y } -> x + y,
}
```

---

## 10. Borrowing & Tham chiếu

```mellis
fn modify(val: &rw i32) {
    *val = *val + 1;
}

fn read(val: &i32) -> i32 {
    *val
}

dec mut x = 10;
modify(&rw x);
dec y = read(&x);
```

**Struct field borrowing:**
```mellis
struct Vector3 {
    x: i32;
    y: i32;
    z: i32;
}

dec v: Vector3;
v.z = 30;

{
    dec ref_z: &rw i32 = &rw v.z;
    *ref_z = 100;  // v.z = 100
}
```

---

## 11. Modules

**Library file (mylib.ms):**
```mellis
export fn my_add(a: i32, b: i32) -> i32 {
    return a + b;
}
```

**Import:**
```mellis
extern fn printf(format: str, ...) -> i32;

mod mylib;
use mylib::my_add;

fn main() -> i32 {
    dec result = my_add(10, 20);
    printf("Result is %d\n", result);
    return result;
}
```

---

## 12. Comptime

```mellis
// Hàm chạy lúc compile-time
comptime fn calc_max_entities() -> u32 {
    return 1024 * 16;
}

// Const với comptime
const MAX_ENTITIES: u32 = calc_max_entities();

// sizeof/alignof
const SZ = sizeof(i32);
const AL = alignof(i32);

// Block comptime
comptime {
    dec check = sizeof(i32) == 4;
}
```

---

## 13. Macros

```mellis
// print/println
print("Hello, {}!\n", name);
println!("Line with newline");

// assert
assert!(condition);
assert_eq!(a, b);

// vec! literal
dec nums = vec![1, 2, 3, 4, 5];

// format!
dec msg = format!("Value: {}", x);
```

---

## 14. Attributes & Derive

```mellis
// Derive common traits
#[derive(Clone, PartialEq, Default, Debug)]
struct Config {
    name: str,
    value: i32,
}

// C-compatible struct layout
#[repr(C)]
struct NativeData {
    id: u32,
    data: *u8,
}

// Packed struct
#[packed]
struct PackedData {
    a: u8,
    b: u32,
}

// Test function
#[test]
fn test_example() {
    assert!(true);
}

// Inline hint
#[inline]
fn fast_path() { }
```

---

## 15. Async/Await

```mellis
async fn fetch_data() -> Result<str, i32> {
    dec response = http_get("https://api.example.com").await?;
    return Result::Ok(response);
}

async fn main() {
    dec data = fetch_data().await;
    match data {
        Ok(content) -> print("Got: {}\n", content),
        Err(e) -> print("Error: {}\n", e),
    }
}
```

---

## 16. Unsafe & Pointers

```mellis
extern fn malloc(size: u64) -> *u8;

unsafe {
    dec ptr: *rw u8 = malloc(1024);
    *ptr = 42;  // dereference
    dec val = *ptr;
}
```

**Pointer cast:**
```mellis
dec raw: *u8 = ...;
dec typed: *i32 = raw as *i32;
```

---

## 17. Closures

```mellis
dec factor = 2;
dec multiply = |x: i32| -> i32 {
    x * factor
};

dec result = multiply(5);  // 10
```

---

## 18. Literals

```mellis
// Integer
42, 0xFF, 0o755, 0b1010
42i32, 100u64

// Float
3.14, 3.14f32, 6.02e23f64

// Character
'a', '\n', '\xFF'

// String
"hello", "line\n"

// Byte
b'X', b"hello"

// Boolean
true, false

// Array
[1, 2, 3, 4, 5]

// Tuple
(1, "two", 3.0)
()  // unit

// Range (trong for loop)
for i in 0..10 {
    // i từ 0 đến 9
}
```

---

## Tính năng chưa triển khai đầy đủ

| Tính năng | Trạng thái | Ghi chú |
|------------|------------|---------|
| Async I/O | Partial | Framework async có, stdlib chưa |
| Closures capture modes | Partial | Cú pháp có, capture logic chưa đầy đủ |
| Comptime type_of, type_info | Partial | MVIR infrastructure có, stdlib chưa |
| For loop (foreach) | Partial | C-syntax có, foreach-style chưa |

---

## Tổng kết

Mellis là ngôn ngữ hệ thống với:
- **Type safety** - borrow checker, generics, trait bounds
- **Zero-cost abstractions** - monomorphization
- **Compile-time execution** - comptime functions, sizeof/alignof
- **Metaprogramming** - macros
- **Async support** - async/await syntax
- **Unsafe blocks** - raw pointers khi cần
