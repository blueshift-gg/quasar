//! Fixed-capacity inline vector for zero-copy account data.
//!
//! `PodVec<T, N>` stores up to `N` elements of type `T` with a `PodU16` length
//! prefix. Unlike the dynamic `Vec<T, P, N>` type, writes never trigger
//! `realloc` or `memmove` — they are a direct write to the account buffer.
//!
//! The tradeoff is rent: the full `N * size_of::<T>()` bytes are always
//! allocated. Use `PodVec` for small, frequently-mutated collections
//! (signers, whitelist entries, scores).
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
//!
//! # Usage in account structs
//!
//! `PodVec<T, N>` is a fixed-size Pod type — use it directly in `#[account]`
//! structs. Writes go through `DerefMut` on `Account<T>`:
//!
//! ```ignore
//! #[account(discriminator = 1)]
//! pub struct Multisig {
//!     pub threshold: u8,
//!     pub signers: PodVec<Address, 10>,  // 2 + 320 bytes in account data
//! }
//!
//! // In instruction handler:
//! ctx.accounts.multisig.signers.push(new_signer);
//! ctx.accounts.multisig.signers[0] = replacement;
//! ```

use {super::PodU16, core::mem::MaybeUninit};

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
    const _ALIGN_CHECK: () = assert!(
        core::mem::align_of::<T>() == 1,
        "PodVec<T, N>: T must have alignment 1. Use Pod types (PodU64, etc.) instead of native \
         integers."
    );

    const _CAP_CHECK: () = assert!(
        N <= 65535,
        "PodVec<T, N>: N cannot exceed 65535 (u16 length prefix)"
    );

    /// Number of active elements.
    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        // Clamp to N to prevent out-of-bounds on corrupted account data.
        (self.len.get() as usize).min(N)
    }

    /// Returns `true` if the vector is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    /// Maximum number of elements this vector can hold.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns the active elements as a slice.
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        // SAFETY: `data[..len]` was written by write methods. `len` is
        // clamped to N, so the slice is always in-bounds. T has alignment 1
        // (compile-time assertion), so the pointer cast is valid.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const T, len) }
    }

    /// Returns the active elements as a mutable slice.
    #[inline(always)]
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        let len = self.len();
        // SAFETY: same guarantees as `as_slice`, plus `&mut self` ensures
        // exclusive access.
        unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr() as *mut T, len) }
    }

    /// Get element at index, or `None` if out of bounds.
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    /// Get mutable element at index, or `None` if out of bounds.
    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.as_slice_mut().get_mut(index)
    }

    /// Iterate over active elements.
    #[inline(always)]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Iterate mutably over active elements.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_slice_mut().iter_mut()
    }

    /// Set all elements from a slice. Returns `false` if `values.len() > N`.
    #[inline(always)]
    pub fn set_from_slice(&mut self, values: &[T]) -> bool {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        let vlen = values.len();
        if vlen > N {
            return false;
        }
        // SAFETY: `vlen <= N` checked. T is Copy so bitwise copy is valid.
        unsafe {
            core::ptr::copy_nonoverlapping(values.as_ptr(), self.data.as_mut_ptr() as *mut T, vlen);
        }
        self.len = PodU16::from(vlen as u16);
        true
    }

    /// Push an element to the end. Returns `false` if the vector is full.
    #[inline(always)]
    pub fn push(&mut self, value: T) -> bool {
        let cur = self.len();
        if cur >= N {
            return false;
        }
        self.data[cur] = MaybeUninit::new(value);
        self.len = PodU16::from((cur + 1) as u16);
        true
    }

    /// Remove and return the last element, or `None` if empty.
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        let cur = self.len();
        if cur == 0 {
            return None;
        }
        let new_len = cur - 1;
        // SAFETY: `new_len < cur <= N`, so `data[new_len]` was initialized.
        let val = unsafe { self.data[new_len].assume_init() };
        self.len = PodU16::from(new_len as u16);
        Some(val)
    }

    /// Remove element at `index` by swapping with the last element.
    /// O(1) but does not preserve order. Returns the removed element,
    /// or `None` if `index >= len`.
    #[inline(always)]
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        let cur = self.len();
        if index >= cur {
            return None;
        }
        let last = cur - 1;
        // SAFETY: index < cur <= N and last < cur <= N, both initialized.
        let removed = unsafe { self.data[index].assume_init() };
        if index != last {
            self.data[index] = self.data[last];
        }
        self.len = PodU16::from(last as u16);
        Some(removed)
    }

    /// Remove element at `index`, shifting subsequent elements left.
    /// O(n) but preserves order. Returns the removed element,
    /// or `None` if `index >= len`.
    #[inline(always)]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let cur = self.len();
        if index >= cur {
            return None;
        }
        // SAFETY: `index < cur <= N`, so `data[index]` is initialized.
        let removed = unsafe { self.data[index].assume_init() };
        // Shift elements left: data[index..cur-1] = data[index+1..cur]
        let tail = cur - index - 1;
        if tail > 0 {
            // SAFETY: src and dst are within the same allocation, both
            // within `data[0..N]`. Using copy (not copy_nonoverlapping)
            // because regions overlap.
            unsafe {
                core::ptr::copy(
                    self.data.as_ptr().add(index + 1),
                    self.data.as_mut_ptr().add(index),
                    tail,
                );
            }
        }
        self.len = PodU16::from((cur - 1) as u16);
        Some(removed)
    }

    /// Append elements from a slice. Returns `false` if there isn't
    /// enough remaining capacity.
    #[inline(always)]
    pub fn extend_from_slice(&mut self, values: &[T]) -> bool {
        let cur = self.len();
        let new_len = cur + values.len();
        if new_len > N {
            return false;
        }
        // SAFETY: `new_len <= N` checked. Copy into `data[cur..new_len]`.
        unsafe {
            core::ptr::copy_nonoverlapping(
                values.as_ptr(),
                (self.data.as_mut_ptr() as *mut T).add(cur),
                values.len(),
            );
        }
        self.len = PodU16::from(new_len as u16);
        true
    }

    /// Shorten the vector to `new_len` elements. No-op if `new_len >= len`.
    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        let cur = self.len();
        if new_len < cur {
            self.len = PodU16::from(new_len as u16);
        }
    }

    /// Retain only elements for which `f` returns `true`. Preserves order.
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        let mut write = 0;
        let cur = self.len();
        for read in 0..cur {
            // SAFETY: `read < cur <= N`, so `data[read]` is initialized.
            let val = unsafe { self.data[read].assume_init() };
            if f(&val) {
                self.data[write] = MaybeUninit::new(val);
                write += 1;
            }
        }
        self.len = PodU16::from(write as u16);
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
            data: [MaybeUninit::uninit(); N],
        }
    }
}

