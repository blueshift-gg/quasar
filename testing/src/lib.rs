//! Project-aware testing utilities for Quasar programs.
//!
//! [`QuasarTest`] loads the program artifact produced by `quasar build` and
//! delegates execution to [`quasar_svm::QuasarSvm`]. Its world-building methods
//! cover the account setup repeated by most program tests, while
//! [`ExecutionResultExt`] makes outcomes readable as protocol expectations.

pub use quasar_svm::{Account, Pubkey, QuasarSvm, QuasarSvmConfig};
use std::{
    env,
    error::Error,
    fmt, fs,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

/// Environment variable set by `quasar test` to the freshly built program.
pub const PROGRAM_PATH_ENV: &str = "QUASAR_PROGRAM_PATH";

/// Default balance assigned by [`QuasarTest::actor`]: ten SOL.
pub const DEFAULT_ACTOR_LAMPORTS: u64 = 10_000_000_000;

/// A project-aware wrapper around [`QuasarSvm`].
///
/// Constructing the wrapper loads the current project's program. All
/// [`QuasarSvm`] methods remain available through `Deref`/`DerefMut`.
pub struct QuasarTest {
    svm: QuasarSvm,
    program_path: PathBuf,
}

impl QuasarTest {
    /// Load the current project's compiled program.
    ///
    /// `quasar test` supplies the exact artifact through
    /// [`PROGRAM_PATH_ENV`]. For direct `cargo test` runs, Quasar searches
    /// ancestor `target/deploy` directories and accepts the only `.so` there.
    ///
    /// # Panics
    ///
    /// Panics with an actionable message when no program artifact can be
    /// located or read. Use [`Self::try_new`] when setup errors should be
    /// handled explicitly.
    pub fn new(program_id: impl Into<Pubkey>) -> Self {
        Self::try_new(program_id).unwrap_or_else(|error| panic!("{error}"))
    }

    /// Load the current project's program with explicit SVM configuration.
    ///
    /// This is useful when a test wants to disable bundled SPL programs or
    /// otherwise control the base runtime. See [`QuasarSvmConfig`].
    pub fn new_with_config(program_id: impl Into<Pubkey>, config: QuasarSvmConfig) -> Self {
        Self::try_new_with_config(program_id, config).unwrap_or_else(|error| panic!("{error}"))
    }

    /// Fallible variant of [`Self::new`].
    pub fn try_new(program_id: impl Into<Pubkey>) -> Result<Self, ProjectError> {
        Self::try_new_with_config(program_id, QuasarSvmConfig::default())
    }

    /// Fallible variant of [`Self::new_with_config`].
    pub fn try_new_with_config(
        program_id: impl Into<Pubkey>,
        config: QuasarSvmConfig,
    ) -> Result<Self, ProjectError> {
        let path = resolve_program_path()?;
        Self::from_program_path_with_config(program_id, path, config)
    }

    /// Load a program from an explicit artifact path.
    pub fn from_program_path(
        program_id: impl Into<Pubkey>,
        path: impl AsRef<Path>,
    ) -> Result<Self, ProjectError> {
        Self::from_program_path_with_config(program_id, path, QuasarSvmConfig::default())
    }

    /// Load a program from an explicit artifact path and SVM configuration.
    pub fn from_program_path_with_config(
        program_id: impl Into<Pubkey>,
        path: impl AsRef<Path>,
        config: QuasarSvmConfig,
    ) -> Result<Self, ProjectError> {
        let path = path.as_ref().to_path_buf();
        let elf = fs::read(&path).map_err(|source| ProjectError::ReadProgram {
            path: path.clone(),
            source,
        })?;
        let program_id = program_id.into();
        let svm = QuasarSvm::new_with_config(config).with_program(&program_id, &elf);
        Ok(Self {
            svm,
            program_path: path,
        })
    }

    /// The ELF artifact loaded for this test runtime.
    pub fn program_path(&self) -> &Path {
        &self.program_path
    }

    /// Consume the project wrapper and return the underlying runtime.
    pub fn into_inner(self) -> QuasarSvm {
        self.svm
    }

    /// Load an additional CPI target while retaining the project wrapper.
    pub fn with_program(self, program_id: &Pubkey, elf: &[u8]) -> Self {
        self.svm
            .add_program(program_id, &quasar_svm::loader_keys::LOADER_V3, elf);
        self
    }

    /// Load an additional CPI target with an explicit loader.
    pub fn with_program_loader(self, program_id: &Pubkey, loader: &Pubkey, elf: &[u8]) -> Self {
        self.svm.add_program(program_id, loader, elf);
        self
    }

    /// Pre-populate an account while retaining the project wrapper.
    pub fn with_account(mut self, account: Account) -> Self {
        self.svm.set_account(account);
        self
    }

    /// Set the clock slot while retaining the project wrapper.
    pub fn with_slot(mut self, slot: u64) -> Self {
        self.svm.sysvars.warp_to_slot(slot);
        self
    }

    /// Set the transaction compute budget while retaining the project wrapper.
    pub fn with_compute_budget(mut self, max_units: u64) -> Self {
        self.svm.compute_budget.compute_unit_limit = max_units;
        self
    }

    /// Airdrop lamports while retaining the project wrapper.
    pub fn with_airdrop(mut self, address: &Pubkey, lamports: u64) -> Self {
        self.svm.airdrop(address, lamports);
        self
    }

    /// Create a rent-exempt account while retaining the project wrapper.
    pub fn with_create_account(mut self, address: &Pubkey, space: usize, owner: &Pubkey) -> Self {
        self.svm.create_account(address, space, owner);
        self
    }

    /// Create a funded actor at a fresh address.
    ///
    /// The default balance is [`DEFAULT_ACTOR_LAMPORTS`]. Use
    /// [`Self::actor_with_lamports`] when the balance is part of the test.
    pub fn actor(&mut self) -> Pubkey {
        self.actor_with_lamports(DEFAULT_ACTOR_LAMPORTS)
    }

    /// Create several funded actors at fresh addresses.
    ///
    /// Array destructuring keeps named multi-party scenarios compact:
    /// `let [maker, taker] = q.actors();`.
    pub fn actors<const N: usize>(&mut self) -> [Pubkey; N] {
        std::array::from_fn(|_| self.actor())
    }

    /// Fund a chosen address with the default actor balance.
    ///
    /// Use this when deterministic addresses make a scenario easier to read.
    pub fn actor_at(&mut self, address: Pubkey) -> Pubkey {
        self.fund(address, DEFAULT_ACTOR_LAMPORTS)
    }

    /// Create a funded actor at a fresh address with an explicit balance.
    pub fn actor_with_lamports(&mut self, lamports: u64) -> Pubkey {
        let address = Pubkey::new_unique();
        self.fund(address, lamports)
    }

    /// Create or replace a system account and return its address.
    pub fn fund(&mut self, address: Pubkey, lamports: u64) -> Pubkey {
        self.svm
            .set_account(fixtures::system_account(address, lamports));
        address
    }

    /// Add an empty system account for an instruction that initializes it.
    pub fn empty(&mut self, address: Pubkey) -> Pubkey {
        self.svm.set_account(fixtures::empty_account(address));
        address
    }

    /// Create a six-decimal mint with zero supply at a fresh address.
    pub fn mint(&mut self, authority: Pubkey) -> Pubkey {
        self.mint_with_supply(authority, 0)
    }

    /// Create a six-decimal mint with an explicit supply at a fresh address.
    pub fn mint_with_supply(&mut self, authority: Pubkey, supply: u64) -> Pubkey {
        let address = Pubkey::new_unique();
        self.mint_at(address, authority, supply, 6)
    }

    /// Create a mint at an explicit address and return that address.
    pub fn mint_at(
        &mut self,
        address: Pubkey,
        authority: Pubkey,
        supply: u64,
        decimals: u8,
    ) -> Pubkey {
        self.svm.set_account(fixtures::mint_account_with_supply(
            address, authority, supply, decimals,
        ));
        address
    }

    /// Create an SPL Token account at a fresh address.
    pub fn token_account(&mut self, owner: Pubkey, mint: Pubkey, amount: u64) -> Pubkey {
        let address = Pubkey::new_unique();
        self.token_account_at(address, owner, mint, amount)
    }

    /// Create an SPL Token account at an explicit address.
    pub fn token_account_at(
        &mut self,
        address: Pubkey,
        owner: Pubkey,
        mint: Pubkey,
        amount: u64,
    ) -> Pubkey {
        self.svm
            .set_account(fixtures::token_account(address, mint, owner, amount));
        address
    }

    /// Create an associated token account and return its derived address.
    pub fn ata(&mut self, owner: Pubkey, mint: Pubkey, amount: u64) -> Pubkey {
        let account = fixtures::associated_token_account(owner, mint, amount);
        let address = account.address;
        self.svm.set_account(account);
        address
    }

    /// Execute and commit one instruction using the accounts in this world.
    pub fn send(
        &mut self,
        instruction: impl Into<quasar_svm::Instruction>,
    ) -> quasar_svm::ExecutionResult {
        let instruction = instruction.into();
        self.svm.process_instruction(&instruction, &[])
    }

    /// Execute and commit one instruction with extra one-shot accounts.
    ///
    /// Most tests should register fixtures on the world and use [`Self::send`].
    /// This escape hatch is useful when the supplied account itself is the
    /// subject of a test.
    pub fn send_with(
        &mut self,
        instruction: impl Into<quasar_svm::Instruction>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> quasar_svm::ExecutionResult {
        let instruction = instruction.into();
        let accounts = accounts.into_iter().collect::<Vec<_>>();
        self.svm.process_instruction(&instruction, &accounts)
    }

    /// Simulate one instruction without committing its account changes.
    pub fn simulate(
        &mut self,
        instruction: impl Into<quasar_svm::Instruction>,
    ) -> quasar_svm::ExecutionResult {
        let instruction = instruction.into();
        self.svm.simulate_instruction(&instruction, &[])
    }
}

impl Deref for QuasarTest {
    type Target = QuasarSvm;

    fn deref(&self) -> &Self::Target {
        &self.svm
    }
}

impl DerefMut for QuasarTest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.svm
    }
}

