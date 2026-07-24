//! Fixture-first tests for Solana programs built with Quasar.
//!
//! [`quasar_test`] turns an ordinary Rust test into an isolated [`Test`] world
//! loaded with the current program. [`fixture`] provides composable account
//! setup, while [`Outcome`] keeps execution assertions structured and
//! independent of the SVM that ran the transaction.
//!
//! ```rust,ignore
//! use quasar_test::prelude::*;
//!
//! #[quasar_test]
//! fn initializes(test: &mut Test) {
//!     let authority = test.add(Wallet::new());
//!     test.send(InitializeInstruction { authority }).succeeds();
//! }
//! ```
//!
//! [`fixture::Wallet::new`] funds an actor with the default balance;
//! [`fixture::Wallet::fund`] sets an exact one. Any signer a transaction names
//! but never installs is auto-funded on send, so co-signers cost nothing extra.
//!
//! ## Adapter over Parallax
//!
//! quasar-test is a thin adapter over the [`parallax_svm`] harness. The SVM
//! world, fixtures, `Outcome` reporting, and program discovery all live in
//! Parallax and are re-exported here unchanged. quasar-test adds the
//! Quasar-specific sugar on top: quasar-lang `SeedSlices` PDA derivation
//! ([`Test::derive_pda`]), the strict, discriminator- and owner-checked typed
//! state API ([`Test::read`]/[`Test::write`]/[`Outcome::has_state`]), the
//! `#[quasar_test]` attribute, and the `QUASAR_PROGRAM_PATH` bridge.

#![warn(missing_docs)]

pub mod fixture;
mod outcome;
mod world;

pub use {
    outcome::Outcome,
    quasar_test_derive::quasar_test,
    world::{Snapshot, Test, TestBuilder, PROGRAM_PATH_ENV},
};

// Re-exported unchanged from Parallax so existing imports resolve exactly as
// before: the account/error types, the instruction and address types, program
// discovery errors, the co-signer helper, and the SPL program constants.
pub use parallax_svm::{
    co_signers, system_program, Account, AccountChange, AccountMeta, Instruction, ProgramError,
    Pubkey, SetupError, DEFAULT_WALLET_LAMPORTS, SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
    SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
};

/// Imports used by most program tests.
pub mod prelude {
    pub use crate::{
        co_signers,
        fixture::{
            AssociatedTokenAccount, Fixture, Mint, Program, TokenAccount, TokenProgram, Wallet,
        },
        quasar_test, system_program, Account, AccountChange, AccountMeta, Instruction, Outcome,
        ProgramError, Pubkey, Snapshot, Test, DEFAULT_WALLET_LAMPORTS,
        SPL_ASSOCIATED_TOKEN_PROGRAM_ID, SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
    };
}

#[cfg(test)]
mod tests {
    use crate::{co_signers, Pubkey};

    #[test]
    fn co_signers_are_read_only_signer_metas() {
        let first = Pubkey::new_from_array([1; 32]);
        let second = Pubkey::new_from_array([2; 32]);

        let metas = co_signers(&[first, second]);

        assert_eq!(metas.len(), 2);
        for (meta, expected) in metas.iter().zip([first, second]) {
            assert_eq!(meta.pubkey, expected);
            assert!(meta.is_signer);
            assert!(!meta.is_writable);
        }
        assert!(co_signers(&[]).is_empty());
    }
}
