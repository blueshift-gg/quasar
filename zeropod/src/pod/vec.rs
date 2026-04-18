use {
    super::string::max_n_for_pfx,
    crate::{error::ZeroPodError, traits::ZcElem},
    core::mem::MaybeUninit,
};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodVec<T: ZcElem, const N: usize, const PFX: usize = 2> {
    len: [u8; PFX],
    data: [MaybeUninit<T>; N],
}

// Compile-time layout invariants — PFX=2 (default, backward-compat).
const _: () = assert!(core::mem::size_of::<PodVec<u8, 10>>() == 2 + 10);
const _: () = assert!(core::mem::align_of::<PodVec<u8, 10>>() == 1);
const _: () = assert!(core::mem::size_of::<PodVec<[u8; 32], 10>>() == 2 + 320);
const _: () = assert!(core::mem::align_of::<PodVec<[u8; 32], 10>>() == 1);
// Compile-time layout invariants — PFX=1.
const _: () = assert!(core::mem::size_of::<PodVec<u8, 10, 1>>() == 1 + 10);
const _: () = assert!(core::mem::align_of::<PodVec<u8, 10, 1>>() == 1);
// Compile-time layout invariants — PFX=4.
const _: () = assert!(core::mem::size_of::<PodVec<u8, 10, 4>>() == 4 + 10);
const _: () = assert!(core::mem::align_of::<PodVec<u8, 10, 4>>() == 1);

impl<T: ZcElem, const N: usize, const PFX: usize> PodVec<T, N, PFX> {
    const _CAP_CHECK: () = {
        assert!(
            PFX == 1 || PFX == 2 || PFX == 4 || PFX == 8,
            "PodVec<T, N, PFX>: PFX must be 1, 2, 4, or 8"
        );
        assert!(
            N <= max_n_for_pfx(PFX),
            "PodVec<T, N, PFX>: N exceeds the maximum value representable by the PFX-byte length \
             prefix"
        );
    };

    pub const VALID: () = Self::_CAP_CHECK;

