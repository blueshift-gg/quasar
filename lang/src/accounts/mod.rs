//! Account types for zero-copy Solana program access.
//!
//! Each type wraps an `AccountView` and provides typed, validated access
//! to on-chain account data: `Account<T>` for program-owned data accounts,
//! `Program<T>` for executable program accounts, `Sysvar<T>` for sysvar
//! accounts, and `UncheckedAccount` for unvalidated passthrough.
//!
//! Every wrapper is `#[repr(transparent)]` over `AccountView` and shares one
//! model: generated dispatch runs `AccountLoad::check` (after the header
//! signer/writable/executable flags) first, then reinterprets the validated
//! `&AccountView` as the wrapper by pointer read or cast. Later zero-copy
//! access trusts that validation already ran; the `StaticView` marker vouches
//! for the transparent layout that makes the reinterpretation sound.

/// Program-owned typed account wrappers.
pub mod account;
/// Fixed-size groups of parsed accounts.
pub mod array;
/// Program-interface wrappers.
pub mod interface;
/// Accounts accepted from one of several program owners.
pub mod interface_account;
/// Account layout migration wrappers.
pub mod migration;
/// Executable program account wrappers.
pub mod program;
/// Transaction signer account wrappers.
pub mod signer;
/// System-owned account wrappers.
pub mod system_account;
/// Sysvar account wrappers.
pub mod sysvar;
/// Unvalidated account wrappers.
pub mod unchecked;
/// Deferred account initialization wrappers.
pub mod uninit;

pub use {
    account::*, array::*, interface::*, interface_account::*, migration::*, program::*, signer::*,
    system_account::*, sysvar::*, unchecked::*, uninit::*,
};
