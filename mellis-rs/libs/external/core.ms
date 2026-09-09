#[lang("drop")]
export trait Drop {
    #[lang("drop_fn")]
    fn drop(self: &rw Self);
}

#[lang("copy")]
export trait Copy {}

#[lang("from_residual")]
export trait FromResidual<R> {
    #[lang("from_residual_fn")]
    fn from_residual(residual: R) -> Self;
}

#[lang("try")]
export trait Try {
    type Output;
    type Residual;
    
    #[lang("try_from_output")]
    fn from_output(v: Self::Output) -> Self;
    
    #[lang("try_branch")]
    fn branch(self: Self) -> ControlFlow<Self::Residual, Self::Output>;
}

#[lang("control_flow")]
export enum ControlFlow<B, C> {
    #[lang("continue")]
    Continue(C),
    #[lang("break")]
    Break(B),
}

export enum Result<T, E> {
    Ok(T),
    Err(E),
}

export enum Infallible {
    InfallibleVariant,
}
impl<T, E> FromResidual<Result<Infallible, E> > for Result<T, E> {
    fn from_residual(residual: Result<Infallible, E>) -> Self {
        return match residual {
            Result::Err(e) -> Result::Err(e),
        };
    }
}

impl<T, E> Try for Result<T, E> {
    type Output = T;
    type Residual = Result<Infallible, E>;
    
    fn from_output(v: T) -> Self {
        return Result::Ok(v);
    }
    
    fn branch(self: Self) -> ControlFlow<Result<Infallible, E>, T> {
        return match self {
            Result::Ok(v) -> ControlFlow::Continue(v),
            Result::Err(e) -> ControlFlow::Break(Result::Err(e)),
        };
    }
}