/// Resolve the compiled program path used by [`QuasarTest`].
pub fn resolve_program_path() -> Result<PathBuf, ProjectError> {
    if let Some(path) = env::var_os(PROGRAM_PATH_ENV) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(ProjectError::ConfiguredProgramMissing { path });
    }

    let current_dir = env::current_dir().map_err(ProjectError::CurrentDirectory)?;
    resolve_program_path_from(&current_dir)
}

fn resolve_program_path_from(start: &Path) -> Result<PathBuf, ProjectError> {
    let mut checked = Vec::new();
    for ancestor in start.ancestors() {
        let deploy = ancestor.join("target/deploy");
        checked.push(deploy.clone());
        let mut programs = match fs::read_dir(&deploy) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "so"))
                .collect::<Vec<_>>(),
            Err(_) => continue,
        };
        programs.sort();
        if programs.len() == 1 {
            return Ok(programs.remove(0));
        }
        if programs.len() > 1 {
            return Err(ProjectError::AmbiguousPrograms { deploy, programs });
        }
    }

    Err(ProjectError::ProgramNotFound {
        start: start.to_path_buf(),
        checked,
    })
}

/// Failure to locate or load the current project's compiled program.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProjectError {
    /// The path supplied by `quasar test` no longer exists.
    ConfiguredProgramMissing { path: PathBuf },
    /// The current working directory could not be read.
    CurrentDirectory(std::io::Error),
    /// No unambiguous program was found under an ancestor `target/deploy`.
    ProgramNotFound {
        start: PathBuf,
        checked: Vec<PathBuf>,
    },
    /// More than one program artifact exists in the closest deploy directory.
    AmbiguousPrograms {
        deploy: PathBuf,
        programs: Vec<PathBuf>,
    },
    /// The selected program artifact could not be read.
    ReadProgram {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfiguredProgramMissing { path } => write!(
                formatter,
                "{PROGRAM_PATH_ENV} points to missing program artifact {}; run `quasar test` \
                 without `--no-build`",
                path.display()
            ),
            Self::CurrentDirectory(source) => {
                write!(
                    formatter,
                    "could not resolve the current project directory: {source}"
                )
            }
            Self::ProgramNotFound { start, checked } => {
                write!(
                    formatter,
                    "could not find one compiled Quasar program from {}; run `quasar test` or set \
                     {PROGRAM_PATH_ENV}",
                    start.display()
                )?;
                if !checked.is_empty() {
                    write!(formatter, " (checked")?;
                    for path in checked {
                        write!(formatter, " {}", path.display())?;
                    }
                    write!(formatter, ")")?;
                }
                Ok(())
            }
            Self::AmbiguousPrograms { deploy, programs } => {
                write!(
                    formatter,
                    "found multiple program artifacts in {}; run `quasar test` or set \
                     {PROGRAM_PATH_ENV} to the intended artifact:",
                    deploy.display()
                )?;
                for path in programs {
                    write!(formatter, " {}", path.display())?;
                }
                Ok(())
            }
            Self::ReadProgram { path, source } => write!(
                formatter,
                "could not read program artifact {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for ProjectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentDirectory(source) | Self::ReadProgram { source, .. } => Some(source),
            Self::ConfiguredProgramMissing { .. }
            | Self::ProgramNotFound { .. }
            | Self::AmbiguousPrograms { .. } => None,
        }
    }
}

