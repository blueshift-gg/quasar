//! SPL Token account, mint, and program wrapper types.
//!
//! Defines the zero-copy `TokenData`/`MintData` schemas and the `Token`,
//! `Mint`, and `TokenProgram` wrappers. `Account<Token>`/`Account<Mint>` trust
//! only accounts owned by the SPL Token program; the `Owners` impls also accept
//! Token-2022 so the same schema can back `InterfaceAccount`.

// The `#[derive(quasar_lang::ZeroPod)]` expansion emits unqualified `zeropod::`
// paths and has no crate-path override, so alias the framework's re-export as
// `zeropod` to resolve them. Everything else uses the stable
// `quasar_lang::{ZeroPod, pod, ...}` paths.
use {
    crate::{
        constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID, TOKEN_2022_ID},
        instructions::TokenCpi,
    },
    quasar_lang::{__zeropod as zeropod, prelude::*, traits::Id},
    solana_address::Address,
};

// The upstream derive emits public `*Zc` companions and accessors without
// propagating source documentation. Keep that narrow generated-code exception
// inside this module while retaining documented re-exported schemas below.
#[allow(missing_docs)]
mod layouts {
    use super::*;

    #[derive(quasar_lang::ZeroPod)]
    /// Native schema for the 165-byte SPL token-account layout.
    pub struct TokenData {
        /// Mint associated with this token account.
        pub mint: Address,
        /// Authority that owns this token account.
        pub owner: Address,
        /// Token balance in base units.
        pub amount: u64,
        /// Optional delegated transfer authority.
        pub delegate: quasar_lang::pod::PodOption<Address, 4>,
        /// Account state: uninitialized, initialized, or frozen.
        pub state: u8,
        /// Rent-exempt reserve for wrapped native SOL accounts.
        pub native: quasar_lang::pod::PodOption<quasar_lang::pod::PodU64, 4>,
        /// Balance currently authorized for the delegate.
        pub delegated_amount: u64,
        /// Optional authority allowed to close the account.
        pub close_authority: quasar_lang::pod::PodOption<Address, 4>,
    }

    const _: () = assert!(core::mem::size_of::<TokenDataZc>() == 165);
    const _: () = assert!(core::mem::align_of::<TokenDataZc>() == 1);

    impl TokenDataZc {
        /// Returns whether the encoded account-state byte is recognized.
        #[inline(always)]
        pub fn state_valid(&self) -> bool {
            self.state <= 2
        }

        /// Returns the wrapped SOL reserve for native accounts.
        pub fn native_amount(&self) -> Option<u64> {
            self.native().map(|amount| amount.get())
        }
        /// Returns whether the token account has been initialized.
        pub fn is_initialized(&self) -> bool {
            self.state != 0
        }
        /// Returns whether the token account is frozen.
        pub fn is_frozen(&self) -> bool {
            self.state == 2
        }
    }

    #[derive(quasar_lang::ZeroPod)]
    /// Native schema for the 82-byte SPL mint layout.
    pub struct MintData {
        /// Optional authority allowed to mint new tokens.
        pub mint_authority: quasar_lang::pod::PodOption<Address, 4>,
        /// Total token supply in base units.
        pub supply: u64,
        /// Decimal precision displayed by clients.
        pub decimals: u8,
        #[zeropod(skip_accessor)]
        /// Boolean initialization flag encoded as a byte.
        pub is_initialized: u8,
        /// Optional authority allowed to freeze token accounts.
        pub freeze_authority: quasar_lang::pod::PodOption<Address, 4>,
    }

    const _: () = assert!(core::mem::size_of::<MintDataZc>() == 82);
    const _: () = assert!(core::mem::align_of::<MintDataZc>() == 1);

    impl MintDataZc {
        /// Returns whether the encoded initialization flag is zero or one.
        #[inline(always)]
        pub fn initialized_flag_valid(&self) -> bool {
            self.is_initialized <= 1
        }

