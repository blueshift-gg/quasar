#![no_std]
extern crate self as quasar_core;

#[doc(hidden)]
pub mod __private {
    pub use solana_account_view::{
        AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
    };
}

#[macro_use]
pub mod macros;
#[macro_use]
pub mod sysvars;
pub mod accounts;
pub mod checks;
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub mod client;
pub mod context;
pub mod cpi;
pub mod entrypoint;
pub mod error;
pub mod event;
pub mod log;
pub mod pda;
pub mod pod;
pub mod prelude;
pub mod remaining;
pub mod return_data;
pub mod traits;
