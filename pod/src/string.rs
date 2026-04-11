//! Fixed-capacity inline string for zero-copy account data.
//!
//! `PodString<N>` stores up to `N` bytes (max 255) with a `u8` length prefix.
//! Unlike the dynamic `String<P, N>` type, writes never trigger `realloc` or
//! `memmove` — they are a simple `memcpy` + length update (~5 CU vs ~300-500 CU).
//!
//! The tradeoff is rent: the full `N` bytes are always allocated in the account
//! even if the string is shorter or empty. Use `PodString` for small,
//! frequently-mutated fields (labels, names); use dynamic `String` for large,
//! rarely-written fields (bios, descriptions).
//!
//! # Layout
//!
//! ```text
//! [len: u8][data: [MaybeUninit<u8>; N]]
//! ```
//!
//! - Total size: `1 + N` bytes, alignment 1.
//! - `data[..len]` contains valid UTF-8 bytes.
//! - `data[len..N]` is uninitialized (MaybeUninit).

use core::mem::MaybeUninit;

/// Fixed-capacity inline string stored in account data.
///
/// # Safety invariants
///
/// - `data[..len]` contains valid UTF-8, written by the program's own `set()`.
/// - Only the owning program can modify account data (SVM invariant).
/// - `create_account` zeros the buffer, so a fresh `PodString` has `len=0`.
/// - Reads clamp `len` to `min(len, N)` to prevent panics on corrupted data.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodString<const N: usize> {
    len: u8,
    data: [MaybeUninit<u8>; N],
}

// Compile-time invariants.
const _: () = assert!(core::mem::size_of::<PodString<0>>() == 1);
const _: () = assert!(core::mem::size_of::<PodString<1>>() == 2);
const _: () = assert!(core::mem::size_of::<PodString<32>>() == 33);
const _: () = assert!(core::mem::size_of::<PodString<255>>() == 256);
const _: () = assert!(core::mem::align_of::<PodString<0>>() == 1);
const _: () = assert!(core::mem::align_of::<PodString<32>>() == 1);
const _: () = assert!(core::mem::align_of::<PodString<255>>() == 1);

impl<const N: usize> PodString<N> {
    /// Number of active bytes in the string.
    #[inline(always)]
    pub fn len(&self) -> usize {
        // Clamp to N to prevent out-of-bounds on corrupted account data.
        (self.len as usize).min(N)
    }

    /// Returns `true` if the string is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the string as a `&str`.
    ///
    /// # Safety (internal)
    ///
    /// Uses `from_utf8_unchecked` — sound because only the owning program
    /// can write account data, and `set()` only accepts `&str` (guaranteed
    /// UTF-8 by the Rust type system). A fresh account is zero-initialized,
    /// so `len=0` produces an empty string.
    #[inline(always)]
    pub fn as_str(&self) -> &str {
        let len = self.len();
        // SAFETY: `data[..len]` was written by `set()` with valid UTF-8.
        // `len` is clamped to N, so the slice is always in-bounds.
        unsafe {
            let bytes = core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len);
            core::str::from_utf8_unchecked(bytes)
        }
    }

    /// Returns the raw bytes of the active portion.
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        let len = self.len();
        // SAFETY: same as `as_str` — `data[..len]` is initialized.
        // SAFETY: same as `as_str` — `data[..len]` is initialized.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) }
    }

    /// Set the string contents.
    ///
    /// # Panics
    ///
    /// Panics if `value.len() > N`.
    #[inline(always)]
    pub fn set(&mut self, value: &str) {
        let vlen = value.len();
        assert!(vlen <= N, "PodString::set: value length exceeds capacity");
        // SAFETY: `vlen <= N` checked above. The source is valid UTF-8
        // (Rust `&str` invariant). Writing to MaybeUninit is always safe.
        unsafe {
            core::ptr::copy_nonoverlapping(
                value.as_ptr(),
                self.data.as_mut_ptr() as *mut u8,
                vlen,
            );
        }
        self.len = vlen as u8;
    }

    /// Clear the string (set length to 0).
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> Default for PodString<N> {
    fn default() -> Self {
        Self {
            len: 0,
            // SAFETY: MaybeUninit::uninit() for an array of MaybeUninit is valid.
            data: unsafe { MaybeUninit::<[MaybeUninit<u8>; N]>::uninit().assume_init() },
        }
    }
}

impl<const N: usize> core::fmt::Debug for PodString<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PodString<{}>(\"{}\")", N, self.as_str())
    }
}

impl<const N: usize> core::fmt::Display for PodString<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        let s = PodString::<32>::default();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(s.as_str(), "");
        assert_eq!(s.as_bytes(), b"");
    }

    #[test]
    fn set_and_read() {
        let mut s = PodString::<32>::default();
        s.set("hello");
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.as_bytes(), b"hello");
    }

    #[test]
    fn set_max_length() {
        let mut s = PodString::<5>::default();
        s.set("abcde");
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "abcde");
    }

    #[test]
    #[should_panic(expected = "exceeds capacity")]
    fn set_over_capacity_panics() {
        let mut s = PodString::<3>::default();
        s.set("abcd");
    }

    #[test]
    fn overwrite_shorter() {
        let mut s = PodString::<32>::default();
        s.set("hello world");
        assert_eq!(s.as_str(), "hello world");
        s.set("hi");
        assert_eq!(s.len(), 2);
        assert_eq!(s.as_str(), "hi");
    }

    #[test]
    fn clear() {
        let mut s = PodString::<32>::default();
        s.set("test");
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.as_str(), "");
    }

    #[test]
    fn corrupted_len_clamped() {
        let mut s = PodString::<4>::default();
        s.set("ab");
        // Simulate corrupted len > N
        s.len = 255;
        // Should NOT panic — len is clamped to N
        assert_eq!(s.len(), 4);
        // as_bytes returns 4 bytes (the 2 written + 2 uninit-but-in-bounds)
        assert_eq!(s.as_bytes().len(), 4);
    }

    #[test]
    fn utf8_multibyte() {
        let mut s = PodString::<32>::default();
        s.set("caf\u{00e9}"); // "café" — 5 bytes in UTF-8
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "café");
    }

    #[test]
    fn size_and_alignment() {
        assert_eq!(core::mem::size_of::<PodString<32>>(), 33);
        assert_eq!(core::mem::align_of::<PodString<32>>(), 1);
        assert_eq!(core::mem::size_of::<PodString<0>>(), 1);
        assert_eq!(core::mem::align_of::<PodString<0>>(), 1);
    }
}
