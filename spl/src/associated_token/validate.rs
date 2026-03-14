use {
    super::address::get_associated_token_address_with_program,
    crate::helpers::init::validate_token_account, quasar_core::prelude::*,
};

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
    if !quasar_core::keys_eq(view.address(), &expected) {
        return Err(ProgramError::InvalidSeeds);
    }
    validate_token_account(view, mint, wallet)
}
