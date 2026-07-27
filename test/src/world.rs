//! The [`Ctx`] world and its builder.
//!
//! `Ctx` is a thin newtype over [`parallax_svm::Ctx`]: everything the Parallax
//! harness already provides is reached through [`Deref`], while quasar-test
//! layers its own PDA and typed-state sugar on top as inherent methods that take
//! precedence during method resolution.

use {
    crate::{fixture::Fixture, Account, Outcome, Pubkey, SetupError},
    quasar_lang::{
        __zeropod::{ZcElem, ZcValidate},
        traits::{AccountData, Discriminator, Owner, SeedSlices},
    },
    solana_rent::Rent,
    std::{
        ops::{Deref, DerefMut},
        path::PathBuf,
    },
};

/// Environment variable set by `quasar test` to the freshly built program.
///
/// [`CtxBuilder::build`] bridges it to Parallax's own
/// [`PROGRAM_PATH_ENV`](parallax_svm::PROGRAM_PATH_ENV) so tests run through the
/// `quasar` CLI keep loading the compiled artifact without a build step.
pub const PROGRAM_PATH_ENV: &str = "QUASAR_PROGRAM_PATH";

/// An isolated Solana program test world.
///
/// Wraps [`parallax_svm::Ctx`]; its whole surface — `add`, `execute`, `account`,
/// `warp_to_timestamp`, and the rest — is available directly, plus the
/// quasar-test extras below.
pub struct Ctx(pub(crate) parallax_svm::Ctx);

impl Ctx {
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
    pub fn builder(program_id: impl Into<Pubkey>) -> CtxBuilder {
        CtxBuilder::new(program_id.into())
    }

    /// Install a built-in or application-defined fixture.
    ///
    /// Uses quasar-test's [`Fixture`] trait, so an application fixture's
    /// `install` receives this sugar-carrying `Ctx` and can call
    /// [`Self::derive_pda`], [`Self::write`], and the rest.
    pub fn add<F: Fixture>(&mut self, fixture: F) -> F::Output {
        fixture.install(self)
    }

    /// Derive a PDA from an account type's declared seeds.
    pub fn derive_pda(&self, seeds: impl SeedSlices) -> Pubkey {
        self.derive_pda_with_bump(seeds).0
    }

    /// Derive a PDA and its canonical bump.
    pub fn derive_pda_with_bump(&self, seeds: impl SeedSlices) -> (Pubkey, u8) {
        let program_id = self.0.program_id();
        seeds.with_slices(|slices| Pubkey::find_program_address(slices, &program_id))
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
            .0
            .account(address)
            .unwrap_or_else(|| panic!("read {name}: no account at {address}"));
        let state = validate_typed::<T>("read", &account);
        Snapshot {
            address,
            lamports: account.lamports,
            state,
        }
    }

