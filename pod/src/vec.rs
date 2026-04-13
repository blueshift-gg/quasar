//! Fixed-capacity inline vector for zero-copy account data.
//!
//! `PodVec<T, N, PFX>` stores up to `N` elements of type `T` with a `PFX`-byte
//! little-endian length prefix. It is a fixed-size Pod type: the struct always
//! occupies `PFX + N * size_of::<T>()` bytes in memory and on-disk, regardless
//! of how many elements are active.
//!
//! `PFX` must be `1`, `2`, `4`, or `8`, and defaults to `2` (matching the
//! previous `PodVec<T, N>` signature — no breaking change).
//!
//! # Layout
//!
//! ```text
//! [len: [u8; PFX] LE][data: [MaybeUninit<T>; N]]
//! ```
//!
//! - Total size: `PFX + N * size_of::<T>()` bytes, alignment 1.
//! - `data[..len]` contains initialized `T` values.
//! - `data[len..N]` is uninitialized (MaybeUninit).
//! - `T` must have alignment 1 (enforced at compile time).
//!
//! # Usage in account structs
//!
//! **As `PodVec<T, N>` directly (or via `fixed_capacity`):**
//! The full capacity is always in account data — no realloc ever. Best when
//! the worst-case rent cost is acceptable.
//!
//! ```ignore
//! #[account(discriminator = 1)]
//! pub struct Multisig {
//!     pub threshold: u8,
//!     pub signers: PodVec<Address, 10>,  // always 2 + 320 bytes on-chain (PFX=2 default)
//! }
//!
//! // Direct zero-copy write — no guard needed:
//! let ok = ctx.accounts.multisig.signers.push(new_signer);
//! ctx.accounts.multisig.signers[0] = replacement;
//! ```
//!
//! **As `Vec<T, N>` or `Vec<T, N, u8>` in `#[account]` structs (dynamic
//! sizing):** The derive macro generates a `DynGuard` RAII wrapper. Account
//! data stores only the active elements (`[len: PFX bytes LE][active
//! elements]`), so rent scales with content. `PodVec` is used as the
//! stack-local copy — loaded on guard creation, flushed back (with one realloc
//! CPI if size changes) on drop.

use {super::string::max_n_for_pfx, core::mem::MaybeUninit};

