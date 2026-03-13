//! Borsh-compatible serialization primitives for CPI instruction data.
//!
//! These types wrap raw byte slices and write them in Borsh wire format
//! (u32 LE length prefix + payload) directly into a pre-allocated buffer.
//! Designed for stack-allocated CPI data arrays — no heap, no alloc.

/// A Borsh string: u32 LE length prefix followed by UTF-8 bytes.
///
/// Wraps a `&[u8]` and writes it in Borsh `String` format. Accepts raw
/// UTF-8 bytes from Quasar's zero-copy accessors or any `&str`.
pub struct BorshString<'a>(pub &'a [u8]);

impl<'a> BorshString<'a> {
    #[inline(always)]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    #[inline(always)]
    pub const fn from_str(s: &'a str) -> Self {
        Self(s.as_bytes())
    }

    /// Write this string in Borsh format at `ptr + offset`.
    /// Returns the offset after the last written byte.
    ///
    /// # Safety
    ///
    /// Caller must ensure `ptr.add(offset)..ptr.add(offset + 4 + self.0.len())`
    /// is valid for writes.
    #[inline(always)]
    pub unsafe fn write_to(self, ptr: *mut u8, offset: usize) -> usize {
        let len = self.0.len() as u32;
        core::ptr::copy_nonoverlapping(len.to_le_bytes().as_ptr(), ptr.add(offset), 4);
        core::ptr::copy_nonoverlapping(self.0.as_ptr(), ptr.add(offset + 4), self.0.len());
        offset + 4 + self.0.len()
    }

    /// Total bytes this value occupies when serialized.
    #[inline(always)]
    pub const fn serialized_len(&self) -> usize {
        4 + self.0.len()
    }
}

impl<'a> From<&'a [u8]> for BorshString<'a> {
    #[inline(always)]
    fn from(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
}

impl<'a> From<&'a str> for BorshString<'a> {
    #[inline(always)]
    fn from(s: &'a str) -> Self {
        Self(s.as_bytes())
    }
}

/// A Borsh vector: u32 LE element count followed by pre-serialized element
/// bytes.
///
/// The caller is responsible for ensuring the `bytes` slice contains exactly
/// `count` elements in their Borsh-serialized form (e.g., `#[repr(C)]` Pod
/// types whose memory layout matches the wire format).
pub struct BorshVec<'a> {
    bytes: &'a [u8],
    count: u32,
}

impl<'a> BorshVec<'a> {
    #[inline(always)]
    pub const fn new(bytes: &'a [u8], count: u32) -> Self {
        Self { bytes, count }
    }

    /// An empty Borsh vector (count = 0, no payload).
    #[inline(always)]
    pub const fn empty() -> Self {
        Self {
            bytes: &[],
            count: 0,
        }
    }

    /// Create a BorshVec from a typed slice of fixed-size elements.
    ///
    /// Reinterprets the slice as raw bytes. This is the conversion path
    /// for Quasar's `Vec<'a, T, N>` fields, which become `&'a [T]` at
    /// runtime where `T` is always `#[repr(C)]` alignment-1 Pod.
    ///
    /// # Safety
    ///
    /// `T` must be `#[repr(C)]` with alignment 1 and no padding.
    #[inline(always)]
    pub unsafe fn from_slice<T: Sized>(slice: &'a [T]) -> Self {
        Self {
            bytes: core::slice::from_raw_parts(
                slice.as_ptr() as *const u8,
                core::mem::size_of_val(slice),
            ),
            count: slice.len() as u32,
        }
    }

    /// Write this vector in Borsh format at `ptr + offset`.
    /// Returns the offset after the last written byte.
    ///
    /// # Safety
    ///
    /// Caller must ensure `ptr.add(offset)..ptr.add(offset + 4 +
    /// self.bytes.len())` is valid for writes.
    #[inline(always)]
    pub unsafe fn write_to(self, ptr: *mut u8, offset: usize) -> usize {
        core::ptr::copy_nonoverlapping(self.count.to_le_bytes().as_ptr(), ptr.add(offset), 4);
        core::ptr::copy_nonoverlapping(self.bytes.as_ptr(), ptr.add(offset + 4), self.bytes.len());
        offset + 4 + self.bytes.len()
    }