/// Common account fixtures for QuasarSVM tests.
pub mod fixtures {
    use {
        quasar_svm::{token, Account, Pubkey, Rent, SPL_TOKEN_PROGRAM_ID},
        spl_token::state::{Account as TokenAccount, AccountState, Mint},
    };

    /// Create a system-owned account with the supplied balance.
    pub fn system_account(address: Pubkey, lamports: u64) -> Account {
        token::create_keyed_system_account(&address, lamports)
    }

    /// Create an empty system-owned account, suitable for an init target.
    pub fn empty_account(address: Pubkey) -> Account {
        system_account(address, 0)
    }

    /// Create a rent-exempt program-owned account containing `data`.
    pub fn program_account(address: Pubkey, owner: Pubkey, data: Vec<u8>) -> Account {
        Account {
            address,
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner,
            executable: false,
        }
    }

    /// Create an initialized SPL Token mint with zero supply.
    pub fn mint_account(address: Pubkey, authority: Pubkey, decimals: u8) -> Account {
        mint_account_with_supply(address, authority, 0, decimals)
    }

    /// Create an initialized SPL Token mint with an explicit supply.
    pub fn mint_account_with_supply(
        address: Pubkey,
        authority: Pubkey,
        supply: u64,
        decimals: u8,
    ) -> Account {
        let mint = Mint {
            mint_authority: Some(authority).into(),
            supply,
            decimals,
            is_initialized: true,
            freeze_authority: None.into(),
        };
        token::create_keyed_mint_account(&address, &mint)
    }

