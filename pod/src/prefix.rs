//! Shared length-prefix encoding/decoding for `PodString` and `PodVec`.
//!
//! Both types store a `PFX`-byte little-endian length prefix. This module
//! extracts the common encode/decode logic so each type doesn't duplicate it.

/// Returns the maximum `N` value representable by a `PFX`-byte length prefix.
///
/// Returns `0` for invalid `PFX` values, which causes compile-time cap checks
/// to fire.
pub(crate) const fn max_n_for_pfx(pfx: usize) -> usize {
    match pfx {
        1 => u8::MAX as usize,
        2 => u16::MAX as usize,
        4 => u32::MAX as usize,
        8 => usize::MAX,
        _ => 0,
    }
}

/// Decode a `PFX`-byte little-endian length prefix into a `usize`.
///
/// LLVM constant-folds this per monomorphization (e.g., for PFX=1 it
/// compiles to a single byte load).
#[inline(always)]
pub(crate) fn decode_prefix_len<const PFX: usize>(len: &[u8; PFX]) -> usize {
    let mut buf = [0u8; 8];
    buf[..PFX].copy_from_slice(len);
    u64::from_le_bytes(buf) as usize
}

/// Encode `n` as a `PFX`-byte little-endian prefix into the given buffer.
#[inline(always)]
pub(crate) fn encode_prefix_len<const PFX: usize>(len: &mut [u8; PFX], n: usize) {
    let bytes = (n as u64).to_le_bytes();
    len.copy_from_slice(&bytes[..PFX]);
}

/// Compile-time assertion that `PFX` is valid and `N` fits within the prefix.
///
/// Used by both `PodString` and `PodVec` in their `_CAP_CHECK` constants.
macro_rules! cap_check {
    ($type_name:expr, $N:expr, $PFX:expr) => {{
        assert!(
            $PFX == 1 || $PFX == 2 || $PFX == 4 || $PFX == 8,
            concat!($type_name, ": PFX must be 1, 2, 4, or 8")
        );
        assert!(
            $N <= $crate::prefix::max_n_for_pfx($PFX),
            concat!(
                $type_name,
                ": N exceeds the maximum value representable by the PFX-byte length prefix"
            )
        );
    }};
}

pub(crate) use cap_check;
