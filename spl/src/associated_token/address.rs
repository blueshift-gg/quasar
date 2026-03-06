use quasar_core::cpi::Seed;
use quasar_core::prelude::*;

use crate::helpers::constants::{ATA_PROGRAM_ID, SPL_TOKEN_ID};

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
