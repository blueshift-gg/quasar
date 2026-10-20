//! Checks between accounts that `#[derive(Accounts)]` emits, in field order,
//! right after loading the account they are declared on.

use crate::{Address, ProgramError, Result, error::Error};

/// `address = expected`, for an address that is not a PDA.
pub fn address(key: &Address, expected: &Address) -> Result {
    if key == expected { Ok(()) } else { Err(Error::Address.into()) }
}

/// `check(expression @ error)`, and `has_one(field)`: the account
/// stores the key of the account named `field`.
pub fn constraint(holds: bool, error: impl Into<ProgramError>) -> Result {
    if holds { Ok(()) } else { Err(error.into()) }
}

/// Fails with `error` unless `condition` holds: `require!(amount > 0, MyError::Empty)`.
#[macro_export]
macro_rules! require {
    ($condition:expr, $error:expr $(,)?) => {
        if !($condition) {
            return ::core::result::Result::Err(::core::convert::Into::into($error));
        }
    };
}
