use {
    crate::helpers::constants::{ATA_PROGRAM_ID, SPL_TOKEN_ID},
    quasar_core::prelude::*,
};

/// Derive the associated token account address for a wallet and mint.
///
/// Uses the SPL Token program as the token program. Returns `(address, bump)`.
///
/// On BPF, uses `sol_sha256` + `sol_curve_validate_point` (~544 CU) instead of
/// `find_program_address` syscall (~1,500 CU).
/// Off-chain, use [`get_associated_token_address_const`] instead.
#[inline(always)]
pub fn get_associated_token_address(wallet: &Address, mint: &Address) -> (Address, u8) {
    get_associated_token_address_with_program(wallet, mint, &SPL_TOKEN_ID)
}

/// Derive the associated token account address for a wallet, mint, and token
/// program.
///
/// Returns `(address, bump)`.
///
/// On BPF, uses `sol_sha256` + `sol_curve_validate_point` (~544 CU) instead of
/// `find_program_address` syscall (~1,500 CU).
/// Off-chain, use [`get_associated_token_address_with_program_const`] instead.
#[inline(always)]
pub fn get_associated_token_address_with_program(
    wallet: &Address,
    mint: &Address,
    token_program: &Address,
) -> (Address, u8) {
    match quasar_core::pda::based_try_find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ATA_PROGRAM_ID,
    ) {
        Ok(result) => result,
        Err(_) => panic!("ATA address derivation failed"),
    }
}

/// Const-compatible ATA address derivation (works off-chain and in const
/// contexts).
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
