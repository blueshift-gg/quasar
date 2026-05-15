//! Account types for zero-copy Solana program access.
//!
//! Each type wraps an `AccountView` and provides typed, validated access
//! to on-chain account data: `Account<T>` for program-owned data accounts,
//! `Program<T>` for executable program accounts, `Sysvar<T>` for sysvar
//! accounts, and `UncheckedAccount` for unvalidated passthrough.

pub mod account;
pub mod array;
pub mod interface;
pub mod interface_account;
pub mod migration;
pub mod program;
pub mod signer;
pub mod system_account;
pub mod sysvar;
pub mod unchecked;
pub mod uninit;

pub use {
    account::*, array::*, interface::*, interface_account::*, migration::*, program::*, signer::*,
    system_account::*, sysvar::*, unchecked::*, uninit::*,
};