    #[inline(always)]
    pub fn decode_len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        match PFX {
            1 => self.len[0] as usize,
            2 => u16::from_le_bytes([self.len[0], self.len[1]]) as usize,
            _ => {
                let mut buf = [0u8; 8];
                buf[..PFX].copy_from_slice(&self.len);
                u64::from_le_bytes(buf) as usize
            }
        }
    }

    #[inline(always)]
    fn encode_len(&mut self, n: usize) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        match PFX {
            1 => self.len[0] = n as u8,
            2 => {
                let bytes = (n as u16).to_le_bytes();
                self.len[0] = bytes[0];
                self.len[1] = bytes[1];
            }
            _ => {
                let bytes = (n as u64).to_le_bytes();
                self.len.copy_from_slice(&bytes[..PFX]);
            }
        }
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
    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        // SAFETY: data[..len] written by push/set methods. MaybeUninit<T> and T have
        // identical layout. len clamped to N.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const T, len) }
    }

    #[inline(always)]
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        let len = self.len();
        unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, len) }
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.as_slice_mut().get_mut(index)
    }

    #[inline(always)]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_slice_mut().iter_mut()
    }

    pub fn try_push(&mut self, value: T) -> Result<(), ZeroPodError> {
        let cur = self.len();
        if cur >= N {
            return Err(ZeroPodError::Overflow);
        }
        self.data[cur] = MaybeUninit::new(value);
        self.encode_len(cur + 1);
        Ok(())
    }

    pub fn try_set_from_slice(&mut self, values: &[T]) -> Result<(), ZeroPodError> {
        let vlen = values.len();
        if vlen > N {
            return Err(ZeroPodError::Overflow);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(values.as_ptr(), self.data.as_mut_ptr() as *mut T, vlen);
        }
        self.encode_len(vlen);
        Ok(())
    }

    pub fn try_extend_from_slice(&mut self, values: &[T]) -> Result<(), ZeroPodError> {
        let cur = self.len();
        let new_len = cur + values.len();
        if new_len > N {
            return Err(ZeroPodError::Overflow);
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr(),
                (self.data.as_mut_ptr() as *mut T).add(cur),
                values.len(),
            );
        }
        self.encode_len(new_len);
        Ok(())
    }

    #[must_use = "returns false if values.len() exceeds capacity — unhandled means the write was \
                  silently skipped"]
    #[inline(always)]
    pub fn set_from_slice(&mut self, values: &[T]) -> bool {
        self.try_set_from_slice(values).is_ok()
    }

    #[must_use = "returns false if capacity is exceeded — unhandled means the push was silently \
                  skipped"]
    #[inline(always)]
    pub fn push(&mut self, value: T) -> bool {
        self.try_push(value).is_ok()
    }

    #[must_use = "returns None if the vector is empty"]
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        let cur = self.len();
        if cur == 0 {
            return None;
        }
        let new_len = cur - 1;
        let val = unsafe { self.data[new_len].assume_init() };
        self.encode_len(new_len);
        Some(val)
    }

    #[must_use = "returns None if index is out of bounds"]
    #[inline(always)]
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        let cur = self.len();
        if index >= cur {
            return None;
        }
        let last = cur - 1;
        let removed = unsafe { self.data[index].assume_init() };
        if index != last {
            self.data[index] = self.data[last];
        }
        self.encode_len(last);
        Some(removed)
    }

    #[must_use = "returns None if index is out of bounds"]
    #[inline(always)]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let cur = self.len();
        if index >= cur {
            return None;
        }
        let removed = unsafe { self.data[index].assume_init() };
        let tail = cur - index - 1;
        if tail > 0 {
            unsafe {
                core::ptr::copy(
                    self.data.as_ptr().add(index + 1),
                    self.data.as_mut_ptr().add(index),
                    tail,
                );
            }
        }
        self.encode_len(cur - 1);
        Some(removed)
    }

    #[must_use = "returns false if there is insufficient remaining capacity — unhandled means the \
                  append was silently skipped"]
    #[inline(always)]
    pub fn extend_from_slice(&mut self, values: &[T]) -> bool {
        self.try_extend_from_slice(values).is_ok()
    }

    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        let cur = self.len();
        if new_len < cur {
            self.encode_len(new_len);
        }
    }

    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        let mut write = 0;
        let cur = self.len();
        for read in 0..cur {
            let val = unsafe { self.data[read].assume_init() };
            if f(&val) {
                self.data[write] = MaybeUninit::new(val);
                write += 1;
            }
        }
        self.encode_len(write);
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = [0u8; PFX];
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> Default for PodVec<T, N, PFX> {
    fn default() -> Self {
        Self {
            len: [0u8; PFX],
            data: [MaybeUninit::uninit(); N],
        }
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> core::ops::Deref for PodVec<T, N, PFX> {
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> core::ops::DerefMut for PodVec<T, N, PFX> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> AsRef<[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: ZcElem, const N: usize, const PFX: usize> AsMut<[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: ZcElem + PartialEq, const N: usize, const PFX: usize> PartialEq for PodVec<T, N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: ZcElem + PartialEq, const N: usize, const PFX: usize> PartialEq<[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &[T]) -> bool {
        self.as_slice() == other
    }
}

impl<T: ZcElem + PartialEq, const N: usize, const PFX: usize> PartialEq<&[T]>
    for PodVec<T, N, PFX>
{
    #[inline(always)]
    fn eq(&self, other: &&[T]) -> bool {
        self.as_slice() == *other
    }
}

impl<T: ZcElem + Eq, const N: usize, const PFX: usize> Eq for PodVec<T, N, PFX> {}

impl<T: ZcElem + core::fmt::Debug, const N: usize, const PFX: usize> core::fmt::Debug
    for PodVec<T, N, PFX>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl<T: ZcElem + core::hash::Hash, const N: usize, const PFX: usize> core::hash::Hash
    for PodVec<T, N, PFX>
{
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn encode_decode_roundtrip_pfx1() {
        let n: usize = kani::any();
        kani::assume(n <= u8::MAX as usize);
        let mut v = PodVec::<u8, 255, 1>::default();
        v.encode_len(n);
        assert!(v.decode_len() == n);
    }

    #[kani::proof]
    fn encode_decode_roundtrip_pfx2() {
        let n: usize = kani::any();
        kani::assume(n <= u16::MAX as usize);
        let mut v = PodVec::<u8, 255, 2>::default();
        v.encode_len(n);
        assert!(v.decode_len() == n);
    }

    #[kani::proof]
    fn encode_decode_roundtrip_pfx4() {
        let n: usize = kani::any();
        kani::assume(n <= u32::MAX as usize);
        let mut v = PodVec::<u8, 255, 4>::default();
        v.encode_len(n);
        assert!(v.decode_len() == n);
    }

    #[kani::proof]
    fn len_clamp_pfx2() {
        let raw: [u8; 2] = kani::any();
        let v = PodVec::<u8, 8, 2> {
            len: raw,
            data: [MaybeUninit::uninit(); 8],
        };
        assert!(v.len() <= 8);
    }

    #[kani::proof]
    fn len_clamp_pfx1() {
        let raw: [u8; 1] = kani::any();
        let v = PodVec::<u8, 8, 1> {
            len: raw,
            data: [MaybeUninit::uninit(); 8],
        };
        assert!(v.len() <= 8);
    }

    #[kani::proof]
    fn push_pop_roundtrip() {
        let val: u8 = kani::any();
        let mut v = PodVec::<u8, 4, 1>::default();
        assert!(v.push(val));
        assert!(v.len() == 1);
        assert!(v.pop() == Some(val));
        assert!(v.is_empty());
    }

    #[kani::proof]
    fn push_full_rejects() {
        let mut v = PodVec::<u8, 2, 1>::default();
        assert!(v.push(1));
        assert!(v.push(2));
        assert!(!v.push(3));
        assert!(v.len() == 2);
    }

    #[kani::proof]
    fn push_pop_lifo() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        let mut v = PodVec::<u8, 4, 1>::default();
        assert!(v.push(a));
        assert!(v.push(b));
        assert!(v.pop() == Some(b));
        assert!(v.pop() == Some(a));
    }

    #[kani::proof]
    fn swap_remove_correctness() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        let c: u8 = kani::any();
        let mut v = PodVec::<u8, 4, 1>::default();
        assert!(v.push(a));
        assert!(v.push(b));
        assert!(v.push(c));
        assert!(v.swap_remove(0) == Some(a));
        assert!(v.len() == 2);
        assert!(v.as_slice()[0] == c);
        assert!(v.as_slice()[1] == b);
    }

    #[kani::proof]
    fn swap_remove_oob() {
        let idx: usize = kani::any();
        let mut v = PodVec::<u8, 4, 1>::default();
        assert!(v.push(1));
        assert!(v.push(2));
        kani::assume(idx >= 2);
        kani::assume(idx <= 8);
        assert!(v.swap_remove(idx).is_none());
        assert!(v.len() == 2);
    }

    #[kani::proof]
    fn set_from_slice_rejects_over_capacity() {
        let count: usize = kani::any();
        kani::assume(count > 4);
        kani::assume(count <= 8);
        let data = [0u8; 8];
        let mut v = PodVec::<u8, 4, 1>::default();
        assert!(!v.set_from_slice(&data[..count]));
        assert!(v.is_empty());
    }
}
