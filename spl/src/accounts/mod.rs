//! SPL behavior modules for `#[derive(Accounts)]`.
//!
//! Each module exports `Args`, `Args::builder()`, and `Behavior`, implementing
//! `AccountBehavior` for supported account wrapper types.
//!
//! # Adding a new behavior
//!
//! 1. Create `accounts/foo.rs`
//! 2. Define `Args` struct + `ArgsBuilder` with `build_init()`,
//!    `build_check()`, `build_exit()` methods
//! 3. Define `Behavior` unit struct
//! 4. Implement `AccountBehavior<T>` for each supported wrapper type
//! 5. Export from `accounts/mod.rs` and `prelude.rs`
//! 6. Add compile-pass and compile-fail tests in `spl/tests/`

pub mod associated_token;
pub mod mint;
pub mod token;
pub mod token_close;
pub mod token_sweep;
