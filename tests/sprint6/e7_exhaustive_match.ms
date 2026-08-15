enum Status {
    Ok,
    Err,
    Pending
}

fn main() -> int_32 {
    dec s = Status::Pending;
    dec result = match s {
        Status::Ok -> 0,
        Status::Err -> 1,
        Status::Pending -> 2
    };
    return result;
}
