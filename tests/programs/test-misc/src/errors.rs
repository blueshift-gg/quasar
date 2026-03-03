use quasar_core::prelude::*;

#[error_code]
pub enum TestError {
    Unauthorized,
    InvalidAddress,
    CustomConstraint,
}