/// Fixed-capacity inline vector stored in account data.
///
/// `PFX` is the byte width of the on-disk length prefix (`1`, `2`, `4`, or
/// `8`). It defaults to `2`, preserving backward compatibility with existing
/// `PodVec<T, N>` usage. Use `PodVec<T, N, 1>` for element counts up to 255,
/// or `PodVec<T, N, 4>` for up to ~4 billion.
///
/// # Safety invariants
///
/// - `T` must have alignment 1 (compile-time assertion in every impl block).
/// - `data[..len]` was written by the program's write methods.
/// - Only the owning program can modify account data (SVM invariant).
/// - Reads clamp `len` to `min(len, N)` to prevent panics on corrupted data.
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

    /// Compile-time validity check. Reference this in a `const` context to
    /// verify that `T`, `N`, and `PFX` are valid at the call site.
    ///
    /// ```ignore
    /// const _: () = PodVec::<u8, 300, 1>::VALID; // compile error: N exceeds prefix range
    /// ```
    pub const VALID: () = {
        Self::_ALIGN_CHECK;
        Self::_CAP_CHECK;
    };

    /// Decode the on-disk length prefix into a `usize`.
    ///
    /// LLVM constant-folds this per monomorphization (e.g., for PFX=2 it
    /// compiles to a 16-bit load).
    #[inline(always)]
    fn decode_len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        let mut buf = [0u8; 8];
        buf[..PFX].copy_from_slice(&self.len);
        u64::from_le_bytes(buf) as usize
    }

    /// Encode `n` as a `PFX`-byte little-endian prefix into `self.len`.
    #[inline(always)]
    fn encode_len(&mut self, n: usize) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        let bytes = (n as u64).to_le_bytes();
        self.len.copy_from_slice(&bytes[..PFX]);
    }

    /// Number of active elements.
    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_ALIGN_CHECK;
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        // Clamp to N to prevent out-of-bounds on corrupted account data.
        self.decode_len().min(N)
    }

    /// Returns `true` if the vector is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.decode_len() == 0
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
    #[must_use = "returns false if values.len() exceeds capacity — unhandled means the write was \
                  silently skipped"]
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
        self.encode_len(vlen);
        true
    }

    /// Push an element to the end. Returns `false` if the vector is full.
    #[must_use = "returns false if capacity is exceeded — unhandled means the push was silently \
                  skipped"]
    #[inline(always)]
    pub fn push(&mut self, value: T) -> bool {
        let cur = self.len();
        if cur >= N {
            return false;
        }
        self.data[cur] = MaybeUninit::new(value);
        self.encode_len(cur + 1);
        true
    }

    /// Remove and return the last element, or `None` if empty.
    #[must_use = "returns None if the vector is empty"]
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        let cur = self.len();
        if cur == 0 {
            return None;
        }
        let new_len = cur - 1;
        // SAFETY: `new_len < cur <= N`, so `data[new_len]` was initialized.
        let val = unsafe { self.data[new_len].assume_init() };
        self.encode_len(new_len);
        Some(val)
    }

    /// Remove element at `index` by swapping with the last element.
    /// O(1) but does not preserve order. Returns the removed element,
    /// or `None` if `index >= len`.
    #[must_use = "returns None if index is out of bounds"]
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
        self.encode_len(last);
        Some(removed)
    }

    /// Remove element at `index`, shifting subsequent elements left.
    /// O(n) but preserves order. Returns the removed element,
    /// or `None` if `index >= len`.
    #[must_use = "returns None if index is out of bounds"]
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
        self.encode_len(cur - 1);
        Some(removed)
    }

    /// Append elements from a slice. Returns `false` if there isn't
    /// enough remaining capacity.
    #[must_use = "returns false if there is insufficient remaining capacity — unhandled means the \
                  append was silently skipped"]
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
        self.encode_len(new_len);
        true
    }

    /// Shorten the vector to `new_len` elements. No-op if `new_len >= len`.
    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        let cur = self.len();
        if new_len < cur {
            self.encode_len(new_len);
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
        self.encode_len(write);
    }

    /// Clear the vector (set length to 0).
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = [0u8; PFX];
    }

    /// Load from a byte slice containing `[len: PFX bytes LE][elements...]`.
    ///
    /// Copies `min(len, N)` elements into self. Returns the number of bytes
    /// consumed from the source slice (prefix + data).
    ///
    /// The caller must ensure `bytes.len() >= PFX + min(encoded_len, N) *
    /// size_of::<T>()`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if the slice is shorter than the encoded length.
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
        // SAFETY: T has alignment 1 (compile-time assertion). `count` is
        // clamped to N, so we write at most `N * size_of::<T>()` bytes
        // into `self.data`, which has exactly that capacity. Source and
        // destination are different allocations (account data vs stack),
        // so they cannot overlap.
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

    /// Write `[len: PFX bytes LE][elements...]` to a byte slice.
    ///
    /// Returns the number of bytes written (prefix + data).
    ///
    /// The caller must ensure `dest.len() >= PFX + self.len() *
    /// size_of::<T>()`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `dest` is shorter than the encoded length.
    #[inline(always)]
    pub fn write_to_bytes(&self, dest: &mut [u8]) -> usize {
        let count = self.len();
        let data_bytes = count * core::mem::size_of::<T>();
        debug_assert!(
            dest.len() >= PFX + data_bytes,
            "write_to_bytes: dest too short for encoded length"
        );
        // Write the (possibly clamped) count as PFX LE bytes.
        dest[..PFX].copy_from_slice(&(count as u64).to_le_bytes()[..PFX]);
        // SAFETY: T has alignment 1 (compile-time assertion). `count` is
        // clamped to N via `len()`, so we read at most `N * size_of::<T>()`
        // bytes from `self.data`. Source (stack) and destination (account
        // data) are different allocations, so they cannot overlap.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.data.as_ptr() as *const u8,
                dest[PFX..].as_mut_ptr(),
                data_bytes,
            );
        }
        PFX + data_bytes
    }

    /// Total bytes this field occupies when serialized: `PFX + len *
    /// size_of::<T>()`.
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
        assert!(v.set_from_slice(&[1, 2, 3, 4])); // initialize all 4 elements so no MaybeUninit read after corruption
                                                  // Simulate corrupted len > N (PFX=2 → [u8; 2])
        v.len = [0xFF, 0xFF]; // u16::MAX = 65535
        assert_eq!(v.len(), 4); // clamped
                                // as_slice() is over fully-initialized data — no UB
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
    fn partial_eq_slice() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.set_from_slice(&[1, 2, 3]));
        assert_eq!(v, [1u8, 2, 3].as_slice());
        assert_eq!(v, &[1u8, 2, 3][..]);
    }

    #[test]
    fn as_mut_slice() {
        let mut v = PodVec::<u8, 4>::default();
        assert!(v.set_from_slice(&[1, 2, 3]));
        let s: &mut [u8] = v.as_mut();
        s[0] = 99;
        assert_eq!(v.as_slice(), &[99, 2, 3]);
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
        // PFX=2 (default)
        assert_eq!(core::mem::size_of::<PodVec<u8, 10>>(), 12);
        assert_eq!(core::mem::align_of::<PodVec<u8, 10>>(), 1);
        assert_eq!(core::mem::size_of::<PodVec<[u8; 32], 10>>(), 322);
        assert_eq!(core::mem::align_of::<PodVec<[u8; 32], 10>>(), 1);
        // PFX=1
        assert_eq!(core::mem::size_of::<PodVec<u8, 10, 1>>(), 11);
        assert_eq!(core::mem::align_of::<PodVec<u8, 10, 1>>(), 1);
        // PFX=4
        assert_eq!(core::mem::size_of::<PodVec<u8, 10, 4>>(), 14);
        assert_eq!(core::mem::align_of::<PodVec<u8, 10, 4>>(), 1);
    }

    #[test]
    fn load_from_bytes_empty() {
        let mut v = PodVec::<u8, 10>::default();
        let bytes = [0u8, 0]; // len=0, no data
        let consumed = v.load_from_bytes(&bytes);
        assert_eq!(consumed, 2);
        assert!(v.is_empty());
    }

    #[test]
    fn load_from_bytes_partial() {
        let mut v = PodVec::<u8, 10>::default();
        let bytes = [3u8, 0, 10, 20, 30]; // len=3, data=[10,20,30]
        let consumed = v.load_from_bytes(&bytes);
        assert_eq!(consumed, 5);
        assert_eq!(v.len(), 3);
        assert_eq!(v.as_slice(), &[10, 20, 30]);
    }

    #[test]
    fn load_from_bytes_full_capacity() {
        let mut v = PodVec::<u8, 4>::default();
        let bytes = [4u8, 0, 1, 2, 3, 4]; // len=4
        let consumed = v.load_from_bytes(&bytes);
        assert_eq!(consumed, 6);
        assert_eq!(v.len(), 4);
        assert_eq!(v.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn load_from_bytes_clamps_to_n() {
        let mut v = PodVec::<u8, 2>::default();
        // Source says len=5 but N=2, should clamp
        let bytes = [5u8, 0, 10, 20, 30, 40, 50];
        let consumed = v.load_from_bytes(&bytes);
        assert_eq!(consumed, 4); // 2 + 2*1
        assert_eq!(v.len(), 2);
        assert_eq!(v.as_slice(), &[10, 20]);
    }

    #[test]
    fn write_to_bytes_empty() {
        let v = PodVec::<u8, 10>::default();
        let mut buf = [0xFFu8; 12];
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 2);
        assert_eq!(&buf[0..2], &[0, 0]); // len=0
    }

    #[test]
    fn write_to_bytes_with_data() {
        let mut v = PodVec::<u8, 10>::default();
        assert!(v.set_from_slice(&[10, 20, 30]));
        let mut buf = [0u8; 12];
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 5);
        assert_eq!(&buf[0..2], &[3, 0]); // len=3 LE
        assert_eq!(&buf[2..5], &[10, 20, 30]);
    }

    #[test]
    fn load_write_roundtrip() {
        let mut v = PodVec::<u8, 10>::default();
        assert!(v.set_from_slice(&[1, 2, 3, 4, 5]));

        // Write to buffer
        let mut buf = [0u8; 12];
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 7);

        // Mutate, then load back from buffer
        v.clear();
        assert!(v.is_empty());
        let consumed = v.load_from_bytes(&buf);
        assert_eq!(consumed, 7);
        assert_eq!(v.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn load_write_roundtrip_address_sized() {
        let mut v = PodVec::<[u8; 32], 3>::default();
        let addr1 = [1u8; 32];
        let addr2 = [2u8; 32];
        assert!(v.push(addr1));
        assert!(v.push(addr2));

        let mut buf = [0u8; 70]; // 2 + 2*32 = 66
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 66);

        let mut v2 = PodVec::<[u8; 32], 3>::default();
        let consumed = v2.load_from_bytes(&buf);
        assert_eq!(consumed, 66);
        assert_eq!(v2.len(), 2);
        assert_eq!(v2[0], addr1);
        assert_eq!(v2[1], addr2);
    }

    #[test]
    fn serialized_len() {
        let mut v = PodVec::<u8, 10>::default();
        assert_eq!(v.serialized_len(), 2); // empty: just prefix
        assert!(v.set_from_slice(&[1, 2, 3]));
        assert_eq!(v.serialized_len(), 5); // 2 + 3

        let mut v2 = PodVec::<[u8; 32], 5>::default();
        assert_eq!(v2.serialized_len(), 2);
        assert!(v2.push([0u8; 32]));
        assert_eq!(v2.serialized_len(), 34); // 2 + 32
    }

    #[test]
    fn load_mutate_write_roundtrip() {
        // Simulate the stack-cache pattern: load → mutate → write back
        let original = [2u8, 0, 10, 20]; // len=2, data=[10,20]

        let mut v = PodVec::<u8, 10>::default();
        v.load_from_bytes(&original);
        assert_eq!(v.as_slice(), &[10, 20]);

        // Mutate on "stack"
        assert!(v.push(30));
        v[0] = 99;

        // Write back
        let mut buf = [0u8; 12];
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 5); // 2 + 3
        assert_eq!(&buf[0..2], &[3, 0]); // len=3
        assert_eq!(&buf[2..5], &[99, 20, 30]);
    }

    // --- PFX=1 tests ---

    #[test]
    fn pfx1_roundtrip() {
        let mut v = PodVec::<u8, 100, 1>::default();
        assert!(v.set_from_slice(&[10, 20, 30]));

        let mut buf = [0u8; 104];
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 4); // 1 + 3
        assert_eq!(buf[0], 3u8); // len=3 in 1 byte

        let mut v2 = PodVec::<u8, 100, 1>::default();
        let consumed = v2.load_from_bytes(&buf);
        assert_eq!(consumed, 4);
        assert_eq!(v2.as_slice(), &[10, 20, 30]);
    }

    #[test]
    fn pfx1_serialized_len() {
        let mut v = PodVec::<u8, 10, 1>::default();
        assert_eq!(v.serialized_len(), 1); // just 1-byte prefix
        assert!(v.set_from_slice(&[1, 2, 3]));
        assert_eq!(v.serialized_len(), 4); // 1 + 3
    }

    // --- PFX=4 tests ---

    #[test]
    fn pfx4_roundtrip() {
        let mut v = PodVec::<u8, 100, 4>::default();
        assert!(v.set_from_slice(&[10, 20, 30]));

        let mut buf = [0u8; 105];
        let written = v.write_to_bytes(&mut buf);
        assert_eq!(written, 7); // 4 + 3
        assert_eq!(&buf[..4], &[3u8, 0, 0, 0]); // len=3 in LE u32

        let mut v2 = PodVec::<u8, 100, 4>::default();
        let consumed = v2.load_from_bytes(&buf);
        assert_eq!(consumed, 7);
        assert_eq!(v2.as_slice(), &[10, 20, 30]);
    }

    #[test]
    fn pfx4_serialized_len() {
        let mut v = PodVec::<u8, 10, 4>::default();
        assert_eq!(v.serialized_len(), 4); // just 4-byte prefix
        assert!(v.set_from_slice(&[1, 2, 3]));
        assert_eq!(v.serialized_len(), 7); // 4 + 3
    }
}
