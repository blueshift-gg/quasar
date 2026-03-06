use quasar_core::cpi::{CpiCall, InstructionAccount, Seed};
use quasar_core::prelude::*;
use quasar_core::traits::Id;

use crate::constants::{ATA_PROGRAM_BYTES, ATA_PROGRAM_ID, SPL_TOKEN_ID};
use crate::cpi::TokenCpi;
use crate::init::validate_token_account;
use crate::state::TokenAccountState;

// ATA program instruction discriminators.
const ATA_CREATE: u8 = 0;
const ATA_CREATE_IDEMPOTENT: u8 = 1;

// ---------------------------------------------------------------------------
// AssociatedTokenProgram — program account type
// ---------------------------------------------------------------------------

quasar_core::define_account!(pub struct AssociatedTokenProgram => [checks::Executable, checks::Address]);

impl Id for AssociatedTokenProgram {
    const ID: Address = Address::new_from_array(ATA_PROGRAM_BYTES);
}

// ---------------------------------------------------------------------------
// AssociatedToken — account marker type
// ---------------------------------------------------------------------------

/// Associated token account marker — validates owner is SPL Token program.
///
/// Use as `Account<AssociatedToken>` for SPL Token-only ATAs, or
/// `InterfaceAccount<AssociatedToken>` for both SPL Token and Token-2022.
///
/// The derive macro recognizes this type and auto-derives the ATA address
/// from `associated_token::mint` + `associated_token::authority` attributes.
pub struct AssociatedToken;
impl_single_owner!(AssociatedToken, SPL_TOKEN_ID, TokenAccountState);

// ---------------------------------------------------------------------------
// ATA address derivation
// ---------------------------------------------------------------------------

/// Derive the associated token account address for a wallet and mint.
///
/// Uses the SPL Token program as the token program. Returns `(address, bump)`.
///
/// On BPF, uses the `find_program_address` syscall (~1,500 CU).
/// Off-chain, use [`get_associated_token_address_const`] instead.
#[inline(always)]
pub fn get_associated_token_address(wallet: &Address, mint: &Address) -> (Address, u8) {
    get_associated_token_address_with_program(wallet, mint, &SPL_TOKEN_ID)
}

/// Derive the associated token account address for a wallet, mint, and token program.
///
/// Returns `(address, bump)`.
///
/// On BPF, uses the `find_program_address` syscall (~1,500 CU).
/// Off-chain, use [`get_associated_token_address_with_program_const`] instead.
#[inline(always)]
pub fn get_associated_token_address_with_program(
    wallet: &Address,
    mint: &Address,
    token_program: &Address,
) -> (Address, u8) {
    let seeds = [
        Seed::from(wallet.as_ref()),
        Seed::from(token_program.as_ref()),
        Seed::from(mint.as_ref()),
    ];
    quasar_core::pda::find_program_address(&seeds, &ATA_PROGRAM_ID)
}

/// Const-compatible ATA address derivation (works off-chain and in const contexts).
///
/// Uses `const_crypto` for SHA-256 and Ed25519 off-curve evaluation.
pub const fn get_associated_token_address_const(wallet: &Address, mint: &Address) -> (Address, u8) {
    get_associated_token_address_with_program_const(wallet, mint, &SPL_TOKEN_ID)
}

/// Const-compatible ATA address derivation with explicit token program.
pub const fn get_associated_token_address_with_program_const(
    wallet: &Address,
    mint: &Address,
    token_program: &Address,
) -> (Address, u8) {
    quasar_core::pda::find_program_address_const(
        &[wallet.as_array(), token_program.as_array(), mint.as_array()],
        &ATA_PROGRAM_ID,
    )
}

// ---------------------------------------------------------------------------
// CPI — create / create_idempotent
// ---------------------------------------------------------------------------

/// Build a CPI to the ATA program's `Create` instruction.
///
/// Fails if the associated token account already exists.
///
/// Accounts: payer (signer, writable), ata (writable), wallet, mint,
/// system_program, token_program.
#[inline(always)]
pub fn create<'a>(
    ata_program: &'a AssociatedTokenProgram,
    payer: &'a impl AsAccountView,
    ata: &'a AccountView,
    wallet: &'a impl AsAccountView,
    mint: &'a impl AsAccountView,
    system_program: &'a Program<System>,
    token_program: &'a impl TokenCpi,
) -> CpiCall<'a, 6, 1> {
    build_ata_cpi(
        ata_program,
        payer,
        ata,
        wallet,
        mint,
        system_program,
        token_program,
        ATA_CREATE,
    )
}

