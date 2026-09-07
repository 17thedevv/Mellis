

module custom_try_mod {
    export enum MyResult<T, E> {
        MyOk(T),
        MyErr(E),
    }

    export struct MyResidual<E> {
        err: E,
    }

    impl<T, E> core::Try for MyResult<T, E> {
        type Output = T;
        type Residual = MyResidual<E>;

        fn from_output(x: T) -> Self {
            return MyResult::MyOk(x);
        }

        fn branch(self: Self) -> core::ControlFlow<MyResidual<E>, T> {
            return match self {
                MyResult::MyOk(t) -> core::ControlFlow::Continue(t),
                MyResult::MyErr(e) -> core::ControlFlow::Break(MyResidual { err: e }),
            };
        }
    }

    impl<T, E> core::FromResidual<MyResidual<E> > for MyResult<T, E> {
        fn from_residual(r: MyResidual<E>) -> Self {
            return MyResult::MyErr(r.err);
        }
    }
}
