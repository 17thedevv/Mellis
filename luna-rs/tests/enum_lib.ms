module std {
    export enum Option<T> {
        None,
        Some(T),
    }

    export fn get_or_default<T>(opt: std::Option<T>, default_val: T) -> T {
        match opt {
            std::Option::Some(val) -> {
                return val;
            }
            std::Option::None -> {
                return default_val;
            }
        }
    }
}
