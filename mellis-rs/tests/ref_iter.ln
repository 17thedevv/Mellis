module std {
    export enum Option<T> { None, Some(T) }
    
    export trait Iterator<T> {
        fn next(self: &rw Self) -> std::Option<T>;
    }

    export struct VecIter<T> {
        ptr: *T,
        end: *T,
    }

    impl<T> Iterator<&T> for VecIter<T> {
        fn next(self: &rw Self) -> std::Option<&T> {
            return std::Option::None::<&T>;
        }
    }
}
fn main() {}
