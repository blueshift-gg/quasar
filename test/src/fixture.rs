//! Composable fixtures for common Solana accounts and programs.

use crate::{fixtures, Account, Pubkey, Test, SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID};

/// State that can install itself into a test world.
///
/// Applications can implement this trait for protocol-level fixtures and
/// compose the built-in account fixtures inside [`Fixture::install`]. Arrays
/// of one fixture type are fixtures too, so repeated setup can be installed
/// with `test.add([Wallet::new(); 3])`. Each fixture returns the address it
/// placed, so tests thread those handles instead of pinning addresses up front.
pub trait Fixture {
    /// Handle or state returned after installation.
    type Output;

    /// Install the fixture and return the handles needed by the test.
    fn install(self, test: &mut Test) -> Self::Output;
}

impl<F: Fixture, const N: usize> Fixture for [F; N] {
    type Output = [F::Output; N];

    fn install(self, test: &mut Test) -> Self::Output {
        self.map(|fixture| fixture.install(test))
    }
}

impl Fixture for Account {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        let address = self.address;
        test.set_account(self);
        address
    }
}

/// A system-owned, funded account.
#[derive(Debug, Clone, Copy)]
pub struct Wallet {
    address: Option<Pubkey>,
    lamports: u64,
}

impl Wallet {
    /// Create a wallet with [`crate::DEFAULT_WALLET_LAMPORTS`].
    pub fn new() -> Self {
        Self {
            address: None,
            lamports: crate::DEFAULT_WALLET_LAMPORTS,
        }
    }

    /// Use a specific address instead of the world's next deterministic one.
    pub fn at(mut self, address: Pubkey) -> Self {
        self.address = Some(address);
        self
    }

    /// Fund the wallet with an exact lamport balance, replacing the default.
    pub fn fund(mut self, lamports: u64) -> Self {
        self.lamports = lamports;
        self
    }
}

impl Default for Wallet {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture for Wallet {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        let address = self.address.unwrap_or_else(|| test.fresh_address());
        test.set_account(fixtures::system_account(address, self.lamports));
        address
    }
}

/// Which token program owns a mint or token account fixture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TokenProgram {
    /// The original SPL Token program.
    #[default]
    Legacy,
    /// The Token-2022 program.
    Token2022,
}

impl TokenProgram {
    pub(crate) fn id(self) -> Pubkey {
        match self {
            Self::Legacy => SPL_TOKEN_PROGRAM_ID,
            Self::Token2022 => SPL_TOKEN_2022_PROGRAM_ID,
        }
    }
}

/// An initialized token mint.
///
/// A bare [`Mint::new`] has no mint authority, modelling a fixed-supply mint.
/// [`Self::with_authority`] makes it mintable and [`Self::with_freeze_authority`]
/// installs a freeze authority. [`Self::with_holder`] additionally installs one
/// associated-token account per holder, so a funded mint and its holders can be
/// set up in a single fixture.
#[derive(Debug, Clone)]
pub struct Mint {
    authority: Option<Pubkey>,
    freeze_authority: Option<Pubkey>,
    supply: u64,
    decimals: u8,
    token_program: TokenProgram,
    holders: Vec<(Pubkey, u64)>,
}

impl Mint {
    /// Create a six-decimal legacy Token mint with zero supply and no mint or
    /// freeze authority. Without [`Self::with_authority`] the mint is
    /// fixed-supply (its `mint_authority` is `COption::None`). Installing the
    /// mint returns its address, so tests read that back rather than pinning it.
    pub fn new() -> Self {
        Self {
            authority: None,
            freeze_authority: None,
            supply: 0,
            decimals: 6,
            token_program: TokenProgram::Legacy,
            holders: Vec::new(),
        }
    }

    /// Set the mint authority, making the mint mintable. Left unset, the mint
    /// is fixed-supply.
    pub fn with_authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the freeze authority. Left unset, the mint cannot freeze accounts.
    pub fn with_freeze_authority(mut self, freeze_authority: Pubkey) -> Self {
        self.freeze_authority = Some(freeze_authority);
        self
    }

