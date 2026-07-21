use {
    crate::{fixtures, Account, Pubkey, QuasarSvm, QuasarSvmConfig, QuasarTestBuilder, SetupError},
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{AccountData, Discriminator, Owner, SeedSlices},
    },
    std::{
        ops::{Deref, DerefMut},
        path::{Path, PathBuf},
    },
};

/// Default balance assigned by [`QuasarTest::add_wallet`]: ten SOL.
pub const DEFAULT_WALLET_LAMPORTS: u64 = 10_000_000_000;

/// A project-aware wrapper around [`QuasarSvm`].
///
/// Constructing the wrapper loads the current project's program. All
/// [`QuasarSvm`] methods remain available through `Deref`/`DerefMut`.
pub struct QuasarTest {
    pub(super) svm: QuasarSvm,
    pub(super) program_id: Pubkey,
    pub(super) program_path: PathBuf,
    pub(super) fresh_addresses: u64,
}

impl QuasarTest {
    /// Load the current project's compiled program.
    ///
    /// `quasar test` supplies the exact artifact through
    /// [`crate::PROGRAM_PATH_ENV`]. For direct `cargo test` runs, Quasar prefers
    /// `target/deploy/{crate_name}.so` when a crate name is known (see
    /// [`QuasarTest::builder`]) and otherwise accepts the only `.so` in an
    /// ancestor `target/deploy` directory.
    ///
    /// # Panics
    ///
    /// Panics with an actionable message when no program artifact can be
    /// located or read. Use [`Self::try_new`] when setup errors should be
    /// handled explicitly.
    pub fn new(program_id: impl Into<Pubkey>) -> Self {
        Self::builder(program_id)
            .build()
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Fallible variant of [`Self::new`].
    pub fn try_new(program_id: impl Into<Pubkey>) -> Result<Self, SetupError> {
        Self::builder(program_id).build()
    }

    /// Customize world setup before loading the program.
    ///
    /// ```text
    /// let mut q = QuasarTest::builder(other_program::ID)
    ///     .crate_name("other-program")
    ///     .config(QuasarSvmConfig::default())
    ///     .build()?;
    /// ```
    pub fn builder(program_id: impl Into<Pubkey>) -> QuasarTestBuilder {
        QuasarTestBuilder {
            program_id: program_id.into(),
            config: QuasarSvmConfig::default(),
            program_path: None,
            crate_name: None,
        }
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

    /// Load an additional program (a CPI target) into the world.
    pub fn load_program(&mut self, program_id: &Pubkey, elf: &[u8]) {
        self.svm
            .add_program(program_id, &quasar_svm::loader_keys::LOADER_V3, elf);
    }

    /// Register a funded wallet at a fresh address.
    ///
    /// The default balance is [`DEFAULT_WALLET_LAMPORTS`]. Use
    /// [`Self::add_wallet_with_lamports`] when the balance is part of the
    /// test.
    pub fn add_wallet(&mut self) -> Pubkey {
        self.add_wallet_with_lamports(DEFAULT_WALLET_LAMPORTS)
    }

    /// Register several funded wallets at fresh addresses.
    ///
    /// Array destructuring keeps named multi-party scenarios compact:
    /// `let [maker, taker] = q.add_wallets();`.
    pub fn add_wallets<const N: usize>(&mut self) -> [Pubkey; N] {
        std::array::from_fn(|_| self.add_wallet())
    }

    /// Register a funded wallet at a fresh address with an explicit balance.
    pub fn add_wallet_with_lamports(&mut self, lamports: u64) -> Pubkey {
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

    /// Register a six-decimal mint with zero supply at a fresh address.
    pub fn add_mint(&mut self, authority: Pubkey) -> Pubkey {
        self.add_mint_with_supply(authority, 0)
    }

    /// Register a six-decimal mint with an explicit supply at a fresh
    /// address.
    pub fn add_mint_with_supply(&mut self, authority: Pubkey, supply: u64) -> Pubkey {
        let address = self.fresh_address();
        self.add_mint_at(address, authority, supply, 6)
    }

    /// Register a mint at an explicit address and return that address.
    pub fn add_mint_at(
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

    /// Register a token account at a fresh address.
    pub fn add_token_account(&mut self, owner: Pubkey, mint: Pubkey, amount: u64) -> Pubkey {
        let address = self.fresh_address();
        self.add_token_account_at(address, owner, mint, amount)
    }

    /// Register a token account at an explicit address.
    pub fn add_token_account_at(
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

    /// Derive an associated token address without registering an account.
    ///
    /// Use this for init targets and assertions: [`Self::add_ata`] would
    /// register an existing account, which is exactly what an init
    /// instruction must not find.
    pub fn derive_ata(&self, owner: Pubkey, mint: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[
                owner.as_ref(),
                quasar_svm::SPL_TOKEN_PROGRAM_ID.as_ref(),
                mint.as_ref(),
            ],
            &quasar_svm::SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .0
    }

    /// Register an associated token account and return its derived address.
    pub fn add_ata(&mut self, owner: Pubkey, mint: Pubkey, amount: u64) -> Pubkey {
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
    /// ```text
    /// let vault = q.derive_pda(Vault::seeds(&maker));
    /// ```
    pub fn derive_pda(&self, seeds: impl SeedSlices) -> Pubkey {
        self.derive_pda_with_bump(seeds).0
    }

    /// Derive the canonical PDA and bump for a seed set.
    pub fn derive_pda_with_bump(&self, seeds: impl SeedSlices) -> (Pubkey, u8) {
        seeds.with_slices(|slices| Pubkey::find_program_address(slices, &self.program_id))
    }

    /// Read an account as its typed state.
    ///
    /// Runs the same checks the program applies when loading `T`: the account
    /// must exist, be owned by `T`'s program, carry `T`'s discriminator, and
    /// hold enough valid data. The snapshot derefs to the account's data
    /// layout, so assertions read in field terms:
    ///
    /// ```text
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

    /// An account's current lamport balance in this world.
    pub fn lamports(&self, address: Pubkey) -> u64 {
        self.world_account(address).lamports
    }

    /// A token account's current balance in this world.
    pub fn tokens(&self, address: Pubkey) -> u64 {
        use spl_token::{solana_program::program_pack::Pack, state::Account as TokenAccount};

        let account = self.world_account(address);
        TokenAccount::unpack(&account.data)
            .unwrap_or_else(|error| {
                panic!("could not decode {address} as an SPL Token account: {error}")
            })
            .amount
    }

    /// A mint's current supply in this world.
    pub fn supply(&self, address: Pubkey) -> u64 {
        use spl_token::{solana_program::program_pack::Pack, state::Mint};

        let account = self.world_account(address);
        Mint::unpack(&account.data)
            .unwrap_or_else(|error| {
                panic!("could not decode {address} as an SPL Token mint: {error}")
            })
            .supply
    }

    fn world_account(&self, address: Pubkey) -> Account {
        self.svm
            .get_account(&address)
            .unwrap_or_else(|| panic!("no account at {address}"))
    }

    /// Register a rent-exempt program account holding `state`.
    ///
    /// The account gets the owning type's discriminator and owner, so a
    /// follow-up [`Self::read`] (or the program itself) accepts it:
    ///
    /// ```text
    /// q.write(vault, VaultData { authority: maker, balance: 500.into(), bump });
    /// ```
    pub fn write<D>(&mut self, address: Pubkey, state: D) -> Pubkey
    where
        D: AccountData + ZcElem + Copy,
        D::Wrapper: Discriminator + Owner,
    {
        let disc = <D::Wrapper as Discriminator>::DISCRIMINATOR;
        let size = core::mem::size_of::<D>();
        let mut data = vec![0u8; disc.len() + size];
        data[..disc.len()].copy_from_slice(disc);
        // SAFETY: `D` is `ZcElem` (`#[repr(C)]`, alignment 1, no padding),
        // so its `size` bytes are initialized and the destination range was
        // sized for them.
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&state as *const D).cast::<u8>(),
                data[disc.len()..].as_mut_ptr(),
                size,
            );
        }
        self.svm.set_account(fixtures::program_account(
            address,
            <D::Wrapper as Owner>::OWNER,
            data,
        ));
        address
    }

    /// Execute and commit one instruction using the accounts in this world.
    ///
    /// Writable instruction accounts missing from the world are registered as
    /// empty system accounts first, so freshly initialized accounts survive
    /// into follow-up sends and [`Self::read`] without any setup.
    #[must_use = "assert the outcome: .succeeds(), .fails_with(..), or inspect the result"]
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
    #[must_use = "assert the outcome: .succeeds(), .fails_with(..), or inspect the result"]
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
    #[must_use = "assert the outcome: .succeeds(), .fails_with(..), or inspect the result"]
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
