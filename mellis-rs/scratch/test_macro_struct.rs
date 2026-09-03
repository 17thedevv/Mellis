use mellis_driver::{compile, CompilerOptions};

fn main() {
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

    let options = CompilerOptions {
        quiet: true,
        no_link: true,
        ..Default::default()
    };
    compile("test_struct.ms", code.to_string(), &options).unwrap();
}
