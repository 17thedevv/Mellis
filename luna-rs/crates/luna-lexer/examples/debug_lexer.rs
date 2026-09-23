use luna_lexer::{Lexer, TokenKind};
use luna_common::ids::FileId;
fn main() {
    let source = "dec my_vec: Vec<int_32> = Vec<int_32> { data: null };";
    let mut lexer = Lexer::new(source, FileId(0));
    loop {
        let token = lexer.next().unwrap();
        println!("{:?} {:?}", token.kind, &source[token.span.start as usize .. token.span.end as usize]);
        if token.kind == TokenKind::Eof || token.kind == TokenKind::Error {
            break;
        }
    }
}
