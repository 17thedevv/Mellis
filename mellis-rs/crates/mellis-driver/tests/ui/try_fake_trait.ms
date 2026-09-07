

struct Fake<T> {
    value: T,
}

trait FakeTry<R> {
    fn branch(self: Self) -> ControlFlow<R, Self>;
    fn from_residual(residual: R) -> Self;
}

impl<T> FakeTry<Fake<T>> for Fake<T> {
    fn branch(self: Self) -> ControlFlow<Fake<T>, Fake<T>> {
        return ControlFlow::Continue(self);
    }

    fn from_residual(residual: Fake<T>) -> Self {
        return residual;
    }
}

fn test_fake() -> Fake<i32> {
    dec x = Fake { value: 1 };
    dec y = x?; //~ ERROR The `?` operator can only be applied to types that implement `Try` //~ ERROR The `?` operator cannot be used in a function that returns this type, as it does not implement `FromResidual` for the residual type
    return Fake { value: y };
}

fn main() -> i32 {
    return 0;
}
