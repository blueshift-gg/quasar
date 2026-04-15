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
