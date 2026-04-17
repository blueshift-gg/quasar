use {super::string::max_n_for_pfx, core::mem::MaybeUninit, crate::error::ZeroPodError};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodVec<T: Copy, const N: usize, const PFX: usize = 2> {
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

impl<T: Copy, const N: usize, const PFX: usize> PodVec<T, N, PFX> {
    const _ALIGN_CHECK: () = assert!(
        core::mem::align_of::<T>() == 1,
        "PodVec<T, N, PFX>: T must have alignment 1. Use Pod types (PodU64, etc.) instead of \
         native integers."
    );

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

    #[allow(clippy::let_unit_value)]
    pub const VALID: () = {
        let _ = Self::_ALIGN_CHECK;
        let _ = Self::_CAP_CHECK;
    };

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
        let _ = Self::_ALIGN_CHECK;
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
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
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

    #[inline(always)]
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        debug_assert!(
            bytes.len() >= PFX,
            "load_from_bytes: slice must have at least PFX bytes"
        );
        let mut buf = [0u8; 8];
        buf[..PFX].copy_from_slice(&bytes[..PFX]);
        let count = (u64::from_le_bytes(buf) as usize).min(N);
        let data_bytes = count * core::mem::size_of::<T>();
        debug_assert!(
            bytes.len() >= PFX + data_bytes,
            "load_from_bytes: slice too short for encoded length"
        );
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes[PFX..].as_ptr(),
                self.data.as_mut_ptr() as *mut u8,
                data_bytes,
            );
        }
        self.encode_len(count);
        PFX + data_bytes
    }

    #[inline(always)]
    pub fn write_to_bytes(&self, dest: &mut [u8]) -> usize {
        let count = self.len();
        let data_bytes = count * core::mem::size_of::<T>();
        debug_assert!(
            dest.len() >= PFX + data_bytes,
            "write_to_bytes: dest too short for encoded length"
        );
        dest[..PFX].copy_from_slice(&(count as u64).to_le_bytes()[..PFX]);
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.data.as_ptr() as *const u8,
                dest[PFX..].as_mut_ptr(),
                data_bytes,
            );
        }
        PFX + data_bytes
    }

    #[inline(always)]
    pub fn serialized_len(&self) -> usize {
        PFX + self.len() * core::mem::size_of::<T>()
    }
}

impl<T: Copy, const N: usize, const PFX: usize> Default for PodVec<T, N, PFX> {
    fn default() -> Self {
        Self {
            len: [0u8; PFX],
            data: [MaybeUninit::uninit(); N],
        }
    }
}

impl<T: Copy, const N: usize, const PFX: usize> core::ops::Deref for PodVec<T, N, PFX> {
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy, const N: usize, const PFX: usize> core::ops::DerefMut for PodVec<T, N, PFX> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: Copy, const N: usize, const PFX: usize> AsRef<[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy, const N: usize, const PFX: usize> AsMut<[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: Copy + PartialEq, const N: usize, const PFX: usize> PartialEq for PodVec<T, N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + PartialEq, const N: usize, const PFX: usize> PartialEq<[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &[T]) -> bool {
        self.as_slice() == other
    }
}

impl<T: Copy + PartialEq, const N: usize, const PFX: usize> PartialEq<&[T]> for PodVec<T, N, PFX> {
    #[inline(always)]
    fn eq(&self, other: &&[T]) -> bool {
        self.as_slice() == *other
    }
}

impl<T: Copy + Eq, const N: usize, const PFX: usize> Eq for PodVec<T, N, PFX> {}

impl<T: Copy + core::fmt::Debug, const N: usize, const PFX: usize> core::fmt::Debug
    for PodVec<T, N, PFX>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PodVec")
            .field("len", &self.len())
            .field("capacity", &N)
            .field("pfx", &PFX)
            .field("data", &self.as_slice())
            .finish()
    }
}

impl<T: Copy + core::hash::Hash, const N: usize, const PFX: usize> core::hash::Hash for PodVec<T, N, PFX> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}
