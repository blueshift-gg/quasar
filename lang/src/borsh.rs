//! Borsh-compatible serialization primitives for CPI instruction data.
//!
//! [`CpiEncode`] writes values in length-prefixed wire format directly into
//! a pre-allocated buffer. Designed for stack-allocated CPI data arrays —
//! no heap, no alloc.

// ---------------------------------------------------------------------------
// Codec-aware CPI encoding
// ---------------------------------------------------------------------------

/// Write a value into a CPI buffer with a specific prefix size.
///
/// The `TARGET_PREFIX` const generic determines the wire format:
/// - `1` → u8 prefix
/// - `2` → u16 LE prefix
/// - `4` → u32 LE prefix (Borsh-compatible)
///
/// Implementations exist for:
/// - `&str` / `&[u8]` → always encode from scratch
pub trait CpiEncode<const TARGET_PREFIX: usize> {
    /// Bytes needed in the CPI buffer for this value.
    fn encoded_len(&self) -> usize;

    /// Write this value into the CPI buffer at the given offset.
    /// Returns the new offset after writing.
    ///
    /// # Safety
    ///
    /// Caller must ensure `ptr.add(offset)..ptr.add(offset +
    /// self.encoded_len())` is valid for writes.
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize;
}

/// Marker for CPI arguments that encode to Borsh string/bytes layout
/// (little-endian `u32` length prefix).
pub trait BorshCpiEncode: CpiEncode<4> {}

impl<T: CpiEncode<4>> BorshCpiEncode for T {}

/// Write a length/count value as a little-endian prefix of the given size.
///
/// # Safety
///
/// Caller must ensure `ptr.add(offset)..ptr.add(offset + PREFIX_BYTES)` is
/// valid.
#[inline(always)]
unsafe fn write_prefix<const PREFIX_BYTES: usize>(ptr: *mut u8, offset: usize, value: u32) {
    const {
        assert!(PREFIX_BYTES == 1 || PREFIX_BYTES == 2 || PREFIX_BYTES == 4);
    }
    match PREFIX_BYTES {
        1 => {
            *ptr.add(offset) = value as u8;
        }
        2 => {
            let le = (value as u16).to_le_bytes();
            core::ptr::copy_nonoverlapping(le.as_ptr(), ptr.add(offset), 2);
        }
        4 => {
            let le = value.to_le_bytes();
            core::ptr::copy_nonoverlapping(le.as_ptr(), ptr.add(offset), 4);
        }
        _ => core::hint::unreachable_unchecked(),
    }
}

// &str → any target prefix
impl<const T: usize> CpiEncode<T> for &str {
    #[inline(always)]
    fn encoded_len(&self) -> usize {
        const {
            assert!(T == 1 || T == 2 || T == 4);
        }
        T + self.len()
    }

    #[inline(always)]
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize {
        write_prefix::<T>(ptr, offset, self.len() as u32);
        core::ptr::copy_nonoverlapping(self.as_ptr(), ptr.add(offset + T), self.len());
        offset + T + self.len()
    }
}

// &[u8] → any target prefix (for raw byte strings)
impl<const T: usize> CpiEncode<T> for &[u8] {
    #[inline(always)]
    fn encoded_len(&self) -> usize {
        const {
            assert!(T == 1 || T == 2 || T == 4);
        }
        T + self.len()
    }

    #[inline(always)]
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize {
        write_prefix::<T>(ptr, offset, self.len() as u32);
        core::ptr::copy_nonoverlapping(self.as_ptr(), ptr.add(offset + T), self.len());
        offset + T + self.len()
    }
}
