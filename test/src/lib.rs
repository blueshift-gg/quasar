//! Project-aware testing utilities for Quasar programs.
//!
//! [`QuasarTest`] loads the program artifact produced by `quasar build` and
//! delegates execution to [`quasar_svm::QuasarSvm`]. Its world-building methods
//! cover the account setup repeated by most program tests, while
//! [`ExecutionResultExt`] makes outcomes readable as protocol expectations.
//!
//! ```rust
//! use quasar_test::{fixtures, Pubkey};
//!
//! let address = Pubkey::new_unique();
//! let wallet = fixtures::system_account(address, 1_000_000);
//! assert_eq!(wallet.address, address);
//! assert_eq!(wallet.lamports, 1_000_000);
//! ```

mod assertions;
pub mod fixtures;
mod setup;
mod world;

pub use {
    assertions::{ExecutionResultExt, InstructionExt},
    quasar_svm::{Account, ProgramError, Pubkey, QuasarSvm, QuasarSvmConfig},
    quasar_test_derive::quasar_test,
    setup::{QuasarTestBuilder, SetupError, PROGRAM_PATH_ENV},
    world::{QuasarTest, Snapshot, DEFAULT_WALLET_LAMPORTS},
};

/// Convenient imports for Quasar program tests.
pub mod prelude {
    pub use {
        crate::{
            quasar_test, ExecutionResultExt, InstructionExt, QuasarSvmConfig, QuasarTest, Snapshot,
        },
        quasar_svm::{
            system_program, Account, AccountMeta, ExecutionResult, Instruction, ProgramError,
            Pubkey, SPL_ASSOCIATED_TOKEN_PROGRAM_ID, SPL_TOKEN_2022_PROGRAM_ID,
            SPL_TOKEN_PROGRAM_ID,
        },
    };
}

pub use quasar_svm;

#[cfg(test)]
mod tests {
    use {
        super::{fixtures, setup::resolve_program_path_from_named, ExecutionResultExt, SetupError},
        quasar_svm::{ExecutionResult, ExecutionTrace, InstructionError, Pubkey},
        std::{fs, path::PathBuf},
    };

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "quasar-test-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
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
    fn fixtures_have_expected_owners_and_balances() {
        let address = Pubkey::new_unique();
        let system = fixtures::system_account(address, 42);
        assert_eq!(system.address, address);
        assert_eq!(system.lamports, 42);
        assert_eq!(system.owner, quasar_svm::system_program::ID);

        let mint = fixtures::mint_account_with_supply(Pubkey::new_unique(), address, 42, 6);
        assert_eq!(mint.owner, quasar_svm::SPL_TOKEN_PROGRAM_ID);
        assert_eq!(
            u64::from_le_bytes(mint.data[36..44].try_into().unwrap()),
            42
        );
    }

    #[test]
    fn typed_error_and_compute_assertions_accept_generated_shape() {
        #[derive(Clone, Copy)]
        enum ExampleError {
            Failure = 6000,
        }
        impl From<ExampleError> for u32 {
            fn from(error: ExampleError) -> Self {
                error as u32
            }
        }

        let address = Pubkey::new_unique();
        let mint_address = Pubkey::new_unique();
        let token_address = Pubkey::new_unique();
        let closed_address = Pubkey::new_unique();
        let result = ExecutionResult {
            compute_units_consumed: 99,
            execution_time_us: 0,
            raw_result: Err(InstructionError::Custom(6000)),
            return_data: Vec::new(),
            accounts: vec![
                fixtures::system_account(address, 42),
                fixtures::mint_account_with_supply(mint_address, address, 55, 6),
                fixtures::token_account(token_address, mint_address, address, 89),
                fixtures::empty_account(closed_address),
            ],
            logs: Vec::new(),
            pre_balances: Vec::new(),
            post_balances: Vec::new(),
            pre_token_balances: Vec::new(),
            post_token_balances: Vec::new(),
            execution_trace: ExecutionTrace {
                instructions: Vec::new(),
            },
        };

        result
            .fails_with(ExampleError::Failure)
            .cu_below(100)
            .has_lamports(address, 42)
            .has_supply(mint_address, 55)
            .has_tokens(token_address, 89)
            .is_closed(closed_address);
    }
}
