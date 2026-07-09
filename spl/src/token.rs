// Re-export zeropod so #[derive(ZeroPod)] expansion resolves `zeropod::*`
// paths.
use {
    crate::{
        constants::{SPL_TOKEN_BYTES, SPL_TOKEN_ID, TOKEN_2022_ID},
        instructions::TokenCpi,
    },
    quasar_lang::{__zeropod as zeropod, prelude::*, traits::Id},
    solana_address::Address,
};

#[derive(quasar_lang::__zeropod::ZeroPod)]
pub struct TokenData {
    pub mint: Address,
    pub owner: Address,
    pub amount: u64,
    pub delegate: quasar_lang::__zeropod::pod::PodOption<Address, 4>,
    pub state: u8,
    pub native: quasar_lang::__zeropod::pod::PodOption<quasar_lang::__zeropod::pod::PodU64, 4>,
    pub delegated_amount: u64,
    pub close_authority: quasar_lang::__zeropod::pod::PodOption<Address, 4>,
}

const _: () = assert!(core::mem::size_of::<TokenDataZc>() == 165);
const _: () = assert!(core::mem::align_of::<TokenDataZc>() == 1);

impl TokenDataZc {
    #[inline(always)]
    pub fn state_valid(&self) -> bool {
        self.state <= 2
    }

    pub fn native_amount(&self) -> Option<u64> {
        self.native().map(|amount| amount.get())
    }
    pub fn is_initialized(&self) -> bool {
        self.state != 0
    }
    pub fn is_frozen(&self) -> bool {
        self.state == 2
    }
}

#[derive(quasar_lang::__zeropod::ZeroPod)]
pub struct MintData {
    pub mint_authority: quasar_lang::__zeropod::pod::PodOption<Address, 4>,
    pub supply: u64,
    pub decimals: u8,
    #[zeropod(skip_accessor)]
    pub is_initialized: u8,
    pub freeze_authority: quasar_lang::__zeropod::pod::PodOption<Address, 4>,
}

const _: () = assert!(core::mem::size_of::<MintDataZc>() == 82);
const _: () = assert!(core::mem::align_of::<MintDataZc>() == 1);

impl MintDataZc {
    #[inline(always)]
    pub fn initialized_flag_valid(&self) -> bool {
        self.is_initialized <= 1
    }

    pub fn is_initialized(&self) -> bool {
        self.is_initialized != 0
    }
}

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

// SPL Token program marker. Use as `Program<TokenProgram>`.
quasar_lang::define_account!(pub struct TokenProgram => [checks::Executable, checks::Address]);

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
        mint: &'a AccountView,
        authority: &'a Address,
        token_program: &'a AccountView,
    },
    /// ATA init via the associated token program.
    AssociatedToken {
        mint: &'a AccountView,
        authority: &'a AccountView,
        token_program: &'a AccountView,
        system_program: &'a AccountView,
        ata_program: &'a AccountView,
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
        decimals: u8,
        authority: &'a Address,
        freeze_authority: Option<&'a Address>,
        token_program: &'a AccountView,
    },
}
