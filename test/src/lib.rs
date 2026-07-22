//! Fixture-first tests for Solana programs built with Quasar.
//!
//! [`quasar_test`] turns an ordinary Rust test into an isolated [`Test`]
//! world loaded with the current program. [`fixture`] provides composable
//! account setup, while [`Outcome`] keeps execution assertions structured and
//! independent of the SVM that ran the transaction.
//!
//! ```rust,ignore
//! use quasar_test::{fixture::Wallet, prelude::*};
//!
//! #[quasar_test]
//! fn initializes(test: &mut Test) {
//!     let authority = test.add(Wallet::new());
//!     test.send(InitializeInstruction { authority }).succeeds();
//! }
//! ```

#![warn(missing_docs)]

mod backend;
pub mod fixture;
mod fixtures;
mod outcome;
mod setup;
mod types;
mod world;

pub use {
    outcome::Outcome,
    quasar_svm::{
        system_program, AccountMeta, Instruction, Pubkey, SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID,
    },
    quasar_test_derive::quasar_test,
    setup::{SetupError, TestBuilder, PROGRAM_PATH_ENV},
    types::{Account, AccountChange, ProgramError},
    world::{Snapshot, Test, DEFAULT_WALLET_LAMPORTS},
};

/// Imports used by most program tests.
pub mod prelude {
    pub use crate::{
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
    use {
        super::{
            backend::Backend,
            fixture::{AssociatedTokenAccount, Fixture, Mint, TokenAccount, TokenProgram, Wallet},
            setup::resolve_program_path_from_named,
            Account, ProgramError, Pubkey, SetupError, Test, SPL_TOKEN_2022_PROGRAM_ID,
        },
        std::{fs, path::PathBuf},
    };

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "quasar-test-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    fn empty_test() -> Test {
        Test {
            backend: Backend::new(),
            program_id: Pubkey::new_from_array([42; 32]),
            program_path: PathBuf::new(),
            fresh_addresses: 0,
        }
    }

    #[test]
    fn resolves_the_only_deployed_program() {
        let root = temp_dir("resolve");
        let nested = root.join("program/tests");
        let deploy = root.join("target/deploy");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&deploy).unwrap();
        fs::write(deploy.join("example.so"), b"elf").unwrap();

        assert_eq!(
            resolve_program_path_from_named(&nested, None).unwrap(),
            deploy.join("example.so")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_the_named_crate_among_many_programs() {
        let root = temp_dir("named");
        let deploy = root.join("target/deploy");
        fs::create_dir_all(&deploy).unwrap();
        fs::write(deploy.join("my_program.so"), b"elf").unwrap();
        fs::write(deploy.join("other_program.so"), b"elf").unwrap();

        assert_eq!(
            resolve_program_path_from_named(&root, Some("my-program")).unwrap(),
            deploy.join("my_program.so")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_ambiguous_deploy_directories() {
        let root = temp_dir("ambiguous");
        let deploy = root.join("target/deploy");
        fs::create_dir_all(&deploy).unwrap();
        fs::write(deploy.join("one.so"), b"elf").unwrap();
        fs::write(deploy.join("two.so"), b"elf").unwrap();

        assert!(matches!(
            resolve_program_path_from_named(&root, None),
            Err(SetupError::AmbiguousPrograms { .. })
        ));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixtures_are_deterministic_and_compose() {
        let mut test = empty_test();
        let wallet = test.add(Wallet::new().lamports(42));
        let mint = test.add(
            Mint::new(wallet)
                .supply(1_000)
                .decimals(9)
                .token_program(TokenProgram::Token2022),
        );
        let tokens = test.add(
            TokenAccount::new(mint, wallet)
                .amount(600)
                .token_program(TokenProgram::Token2022),
        );
        let associated = test.add(
            AssociatedTokenAccount::new(mint, wallet)
                .amount(400)
                .token_program(TokenProgram::Token2022),
        );

        assert_eq!(test.lamports(wallet), 42);
        assert_eq!(test.supply(mint), 1_000);
        assert_eq!(test.tokens(tokens), 600);
        assert_eq!(test.tokens(associated), 400);
        assert_eq!(test.account(mint).unwrap().owner, SPL_TOKEN_2022_PROGRAM_ID);
        assert_eq!(
            associated,
            test.derive_ata(wallet, mint, TokenProgram::Token2022)
        );
    }

    #[test]
    fn arrays_install_repeated_fixtures_without_helper_methods() {
        let mut test = empty_test();
        let [alice, bob, carol] = test.add([Wallet::new().lamports(7); 3]);

        assert_eq!(test.lamports(alice), 7);
        assert_eq!(test.lamports(bob), 7);
        assert_eq!(test.lamports(carol), 7);
        assert_ne!(alice, bob);
        assert_ne!(bob, carol);
    }

    #[test]
    fn applications_can_define_protocol_fixtures() {
        struct ProtocolFixture;

        impl Fixture for ProtocolFixture {
            type Output = (Pubkey, Pubkey);

            fn install(self, test: &mut Test) -> Self::Output {
                let authority = test.add(Wallet::new());
                let state = test.fresh_address();
                test.add(Account::new(state, test.program_id(), 1, vec![1, 2, 3]));
                (authority, state)
            }
        }

        let mut test = empty_test();
        let (authority, state) = test.add(ProtocolFixture);
        assert_ne!(authority, state);
        assert_eq!(test.account(state).unwrap().data, [1, 2, 3]);
    }

    #[test]
    fn stable_program_errors_do_not_expose_the_backend_type() {
        let error = ProgramError::from(quasar_svm::ProgramError::InvalidInstructionData);
        assert_eq!(error, ProgramError::InvalidInstructionData);
    }
}