        /// Returns whether the mint has been initialized.
        pub fn is_initialized(&self) -> bool {
            self.is_initialized != 0
        }
    }
}

pub use layouts::{MintData, MintDataZc, TokenData, TokenDataZc};

quasar_lang::define_account!(
    /// Token account data; validates owner is SPL Token program.
    ///
    /// Use as `Account<Token>` for single-program token accounts,
    /// or `InterfaceAccount<Token>` to accept both SPL Token and Token-2022.
    pub struct Token => [checks::ZeroPod]: TokenData
);

impl Owner for Token {
    const OWNER: Address = SPL_TOKEN_ID;
}

quasar_lang::define_account!(
    /// SPL Token executable account marker.
    ///
    /// Use as `Program<TokenProgram>`.
    pub struct TokenProgram => [checks::Executable, checks::Address]
);

impl Id for TokenProgram {
    const ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
}

quasar_lang::define_account!(
    /// Mint account; validates owner is SPL Token program.
    ///
    /// Use as `Account<Mint>` for single-program mints,
    /// or `InterfaceAccount<Mint>` to accept both SPL Token and Token-2022.
    pub struct Mint => [checks::ZeroPod]: MintData
);

impl Owner for Mint {
    const OWNER: Address = SPL_TOKEN_ID;
}

impl quasar_lang::traits::Owners for Token {
    #[inline(always)]
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        let owner = view.owner();
        if quasar_lang::utils::hint::unlikely(
            !quasar_lang::keys_eq(owner, &SPL_TOKEN_ID)
                && !quasar_lang::keys_eq(owner, &TOKEN_2022_ID),
        ) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

impl quasar_lang::traits::Owners for Mint {
    #[inline(always)]
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        let owner = view.owner();
        if quasar_lang::utils::hint::unlikely(
            !quasar_lang::keys_eq(owner, &SPL_TOKEN_ID)
                && !quasar_lang::keys_eq(owner, &TOKEN_2022_ID),
        ) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }
}

impl TokenCpi for Program<TokenProgram> {}

impl_token_account_init!(Token);
impl_mint_account_init!(Mint);

/// Init params for token account creation via CPI.
///
/// The derive constructs `Default` (Unset) and behavior modules fill the
/// variant via `AccountBehavior::set_init_param`. Calling `AccountInit::init`
/// with `Unset` is a program error.
#[derive(Default)]
pub enum TokenInitKind<'a> {
    /// No behavior has filled init params yet.
    #[default]
    Unset,
    /// Direct token account init via system program + initialize_account3.
    Token {
        /// Token mint account.
        mint: &'a AccountView,
        /// Address that will own the new token account.
        authority: &'a Address,
        /// Token program used for initialization.
        token_program: &'a AccountView,
    },
    /// ATA init via the associated token program.
    AssociatedToken {
        /// Token mint account.
        mint: &'a AccountView,
        /// Wallet or PDA that will own the ATA.
        authority: &'a AccountView,
        /// Token program used for initialization.
        token_program: &'a AccountView,
        /// System Program account.
        system_program: &'a AccountView,
        /// Associated Token Program account.
        ata_program: &'a AccountView,
        /// Whether creation accepts an already existing ATA.
        idempotent: bool,
    },
}

/// Init params for mint account creation via CPI.
///
/// The derive constructs `Default` (Unset) and behavior modules fill params
/// via `AccountBehavior::set_init_param`. Calling `AccountInit::init` with
/// `Unset` is a program error.
#[derive(Default)]
pub enum MintInitParams<'a> {
    /// No behavior has filled init params yet.
    #[default]
    Unset,
    /// Mint init parameters filled by a behavior module.
    Mint {
        /// Decimal precision for the mint.
        decimals: u8,
        /// Authority allowed to mint new tokens.
        authority: &'a Address,
        /// Optional authority allowed to freeze token accounts.
        freeze_authority: Option<&'a Address>,
        /// Token program used for initialization.
        token_program: &'a AccountView,
    },
}
