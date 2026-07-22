use {
    crate::{
        backend::Backend,
        fixture::{Fixture, TokenProgram},
        fixtures,
        outcome::{mint_supply, token_amount, TrackedAccount},
        Account, Instruction, Outcome, Pubkey, SetupError, TestBuilder,
    },
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{AccountData, Discriminator, Owner, SeedSlices},
    },
    std::{ops::Deref, path::Path},
};

/// Default balance assigned by [`crate::fixture::Wallet`]: ten SOL.
pub const DEFAULT_WALLET_LAMPORTS: u64 = 10_000_000_000;

/// An isolated Solana program test world.
///
/// Each `Test` owns its runtime and account state. The public API describes
/// test behavior rather than a particular SVM, so the same test can be hosted
/// by additional runtimes without becoming generic over a backend.
pub struct Test {
    pub(super) backend: Backend,
    pub(super) program_id: Pubkey,
    pub(super) program_path: std::path::PathBuf,
    pub(super) fresh_addresses: u64,
}

impl Test {
    /// Load the current project's compiled program.
    ///
    /// # Panics
    ///
    /// Panics with an actionable message when no program artifact can be
    /// located or read. Use [`Self::try_new`] to handle setup errors.
    pub fn new(program_id: impl Into<Pubkey>) -> Self {
        Self::builder(program_id)
            .build()
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Fallible variant of [`Self::new`].
    pub fn try_new(program_id: impl Into<Pubkey>) -> Result<Self, SetupError> {
        Self::builder(program_id).build()
    }

    /// Configure artifact discovery and runtime limits before loading a world.
    pub fn builder(program_id: impl Into<Pubkey>) -> TestBuilder {
        TestBuilder::new(program_id.into())
    }

    /// The primary program under test.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// The ELF artifact loaded for the primary program.
    pub fn program_path(&self) -> &Path {
        &self.program_path
    }

    /// Install a built-in or application-defined fixture.
    pub fn add<F: Fixture>(&mut self, fixture: F) -> F::Output {
        fixture.install(self)
    }

    /// Insert or replace a raw account.
    pub fn set_account(&mut self, account: Account) {
        self.backend.set_account(account);
    }

    /// Read a raw account from the current world.
    pub fn account(&self, address: Pubkey) -> Option<Account> {
        self.backend.account(&address)
    }

    /// Decode an account with a generated client decoder.
    pub fn account_as<T>(
        &self,
        address: Pubkey,
        decode: impl FnOnce(&[u8]) -> Option<T>,
    ) -> Option<T> {
        self.account(address)
            .and_then(|account| decode(&account.data))
    }

    /// Preload a program for cross-program invocations.
    pub fn load_program(&mut self, program_id: Pubkey, elf: &[u8]) {
        self.backend.load_program(&program_id, elf);
    }

    /// Produce a deterministic address unused by earlier fixtures in this
    /// world. The sequence is independent for every test.
    pub fn fresh_address(&mut self) -> Pubkey {
        self.fresh_addresses += 1;
        let mut bytes = *b"quasar-test/fresh-address\0\0\0\0\0\0\0";
        bytes[24..].copy_from_slice(&self.fresh_addresses.to_le_bytes());
        Pubkey::new_from_array(bytes)
    }

    /// Derive an associated-token address without installing the account.
    pub fn derive_ata(&self, owner: Pubkey, mint: Pubkey, token_program: TokenProgram) -> Pubkey {
        Pubkey::find_program_address(
            &[owner.as_ref(), token_program.id().as_ref(), mint.as_ref()],
            &crate::SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .0
    }

    /// Derive a PDA from an account type's declared seeds.
    pub fn derive_pda(&self, seeds: impl SeedSlices) -> Pubkey {
        self.derive_pda_with_bump(seeds).0
    }

    /// Derive a PDA and its canonical bump.
    pub fn derive_pda_with_bump(&self, seeds: impl SeedSlices) -> (Pubkey, u8) {
        seeds.with_slices(|slices| Pubkey::find_program_address(slices, &self.program_id))
    }

    /// Read a fixed-size Quasar account through its on-chain wrapper type.
    ///
    /// Ownership, discriminator, length, and zero-copy validity are checked
    /// before the snapshot is created.
    pub fn read<T>(&self, address: Pubkey) -> Snapshot<T>
    where
        T: Discriminator + Owner + Deref,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        let name = core::any::type_name::<T>();
        let account = self
            .account(address)
            .unwrap_or_else(|| panic!("read {name}: no account at {address}"));
        if account.owner != T::OWNER {
            panic!(
                "read {name}: account {address} is owned by {}, expected {}",
                account.owner,
                T::OWNER
            );
        }
        let discriminator = T::DISCRIMINATOR;
        let expected_len = discriminator.len() + core::mem::size_of::<T::Target>();
        if account.data.len() < expected_len {
            panic!(
                "read {name}: account {address} holds {} bytes, expected at least {expected_len}",
                account.data.len()
            );
        }
        if &account.data[..discriminator.len()] != discriminator {
            panic!(
                "read {name}: account {address} discriminator is {:?}, expected {discriminator:?}",
                &account.data[..discriminator.len()]
            );
        }
        // SAFETY: `T::Target` is `ZcElem` (alignment one and no padding), and
        // the length check above proves the target bytes are in bounds.
        let state = unsafe { &*(account.data[discriminator.len()..].as_ptr() as *const T::Target) };
        if let Err(error) = <T::Target as ZcValidate>::validate_ref(state) {
            panic!("read {name}: account {address} holds invalid data: {error:?}");
        }
        Snapshot {
            address,
            lamports: account.lamports,
            state: *state,
        }
    }

    /// Install a rent-exempt fixed-size Quasar account.
    pub fn write<D>(&mut self, address: Pubkey, state: D) -> Pubkey
    where
        D: AccountData + ZcElem + Copy,
        D::Wrapper: Discriminator + Owner,
    {
        let discriminator = <D::Wrapper as Discriminator>::DISCRIMINATOR;
        let size = core::mem::size_of::<D>();
        let mut data = vec![0; discriminator.len() + size];
        data[..discriminator.len()].copy_from_slice(discriminator);
        // SAFETY: `D` is `ZcElem`, so all `size` bytes are initialized, its
        // alignment is one, and the destination was allocated to fit them.
        unsafe {
            std::ptr::copy_nonoverlapping(
                (&state as *const D).cast::<u8>(),
                data[discriminator.len()..].as_mut_ptr(),
                size,
            );
        }
        self.set_account(fixtures::program_account(
            address,
            <D::Wrapper as Owner>::OWNER,
            data,
        ));
        address
    }

    /// An account's current lamport balance.
    pub fn lamports(&self, address: Pubkey) -> u64 {
        self.required_account(address).lamports
    }

    /// A base Token or Token-2022 account's current amount.
    pub fn tokens(&self, address: Pubkey) -> u64 {
        token_amount(&self.required_account(address))
    }

    /// A base Token or Token-2022 mint's current supply.
    pub fn supply(&self, address: Pubkey) -> u64 {
        mint_supply(&self.required_account(address))
    }

    /// Set the runtime clock's Unix timestamp.
    pub fn warp_to_timestamp(&mut self, timestamp: i64) {
        self.backend.warp_to_timestamp(timestamp);
    }

    /// Execute and commit one instruction.
    pub fn send(&mut self, instruction: impl Into<Instruction>) -> Outcome {
        self.execute([instruction.into()], Vec::new(), true)
    }

    /// Execute and commit an atomic instruction sequence.
    pub fn send_all<I, T>(&mut self, instructions: I) -> Outcome
    where
        I: IntoIterator<Item = T>,
        T: Into<Instruction>,
    {
        self.execute(
            instructions.into_iter().map(Into::into).collect::<Vec<_>>(),
            Vec::new(),
            true,
        )
    }

    /// Execute and commit one instruction with raw transaction-input
    /// accounts. Fixtures installed in the world normally make this
    /// unnecessary; it remains useful when malformed input is the test case.
    pub fn send_with(
        &mut self,
        instruction: impl Into<Instruction>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Outcome {
        self.execute([instruction.into()], accounts.into_iter().collect(), true)
    }

    /// Execute an instruction without committing its changes.
    pub fn simulate(&mut self, instruction: impl Into<Instruction>) -> Outcome {
        self.execute([instruction.into()], Vec::new(), false)
    }

    fn execute(
        &mut self,
        instructions: impl AsRef<[Instruction]>,
        mut inputs: Vec<Account>,
        commit: bool,
    ) -> Outcome {
        let instructions = instructions.as_ref();
        assert!(
            !instructions.is_empty(),
            "a transaction needs an instruction"
        );
        assert_unique_accounts(&inputs);

        let mut tracked = Vec::<TrackedAccount>::new();
        for instruction in instructions {
            for meta in &instruction.accounts {
                if let Some(existing) = tracked
                    .iter_mut()
                    .find(|account| account.address == meta.pubkey)
                {
                    existing.writable |= meta.is_writable;
                    continue;
                }
                let before = inputs
                    .iter()
                    .find(|account| account.address == meta.pubkey)
                    .cloned()
                    .or_else(|| self.backend.account(&meta.pubkey));
                tracked.push(TrackedAccount {
                    address: meta.pubkey,
                    writable: meta.is_writable,
                    before,
                    after: None,
                });
            }
        }

        for input in &inputs {
            if tracked
                .iter()
                .all(|account| account.address != input.address)
            {
                tracked.push(TrackedAccount {
                    address: input.address,
                    writable: false,
                    before: Some(input.clone()),
                    after: None,
                });
            }
        }

        // A missing writable account enters the transaction as Solana's empty
        // system account. QuasarSVM commits that transaction input only when
        // execution succeeds, so init targets persist without polluting the
        // world after a failed transaction.
        for account in &tracked {
            if account.writable
                && account.before.is_none()
                && inputs.iter().all(|input| input.address != account.address)
            {
                inputs.push(fixtures::empty_account(account.address));
            }
        }

        let result = self.backend.execute(instructions, &inputs, commit);
        let succeeded = result.raw_result.is_ok();
        for account in &mut tracked {
            account.after = if !succeeded {
                account.before.clone()
            } else if commit {
                self.backend.account(&account.address)
            } else {
                Outcome::simulated_account(&result, &account.address)
            };
        }
        Outcome::from_backend(result, tracked)
    }

    fn required_account(&self, address: Pubkey) -> Account {
        self.account(address)
            .unwrap_or_else(|| panic!("no account at {address}"))
    }
}

fn assert_unique_accounts(accounts: &[Account]) {
    for (index, account) in accounts.iter().enumerate() {
        assert!(
            accounts[..index]
                .iter()
                .all(|earlier| earlier.address != account.address),
            "transaction input contains account {} more than once",
            account.address
        );
    }
}

/// Typed fixed-size account state captured at one point in a test.
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
    /// Address from which the state was read.
    pub fn address(&self) -> Pubkey {
        self.address
    }

    /// Lamport balance captured with the state.
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
