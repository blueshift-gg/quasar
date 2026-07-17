//! Project-aware testing utilities for Quasar programs.
//!
//! [`QuasarTest`] loads the program artifact produced by `quasar build` and
//! delegates execution to [`quasar_svm::QuasarSvm`]. Its world-building methods
//! cover the account setup repeated by most program tests, while
//! [`ExecutionResultExt`] makes outcomes readable as protocol expectations.

use {
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{Discriminator, Owner, SeedSlices},
    },
    std::{
        env,
        error::Error,
        fmt, fs,
        ops::{Deref, DerefMut},
        path::{Path, PathBuf},
    },
};
pub use {
    quasar_svm::{Account, Pubkey, QuasarSvm, QuasarSvmConfig},
    quasar_test_derive::quasar_test,
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
    program_id: Pubkey,
    program_path: PathBuf,
    fresh_addresses: u64,
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

    /// Load a crate's compiled program by name.
    ///
    /// Prefers `target/deploy/{crate_name}.so` (with `-` mapped to `_`) over
    /// the only-artifact rule, so tests resolve their own program in a
    /// workspace that builds several. `#[quasar_test]` calls this with
    /// `env!("CARGO_PKG_NAME")`.
    pub fn new_for_crate(program_id: impl Into<Pubkey>, crate_name: &str) -> Self {
        Self::try_new_for_crate(program_id, crate_name).unwrap_or_else(|error| panic!("{error}"))
    }

    /// Fallible variant of [`Self::new_for_crate`].
    pub fn try_new_for_crate(
        program_id: impl Into<Pubkey>,
        crate_name: &str,
    ) -> Result<Self, ProjectError> {
        let path = resolve_program_path_named(crate_name)?;
        Self::from_program_path_with_config(program_id, path, QuasarSvmConfig::default())
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
            program_id,
            program_path: path,
            fresh_addresses: 0,
        })
    }

    /// The program under test.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
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
        let address = self.fresh_address();
        self.fund(address, lamports)
    }

    /// A fresh address no earlier fixture in this world has used.
    ///
    /// Addresses derive from a per-world counter, not a process-global one,
    /// so a test sees the same addresses on every run regardless of which
    /// other tests exist or run first. Compute-unit records stay comparable
    /// without hand-pinned address constants.
    pub fn fresh_address(&mut self) -> Pubkey {
        self.fresh_addresses += 1;
        let mut bytes = *b"quasar-test/fresh-address\0\0\0\0\0\0\0";
        bytes[24..].copy_from_slice(&self.fresh_addresses.to_le_bytes());
        Pubkey::new_from_array(bytes)
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
        let address = self.fresh_address();
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
        let address = self.fresh_address();
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

    /// Derive the canonical PDA for a seed set under the program under test.
    ///
    /// The seed set comes from the account's own `#[seeds(...)]` definition,
    /// so the test derives addresses from the same source the program
    /// validates against:
    ///
    /// ```rust,ignore
    /// let vault = q.pda(Vault::seeds(&maker));
    /// ```
    pub fn pda(&self, seeds: impl SeedSlices) -> Pubkey {
        self.pda_with_bump(seeds).0
    }

    /// Derive the canonical PDA and bump for a seed set.
    pub fn pda_with_bump(&self, seeds: impl SeedSlices) -> (Pubkey, u8) {
        seeds.with_slices(|slices| Pubkey::find_program_address(slices, &self.program_id))
    }

    /// Read an account as its typed state.
    ///
    /// Runs the same checks the program applies when loading `T`: the account
    /// must exist, be owned by `T`'s program, carry `T`'s discriminator, and
    /// hold enough valid data. The snapshot derefs to the account's data
    /// layout, so assertions read in field terms:
    ///
    /// ```rust,ignore
    /// let state = q.read::<Vault>(vault);
    /// assert_eq!(state.authority, maker);
    /// assert_eq!(state.balance, 500);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics with the failed check when the account does not hold a valid
    /// `T`. Tests asserting on invalid accounts should use
    /// [`QuasarSvm::get_account`] directly.
    pub fn read<T>(&self, address: Pubkey) -> Snapshot<T>
    where
        T: Discriminator + Owner + Deref,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        let name = core::any::type_name::<T>();
        let account = self
            .svm
            .get_account(&address)
            .unwrap_or_else(|| panic!("read {name}: no account at {address}"));
        if account.owner != T::OWNER {
            panic!(
                "read {name}: account {address} is owned by {}, expected {}",
                account.owner,
                T::OWNER
            );
        }
        let disc = T::DISCRIMINATOR;
        let expected_len = disc.len() + core::mem::size_of::<T::Target>();
        if account.data.len() < expected_len {
            panic!(
                "read {name}: account {address} holds {} bytes, expected at least {expected_len}",
                account.data.len()
            );
        }
        if &account.data[..disc.len()] != disc {
            panic!(
                "read {name}: account {address} discriminator is {:?}, expected {disc:?}",
                &account.data[..disc.len()]
            );
        }
        // SAFETY: `T::Target` is `ZcElem` (`#[repr(C)]`, alignment 1), and the
        // length check above proves the value's bytes are in bounds.
        let state = unsafe { &*(account.data[disc.len()..].as_ptr() as *const T::Target) };
        if let Err(error) = <T::Target as ZcValidate>::validate_ref(state) {
            panic!("read {name}: account {address} holds invalid data: {error:?}");
        }
        Snapshot {
            address,
            lamports: account.lamports,
            state: *state,
        }
    }

    /// Register a rent-exempt program account holding `state`.
    ///
    /// The account gets `T`'s owner and discriminator, so a follow-up
    /// [`Self::read`] (or the program itself) accepts it:
    ///
    /// ```rust,ignore
    /// q.write::<Vault>(vault, VaultData { authority: maker, balance: 500.into(), bump });
    /// ```
    pub fn write<T>(&mut self, address: Pubkey, state: T::Target) -> Pubkey
    where
        T: Discriminator + Owner + Deref,
        T::Target: ZcElem + Copy,
    {
        let disc = T::DISCRIMINATOR;
        let size = core::mem::size_of::<T::Target>();
        let mut data = vec![0u8; disc.len() + size];
        data[..disc.len()].copy_from_slice(disc);
        // SAFETY: `T::Target` is `ZcElem` (`#[repr(C)]`, alignment 1, no
        // padding), so its `size` bytes are initialized and the destination
        // range was sized for them.
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&state as *const T::Target).cast::<u8>(),
                data[disc.len()..].as_mut_ptr(),
                size,
            );
        }
        self.svm
            .set_account(fixtures::program_account(address, T::OWNER, data));
        address
    }

    /// Execute and commit one instruction using the accounts in this world.
    ///
    /// Writable instruction accounts missing from the world are registered as
    /// empty system accounts first, so freshly initialized accounts survive
    /// into follow-up sends and [`Self::read`] without an explicit
    /// [`Self::empty`] call.
    pub fn send(
        &mut self,
        instruction: impl Into<quasar_svm::Instruction>,
    ) -> quasar_svm::ExecutionResult {
        let instruction = instruction.into();
        self.register_writable_accounts(&instruction);
        self.svm.process_instruction(&instruction, &[])
    }

    /// Execute and commit one instruction with extra accounts.
    ///
    /// Most tests should register fixtures on the world and use [`Self::send`].
    /// This escape hatch is useful when the supplied account itself is the
    /// subject of a test. Successful execution commits those accounts to the
    /// world like every other transaction account.
    pub fn send_with(
        &mut self,
        instruction: impl Into<quasar_svm::Instruction>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> quasar_svm::ExecutionResult {
        let instruction = instruction.into();
        self.register_writable_accounts(&instruction);
        let accounts = accounts.into_iter().collect::<Vec<_>>();
        self.svm.process_instruction(&instruction, &accounts)
    }

    /// Back missing writable instruction accounts with empty system accounts.
    ///
    /// The SVM only commits accounts that existed before the transaction, so
    /// an unregistered init target would execute correctly and then vanish
    /// from the world.
    fn register_writable_accounts(&mut self, instruction: &quasar_svm::Instruction) {
        for meta in &instruction.accounts {
            if meta.is_writable && self.svm.get_account(&meta.pubkey).is_none() {
                self.svm.set_account(fixtures::empty_account(meta.pubkey));
            }
        }
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

/// Typed account state captured by [`QuasarTest::read`].
///
/// Derefs to the account's data layout (`{Name}Data`), so fields read
/// directly: `snapshot.authority`, `snapshot.value`.
pub struct Snapshot<T: Deref>
where
    T::Target: Sized + Copy,
{
    address: Pubkey,
    lamports: u64,
    state: T::Target,
}

impl<T: Deref> Snapshot<T>
where
    T::Target: Sized + Copy,
{
    /// The account's address.
    pub fn address(&self) -> Pubkey {
        self.address
    }

    /// The account's lamport balance at read time.
    pub fn lamports(&self) -> u64 {
        self.lamports
    }
}

impl<T: Deref> Deref for Snapshot<T>
where
    T::Target: Sized + Copy,
{
    type Target = T::Target;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

/// Resolve the compiled program path used by [`QuasarTest`].
pub fn resolve_program_path() -> Result<PathBuf, ProjectError> {
    if let Some(path) = configured_program_path()? {
        return Ok(path);
    }
    let current_dir = env::current_dir().map_err(ProjectError::CurrentDirectory)?;
    resolve_program_path_from(&current_dir)
}

/// Resolve a crate's compiled program path by name.
pub fn resolve_program_path_named(crate_name: &str) -> Result<PathBuf, ProjectError> {
    if let Some(path) = configured_program_path()? {
        return Ok(path);
    }
    let current_dir = env::current_dir().map_err(ProjectError::CurrentDirectory)?;
    resolve_program_path_from_named(&current_dir, Some(crate_name))
}

fn configured_program_path() -> Result<Option<PathBuf>, ProjectError> {
    let Some(path) = env::var_os(PROGRAM_PATH_ENV) else {
        return Ok(None);
    };
    let path = PathBuf::from(path);
    if path.is_file() {
        return Ok(Some(path));
    }
    Err(ProjectError::ConfiguredProgramMissing { path })
}

fn resolve_program_path_from(start: &Path) -> Result<PathBuf, ProjectError> {
    resolve_program_path_from_named(start, None)
}

fn resolve_program_path_from_named(
    start: &Path,
    crate_name: Option<&str>,
) -> Result<PathBuf, ProjectError> {
    let artifact = crate_name.map(|name| format!("{}.so", name.replace('-', "_")));
    let mut checked = Vec::new();
    for ancestor in start.ancestors() {
        let deploy = ancestor.join("target/deploy");
        checked.push(deploy.clone());
        if let Some(ref artifact) = artifact {
            let path = deploy.join(artifact);
            if path.is_file() {
                return Ok(path);
            }
        }
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

/// Test-side adjustments to a built [`quasar_svm::Instruction`].
///
/// Generated client instructions encode the canonical call. These helpers
/// express a test's deliberate deviations from it by address, so the
/// deviation is visible where the test constructs the instruction.
pub trait InstructionExt: Sized {
    /// Mark the given accounts as signers.
    fn signed_by(self, signers: &[Pubkey]) -> quasar_svm::Instruction;

    /// Replace every meta for `from` with `to`, e.g. to hand an instruction
    /// the wrong program or a foreign account on purpose.
    fn swap_account(self, from: Pubkey, to: Pubkey) -> quasar_svm::Instruction;
}

impl<T: Into<quasar_svm::Instruction>> InstructionExt for T {
    fn signed_by(self, signers: &[Pubkey]) -> quasar_svm::Instruction {
        let mut instruction = self.into();
        for meta in &mut instruction.accounts {
            if signers.contains(&meta.pubkey) {
                meta.is_signer = true;
            }
        }
        instruction
    }

    fn swap_account(self, from: Pubkey, to: Pubkey) -> quasar_svm::Instruction {
        let mut instruction = self.into();
        for meta in &mut instruction.accounts {
            if meta.pubkey == from {
                meta.pubkey = to;
            }
        }
        instruction
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
        crate::{
            quasar_test, ExecutionResultExt, InstructionExt, QuasarSvmConfig, QuasarTest, Snapshot,
        },
        quasar_svm::{
            system_program, Account, AccountMeta, ExecutionResult, Instruction, ProgramError,
            Pubkey,
        },
    };
}

pub use quasar_svm;

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
    fn resolves_the_named_crate_among_many_programs() {
        let root = temp_dir("named");
        let deploy = root.join("target/deploy");
        fs::create_dir_all(&deploy).unwrap();
        fs::write(deploy.join("my_program.so"), b"elf").unwrap();
        fs::write(deploy.join("other_program.so"), b"elf").unwrap();

        assert_eq!(
            super::resolve_program_path_from_named(&root, Some("my-program")).unwrap(),
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
