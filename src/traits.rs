use std::ops::Try;
use crate::{Result, IksError, parser::SaxHandler};

impl<H: SaxHandler> Try for crate::Parser<H> {
    type Output = ();
    type Residual = Result<()>;

    fn from_output(_: Self::Output) -> Self {
        unimplemented!()
    }

    fn branch(self) -> std::ops::ControlFlow<Self::Residual, Self::Output> {
        std::ops::ControlFlow::Continue(())
    }
}

impl Try for crate::DomParser {
    type Output = ();
    type Residual = Result<()>;

    fn from_output(_: Self::Output) -> Self {
        unimplemented!()
    }

    fn branch(self) -> std::ops::ControlFlow<Self::Residual, Self::Output> {
        std::ops::ControlFlow::Continue(())
    }
}

// Implement FromResidual for Result to allow ? operator
impl<T> std::ops::FromResidual<Result<()>> for Result<T> {
    fn from_residual(residual: Result<()>) -> Self {
        residual.map(|_| unreachable!())
    }
} 