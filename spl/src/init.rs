use {
    crate::{instructions, state},
    quasar_lang::{cpi::Signer, prelude::*, sysvars::rent::Rent},
};

/// Create token account + initialize_account3.
#[inline(always)]
pub fn init_token_account(
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
        state::TokenAccountState::LEN as u64,
        token_program.address(),
        signers,
        rent,
    )?;
    instructions::initialize_account3(token_program, account, mint, authority).invoke()
}

/// Create mint account + initialize_mint2.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub fn init_mint_account(
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
        state::MintAccountState::LEN as u64,
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
pub fn init_ata(
    ata_program: &AccountView,
    payer: &AccountView,
    ata: &AccountView,
    wallet: &AccountView,
    mint: &AccountView,
    system_program: &AccountView,
    token_program: &AccountView,
    idempotent: bool,
) -> Result<(), ProgramError> {
    let instruction_byte: u8 = if idempotent { 1 } else { 0 };
    quasar_lang::cpi::CpiCall::new(
        ata_program.address(),
        [
            quasar_lang::cpi::InstructionAccount::writable_signer(payer.address()),
            quasar_lang::cpi::InstructionAccount::writable(ata.address()),
            quasar_lang::cpi::InstructionAccount::readonly(wallet.address()),
            quasar_lang::cpi::InstructionAccount::readonly(mint.address()),
            quasar_lang::cpi::InstructionAccount::readonly(system_program.address()),
            quasar_lang::cpi::InstructionAccount::readonly(token_program.address()),
        ],
        [payer, ata, wallet, mint, system_program, token_program],
        [instruction_byte],
    )
    .invoke()
}
