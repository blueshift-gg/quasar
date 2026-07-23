//! Fixture-first tests for Solana programs built with Quasar.
//!
//! [`quasar_test`] turns an ordinary Rust test into an isolated [`Test`]
//! world loaded with the current program. [`fixture`] provides composable
//! account setup, while [`Outcome`] keeps execution assertions structured and
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

/// Build read-only signer metas for co-signers, such as multisig members.
///
/// Each address becomes an [`AccountMeta`] that is read-only and a signer, the
/// shape a program expects for an authority it only needs to have signed.
/// [`Test::send`] auto-registers any co-signer the world has not installed as a
/// funded system account — as it does for every signer it backfills — so tests
/// pass the addresses alone without hand-rolling metas or wallets.
pub fn co_signers(addresses: &[Pubkey]) -> Vec<AccountMeta> {
    addresses
        .iter()
        .map(|&address| AccountMeta::new_readonly(address, true))
        .collect()
}

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
    use {
        super::{
            backend::Backend,
            co_signers,
            fixture::{AssociatedTokenAccount, Fixture, Mint, TokenAccount, TokenProgram, Wallet},
            setup::resolve_program_path_from_named,
            system_program, Account, AccountMeta, Instruction, ProgramError, Pubkey, SetupError,
            Test, DEFAULT_WALLET_LAMPORTS, SPL_TOKEN_2022_PROGRAM_ID,
        },
        spl_token::solana_program::program_option::COption,
        std::{fs, path::PathBuf},
    };

    /// Encode a System program `Transfer` (`SystemInstruction` variant 2).
    fn system_transfer(from: Pubkey, to: Pubkey, lamports: u64) -> Instruction {
        let mut data = vec![2, 0, 0, 0];
        data.extend_from_slice(&lamports.to_le_bytes());
        Instruction {
            program_id: system_program::ID,
            accounts: vec![AccountMeta::new(from, true), AccountMeta::new(to, false)],
            data,
        }
    }

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
        let wallet = test.add(Wallet::new().fund(42));
        let mint = test.add(
            Mint::new()
                .with_authority(wallet)
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
        let [alice, bob, carol] = test.add([Wallet::new().fund(7); 3]);

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
                let state = Pubkey::new_from_array([200; 32]);
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

    #[test]
    fn with_holder_installs_one_associated_account_per_holder() {
        let mut test = empty_test();
        let authority = test.add(Wallet::new());
        let alice = test.add(Wallet::new());
        let bob = test.add(Wallet::new());

        let mint = test.add(
            Mint::new()
                .with_authority(authority)
                .supply(1_000)
                .token_program(TokenProgram::Token2022)
                .with_holder([(alice, 400), (bob, 600)]),
        );

        let alice_ata = test.derive_ata(alice, mint, TokenProgram::Token2022);
        let bob_ata = test.derive_ata(bob, mint, TokenProgram::Token2022);
        assert_eq!(test.supply(mint), 1_000);
        assert_eq!(test.tokens(alice_ata), 400);
        assert_eq!(test.tokens(bob_ata), 600);
        assert_eq!(
            test.account(alice_ata).unwrap().owner,
            SPL_TOKEN_2022_PROGRAM_ID
        );
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

    #[test]
    fn mint_new_is_fixed_supply_without_authorities() {
        use spl_token::{solana_program::program_pack::Pack, state::Mint as SplMint};

        let mut test = empty_test();
        let mint = test.add(Mint::new().supply(1_000));
        let decoded = SplMint::unpack(&test.account(mint).unwrap().data).unwrap();

        assert_eq!(decoded.mint_authority, COption::None);
        assert_eq!(decoded.freeze_authority, COption::None);
        assert_eq!(decoded.supply, 1_000);
    }

    #[test]
    fn mint_authorities_come_from_the_builders() {
        use spl_token::{solana_program::program_pack::Pack, state::Mint as SplMint};

        let mut test = empty_test();
        let authority = Pubkey::new_from_array([1; 32]);
        let freeze = Pubkey::new_from_array([2; 32]);
        let mint = test.add(
            Mint::new()
                .with_authority(authority)
                .with_freeze_authority(freeze),
        );
        let decoded = SplMint::unpack(&test.account(mint).unwrap().data).unwrap();

        assert_eq!(decoded.mint_authority, COption::Some(authority));
        assert_eq!(decoded.freeze_authority, COption::Some(freeze));
    }

    // A missing writable account is an init target and enters empty — even
    // when it signs, as a keypair account being created does. Payers are
    // world state: a payer the test never installs has nothing to move.
    #[test]
    fn a_missing_writable_signer_is_an_empty_init_target_not_a_funded_payer() {
        let mut test = empty_test();
        // Raw addresses the world never installs, so send must backfill them.
        let payer = Pubkey::new_from_array([1; 32]);
        let recipient = Pubkey::new_from_array([2; 32]);
        let amount = 1_000_000_000;

        // The uninstalled payer enters empty, so the transfer must fail —
        // proof that writable-signer init targets are not silently funded.
        assert!(test
            .simulate(system_transfer(payer, recipient, amount))
            .is_err());

        // Installed as a wallet, the same transfer goes through.
        test.add(Wallet::new().at(payer));
        test.send(system_transfer(payer, recipient, amount))
            .succeeds()
            .has_lamports(recipient, amount);
    }

    // A read-only signer (a co-signer, e.g. a multisig member) is an actor
    // and enters funded, even though the world never installed it.
    #[test]
    fn a_read_only_signer_is_backfilled_funded() {
        let mut test = empty_test();
        let payer = Pubkey::new_from_array([1; 32]);
        let recipient = Pubkey::new_from_array([2; 32]);
        let cosigner = Pubkey::new_from_array([3; 32]);
        let amount = 1_000_000_000;
        test.add(Wallet::new().at(payer));

        let mut transfer = system_transfer(payer, recipient, amount);
        transfer
            .accounts
            .push(AccountMeta::new_readonly(cosigner, true));

        test.send(transfer)
            .succeeds()
            .has_lamports(cosigner, DEFAULT_WALLET_LAMPORTS);
    }
}
