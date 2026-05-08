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

// ---------------------------------------------------------------------------
// Token account schema — #[derive(ZeroPod)] replaces manual TokenAccountState
// ---------------------------------------------------------------------------

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
const _: () = assert!(core::mem::offset_of!(TokenDataZc, mint) == 0);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, owner) == 32);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, amount) == 64);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, delegate) == 72);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, state) == 108);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, native) == 109);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, delegated_amount) == 121);
const _: () = assert!(core::mem::offset_of!(TokenDataZc, close_authority) == 129);

#[inline(always)]
fn coption4_tag_valid(base: *const u8, offset: usize) -> bool {
    // SAFETY: callers pass offsets to the 4-byte COption tag prefix in an
    // already length-checked SPL token account buffer.
    unsafe { (base.add(offset) as *const u32).read_unaligned() <= 1 }
}

/// Semantic accessors for COption fields (auto-generated accessors don't cover
/// PFX=4).
impl TokenDataZc {
    #[inline(always)]
    pub fn coption_tags_valid(&self) -> bool {
        let base = self as *const Self as *const u8;
        coption4_tag_valid(base, 72)
            && coption4_tag_valid(base, 109)
            && coption4_tag_valid(base, 129)
    }
    #[inline(always)]
    pub fn state_valid(&self) -> bool {
        self.state <= 2
    }

    pub fn has_delegate(&self) -> bool {
        self.delegate.is_some()
    }
    pub fn delegate(&self) -> Option<&Address> {
        self.delegate.get_ref()
    }
    pub fn delegate_unchecked(&self) -> &Address {
        self.delegate.value_unchecked()
    }
    pub fn is_native(&self) -> bool {
        self.native.is_some()
    }
    pub fn native_amount(&self) -> Option<u64> {
        if self.native.is_some() {
            Some(self.native.value_unchecked().get())
        } else {
            None
        }
    }
    pub fn is_initialized(&self) -> bool {
        self.state != 0
    }
    pub fn is_frozen(&self) -> bool {
        self.state == 2
    }
    pub fn has_close_authority(&self) -> bool {
        self.close_authority.is_some()
    }
    pub fn close_authority(&self) -> Option<&Address> {
        self.close_authority.get_ref()
    }
    pub fn close_authority_unchecked(&self) -> &Address {
        self.close_authority.value_unchecked()
    }
}

// ---------------------------------------------------------------------------
// Mint account schema
// ---------------------------------------------------------------------------

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
const _: () = assert!(core::mem::offset_of!(MintDataZc, mint_authority) == 0);
const _: () = assert!(core::mem::offset_of!(MintDataZc, supply) == 36);
const _: () = assert!(core::mem::offset_of!(MintDataZc, decimals) == 44);
const _: () = assert!(core::mem::offset_of!(MintDataZc, is_initialized) == 45);
const _: () = assert!(core::mem::offset_of!(MintDataZc, freeze_authority) == 46);

impl MintDataZc {
    #[inline(always)]
    pub fn coption_tags_valid(&self) -> bool {
        let base = self as *const Self as *const u8;
        coption4_tag_valid(base, 0) && coption4_tag_valid(base, 46)
    }
    #[inline(always)]
    pub fn initialized_flag_valid(&self) -> bool {
        self.is_initialized <= 1
    }

    pub fn is_initialized(&self) -> bool {
        self.is_initialized != 0
    }
    pub fn has_mint_authority(&self) -> bool {
        self.mint_authority.is_some()
    }
    pub fn mint_authority(&self) -> Option<&Address> {
        self.mint_authority.get_ref()
    }
    pub fn mint_authority_unchecked(&self) -> &Address {
        self.mint_authority.value_unchecked()
    }
    pub fn has_freeze_authority(&self) -> bool {
        self.freeze_authority.is_some()
    }
    pub fn freeze_authority(&self) -> Option<&Address> {
        self.freeze_authority.get_ref()
    }
    pub fn freeze_authority_unchecked(&self) -> &Address {
        self.freeze_authority.value_unchecked()
    }
}

// ---------------------------------------------------------------------------
// Account wrappers
// ---------------------------------------------------------------------------

quasar_lang::define_account!(
    /// Token account data — validates owner is SPL Token program.
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
    /// Mint account — validates owner is SPL Token program.
    ///
    /// Use as `Account<Mint>` for single-program mints,
    /// or `InterfaceAccount<Mint>` to accept both SPL Token and Token-2022.
    pub struct Mint => [checks::ZeroPod]: MintData
);

impl Owner for Mint {
    const OWNER: Address = SPL_TOKEN_ID;
}

/// Valid owner programs for `InterfaceAccount<Token>` and
/// `InterfaceAccount<Mint>`.
static SPL_TOKEN_OWNERS: [Address; 2] = [SPL_TOKEN_ID, TOKEN_2022_ID];

impl quasar_lang::traits::Owners for Token {
    #[inline(always)]
    fn owners() -> &'static [Address] {
        &SPL_TOKEN_OWNERS
    }

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
    fn owners() -> &'static [Address] {
        &SPL_TOKEN_OWNERS
    }

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

// ---------------------------------------------------------------------------
// Shared trait impls (TokenClose, TokenSweep, AccountInit)
// ---------------------------------------------------------------------------

impl_token_account_traits!(Token);
impl_token_account_init!(Token);
impl_mint_account_init!(Mint);

// ---------------------------------------------------------------------------
// Init param types (shared by Token2022/Mint2022)
// ---------------------------------------------------------------------------

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
