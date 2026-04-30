//! Account types for zero-copy Solana program access.
//!
//! Each type wraps an `AccountView` and provides typed, validated access
//! to on-chain account data: `Account<T>` for program-owned data accounts,
//! `Program<T>` for executable program accounts, `Sysvar<T>` for sysvar
//! accounts, and `UncheckedAccount` for unvalidated passthrough.

pub mod unchecked;
pub use unchecked::*;
pub mod signer;
pub use signer::*;
pub mod system_account;
pub use system_account::*;
pub mod sysvar;
pub use sysvar::*;
pub mod account;
pub use account::*;
pub mod program;
pub use program::*;
pub mod interface;
pub use interface::*;
pub mod interface_account;
pub use interface_account::*;
pub mod migration;
pub use migration::*;