    /// Read a fixed-size Quasar account whose typed state starts at `offset`
    /// within the account data (after the discriminator), with the same
    /// ownership, discriminator, and zero-copy validation as [`Self::read`].
    ///
    /// The schema-only wincode pair stays reachable through deref as
    /// `(*test).read(..)` / `(*test).write(..)`.
    pub fn read_at<T>(&self, address: Pubkey, offset: usize) -> Snapshot<T>
    where
        T: Discriminator + Owner + Deref,
        T::Target: ZcElem + ZcValidate + Copy,
    {
        let name = core::any::type_name::<T>();
        let account = self
            .0
            .account(address)
            .unwrap_or_else(|| panic!("read_at {name}: no account at {address}"));
        let state = validate_typed_at::<T>("read_at", &account, offset);
        Snapshot {
            address,
            lamports: account.lamports,
            state,
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
        self.0
            .set_account(program_account(address, <D::Wrapper as Owner>::OWNER, data));
        address
    }

    /// Execute and commit one transaction: a single instruction, or a chain
    /// as a tuple, array, or `Vec`.
    pub fn execute<M>(&mut self, instructions: impl parallax_svm::IntoInstructions<M>) -> Outcome {
        Outcome::new(self.0.execute(instructions))
    }

    /// Execute and commit with raw transaction-input accounts seeding or
    /// overriding world state.
    pub fn execute_with<M>(
        &mut self,
        instructions: impl parallax_svm::IntoInstructions<M>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Outcome {
        Outcome::new(self.0.execute_with(instructions, accounts))
    }

    /// Execute one transaction without committing its changes.
    pub fn simulate<M>(&mut self, instructions: impl parallax_svm::IntoInstructions<M>) -> Outcome {
        Outcome::new(self.0.simulate(instructions))
    }

    /// Simulate with raw transaction-input accounts, committing nothing.
    pub fn simulate_with<M>(
        &mut self,
        instructions: impl parallax_svm::IntoInstructions<M>,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Outcome {
        Outcome::new(self.0.simulate_with(instructions, accounts))
    }
}

impl Deref for Ctx {
    type Target = parallax_svm::Ctx;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Ctx {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Create a rent-exempt program-owned account containing `data`.
fn program_account(address: Pubkey, owner: Pubkey, data: Vec<u8>) -> Account {
    Account::new(
        address,
        owner,
        Rent::default().minimum_balance(data.len()),
        data,
    )
}

/// World setup: which program artifact to load and its runtime limits.
///
/// Created by [`Ctx::builder`]. Wraps [`parallax_svm::CtxBuilder`] and adds
/// the [`PROGRAM_PATH_ENV`] bridge in [`Self::build`].
pub struct CtxBuilder {
    inner: parallax_svm::CtxBuilder,
    program_path_set: bool,
}

impl CtxBuilder {
    fn new(program_id: Pubkey) -> Self {
        Self {
            inner: parallax_svm::Ctx::builder(program_id),
            program_path_set: false,
        }
    }

    /// Set the transaction compute-unit limit for this world.
    pub fn compute_unit_limit(mut self, limit: u64) -> Self {
        self.inner = self.inner.compute_unit_limit(limit);
        self
    }

    /// RPC endpoint that `Dump` fixtures fetch from on a store miss. Code-only
    /// and set once; unset, it defaults to the public mainnet-beta RPC.
    pub fn rpc(mut self, url: impl Into<String>) -> Self {
        self.inner = self.inner.rpc(url);
        self
    }

    /// Project directory whose committed `.parallax/` store dumps read and
    /// write, when the default manifest walk-up is not wanted.
    pub fn project_dir(mut self, dir: impl Into<String>) -> Self {
        self.inner = self.inner.project_dir(dir);
        self
    }

    /// Load the primary program from in-memory ELF bytes, skipping artifact
    /// discovery. Empty bytes are equivalent to [`Self::no_program`].
    pub fn program_bytes(mut self, elf: impl Into<Vec<u8>>) -> Self {
        self.inner = self.inner.program_bytes(elf);
        self.program_path_set = true;
        self
    }

    /// Build a world with no primary program, loading only the runtime's
    /// built-in programs.
    pub fn no_program(mut self) -> Self {
        self.inner = self.inner.no_program();
        self.program_path_set = true;
        self
    }

    /// Load an explicit program artifact instead of discovering one.
    pub fn program_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.inner = self.inner.program_path(path);
        self.program_path_set = true;
        self
    }

    /// Prefer `target/deploy/{crate_name}.so` (with `-` mapped to `_`) during
    /// discovery, so tests resolve their own program in a workspace that builds
    /// several. `#[quasar_test]` passes `env!("CARGO_PKG_NAME")`.
    pub fn crate_name(mut self, name: impl Into<String>) -> Self {
        self.inner = self.inner.crate_name(name);
        self
    }

    /// Load the program and start the world.
    pub fn build(mut self) -> Result<Ctx, SetupError> {
        // Bridge the `quasar test` override to Parallax's own env var without
        // mutating process state (tests run on parallel threads). An explicit
        // `program_path`, or Parallax's env var, both take precedence.
        if !self.program_path_set && std::env::var_os(parallax_svm::PROGRAM_PATH_ENV).is_none() {
            if let Some(path) = std::env::var_os(PROGRAM_PATH_ENV) {
                let path = PathBuf::from(path);
                if !path.is_file() {
                    return Err(SetupError::ConfiguredProgramMissing {
                        env: PROGRAM_PATH_ENV,
                        path,
                    });
                }
                self.inner = self.inner.program_path(path);
            }
        }
        self.inner.build().map(Ctx)
    }
}

/// Validate `account` as a fixed-size Quasar account of type `T` and return a
/// copy of its typed state. `context` names the calling operation so panics stay
/// actionable (`read`, `State`, ...). Shared by [`Ctx::read`] and
/// [`crate::State`] so both apply identical ownership,
/// discriminator, length, and zero-copy checks.
pub(crate) fn validate_typed<T>(context: &str, account: &Account) -> T::Target
where
    T: Discriminator + Owner + Deref,
    T::Target: ZcElem + ZcValidate + Copy,
{
    validate_typed_at::<T>(context, account, 0)
}

/// [`validate_typed`] with the typed state starting `offset` bytes past the
/// discriminator; backs [`Ctx::read_at`].
pub(crate) fn validate_typed_at<T>(context: &str, account: &Account, offset: usize) -> T::Target
where
    T: Discriminator + Owner + Deref,
    T::Target: ZcElem + ZcValidate + Copy,
{
    let name = core::any::type_name::<T>();
    let address = account.address;
    if account.owner != T::OWNER {
        panic!(
            "{context} {name}: account {address} is owned by {}, expected {}",
            account.owner,
            T::OWNER
        );
    }
    let discriminator = T::DISCRIMINATOR;
    let start = discriminator.len() + offset;
    let expected_len = start + core::mem::size_of::<T::Target>();
    if account.data.len() < expected_len {
        panic!(
            "{context} {name}: account {address} holds {} bytes, expected at least {expected_len}",
            account.data.len()
        );
    }
    if &account.data[..discriminator.len()] != discriminator {
        panic!(
            "{context} {name}: account {address} discriminator is {:?}, expected {discriminator:?}",
            &account.data[..discriminator.len()]
        );
    }
    // SAFETY: `T::Target` is `ZcElem` (alignment one and no padding), and the
    // length check above proves the target bytes are in bounds.
    let state = unsafe { &*(account.data[start..].as_ptr() as *const T::Target) };
    if let Err(error) = <T::Target as ZcValidate>::validate_ref(state) {
        panic!("{context} {name}: account {address} holds invalid data: {error:?}");
    }
    *state
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

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal fixed-size Quasar account whose payload is a single address,
    // enough to exercise the typed validation path without a program.
    #[repr(transparent)]
    #[derive(Clone, Copy)]
    struct Marker(Pubkey);

    impl Deref for Marker {
        type Target = Pubkey;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Discriminator for Marker {
        const DISCRIMINATOR: &'static [u8] = &[0xAB];
    }

    impl Owner for Marker {
        const OWNER: Pubkey = Pubkey::new_from_array([9; 32]);
    }

    fn marker_account(address: Pubkey, owner: Pubkey, stored: Pubkey) -> Account {
        let mut data = Marker::DISCRIMINATOR.to_vec();
        data.extend_from_slice(stored.as_ref());
        Account::new(address, owner, 42, data)
    }

    #[test]
    fn validate_typed_reads_the_stored_state() {
        let address = Pubkey::new_from_array([5; 32]);
        let stored = Pubkey::new_from_array([7; 32]);
        let account = marker_account(address, Marker::OWNER, stored);

        let state = validate_typed::<Marker>("read", &account);
        assert_eq!(state, stored);
    }

    #[test]
    #[should_panic(expected = "read")]
    fn validate_typed_panics_when_ownership_is_wrong() {
        let address = Pubkey::new_from_array([5; 32]);
        let foreign = Pubkey::new_from_array([1; 32]);
        let account = marker_account(address, foreign, address);

        validate_typed::<Marker>("read", &account);
    }
}
