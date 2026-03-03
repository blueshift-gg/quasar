use quasar_core::prelude::*;

#[error_code]
pub enum TestError {
    Hello = 0,
    World,
    ExplicitNum = 100,
    RequireFailed,
    RequireEqFailed,
    ConstraintCustom,
    AddressCustom,
}
