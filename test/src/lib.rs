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
//!     let authority = test.add(Wallet::account());
//!     test.execute(InitializeInstruction { authority }).succeeds();
//! }
//! ```
//!
//! [`fixture::Wallet::account`] funds an actor with the default balance;
//! [`fixture::Wallet::fund`] sets an exact one. Any signer a transaction names
//! but never installs is auto-funded on execute, so co-signers cost nothing extra.
//!
//! ## Adapter over Parallax
//!
//! quasar-test is a thin adapter over the [`parallax_svm`] harness. The SVM
//! world, fixtures, `Outcome` reporting, and program discovery all live in
//! Parallax and are re-exported here unchanged. quasar-test adds the
//! Quasar-specific sugar on top: quasar-lang `SeedSlices` PDA derivation
//! ([`Test::derive_pda`]), the strict, discriminator- and owner-checked typed
//! state API ([`Test::read`]/[`Test::write`]/the strict [`State`] checks), the
//! `#[quasar_test]` attribute, and the `QUASAR_PROGRAM_PATH` bridge.

#![warn(missing_docs)]

pub mod fixture;
mod outcome;
mod world;

pub use {
    outcome::{Outcome, State, SucceededTransaction},
    quasar_test_derive::quasar_test,
    world::{Snapshot, Test, TestBuilder, PROGRAM_PATH_ENV},
};

// Re-exported unchanged from Parallax so existing imports resolve exactly as
// before: the account/error types, the instruction and address types, the
// check grammar (`CheckFn`, `bundle`, `Cu`, the failed-transaction witness),
// program discovery errors, the co-signer helper, and the SPL program
// constants.
pub use parallax_svm::{
    bundle, co_signers, system_program, Account, AccountChange, AccountMeta, CheckFn, Cu,
    DataExpected, Expected, ExpectedBytes, FailedTransaction, Instruction, IntoInstructions, Many,
    One, ProgramError, Pubkey, Raw, ReturnData, SetupError, Typed, DEFAULT_WALLET_LAMPORTS,
    SPL_ASSOCIATED_TOKEN_PROGRAM_ID, SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
};

/// Parallax's schema-only snapshot, distinct from quasar-test's strict
/// [`Snapshot`]; returned by the deref-reachable `(*test).read(..)`.
pub use parallax_svm::Snapshot as SchemaSnapshot;

/// Imports used by most program tests.
pub mod prelude {
    pub use crate::{
        bundle, co_signers,
        fixture::{
            AssociatedTokenAccount, Dump, Fixture, Load, Mint, Program, TokenAccount, TokenProgram,
            Wallet,
        },
        quasar_test, system_program, Account, AccountChange, AccountMeta, CheckFn, Cu, Expected,
        ExpectedBytes, FailedTransaction, Instruction, IntoInstructions, Outcome, ProgramError,
        Pubkey, ReturnData, Snapshot, State, SucceededTransaction, Test, DEFAULT_WALLET_LAMPORTS,
        SPL_ASSOCIATED_TOKEN_PROGRAM_ID, SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        co_signers,
        fixture::{Dump, Load, Wallet},
        AccountMeta, Instruction, Pubkey, Test,
    };

    /// Compile-level proof that the `Dump`/`Load` fixtures delegate through
    /// quasar-test's `Fixture` trait; network- and file-backed installs run in
    /// the example suites, not here.
    #[allow(dead_code)]
    fn dump_and_load_fixtures_delegate(test: &mut Test) {
        let [_pool, _oracle] = test.add(Dump::accounts([
            Pubkey::new_from_array([1; 32]),
            Pubkey::new_from_array([2; 32]),
        ]));
        let _accounts: Vec<Pubkey> = test.add(Load::accounts("fixtures/pool.dump"));
        let _program: Pubkey = test.add(Load::program("fixtures/amm.dump"));
        let _refreshed: Vec<Pubkey> = test.add(Dump::refresh_all());
    }

    // The delegated builder options end-to-end: a program-less world with a
    // configured RPC executes a bare system transfer against the built-in
    // programs.
    #[test]
    fn builder_delegations_build_a_program_less_world() {
        let mut test = Test::builder(Pubkey::new_from_array([42; 32]))
            .rpc("http://127.0.0.1:1")
            .no_program()
            .build()
            .expect("program-less world builds");

        let payer = test.add(Wallet::account());
        let recipient = Pubkey::new_from_array([7; 32]);
        let mut data = vec![2, 0, 0, 0];
        data.extend_from_slice(&1_000_000u64.to_le_bytes());

        test.execute(Instruction {
            program_id: crate::system_program::ID,
            accounts: vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(recipient, false),
            ],
            data,
        })
        .succeeds()
        .check(crate::Account::lamports(recipient, 1_000_000));
    }

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
