#![allow(unexpected_cfgs)]
//! Two `#[error_code]` variants resolving to the same discriminant are a hard,
//! spanned error naming both variants (here: an explicit collision at 6000).

use quasar_lang::prelude::*;

#[error_code]
pub enum MyError {
    First = 6000,
    Second = 6000,
}

fn main() {}