impl<T: Copy, const N: usize> core::ops::Deref for PodVec<T, N> {
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy, const N: usize> core::ops::DerefMut for PodVec<T, N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: Copy, const N: usize> AsRef<[T]> for PodVec<T, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy + PartialEq, const N: usize> PartialEq for PodVec<T, N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Eq, const N: usize> Eq for PodVec<T, N> {}

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
        assert!(v.set_from_slice(&[1, 2, 3]));
        assert_eq!(v.len(), 3);
        assert_eq!(v.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn push() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.push(10));
        assert!(v.push(20));
        assert!(v.push(30));
        assert_eq!(v.len(), 3);
        assert_eq!(v.as_slice(), &[10, 20, 30]);
    }

    #[test]
    fn push_full_returns_false() {
        let mut v = PodVec::<u8, 2>::default();
        assert!(v.push(1));
        assert!(v.push(2));
        assert!(!v.push(3));
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn pop() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.push(10));
        assert!(v.push(20));
        assert_eq!(v.pop(), Some(20));
        assert_eq!(v.pop(), Some(10));
        assert_eq!(v.pop(), None);
        assert!(v.is_empty());
    }

    #[test]
    fn set_from_slice_overflow_returns_false() {
        let mut v = PodVec::<u8, 2>::default();
        assert!(!v.set_from_slice(&[1, 2, 3]));
        assert!(v.is_empty());
    }

    #[test]
    fn clear() {
        let mut v = PodVec::<u8, 10>::default();
        assert!(v.set_from_slice(&[1, 2, 3]));
        v.clear();
        assert!(v.is_empty());
        assert_eq!(v.as_slice(), &[]);
    }

    #[test]
    fn overwrite() {
        let mut v = PodVec::<u8, 10>::default();
        assert!(v.set_from_slice(&[1, 2, 3, 4, 5]));
        assert_eq!(v.len(), 5);
        assert!(v.set_from_slice(&[10, 20]));
        assert_eq!(v.len(), 2);
        assert_eq!(v.as_slice(), &[10, 20]);
    }

    #[test]
    fn corrupted_len_clamped() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.set_from_slice(&[1, 2]));
        // Simulate corrupted len > N
        v.len = PodU16::from(u16::MAX);
        assert_eq!(v.len(), 4); // clamped
        assert_eq!(v.as_slice().len(), 4);
    }

    #[test]
    fn with_address_sized_elements() {
        let mut v = PodVec::<[u8; 32], 3>::default();
        let addr1 = [1u8; 32];
        let addr2 = [2u8; 32];
        assert!(v.push(addr1));
        assert!(v.push(addr2));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], addr1);
        assert_eq!(v[1], addr2);
    }

    #[test]
    fn get_and_get_mut() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.push(10));
        assert!(v.push(20));
        assert_eq!(v.get(0), Some(&10));
        assert_eq!(v.get(1), Some(&20));
        assert_eq!(v.get(2), None);

        *v.get_mut(0).unwrap() = 99;
        assert_eq!(v[0], 99);
    }

    #[test]
    fn deref_to_slice() {
        let mut v = PodVec::<u8, 10>::default();
        assert!(v.set_from_slice(&[1, 2, 3]));
        // Slice methods via Deref
        assert!(v.contains(&2));
        assert_eq!(v.first(), Some(&1));
        assert_eq!(v.last(), Some(&3));
    }

    #[test]
    fn deref_mut_slice() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.set_from_slice(&[3, 1, 2]));
        // Mutable slice methods via DerefMut
        v.sort();
        assert_eq!(v.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn iter_and_iter_mut() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.set_from_slice(&[1, 2, 3]));
        let sum: u8 = v.iter().sum();
        assert_eq!(sum, 6);

        for x in v.iter_mut() {
            *x *= 2;
        }
        assert_eq!(v.as_slice(), &[2, 4, 6]);
    }

    #[test]
    fn partial_eq() {
        let mut a = PodVec::<u8, 4>::default();
        let mut b = PodVec::<u8, 4>::default();
        assert!(a.set_from_slice(&[1, 2]));
        assert!(b.set_from_slice(&[1, 2]));
        assert_eq!(a, b);
        assert!(b.push(3));
        assert_ne!(a, b);
    }

    #[test]
    fn capacity() {
        let v = PodVec::<u8, 42>::default();
        assert_eq!(v.capacity(), 42);
    }

    #[test]
    fn swap_remove() {
        let mut v = PodVec::<u8, 6>::default();
        assert!(v.set_from_slice(&[10, 20, 30, 40, 50]));
        // Remove middle element — last element fills the gap.
        assert_eq!(v.swap_remove(1), Some(20));
        assert_eq!(v.as_slice(), &[10, 50, 30, 40]);
        // Remove last element — no swap needed.
        assert_eq!(v.swap_remove(3), Some(40));
        assert_eq!(v.as_slice(), &[10, 50, 30]);
        // Out of bounds.
        assert_eq!(v.swap_remove(5), None);
    }

    #[test]
    fn remove_preserves_order() {
        let mut v = PodVec::<u8, 6>::default();
        assert!(v.set_from_slice(&[10, 20, 30, 40, 50]));
        assert_eq!(v.remove(1), Some(20));
        assert_eq!(v.as_slice(), &[10, 30, 40, 50]);
        assert_eq!(v.remove(0), Some(10));
        assert_eq!(v.as_slice(), &[30, 40, 50]);
        // Remove last.
        assert_eq!(v.remove(2), Some(50));
        assert_eq!(v.as_slice(), &[30, 40]);
        // Out of bounds.
        assert_eq!(v.remove(5), None);
    }

    #[test]
    fn extend_from_slice() {
        let mut v = PodVec::<u8, 6>::default();
        assert!(v.set_from_slice(&[1, 2]));
        assert!(v.extend_from_slice(&[3, 4, 5]));
        assert_eq!(v.as_slice(), &[1, 2, 3, 4, 5]);
        // Exceeds remaining capacity.
        assert!(!v.extend_from_slice(&[6, 7]));
        assert_eq!(v.len(), 5); // unchanged
    }

    #[test]
    fn truncate() {
        let mut v = PodVec::<u8, 6>::default();
        assert!(v.set_from_slice(&[1, 2, 3, 4, 5]));
        v.truncate(3);
        assert_eq!(v.as_slice(), &[1, 2, 3]);
        // Truncate to same or larger — no-op.
        v.truncate(10);
        assert_eq!(v.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn retain() {
        let mut v = PodVec::<u8, 8>::default();
        assert!(v.set_from_slice(&[1, 2, 3, 4, 5, 6]));
        v.retain(|x| x % 2 == 0);
        assert_eq!(v.as_slice(), &[2, 4, 6]);
    }

    #[test]
    fn size_and_alignment() {
        assert_eq!(core::mem::size_of::<PodVec<u8, 10>>(), 12);
        assert_eq!(core::mem::align_of::<PodVec<u8, 10>>(), 1);
        assert_eq!(core::mem::size_of::<PodVec<[u8; 32], 10>>(), 322);
        assert_eq!(core::mem::align_of::<PodVec<[u8; 32], 10>>(), 1);
    }
}
