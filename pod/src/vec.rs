//! Fixed-capacity inline vector for zero-copy account data.
//!
//! `PodVec<T, N>` stores up to `N` elements of type `T` with a `PodU16` length
//! prefix. Unlike the dynamic `Vec<T, P, N>` type, writes never trigger
//! `realloc` or `memmove` — they are a direct write to the account buffer.
//!
//! The tradeoff is rent: the full `N * size_of::<T>()` bytes are always
//! allocated. Use `PodVec` for small, frequently-mutated collections;
//! use dynamic `Vec` for large, sparse collections.
//!
//! # Layout
//!
//! ```text
//! [len: PodU16][data: [MaybeUninit<T>; N]]
//! ```
//!
//! - Total size: `2 + N * size_of::<T>()` bytes, alignment 1.
//! - `data[..len]` contains initialized `T` values.
//! - `data[len..N]` is uninitialized (MaybeUninit).
//! - `T` must have alignment 1 (enforced at compile time).

use {
    super::PodU16,
    core::mem::MaybeUninit,
};

/// Fixed-capacity inline vector stored in account data.
///
/// # Safety invariants
///
/// - `T` must have alignment 1 (compile-time assertion in every impl block).
/// - `data[..len]` was written by the program's write methods.
/// - Only the owning program can modify account data (SVM invariant).
/// - Reads clamp `len` to `min(len, N)` to prevent panics on corrupted data.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodVec<T: Copy, const N: usize> {
    len: PodU16,
    data: [MaybeUninit<T>; N],
}

// Compile-time invariants for common instantiations.
const _: () = assert!(core::mem::size_of::<PodVec<u8, 10>>() == 2 + 10);
const _: () = assert!(core::mem::align_of::<PodVec<u8, 10>>() == 1);
const _: () = assert!(core::mem::size_of::<PodVec<[u8; 32], 10>>() == 2 + 320);
const _: () = assert!(core::mem::align_of::<PodVec<[u8; 32], 10>>() == 1);

impl<T: Copy, const N: usize> PodVec<T, N> {
    // Enforce alignment 1 for T in every impl block.
    const _ALIGN_CHECK: () = assert!(
        core::mem::align_of::<T>() == 1,
        "PodVec<T, N>: T must have alignment 1. Use Pod types (PodU64, etc.) instead of native integers."
    );

    /// Number of active elements.
    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        // Clamp to N to prevent out-of-bounds on corrupted account data.
        (self.len.get() as usize).min(N)
    }

    /// Returns `true` if the vector is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    /// Returns the active elements as a slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        // SAFETY: `data[..len]` was written by write methods. `len` is
        // clamped to N, so the slice is always in-bounds.
        // SAFETY: `data[..len]` was written by write methods. `len` is
        // clamped to N, so the slice is always in-bounds. T has alignment 1
        // (compile-time assertion), so the pointer cast is valid.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const T, len) }
    }

    /// Set all elements from a slice.
    ///
    /// # Panics
    ///
    /// Panics if `values.len() > N`.
    #[inline(always)]
    pub fn set_from_slice(&mut self, values: &[T]) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        let vlen = values.len();
        assert!(vlen <= N, "PodVec::set_from_slice: length exceeds capacity");
        // SAFETY: `vlen <= N` checked. T is Copy so bitwise copy is valid.
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr(),
                self.data.as_mut_ptr() as *mut T,
                vlen,
            );
        }
        self.len = PodU16::from(vlen as u16);
    }

    /// Write a single element at `index` without updating `len`.
    ///
    /// For incremental construction: call `write()` for each element,
    /// then `set_len()` once. This avoids temporary stack buffers and
    /// double copies.
    ///
    /// # Panics
    ///
    /// Panics if `index >= N`.
    #[inline(always)]
    pub fn write(&mut self, index: usize, value: T) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        assert!(index < N, "PodVec::write: index out of bounds");
        self.data[index] = MaybeUninit::new(value);
    }

    /// Push an element to the end.
    ///
    /// # Panics
    ///
    /// Panics if the vector is full (`len == N`).
    #[inline(always)]
    pub fn push(&mut self, value: T) {
        let cur = self.len();
        assert!(cur < N, "PodVec::push: vector is full");
        self.data[cur] = MaybeUninit::new(value);
        self.len = PodU16::from((cur + 1) as u16);
    }

    /// Set the active length.
    ///
    /// # Safety contract
    ///
    /// Caller must ensure `data[..n]` has been initialized via `write()`
    /// or `set_from_slice()` before any subsequent `as_slice()` call.
    ///
    /// # Panics
    ///
    /// Panics if `n > N`.
    #[inline(always)]
    pub fn set_len(&mut self, n: usize) {
        assert!(n <= N, "PodVec::set_len: length exceeds capacity");
        self.len = PodU16::from(n as u16);
    }

    /// Clear the vector (set length to 0).
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = PodU16::ZERO;
    }
}

