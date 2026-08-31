export module mock_core {
    export enum Result<T, E> {
        Ok(T),
        Err(E),
    }

    export struct Error {
        code: i32,
    }

    fn mock_internal() {}
}

