//! Suite-local SVM compat layer.
//!
//! This module presents the exact call shape the SPL/raw test suite was
//! written against (`SuiteSvm::new().with_program(..)`,
//! `svm.process_instruction(&ix, &[Account { .. }])`, `ExecutionResult`,
//! `ProgramError`, and a `token` factory module) but is implemented entirely
//! on top of [`mollusk_svm::Mollusk`], which the rest of the suite already
//! uses. It exists so the instruction-level tests run on a single SVM without
//! depending on the external `quasar-svm` crate.
//!
//! Behavioral parity with the previous `quasar-svm` backend is preserved:
//!
//! * Both run the Agave 3.x program runtime with `FeatureSet::all_enabled()`
//!   and `ComputeBudget::new_with_defaults(true, true)`, so error codes, logs,
//!   and compute-unit counts are identical.
//! * `SuiteSvm` keeps its own account store. `process_instruction` merges the
//!   caller-provided accounts over the stored ones (explicit wins), executes,
//!   and — only on success — commits the resulting accounts back into the
//!   store. This mirrors the previous backend, so tests that seed state with
//!   one instruction and then run another with a partial (or empty) account
//!   list continue to work.
//! * The bundled SPL Token, Token-2022, and Associated-Token programs are
//!   loaded by default with the same loaders the previous backend used.

use {
    mollusk_svm::{
        program::{
            create_program_account_loader_v2, create_program_account_loader_v3,
            keyed_account_for_system_program, loader_keys,
        },
        Mollusk,
    },
    solana_account::Account as SolanaAccount,
    solana_svm_log_collector::LogCollector,
    std::collections::{HashMap, HashSet},
};

pub use {
    solana_address::Address as Pubkey,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_sdk_ids::system_program,
};

pub mod token;

// ---------------------------------------------------------------------------
// Bundled SPL program IDs (identical to the previous backend).
// ---------------------------------------------------------------------------

pub const SPL_TOKEN_PROGRAM_ID: Pubkey =
    solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub const SPL_TOKEN_2022_PROGRAM_ID: Pubkey =
    solana_address::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

pub const SPL_ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    solana_address::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

const SPL_TOKEN_ELF: &[u8] = include_bytes!("../programs/spl_token.so");
const SPL_TOKEN_2022_ELF: &[u8] = include_bytes!("../programs/spl_token_2022.so");
const SPL_ASSOCIATED_TOKEN_ELF: &[u8] = include_bytes!("../programs/spl_associated_token.so");

// ---------------------------------------------------------------------------
// Account
// ---------------------------------------------------------------------------

/// A keyed account: an address paired with its on-chain state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub address: Pubkey,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
}

impl Account {
    pub fn from_pair(address: Pubkey, account: SolanaAccount) -> Self {
        Self {
            address,
            lamports: account.lamports,
            data: account.data,
            owner: account.owner,
            executable: account.executable,
        }
    }

    pub fn to_pair(&self) -> (Pubkey, SolanaAccount) {
        (
            self.address,
            SolanaAccount {
                lamports: self.lamports,
                data: self.data.clone(),
                owner: self.owner,
                executable: self.executable,
                rent_epoch: 0,
            },
        )
    }
}

// ---------------------------------------------------------------------------
// ProgramError
// ---------------------------------------------------------------------------

/// Test-facing program error, mapped from the runtime's `InstructionError`.
///
/// Mirrors the previous backend's enum so error-oracle assertions read the
/// same. `Runtime` carries the debug string for `InstructionError` variants
/// that have no dedicated mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    InvalidArgument,
    InvalidInstructionData,
    InvalidAccountData,
    AccountDataTooSmall,
    InsufficientFunds,
    IncorrectProgramId,
    MissingRequiredSignature,
    AccountAlreadyInitialized,
    UninitializedAccount,
    MissingAccount,
    InvalidSeeds,
    ArithmeticOverflow,
    AccountNotRentExempt,
    InvalidAccountOwner,
    IncorrectAuthority,
    Immutable,
    BorshIoError,
    ComputeBudgetExceeded,
    Custom(u32),
    Runtime(String),
}

