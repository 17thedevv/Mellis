use mellis_driver::check;

#[test]
fn test_macro_repetition_star() {
    let code = r#"
        macro test_rep {
            ($(@x: expr),*) => {
                $(
                    @x;
                )*
            }
        }
        
        fn main() -> i32 {
            test_rep!(1 + 1, 2 * 3, 4);
            return 0;
        }
    "#;

    let res = check("test_repetition.ms", code.to_string(), &mellis_driver::CompilerOptions { search_paths: vec!["../../libs/external".to_string()], ..Default::default() });
    assert!(
        res.is_ok(),
        "Macro repetition check failed: {:?}",
        res.err()
    );
}
