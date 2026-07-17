//! Shared instruction-data encoders for fixed-layout Metaplex CPIs.
//!
//! Helpers that emit a discriminator byte followed by a `u64` or an
//! `Option<u64>`, reused by the individual instruction builders.

// Metaplex-enforced maximum field lengths.
pub(super) const MAX_NAME_LEN: usize = 32;
pub(super) const MAX_SYMBOL_LEN: usize = 10;
pub(super) const MAX_URI_LEN: usize = 200;

#[inline(always)]
pub(super) fn u64_data<const DISCRIMINATOR: u8>(value: u64) -> [u8; 9] {
    let mut data = [0u8; 9];
    data[0] = DISCRIMINATOR;
    data[1..].copy_from_slice(&value.to_le_bytes());
    data
}

#[inline(always)]
pub(super) fn option_u64_data<const DISCRIMINATOR: u8>(value: Option<u64>) -> [u8; 10] {
    let mut data = [0u8; 10];
    data[0] = DISCRIMINATOR;
    if let Some(value) = value {
        data[1] = 1;
        data[2..].copy_from_slice(&value.to_le_bytes());
    }
    data
}
