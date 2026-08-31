use mellis_ast::expr::{Expr, UnaryOp};
use mellis_parser::Parser;
fn main() {
    let source = "unsafe { *rw z = 5; }";
    let (ast, _, _) = Parser::parse_module_from_source(source);
    println!("{:#?}", ast);
}
