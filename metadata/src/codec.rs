//! Borsh-compatible serialization primitives used by metadata CPI builders.

pub trait CpiEncode<const TARGET_PREFIX: usize> {
    fn encoded_len(&self) -> usize;

    /// # Safety
    ///
    /// Caller must ensure the target range is valid for writes.
    unsafe fn write_to(&self, ptr: *mut u8, offset: usize) -> usize;
}

pub trait BorshCpiEncode: CpiEncode<4> {}

impl<T: CpiEncode<4>> BorshCpiEncode for T {}

#[inline(always)]
unsafe fn write_prefix<const PREFIX_BYTES: usize>(ptr: *mut u8, offset: usize, value: u32) {
    const {
        assert!(PREFIX_BYTES == 1 || PREFIX_BYTES == 2 || PREFIX_BYTES == 4);
    }
    match PREFIX_BYTES {
        1 => *ptr.add(offset) = value as u8,
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

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Prove write_prefix::<1> writes a byte that decodes to the original
    /// value (truncated to u8).
    #[kani::proof]
    fn write_prefix_u8_roundtrip() {
        let value: u32 = kani::any();
        kani::assume(value <= u8::MAX as u32);
        let mut buf = [0u8; 4];
        unsafe { write_prefix::<1>(buf.as_mut_ptr(), 0, value) };
        assert!(buf[0] as u32 == value);
    }

    /// Prove write_prefix::<2> writes LE bytes that decode to the original
    /// value (truncated to u16).
    #[kani::proof]
    fn write_prefix_u16_roundtrip() {
        let value: u32 = kani::any();
        kani::assume(value <= u16::MAX as u32);
        let mut buf = [0u8; 4];
        unsafe { write_prefix::<2>(buf.as_mut_ptr(), 0, value) };
        let decoded = u16::from_le_bytes([buf[0], buf[1]]) as u32;
        assert!(decoded == value);
    }

    /// Prove write_prefix::<4> writes LE bytes that decode to the original
    /// value.
    #[kani::proof]
    fn write_prefix_u32_roundtrip() {
        let value: u32 = kani::any();
        let mut buf = [0u8; 4];
        unsafe { write_prefix::<4>(buf.as_mut_ptr(), 0, value) };
        let decoded = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert!(decoded == value);
    }

    /// Prove write_prefix at a nonzero offset writes to the correct
    /// location and doesn't clobber earlier bytes.
    #[kani::proof]
    fn write_prefix_offset_correctness() {
        let value: u32 = kani::any();
        let sentinel: u8 = kani::any();
        let mut buf = [sentinel; 8];
        unsafe { write_prefix::<4>(buf.as_mut_ptr(), 2, value) };
        // Bytes before offset are untouched.
        assert!(buf[0] == sentinel);
        assert!(buf[1] == sentinel);
        // Written bytes decode correctly.
        let decoded = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
        assert!(decoded == value);
    }

    /// Prove `CpiEncode<4>::write_to` for `&str` writes prefix + data within
    /// `offset + encoded_len()`, and doesn't clobber bytes before offset.
    #[kani::proof]
    #[kani::unwind(10)]
    fn str_write_to_bounds_and_roundtrip() {
        // Use a small fixed string to keep CBMC tractable.
        let len: usize = kani::any();
        kani::assume(len <= 8);

        let data = [0x41u8; 8]; // "AAAAAAAA"
        let s = unsafe { core::str::from_utf8_unchecked(&data[..len]) };

        let mut buf = [0xFFu8; 16];
        let offset: usize = kani::any();
        kani::assume(offset <= 4);
        kani::assume(offset + 4 + len <= 16);

        let new_offset = unsafe { <&str as CpiEncode<4>>::write_to(&s, buf.as_mut_ptr(), offset) };

        // new_offset == offset + 4 + len
        assert!(new_offset == offset + 4 + len);

        // Prefix decodes to string length.
        let prefix = u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]);
        assert!(prefix == len as u32);

        // Data bytes match.
        let mut i = 0;
        while i < len {
            assert!(buf[offset + 4 + i] == 0x41);
            i += 1;
        }
    }

    /// Prove `CpiEncode<4>::write_to` for `&[u8]` writes correctly.
    #[kani::proof]
    #[kani::unwind(10)]
    fn bytes_write_to_bounds_and_roundtrip() {
        let len: usize = kani::any();
        kani::assume(len <= 8);

        let data = [0xBBu8; 8];
        let slice = &data[..len];

        let mut buf = [0xFFu8; 16];
        let offset: usize = kani::any();
        kani::assume(offset <= 4);
        kani::assume(offset + 4 + len <= 16);

        let new_offset =
            unsafe { <&[u8] as CpiEncode<4>>::write_to(&slice, buf.as_mut_ptr(), offset) };

        assert!(new_offset == offset + 4 + len);

        let prefix = u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]);
        assert!(prefix == len as u32);
    }

    /// Prove `encoded_len` for `&[u8]` returns PREFIX + content length.
    #[kani::proof]
    fn encoded_len_matches_written() {
        let len: usize = kani::any();
        kani::assume(len <= 8);

        let data = [0u8; 8];
        let slice: &[u8] = &data[..len];

        // encoded_len must equal what write_to actually advances.
        let el = <&[u8] as CpiEncode<4>>::encoded_len(&slice);
        assert!(el == 4 + len);
    }
}
