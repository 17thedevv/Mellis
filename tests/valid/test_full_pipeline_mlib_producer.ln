export enum Result<T, E> {
    Ok(T),
    Err(E)
}

export trait Process {
    fn process(self) -> int_32;
}

export fn handle_result<T: Process, E: Process>(res: Result<T, E>) -> int_32 {
    match res {
        Result::Ok(t) -> { return t.process(); },
        Result::Err(e) -> { return e.process(); }
    }
}