    /// Total bytes this value occupies when serialized.
    #[inline(always)]
    pub const fn serialized_len(&self) -> usize {
        4 + self.bytes.len()
    }
}

impl<'a> From<&'a [u8]> for BorshVec<'a> {
    #[inline(always)]
    fn from(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            count: bytes.len() as u32,
        }
    }
}

// ---------------------------------------------------------------------------
// Codec-aware CPI encoding
// ---------------------------------------------------------------------------

use crate::dynamic::RawEncoded;

/// Write a value into a CPI buffer with a specific prefix size.
///
/// The `TARGET_PREFIX` const generic determines the wire format:
/// - `1` → u8 prefix
/// - `2` → u16 LE prefix
/// - `4` → u32 LE prefix (Borsh-compatible)
///
/// Implementations exist for:
/// - `&str` / `&[u8]` → always encode from scratch
/// - `RawEncoded<N>` → memcpy if `N == TARGET_PREFIX`, re-encode otherwise
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

/// Write a length/count value as a little-endian prefix of the given size.
///
/// # Safety
///
/// Caller must ensure `ptr.add(offset)..ptr.add(offset + PREFIX_BYTES)` is
/// valid.
#[inline(always)]
unsafe fn write_prefix<const PREFIX_BYTES: usize>(ptr: *mut u8, offset: usize, value: u32) {
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
        _ => unreachable!(),
    }
}

// &str → any target prefix
impl<const T: usize> CpiEncode<T> for &str {
    #[inline(always)]
    fn encoded_len(&self) -> usize {
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
        T + self.len()
    }

    #[inline(always)]
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize {
        write_prefix::<T>(ptr, offset, self.len() as u32);
        core::ptr::copy_nonoverlapping(self.as_ptr(), ptr.add(offset + T), self.len());
        offset + T + self.len()
    }
}

// BorshString → u32 prefix (Borsh-compatible)
impl<'a> CpiEncode<4> for BorshString<'a> {
    #[inline(always)]
    fn encoded_len(&self) -> usize {
        4 + self.0.len()
    }

    #[inline(always)]
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize {
        let len = self.0.len() as u32;
        core::ptr::copy_nonoverlapping(len.to_le_bytes().as_ptr(), ptr.add(offset), 4);
        core::ptr::copy_nonoverlapping(self.0.as_ptr(), ptr.add(offset + 4), self.0.len());
        offset + 4 + self.0.len()
    }
}

// RawEncoded<N> → same prefix size N: zero-copy memcpy
impl<'a, const N: usize> CpiEncode<N> for RawEncoded<'a, N> {
    #[inline(always)]
    fn encoded_len(&self) -> usize {
        self.bytes.len()
    }

    #[inline(always)]
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize {
        core::ptr::copy_nonoverlapping(self.bytes.as_ptr(), ptr.add(offset), self.bytes.len());
        offset + self.bytes.len()
    }
}

/// Helper to encode a `RawEncoded` with a different target prefix size.
///
/// When source prefix size differs from target, this re-writes the prefix
/// while memcpy-ing the data. Call via `cpi_reencode::<TARGET>(&raw)`.
///
/// # Safety
///
/// Caller must ensure `ptr.add(offset)..ptr.add(offset + TARGET +
/// raw.data().len())` is valid for writes.
#[inline(always)]
pub unsafe fn cpi_reencode<const TARGET: usize, const SOURCE: usize>(
    raw: &RawEncoded<'_, SOURCE>,
    ptr: *mut u8,
    offset: usize,
) -> usize {
    let value = raw.prefix_value();
    let data = raw.data();
    write_prefix::<TARGET>(ptr, offset, value);
    core::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(offset + TARGET), data.len());
    offset + TARGET + data.len()
}