impl<T: Copy, const N: usize> Default for PodVec<T, N> {
    fn default() -> Self {
        Self {
            len: PodU16::ZERO,
            // SAFETY: MaybeUninit::uninit() for an array of MaybeUninit is valid.
            data: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
        }
    }
}

impl<T: Copy + core::fmt::Debug, const N: usize> core::fmt::Debug for PodVec<T, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PodVec")
            .field("len", &self.len())
            .field("capacity", &N)
            .field("data", &self.as_slice())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_vec() {
        let v = PodVec::<u8, 10>::default();
        assert!(v.is_empty());
        assert_eq!(v.len(), 0);
        assert_eq!(v.as_slice(), &[]);
    }

    #[test]
    fn set_from_slice_and_read() {
        let mut v = PodVec::<u8, 10>::default();
        v.set_from_slice(&[1, 2, 3]);
        assert_eq!(v.len(), 3);
        assert_eq!(v.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn push() {
        let mut v = PodVec::<u8, 4>::default();
        v.push(10);
        v.push(20);
        v.push(30);
        assert_eq!(v.len(), 3);
        assert_eq!(v.as_slice(), &[10, 20, 30]);
    }

    #[test]
    #[should_panic(expected = "vector is full")]
    fn push_overflow_panics() {
        let mut v = PodVec::<u8, 2>::default();
        v.push(1);
        v.push(2);
        v.push(3); // panics
    }

    #[test]
    fn incremental_write_then_set_len() {
        let mut v = PodVec::<u8, 10>::default();
        v.write(0, 0xAA);
        v.write(1, 0xBB);
        v.write(2, 0xCC);
        v.set_len(3);
        assert_eq!(v.as_slice(), &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    #[should_panic(expected = "exceeds capacity")]
    fn set_from_slice_overflow_panics() {
        let mut v = PodVec::<u8, 2>::default();
        v.set_from_slice(&[1, 2, 3]);
    }

    #[test]
    fn clear() {
        let mut v = PodVec::<u8, 10>::default();
        v.set_from_slice(&[1, 2, 3]);
        v.clear();
        assert!(v.is_empty());
        assert_eq!(v.as_slice(), &[]);
    }

    #[test]
    fn overwrite() {
        let mut v = PodVec::<u8, 10>::default();
        v.set_from_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(v.len(), 5);
        v.set_from_slice(&[10, 20]);
        assert_eq!(v.len(), 2);
        assert_eq!(v.as_slice(), &[10, 20]);
    }

    #[test]
    fn corrupted_len_clamped() {
        let mut v = PodVec::<u8, 4>::default();
        v.set_from_slice(&[1, 2]);
        // Simulate corrupted len > N
        v.len = PodU16::from(u16::MAX);
        assert_eq!(v.len(), 4); // clamped
        assert_eq!(v.as_slice().len(), 4);
    }

    #[test]
    fn with_address_sized_elements() {
        // Simulates PodVec<Address, 10> where Address = [u8; 32]
        let mut v = PodVec::<[u8; 32], 3>::default();
        let addr1 = [1u8; 32];
        let addr2 = [2u8; 32];
        v.push(addr1);
        v.push(addr2);
        assert_eq!(v.len(), 2);
        assert_eq!(v.as_slice()[0], addr1);
        assert_eq!(v.as_slice()[1], addr2);
    }

    #[test]
    fn size_and_alignment() {
        assert_eq!(core::mem::size_of::<PodVec<u8, 10>>(), 12);
        assert_eq!(core::mem::align_of::<PodVec<u8, 10>>(), 1);
        assert_eq!(core::mem::size_of::<PodVec<[u8; 32], 10>>(), 322);
        assert_eq!(core::mem::align_of::<PodVec<[u8; 32], 10>>(), 1);
    }
}
