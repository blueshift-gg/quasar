use core::mem::MaybeUninit;

use crate::error::ZeroPodError;

/// Returns the maximum `N` value representable by a `PFX`-byte length prefix.
///
/// Returns `0` for invalid `PFX` values, which causes `_CAP_CHECK` to fire.
pub(crate) const fn max_n_for_pfx(pfx: usize) -> usize {
    match pfx {
        1 => u8::MAX as usize,
        2 => u16::MAX as usize,
        4 => u32::MAX as usize,
        8 => usize::MAX,
        _ => 0,
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodString<const N: usize, const PFX: usize = 1> {
    len: [u8; PFX],
    data: [MaybeUninit<u8>; N],
}

// Compile-time: PFX must be in {1,2,4,8} and N must fit in the prefix.
impl<const N: usize, const PFX: usize> PodString<N, PFX> {
    const _CAP_CHECK: () = {
        assert!(
            PFX == 1 || PFX == 2 || PFX == 4 || PFX == 8,
            "PodString<N, PFX>: PFX must be 1, 2, 4, or 8"
        );
        assert!(
            N <= max_n_for_pfx(PFX),
            "PodString<N, PFX>: N exceeds the maximum value representable by the PFX-byte length \
             prefix"
        );
    };

    pub const VALID: () = Self::_CAP_CHECK;
}

// Compile-time layout invariants — PFX=1 (default, backward-compat).
const _: () = assert!(core::mem::size_of::<PodString<0>>() == 1);
const _: () = assert!(core::mem::size_of::<PodString<1>>() == 2);
const _: () = assert!(core::mem::size_of::<PodString<32>>() == 33);
const _: () = assert!(core::mem::size_of::<PodString<255>>() == 256);
const _: () = assert!(core::mem::align_of::<PodString<0>>() == 1);
const _: () = assert!(core::mem::align_of::<PodString<32>>() == 1);
const _: () = assert!(core::mem::align_of::<PodString<255>>() == 1);
// Compile-time layout invariants — PFX=2.
const _: () = assert!(core::mem::size_of::<PodString<0, 2>>() == 2);
const _: () = assert!(core::mem::size_of::<PodString<100, 2>>() == 102);
const _: () = assert!(core::mem::align_of::<PodString<0, 2>>() == 1);
// Compile-time layout invariants — PFX=4.
const _: () = assert!(core::mem::size_of::<PodString<0, 4>>() == 4);
const _: () = assert!(core::mem::size_of::<PodString<100, 4>>() == 104);
const _: () = assert!(core::mem::align_of::<PodString<0, 4>>() == 1);
// Compile-time layout invariants — PFX=8.
const _: () = assert!(core::mem::size_of::<PodString<0, 8>>() == 8);
const _: () = assert!(core::mem::align_of::<PodString<0, 8>>() == 1);

impl<const N: usize, const PFX: usize> PodString<N, PFX> {
    #[inline(always)]
    pub fn decode_len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        let mut buf = [0u8; 8];
        buf[..PFX].copy_from_slice(&self.len);
        u64::from_le_bytes(buf) as usize
    }

    #[inline(always)]
    fn encode_len(&mut self, n: usize) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        let bytes = (n as u64).to_le_bytes();
        self.len.copy_from_slice(&bytes[..PFX]);
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        self.decode_len().min(N)
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.decode_len() == 0
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        let len = self.len();
        unsafe {
            let bytes = core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len);
            core::str::from_utf8_unchecked(bytes)
        }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.len();
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) }
    }

    pub fn try_set(&mut self, value: &str) -> Result<(), ZeroPodError> {
        let vlen = value.len();
        if vlen > N {
            return Err(ZeroPodError::Overflow);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(value.as_ptr(), self.data.as_mut_ptr() as *mut u8, vlen);
        }
        self.encode_len(vlen);
        Ok(())
    }

    pub fn try_push_str(&mut self, value: &str) -> Result<(), ZeroPodError> {
        let cur = self.len();
        let vlen = value.len();
        let new_len = cur + vlen;
        if new_len > N {
            return Err(ZeroPodError::Overflow);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                value.as_ptr(),
                (self.data.as_mut_ptr() as *mut u8).add(cur),
                vlen,
            );
        }
        self.encode_len(new_len);
        Ok(())
    }

    #[inline(always)]
    pub fn chars(&self) -> core::str::Chars<'_> {
        self.as_str().chars()
    }

    #[inline(always)]
    pub fn bytes(&self) -> core::str::Bytes<'_> {
        self.as_str().bytes()
    }

    #[must_use = "returns false if value exceeds capacity — unhandled means the write was silently \
                  skipped"]
    #[inline(always)]
    pub fn set(&mut self, value: &str) -> bool {
        self.try_set(value).is_ok()
    }

    #[must_use = "returns false if appending would exceed capacity — unhandled means the append \
                  was silently skipped"]
    #[inline(always)]
    pub fn push_str(&mut self, value: &str) -> bool {
        self.try_push_str(value).is_ok()
    }

    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len >= self.len() {
            return;
        }
        let s = self.as_str();
        let mut boundary = new_len;
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        self.encode_len(boundary);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = [0u8; PFX];
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        debug_assert!(
            bytes.len() >= PFX,
            "load_from_bytes: slice must have at least PFX bytes"
        );
        let mut buf = [0u8; 8];
        buf[..PFX].copy_from_slice(&bytes[..PFX]);
        let slen = (u64::from_le_bytes(buf) as usize).min(N);
        debug_assert!(
            bytes.len() >= PFX + slen,
            "load_from_bytes: slice too short for encoded length"
        );
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes[PFX..].as_ptr(),
                self.data.as_mut_ptr() as *mut u8,
                slen,
            );
        }
        self.encode_len(slen);
        PFX + slen
    }

    #[inline(always)]
    pub fn write_to_bytes(&self, dest: &mut [u8]) -> usize {
        let slen = self.len();
        debug_assert!(
            dest.len() >= PFX + slen,
            "write_to_bytes: dest too short for encoded length"
        );
        dest[..PFX].copy_from_slice(&(slen as u64).to_le_bytes()[..PFX]);
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.data.as_ptr() as *const u8,
                dest[PFX..].as_mut_ptr(),
                slen,
            );
        }
        PFX + slen
    }

    #[inline(always)]
    pub fn serialized_len(&self) -> usize {
        PFX + self.len()
    }
}

impl<const N: usize, const PFX: usize> Default for PodString<N, PFX> {
    fn default() -> Self {
        Self {
            len: [0u8; PFX],
            data: [MaybeUninit::uninit(); N],
        }
    }
}

impl<const N: usize, const PFX: usize> core::ops::Deref for PodString<N, PFX> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize, const PFX: usize> AsRef<str> for PodString<N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize, const PFX: usize> AsRef<[u8]> for PodString<N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const N: usize, const PFX: usize> PartialEq for PodString<N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize, const PFX: usize> Eq for PodString<N, PFX> {}

impl<const N: usize, const PFX: usize> PartialEq<str> for PodString<N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize, const PFX: usize> PartialEq<&str> for PodString<N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<const N: usize, const PFX: usize> core::fmt::Debug for PodString<N, PFX> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize, const PFX: usize> core::fmt::Display for PodString<N, PFX> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<const N: usize, const PFX: usize> core::hash::Hash for PodString<N, PFX> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}
