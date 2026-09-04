struct Box<T> {
    val: T,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

struct Vec<T> {
    len: i32,
    cap: i32,
}

struct Tuple<T1, T2> {
    first: T1,
    second: T2,
}

fn test_generic_function<T, E>(t: T, e: E) {
    // 1. Box<T>
    dec bx = Box { val: t }; // Inferred as Box<T> because of t
    take_box(bx, t);

    // 2. Result<T, E>
    dec res: Result<T, E> = Result::Ok(t);
    take_result(res);

    // 3. Vec<Result<T, E>>
    dec vec = Vec { len: 0, cap: 0 };
    dec res2: Result<T, E> = Result::Ok(t);
    take_vec_result(vec, res2);

    // 4. Tuple<Box<T>, Result<T, E>>
    dec tup = Tuple { first: Box { val: t }, second: Result::Err(e) };
    take_tuple(tup);
}

fn take_box<T>(b: Box<T>, val: T) {}
fn take_result<T, E>(r: Result<T, E>) {}
fn take_vec_result<T, E>(v: Vec<Result<T, E> >, r: Result<T, E>) {}
fn take_tuple<T, E>(t: Tuple<Box<T>, Result<T, E> >) {}

fn test_try() -> Result<i32, u8> {
    dec res: Result<i32, u8> = Result::Ok(42);
    return res;
}

fn main() {
    test_generic_function(10, 20);
    dec _ = test_try();
}
