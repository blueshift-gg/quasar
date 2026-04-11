//! Fixed-capacity inline string for zero-copy account data.
//!
//! `PodString<N>` stores up to `N` bytes (max 255) with a `u8` length prefix.
//! Unlike the dynamic `String<P, N>` type, writes never trigger `realloc` or
//! `memmove` — they are a simple `memcpy` + length update (~5 CU vs ~300-500 CU).
//!
//! The tradeoff is rent: the full `N` bytes are always allocated in the account
//! even if the string is shorter or empty. Use `PodString` for small,
//! frequently-mutated fields (labels, names, symbols, tickers).
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
//!
//! # Usage in account structs
//!
//! `PodString<N>` is a fixed-size Pod type — use it directly in `#[account]`
//! structs. Writes go through `DerefMut` on `Account<T>`:
//!
//! ```ignore
//! #[account(discriminator = 1)]
//! pub struct Config {
//!     pub label: PodString<32>,   // 33 bytes in account data
//!     pub owner: Address,
//! }
//!
//! // In instruction handler:
//! ctx.accounts.config.label.set("my-label");
//! ```

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

// Compile-time: N must fit in u8 length prefix.
impl<const N: usize> PodString<N> {
    const _CAP_CHECK: () = assert!(N <= 255, "PodString<N>: N cannot exceed 255 (u8 length prefix)");
}

// Compile-time layout invariants.
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
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        // Clamp to N to prevent out-of-bounds on corrupted account data.
        (self.len as usize).min(N)
    }

    /// Returns `true` if the string is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Maximum number of bytes this string can hold.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns the string as a `&str`.
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
        // SAFETY: `data[..len]` is initialized, `len` clamped to N.
        unsafe { core::slice::from_raw_parts(self.data.as_ptr() as *const u8, len) }
    }

    /// Set the string contents. Returns `false` if `value.len() > N`.
    #[inline(always)]
    pub fn set(&mut self, value: &str) -> bool {
        let vlen = value.len();
        if vlen > N {
            return false;
        }
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
        true
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
            data: [MaybeUninit::uninit(); N],
        }
    }
}

impl<const N: usize> core::ops::Deref for PodString<N> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> AsRef<str> for PodString<N> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> AsRef<[u8]> for PodString<N> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const N: usize> PartialEq for PodString<N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<const N: usize> Eq for PodString<N> {}

impl<const N: usize> PartialEq<str> for PodString<N> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<const N: usize> PartialEq<&str> for PodString<N> {
    #[inline(always)]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
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
        assert!(s.set("hello"));
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.as_bytes(), b"hello");
    }

    #[test]
    fn set_max_length() {
        let mut s = PodString::<5>::default();
        assert!(s.set("abcde"));
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "abcde");
    }

    #[test]
    fn set_over_capacity_returns_false() {
        let mut s = PodString::<3>::default();
        assert!(!s.set("abcd"));
        // Original state unchanged.
        assert!(s.is_empty());
    }

    #[test]
    fn overwrite_shorter() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello world"));
        assert_eq!(s.as_str(), "hello world");
        assert!(s.set("hi"));
        assert_eq!(s.len(), 2);
        assert_eq!(s.as_str(), "hi");
    }

    #[test]
    fn clear() {
        let mut s = PodString::<32>::default();
        assert!(s.set("test"));
        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.as_str(), "");
    }

    #[test]
    fn corrupted_len_clamped() {
        let mut s = PodString::<4>::default();
        assert!(s.set("ab"));
        // Simulate corrupted len > N
        s.len = 255;
        // Should NOT panic — len is clamped to N
        assert_eq!(s.len(), 4);
        assert_eq!(s.as_bytes().len(), 4);
    }

    #[test]
    fn utf8_multibyte() {
        let mut s = PodString::<32>::default();
        assert!(s.set("caf\u{00e9}")); // "café" — 5 bytes in UTF-8
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

    #[test]
    fn deref_to_str() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello"));
        let r: &str = &*s;
        assert_eq!(r, "hello");
        // str methods via Deref
        assert!(s.starts_with("hel"));
        assert!(s.contains("llo"));
    }

    #[test]
    fn partial_eq_str() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello"));
        assert_eq!(s, "hello");
        assert_eq!(s, *"hello");
    }

    #[test]
    fn partial_eq_pod_string() {
        let mut a = PodString::<32>::default();
        let mut b = PodString::<32>::default();
        assert!(a.set("same"));
        assert!(b.set("same"));
        assert_eq!(a, b);
        assert!(b.set("diff"));
        assert_ne!(a, b);
    }

    #[test]
    fn capacity() {
        let s = PodString::<42>::default();
        assert_eq!(s.capacity(), 42);
    }
}
