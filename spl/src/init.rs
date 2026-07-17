//! Account-creation CPI helpers for `#[account(init)]`.
//!
//! Allocate and initialize token accounts, mints, and ATAs: a system-program
//! `create_account` paired with the matching SPL initialize instruction, or the
//! ATA program's create.

use {
    crate::{associated_token, instructions},
    quasar_lang::{
        cpi::{CpiCall, InstructionAccount, Signer},
        prelude::*,
        sysvars::rent::Rent,
    },
};

/// Create token account + initialize_account3.
#[inline(always)]
pub(crate) fn init_token_account(
    payer: &AccountView,
    account: &mut AccountView,
    token_program: &AccountView,
    mint: &AccountView,
    authority: &Address,
    signers: &[Signer],
    rent: &Rent,
) -> Result<(), ProgramError> {
    quasar_lang::cpi::system::init_account_with_rent(
        payer,
        account,
        core::mem::size_of::<crate::token::TokenDataZc>() as u64,
        token_program.address(),
        signers,
        rent,
    )?;
    instructions::initialize_account3(token_program, account, mint, authority).invoke()
}

/// Create mint account + initialize_mint2.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn init_mint_account(
    payer: &AccountView,
    account: &mut AccountView,
    token_program: &AccountView,
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
    signers: &[Signer],
    rent: &Rent,
) -> Result<(), ProgramError> {
    quasar_lang::cpi::system::init_account_with_rent(
        payer,
        account,
        core::mem::size_of::<crate::token::MintDataZc>() as u64,
        token_program.address(),
        signers,
        rent,
    )?;
    instructions::initialize_mint2(
        token_program,
        account,
        decimals,
        mint_authority,
        freeze_authority,
    )
    .invoke()
}

/// Create an ATA via the ATA program. Uses `CreateIdempotent` when `idempotent`
/// is true.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn init_ata(
    ata_program: &AccountView,
    payer: &AccountView,
    ata: &AccountView,
    wallet: &AccountView,
    mint: &AccountView,
    system_program: &AccountView,
    token_program: &AccountView,
    idempotent: bool,
) -> Result<(), ProgramError> {
    let instruction_byte = if idempotent {
        associated_token::ATA_CREATE_IDEMPOTENT
    } else {
        associated_token::ATA_CREATE
    };
    CpiCall::new(
        ata_program.address(),
        [
            InstructionAccount::writable_signer(payer.address()),
            InstructionAccount::writable(ata.address()),
            InstructionAccount::readonly(wallet.address()),
            InstructionAccount::readonly(mint.address()),
            InstructionAccount::readonly(system_program.address()),
            InstructionAccount::readonly(token_program.address()),
        ],
        [payer, ata, wallet, mint, system_program, token_program],
        [instruction_byte],
    )
    .invoke()
}
