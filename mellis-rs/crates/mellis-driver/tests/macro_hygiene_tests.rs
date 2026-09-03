use mellis_driver::check;

#[test]
fn test_macro_hygiene_generic_params() {
    let code = r#"
        macro make_generic_struct {
            (@name: ident) => {
                struct @name<T> {
                    val: T,
                }
                
                impl<T> @name<T> {
                    fn get_val(self: @name<T>) -> T {
                        return self.val;
                    }
                }
            }
        }
        
        make_generic_struct!(Container);
        
        fn main() -> i32 {
            dec c = Container { val: 42 };
            return c.get_val();
        }
    "#;

    let res = check("test_hygiene.ms", code.to_string(), &[], true);
    assert!(res.is_ok(), "Macro generic hygiene check failed: {:?}", res.err());
}