    /// Set the initial token supply.
    pub fn supply(mut self, supply: u64) -> Self {
        self.supply = supply;
        self
    }

    /// Set the mint precision.
    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    /// Select the token program that owns the mint.
    pub fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = token_program;
        self
    }

    /// Fund each `(owner, amount)` holder with `amount` of this mint through its
    /// associated-token account, created when the mint is installed.
    ///
    /// Takes the whole set in one call, from anything iterable (an array,
    /// slice, or vec). Reach for [`AssociatedTokenAccount`] or [`TokenAccount`]
    /// directly when a holder needs a non-associated address or other explicit
    /// control.
    pub fn with_holder(mut self, holders: impl IntoIterator<Item = (Pubkey, u64)>) -> Self {
        self.holders.extend(holders);
        self
    }
}

impl Default for Mint {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture for Mint {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        let address = test.fresh_address();
        test.set_account(fixtures::token_program_mint_account(
            address,
            self.authority,
            self.freeze_authority,
            self.supply,
            self.decimals,
            self.token_program.id(),
        ));
        for (owner, amount) in self.holders {
            test.set_account(fixtures::associated_token_account_with_program(
                owner,
                address,
                amount,
                self.token_program.id(),
            ));
        }
        address
    }
}

/// An initialized token account at an arbitrary address.
#[derive(Debug, Clone, Copy)]
pub struct TokenAccount {
    address: Option<Pubkey>,
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    token_program: TokenProgram,
}

impl TokenAccount {
    /// Create an empty legacy Token account for `mint`, owned by `owner`.
    pub fn new(mint: Pubkey, owner: Pubkey) -> Self {
        Self {
            address: None,
            mint,
            owner,
            amount: 0,
            token_program: TokenProgram::Legacy,
        }
    }

    /// Install the token account at a specific address.
    pub fn at(mut self, address: Pubkey) -> Self {
        self.address = Some(address);
        self
    }

    /// Set the initial token balance.
    pub fn amount(mut self, amount: u64) -> Self {
        self.amount = amount;
        self
    }

    /// Select the token program that owns the account.
    pub fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = token_program;
        self
    }
}

impl Fixture for TokenAccount {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        let address = self.address.unwrap_or_else(|| test.fresh_address());
        test.set_account(fixtures::token_program_account(
            address,
            self.mint,
            self.owner,
            self.amount,
            self.token_program.id(),
        ));
        address
    }
}

/// An initialized token account at its associated-token address.
#[derive(Debug, Clone, Copy)]
pub struct AssociatedTokenAccount {
    mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    token_program: TokenProgram,
}

impl AssociatedTokenAccount {
    /// Create an empty legacy associated-token account.
    pub fn new(mint: Pubkey, owner: Pubkey) -> Self {
        Self {
            mint,
            owner,
            amount: 0,
            token_program: TokenProgram::Legacy,
        }
    }

    /// Set the initial token balance.
    pub fn amount(mut self, amount: u64) -> Self {
        self.amount = amount;
        self
    }

    /// Select the token program used in address derivation and ownership.
    pub fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = token_program;
        self
    }
}

impl Fixture for AssociatedTokenAccount {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        let account = fixtures::associated_token_account_with_program(
            self.owner,
            self.mint,
            self.amount,
            self.token_program.id(),
        );
        let address = account.address;
        test.set_account(account);
        address
    }
}

/// A program to preload for cross-program invocations.
pub struct Program<'a> {
    id: Pubkey,
    elf: &'a [u8],
}

impl<'a> Program<'a> {
    /// Create a program fixture from its address and compiled ELF bytes.
    pub fn new(id: Pubkey, elf: &'a [u8]) -> Self {
        Self { id, elf }
    }
}

impl Fixture for Program<'_> {
    type Output = Pubkey;

    fn install(self, test: &mut Test) -> Self::Output {
        test.load_program(self.id, self.elf);
        self.id
    }
}
