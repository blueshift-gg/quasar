#![no_std]
extern crate self as quasar_core;

#[doc(hidden)]
pub mod __private {
    pub use solana_account_view::{RuntimeAccount, AccountView, NOT_BORROWED, MAX_PERMITTED_DATA_INCREASE};
}

#[macro_use]
pub mod macros;
#[macro_use]
pub mod sysvars;
pub mod cpi;
pub mod pda;
pub mod entrypoint;
pub mod traits;
pub mod checks;
pub mod pod;
pub mod accounts;
pub mod context;
pub mod error;
pub mod log;
pub mod event;
pub mod prelude;
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub mod client;
