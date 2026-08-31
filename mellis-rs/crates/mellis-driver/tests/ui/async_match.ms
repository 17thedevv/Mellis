enum State {
    Active(i32),
    Inactive,
}

async fn helper(x: i32) -> i32 {
    return x * 2;
}

async fn test_match(s: State) -> i32 {
    match s {
        State::Active(x) -> {
            dec res = await helper(x);
            return res;
        }
        State::Inactive -> {
            return 0;
        }
    }
}

fn main() -> i32 {
    dec fut = test_match(State::Active(5));
    return 0;
}
