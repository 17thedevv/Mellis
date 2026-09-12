enum Result<T, E> {
    Ok(T),
    Err(E),
}

struct Vec<T> {
    len: i32,
    cap: i32,
}

fn create_cycle<T>(a: T, b: Vec<Result<T, T> >) {}

fn test_complex() {
    dec vec = Vec { len: 0, cap: 0 }; // vec: Vec<?V>
    // By passing vec as both arguments, we unify `Vec<?V>` with `Vec<Result<Vec<?V>, Vec<?V>>>`
    create_cycle(vec, vec);
}

fn main() {
    dec rw res = Result::Ok(0); // res: Result<i32, ?E>
    
    // Unify ?E with Result<i32, ?E> -> CYCLE!
    res = Result::Err(res);
}