    /// Create an initialized SPL Token account.
    pub fn token_account(address: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) -> Account {
        let token_account = TokenAccount {
            mint,
            owner,
            amount,
            state: AccountState::Initialized,
            ..TokenAccount::default()
        };
        token::create_keyed_token_account(&address, &token_account)
    }

    /// Create an initialized associated token account and derive its address.
    pub fn associated_token_account(wallet: Pubkey, mint: Pubkey, amount: u64) -> Account {
        token::create_keyed_associated_token_account_with_program(
            &wallet,
            &mint,
            amount,
            &SPL_TOKEN_PROGRAM_ID,
        )
    }
}

/// Assertions layered on [`quasar_svm::ExecutionResult`].
pub trait ExecutionResultExt {
    /// Assert success and keep the result available for chained expectations.
    fn succeeds(&self) -> &Self;

    /// Assert a typed custom error and keep the result available for chaining.
    fn fails_with<E>(&self, expected: E) -> &Self
    where
        E: Into<u32>;

    /// Assert a compute-unit ceiling and keep the result available for
    /// chaining.
    fn cu_below(&self, limit: u64) -> &Self;

    /// Assert a lamport balance and keep the result available for chaining.
    fn has_lamports(&self, address: Pubkey, expected: u64) -> &Self;

    /// Assert a token balance and keep the result available for chaining.
    fn has_tokens(&self, address: Pubkey, expected: u64) -> &Self;

    /// Assert a mint supply and keep the result available for chaining.
    fn has_supply(&self, address: Pubkey, expected: u64) -> &Self;

    /// Assert that an account has been fully closed.
    fn is_closed(&self, address: Pubkey) -> &Self;

    /// Assert a program-specific error using a generated client error enum.
    fn assert_custom_error<E>(&self, expected: E)
    where
        E: Into<u32>;

    /// Assert that execution consumed strictly fewer than `limit` units.
    fn assert_compute_units_below(&self, limit: u64);

    /// Return an account's resulting lamport balance.
    fn lamports(&self, address: &Pubkey) -> u64;

    /// Return an SPL Token account's resulting raw token balance.
    fn token_balance(&self, address: &Pubkey) -> u64;

    /// Return an SPL Token mint's resulting supply.
    fn mint_supply(&self, address: &Pubkey) -> u64;

    /// Assert an account's resulting lamport balance.
    fn assert_lamports(&self, address: &Pubkey, expected: u64);

    /// Assert an SPL Token account's resulting raw token balance.
    fn assert_token_balance(&self, address: &Pubkey, expected: u64);

    /// Assert an SPL Token mint's resulting supply.
    fn assert_mint_supply(&self, address: &Pubkey, expected: u64);
}

impl ExecutionResultExt for quasar_svm::ExecutionResult {
    fn succeeds(&self) -> &Self {
        self.assert_success();
        self
    }

    fn fails_with<E>(&self, expected: E) -> &Self
    where
        E: Into<u32>,
    {
        self.assert_custom_error(expected);
        self
    }

    fn cu_below(&self, limit: u64) -> &Self {
        self.assert_compute_units_below(limit);
        self
    }

