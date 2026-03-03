// Borsh-compatible serialization primitives for CPI instruction data.
//
// These types wrap raw byte slices and write them in Borsh wire format
// (u32 LE length prefix + payload) directly into a pre-allocated buffer.
// Designed for stack-allocated CPI data arrays — no heap, no alloc.

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

/// A Borsh vector: u32 LE element count followed by pre-serialized element bytes.
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
    /// Caller must ensure `ptr.add(offset)..ptr.add(offset + 4 + self.bytes.len())`
    /// is valid for writes.
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
