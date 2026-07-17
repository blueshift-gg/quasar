//! Instruction-data encoders shared by the SPL Token CPI builders.

use quasar_lang::prelude::Address;

#[inline(always)]
pub(super) fn amount_data<const DISCRIMINATOR: u8>(amount: u64) -> [u8; 9] {
    let mut data = [0u8; 9];
    data[0] = DISCRIMINATOR;
    data[1..].copy_from_slice(&amount.to_le_bytes());
    data
}

#[inline(always)]
pub(super) fn checked_amount_data<const DISCRIMINATOR: u8>(amount: u64, decimals: u8) -> [u8; 10] {
    let mut data = [0u8; 10];
    data[0] = DISCRIMINATOR;
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;
    data
}

#[inline(always)]
pub(super) fn initialize_account3_data(owner: &Address) -> [u8; 33] {
    let mut data = [0u8; 33];
    data[0] = 18;
    data[1..].copy_from_slice(owner.as_ref());
    data
}

#[inline(always)]
pub(super) fn initialize_mint2_data(
    decimals: u8,
    mint_authority: &Address,
    freeze_authority: Option<&Address>,
) -> [u8; 67] {
    let mut data = [0u8; 67];
    data[0] = 20;
    data[1] = decimals;
    data[2..34].copy_from_slice(mint_authority.as_ref());
    if let Some(freeze_authority) = freeze_authority {
        data[34] = 1;
        data[35..].copy_from_slice(freeze_authority.as_ref());
    }
    data
}
