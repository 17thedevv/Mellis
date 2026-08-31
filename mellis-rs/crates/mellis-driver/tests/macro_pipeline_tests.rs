use mellis_driver::{compile, check, CompilerOptions};

#[test]
fn test_macro_full_pipeline_arithmetic() {
    let code = r#"
    macro square {
        (@x: expr) => { @x * @x }
    }

    macro calc_hypotenuse {
        (@a: expr, @b: expr) => {
            square!(@a) + square!(@b)
        }
    }

    fn main() -> i32 {
        dec res = calc_hypotenuse!(3, 4);
        return res; // 3*3 + 4*4 = 25
    }
    "#;

    let res = check("test_arithmetic.ms", code.to_string(), &[], true);
    assert!(res.is_ok(), "Macro arithmetic check failed: {:?}", res.err());

    let options = CompilerOptions {
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let compile_res = compile("test_arithmetic.ms", code.to_string(), &options);
    assert!(compile_res.is_ok(), "Macro arithmetic compile failed: {:?}", compile_res.err());
}

#[test]
fn test_macro_full_pipeline_struct_and_impl() {
    let code = r#"
    macro make_point_struct {
        (@name: ident) => {
            struct @name {
                x: i32,
                y: i32,
            }
        }
    }

    make_point_struct!(Point);

    impl Point {
        fn sum(self: Point) -> i32 {
            return self.x + self.y;
        }
    }

    fn main() -> i32 {
        dec pt = Point { x: 15, y: 27 };
        return pt.sum();
    }
    "#;

    let res = check("test_struct.ms", code.to_string(), &[], true);
    assert!(res.is_ok(), "Macro struct check failed: {:?}", res.err());

    let options = CompilerOptions {
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let compile_res = compile("test_struct.ms", code.to_string(), &options);
    assert!(compile_res.is_ok(), "Macro struct compile failed: {:?}", compile_res.err());
}

#[test]
fn test_macro_full_pipeline_borrowck_safety() {
    let code = r#"
    macro swap_val {
        (@a: expr, @b: expr) => {
            dec tmp = @a;
            @a = @b;
            @b = tmp;
        }
    }

    fn main() -> i32 {
        dec rw x: i32 = 10;
        dec rw y: i32 = 20;
        swap_val!(x, y);
        return x; // 20
    }
    "#;

    let res = check("test_borrowck.ms", code.to_string(), &[], true);
    assert!(res.is_ok(), "Macro borrowck safety check failed: {:?}", res.err());

    let options = CompilerOptions {
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    let compile_res = compile("test_borrowck.ms", code.to_string(), &options);
    assert!(compile_res.is_ok(), "Macro borrowck compile failed: {:?}", compile_res.err());
}