    fn has_lamports(&self, address: Pubkey, expected: u64) -> &Self {
        self.assert_lamports(&address, expected);
        self
    }

    fn has_tokens(&self, address: Pubkey, expected: u64) -> &Self {
        self.assert_token_balance(&address, expected);
        self
    }

    fn has_supply(&self, address: Pubkey, expected: u64) -> &Self {
        self.assert_mint_supply(&address, expected);
        self
    }

    fn is_closed(&self, address: Pubkey) -> &Self {
        let account = result_account(self, &address);
        assert_eq!(
            account.lamports, 0,
            "expected {address} to be closed, but it still holds lamports"
        );
        assert!(
            account.data.is_empty(),
            "expected {address} to be closed, but it still has account data"
        );
        assert_eq!(
            account.owner,
            quasar_svm::system_program::ID,
            "expected {address} to be closed, but it is not system-owned"
        );
        self
    }

    fn assert_custom_error<E>(&self, expected: E)
    where
        E: Into<u32>,
    {
        self.assert_error(quasar_svm::ProgramError::Custom(expected.into()));
    }

    fn assert_compute_units_below(&self, limit: u64) {
        assert!(
            self.compute_units_consumed < limit,
            "expected fewer than {limit} compute units, consumed {}",
            self.compute_units_consumed
        );
    }

    fn assert_lamports(&self, address: &Pubkey, expected: u64) {
        assert_eq!(
            self.lamports(address),
            expected,
            "unexpected lamport balance for {address}"
        );
    }

    fn lamports(&self, address: &Pubkey) -> u64 {
        result_account(self, address).lamports
    }

    fn token_balance(&self, address: &Pubkey) -> u64 {
        use spl_token::{solana_program::program_pack::Pack, state::Account as TokenAccount};

        let account = result_account(self, address);
        let token_account = TokenAccount::unpack(&account.data).unwrap_or_else(|error| {
            panic!("could not decode {address} as an SPL Token account: {error}")
        });
        token_account.amount
    }

    fn mint_supply(&self, address: &Pubkey) -> u64 {
        use spl_token::{solana_program::program_pack::Pack, state::Mint};

        let account = result_account(self, address);
        let mint = Mint::unpack(&account.data).unwrap_or_else(|error| {
            panic!("could not decode {address} as an SPL Token mint: {error}")
        });
        mint.supply
    }

    fn assert_token_balance(&self, address: &Pubkey, expected: u64) {
        assert_eq!(
            self.token_balance(address),
            expected,
            "unexpected token balance for {address}"
        );
    }

    fn assert_mint_supply(&self, address: &Pubkey, expected: u64) {
        assert_eq!(
            self.mint_supply(address),
            expected,
            "unexpected mint supply for {address}"
        );
    }
}

fn result_account<'a>(result: &'a quasar_svm::ExecutionResult, address: &Pubkey) -> &'a Account {
    result
        .account(address)
        .unwrap_or_else(|| panic!("execution result does not contain account {address}"))
}

/// Convenient imports for Quasar program tests.
pub mod prelude {
    pub use {
        crate::{quasar_test, ExecutionResultExt, QuasarSvmConfig, QuasarTest},
        quasar_svm::{Account, AccountMeta, ExecutionResult, Instruction, ProgramError, Pubkey},
    };
}

pub use quasar_svm;

/// Define a program test with a freshly loaded [`QuasarTest`] world.
///
/// The program id defaults to `crate::ID`:
///
/// ```rust,ignore
/// quasar_test! {
///     fn rejects_zero_deposit(q) {
///         q.send(make_instruction(0)).fails_with(EscrowError::InvalidAmount);
///     }
/// }
/// ```
///
/// Use `program_id = expression` before the function for an external program.
#[allow(clippy::crate_in_macro_def)]
#[macro_export]
macro_rules! quasar_test {
    ($(#[$meta:meta])* fn $name:ident($world:ident) $body:block) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            let mut $world = $crate::QuasarTest::new(crate::ID);
            $body
        }
    };
    (program_id = $program_id:expr; $(#[$meta:meta])* fn $name:ident($world:ident) $body:block) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            let mut $world = $crate::QuasarTest::new($program_id);
            $body
        }
    };
}

#[cfg(test)]
mod tests {
    use {
        super::{fixtures, resolve_program_path_from, ExecutionResultExt, ProjectError},
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
            resolve_program_path_from(&nested).unwrap(),
            deploy.join("example.so")
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
            resolve_program_path_from(&root),
            Err(ProjectError::AmbiguousPrograms { .. })
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