impl From<solana_instruction::error::InstructionError> for ProgramError {
    fn from(err: solana_instruction::error::InstructionError) -> Self {
        use solana_instruction::error::InstructionError as E;
        #[allow(deprecated)]
        match err {
            E::InvalidArgument => Self::InvalidArgument,
            E::InvalidInstructionData => Self::InvalidInstructionData,
            E::InvalidAccountData => Self::InvalidAccountData,
            E::AccountDataTooSmall => Self::AccountDataTooSmall,
            E::InsufficientFunds => Self::InsufficientFunds,
            E::IncorrectProgramId => Self::IncorrectProgramId,
            E::MissingRequiredSignature => Self::MissingRequiredSignature,
            E::AccountAlreadyInitialized => Self::AccountAlreadyInitialized,
            E::UninitializedAccount => Self::UninitializedAccount,
            E::MissingAccount | E::NotEnoughAccountKeys => Self::MissingAccount,
            E::InvalidSeeds => Self::InvalidSeeds,
            E::ArithmeticOverflow => Self::ArithmeticOverflow,
            E::AccountNotRentExempt => Self::AccountNotRentExempt,
            E::InvalidAccountOwner => Self::InvalidAccountOwner,
            E::IncorrectAuthority => Self::IncorrectAuthority,
            E::Immutable => Self::Immutable,
            E::BorshIoError => Self::BorshIoError,
            E::ComputationalBudgetExceeded => Self::ComputeBudgetExceeded,
            E::Custom(code) => Self::Custom(code),
            other => Self::Runtime(format!("{other:?}")),
        }
    }
}

impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument => write!(f, "invalid argument"),
            Self::InvalidInstructionData => write!(f, "invalid instruction data"),
            Self::InvalidAccountData => write!(f, "invalid account data"),
            Self::AccountDataTooSmall => write!(f, "account data too small"),
            Self::InsufficientFunds => write!(f, "insufficient funds"),
            Self::IncorrectProgramId => write!(f, "incorrect program id"),
            Self::MissingRequiredSignature => write!(f, "missing required signature"),
            Self::AccountAlreadyInitialized => write!(f, "account already initialized"),
            Self::UninitializedAccount => write!(f, "uninitialized account"),
            Self::MissingAccount => write!(f, "missing account"),
            Self::InvalidSeeds => write!(f, "invalid seeds"),
            Self::ArithmeticOverflow => write!(f, "arithmetic overflow"),
            Self::AccountNotRentExempt => write!(f, "account not rent-exempt"),
            Self::InvalidAccountOwner => write!(f, "invalid account owner"),
            Self::IncorrectAuthority => write!(f, "incorrect authority"),
            Self::Immutable => write!(f, "account is immutable"),
            Self::BorshIoError => write!(f, "borsh serialization error"),
            Self::ComputeBudgetExceeded => write!(f, "compute budget exceeded"),
            Self::Custom(code) => write!(f, "custom program error: {code} ({code:#x})"),
            Self::Runtime(msg) => write!(f, "runtime error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionResult
// ---------------------------------------------------------------------------

/// The outcome of executing an instruction, exposing the same surface the
/// suite asserts against.
pub struct ExecutionResult {
    pub raw_result: Result<(), solana_instruction::error::InstructionError>,
    pub accounts: Vec<Account>,
    pub logs: Vec<String>,
}

impl ExecutionResult {
    pub fn is_ok(&self) -> bool {
        self.raw_result.is_ok()
    }

    pub fn is_err(&self) -> bool {
        self.raw_result.is_err()
    }

    /// Look up a resulting account by address.
    pub fn account(&self, address: &Pubkey) -> Option<&Account> {
        self.accounts.iter().find(|a| a.address == *address)
    }

    /// Panic if execution did not succeed.
    pub fn assert_success(&self) {
        if let Err(ref e) = self.raw_result {
            panic!("expected success, got: {}", self.format_error(e));
        }
    }

    /// Panic if execution did not fail with the expected error.
    pub fn assert_error(&self, expected: ProgramError) {
        match &self.raw_result {
            Ok(()) => panic!("expected error {expected:?}, but execution succeeded"),
            Err(e) => {
                let actual = ProgramError::from(e.clone());
                assert_eq!(
                    actual, expected,
                    "expected error {expected:?}, got {actual:?}"
                );
            }
        }
    }

    fn format_error(&self, e: &solana_instruction::error::InstructionError) -> String {
        let err = ProgramError::from(e.clone());
        if self.logs.is_empty() {
            format!("{err}")
        } else {
            format!(
                "{err}\n\nProgram logs:\n{}",
                self.logs
                    .iter()
                    .map(|l| format!("  {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    }
}

// ---------------------------------------------------------------------------
// SuiteSvm
// ---------------------------------------------------------------------------

/// A minimal, stateful SVM wrapper over [`Mollusk`] with a persistent account
/// store and merge/commit semantics matching the previous backend.
pub struct SuiteSvm {
    mollusk: Mollusk,
    accounts: HashMap<Pubkey, SolanaAccount>,
}

impl Default for SuiteSvm {
    fn default() -> Self {
        Self::new()
    }
}

impl SuiteSvm {
    /// Create a new instance with the bundled SPL programs loaded.
    pub fn new() -> Self {
        let mut svm = Self {
            mollusk: Mollusk::default(),
            accounts: HashMap::new(),
        };
        // The System program is a builtin referenced as a CPI target (account
        // meta) by init/realloc/close tests; seed it so the merge supplies it.
        let (system_id, system_account) = keyed_account_for_system_program();
        svm.accounts.insert(system_id, system_account);
        svm.load_program(
            &SPL_TOKEN_PROGRAM_ID,
            &loader_keys::LOADER_V2,
            SPL_TOKEN_ELF,
        );
        svm.load_program(
            &SPL_TOKEN_2022_PROGRAM_ID,
            &loader_keys::LOADER_V3,
            SPL_TOKEN_2022_ELF,
        );
        svm.load_program(
            &SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
            &loader_keys::LOADER_V2,
            SPL_ASSOCIATED_TOKEN_ELF,
        );
        svm
    }

    /// Load a BPF program from an ELF byte slice (loader v3 / upgradeable).
    pub fn with_program(mut self, program_id: &Pubkey, elf: &[u8]) -> Self {
        self.load_program(program_id, &loader_keys::LOADER_V3, elf);
        self
    }

    /// Register a program in the runtime cache and seed a matching program
    /// account into the store.
    ///
    /// Mollusk only auto-provides an account for the instruction's *top-level*
    /// program id; a program referenced through an account meta (e.g. the SPL
    /// Token program handed to a validate instruction) must be present in the
    /// account list. Seeding the store here — as the previous backend did from
    /// its program cache — makes `process_instruction`'s merge supply it. The
    /// program data account is intentionally omitted: execution runs from the
    /// cache, so (matching the previous backend) it never enters the tx.
    fn load_program(&mut self, program_id: &Pubkey, loader: &Pubkey, elf: &[u8]) {
        self.mollusk
            .add_program_with_loader_and_elf(program_id, loader, elf);
        let account = if *loader == loader_keys::LOADER_V3 {
            create_program_account_loader_v3(program_id)
        } else {
            create_program_account_loader_v2(&[])
        };
        self.accounts.insert(*program_id, account);
    }

    /// Read an account from the store.
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        self.accounts
            .get(pubkey)
            .map(|a| Account::from_pair(*pubkey, a.clone()))
    }

    /// Execute a single instruction atomically.
    ///
    /// Caller-provided accounts are merged over the stored accounts (explicit
    /// wins). On success, the resulting accounts are committed back into the
    /// store.
    pub fn process_instruction(
        &mut self,
        instruction: &Instruction,
        accounts: &[Account],
    ) -> ExecutionResult {
        // Fresh log collector per call so `.logs` reflects only this execution.
        let logger = LogCollector::new_ref();
        self.mollusk.logger = Some(logger.clone());

        // Merge stored accounts with the explicit ones; explicit take priority.
        let explicit: HashSet<Pubkey> = accounts.iter().map(|a| a.address).collect();
        let mut merged: Vec<(Pubkey, SolanaAccount)> = self
            .accounts
            .iter()
            .filter(|(k, _)| !explicit.contains(k))
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        merged.extend(accounts.iter().map(|a| a.to_pair()));

        let result = self.mollusk.process_instruction(instruction, &merged);

        // Commit resulting accounts only on success (matches prior backend).
        if result.raw_result.is_ok() {
            for (pubkey, account) in &result.resulting_accounts {
                self.accounts.insert(*pubkey, account.clone());
            }
        }

        let logs = logger.borrow().get_recorded_content().to_vec();
        self.mollusk.logger = None;

        let accounts = result
            .resulting_accounts
            .into_iter()
            .map(|(k, v)| Account::from_pair(k, v))
            .collect();

        ExecutionResult {
            raw_result: result.raw_result,
            accounts,
            logs,
        }
    }
}