/// Build a CPI to the ATA program's `CreateIdempotent` instruction.
///
/// No-ops if the associated token account already exists.
///
/// Accounts: payer (signer, writable), ata (writable), wallet, mint,
/// system_program, token_program.
#[inline(always)]
pub fn create_idempotent<'a>(
    ata_program: &'a AssociatedTokenProgram,
    payer: &'a impl AsAccountView,
    ata: &'a AccountView,
    wallet: &'a impl AsAccountView,
    mint: &'a impl AsAccountView,
    system_program: &'a Program<System>,
    token_program: &'a impl TokenCpi,
) -> CpiCall<'a, 6, 1> {
    build_ata_cpi(
        ata_program,
        payer,
        ata,
        wallet,
        mint,
        system_program,
        token_program,
        ATA_CREATE_IDEMPOTENT,
    )
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn build_ata_cpi<'a>(
    ata_program: &'a AssociatedTokenProgram,
    payer: &'a impl AsAccountView,
    ata: &'a AccountView,
    wallet: &'a impl AsAccountView,
    mint: &'a impl AsAccountView,
    system_program: &'a Program<System>,
    token_program: &'a impl TokenCpi,
    discriminator: u8,
) -> CpiCall<'a, 6, 1> {
    let payer = payer.to_account_view();
    let wallet = wallet.to_account_view();
    let mint = mint.to_account_view();
    let sys = system_program.to_account_view();
    let tok = token_program.to_account_view();

    CpiCall::new(
        ata_program.address(),
        [
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::writable(ata.address()),
            InstructionAccount::readonly(wallet.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly(sys.address()),
            InstructionAccount::readonly(tok.address()),
        ],
        [payer, ata, wallet, mint, sys, tok],
        [discriminator],
    )
}

// ---------------------------------------------------------------------------
// InitAssociatedToken — manual init trait
// ---------------------------------------------------------------------------

/// Extension trait providing `.init()` / `.init_if_needed()` on `Initialize<T>`
/// for associated token account types.
///
/// Unlike [`InitToken`](crate::InitToken) which chains `create_account + initialize_account3`,
/// this delegates to the ATA program which handles creation + initialization in a single CPI.
///
/// ```ignore
/// self.new_ata.init(
///     self.payer,
///     self.wallet,
///     self.mint,
///     self.system_program,
///     self.token_program,
///     self.ata_program,
/// )?;
/// ```
pub trait InitAssociatedToken: AsAccountView + Sized {
    /// Create an associated token account via the ATA program.
    ///
    /// Fails if the account already exists.
    #[inline(always)]
    fn init(
        &self,
        payer: &impl AsAccountView,
        wallet: &impl AsAccountView,
        mint: &impl AsAccountView,
        system_program: &Program<System>,
        token_program: &impl TokenCpi,
        ata_program: &AssociatedTokenProgram,
    ) -> Result<(), ProgramError> {
        create(
            ata_program,
            payer,
            self.to_account_view(),
            wallet,
            mint,
            system_program,
            token_program,
        )
        .invoke()
    }

    /// Create an associated token account if it doesn't already exist.
    ///
    /// Uses `CreateIdempotent` — no-ops if the account is already initialized.
    /// When the account exists, validates mint and authority match.
    #[inline(always)]
    fn init_if_needed(
        &self,
        payer: &impl AsAccountView,
        wallet: &impl AsAccountView,
        mint: &impl AsAccountView,
        system_program: &Program<System>,
        token_program: &impl TokenCpi,
        ata_program: &AssociatedTokenProgram,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if quasar_core::is_system_program(unsafe { view.owner() }) {
            create_idempotent(
                ata_program,
                payer,
                view,
                wallet,
                mint,
                system_program,
                token_program,
            )
            .invoke()
        } else {
            validate_token_account(
                view,
                mint.to_account_view().address(),
                wallet.to_account_view().address(),
            )
        }
    }
}

impl InitAssociatedToken for Initialize<AssociatedToken> {}

// ---------------------------------------------------------------------------
// validate_ata — standalone validation
// ---------------------------------------------------------------------------

/// Validate that an account is the correct ATA for a wallet and mint.
///
/// 1. Derives the expected ATA address from wallet + mint + token_program.
/// 2. Checks the derived address matches the account address.
/// 3. Validates the token account data (mint + authority).
///
/// Use this for custom validation outside the derive macro system.
#[inline(always)]
pub fn validate_ata(
    view: &AccountView,
    wallet: &Address,
    mint: &Address,
    token_program: &Address,
) -> Result<(), ProgramError> {
    let (expected, _) = get_associated_token_address_with_program(wallet, mint, token_program);
    if *view.address() != expected {
        return Err(ProgramError::InvalidSeeds);
    }
    validate_token_account(view, mint, wallet)
}
