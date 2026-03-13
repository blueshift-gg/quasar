use {
    super::AssociatedTokenProgram,
    crate::instructions::TokenCpi,
    quasar_core::{
        cpi::{CpiCall, InstructionAccount},
        prelude::*,
    },
};

// ATA program instruction discriminators.
const ATA_CREATE: u8 = 0;
const ATA_CREATE_IDEMPOTENT: u8 = 1;

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
