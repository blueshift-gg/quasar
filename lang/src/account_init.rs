use {
    crate::{
        cpi::{system, Signer},
        sysvars::rent::Rent,
    },
    solana_account_view::AccountView,
    solana_address::Address,
    solana_program_error::ProgramResult,
};

/// Create account via system program + write discriminator.
#[inline(always)]
pub fn init_account(
    payer: &AccountView,
    account: &mut AccountView,
    space: u64,
    owner: &Address,
    signers: &[Signer],
    rent: &Rent,
    discriminator: &[u8],
) -> ProgramResult {
    system::init_account_with_rent(payer, account, space, owner, signers, rent)?;
    system::write_discriminator(account, discriminator)?;
    Ok(())
}
