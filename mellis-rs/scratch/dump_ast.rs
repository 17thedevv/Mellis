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

    let mut arena = mellis_ast::AstArena::new();
    let mut diagnostics = Vec::new();
    let root = mellis_parser::parse_module("test.ms", code, &mut arena, &mut diagnostics);
    println!("{:#?}", root);
    
    for decl_id in root.items {
        if let mellis_ast::Decl::Impl { methods, .. } = &arena.decls[decl_id.0 as usize] {
            println!("IMPL methods: {:#?}", methods);
            for m in methods {
                if let mellis_ast::Decl::Function { params, .. } = &arena.decls[m.0 as usize] {
                    println!("METHOD params: {:#?}", params);
                }
            }
        }
    }
}
