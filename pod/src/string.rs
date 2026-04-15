//! Fixed-capacity inline string for zero-copy account data.
//!
//! `PodString<N, PFX>` stores up to `N` bytes with a `PFX`-byte little-endian
//! length prefix. It is a fixed-size Pod type: the struct always occupies
//! `PFX + N` bytes in memory and on-disk, regardless of the active string
//! length.
//!
//! `PFX` must be `1`, `2`, `4`, or `8`, and defaults to `1` (matching the
//! previous `PodString<N>` signature — no breaking change).
//!
//! # Layout
//!
//! ```text
//! [len: [u8; PFX] LE][data: [MaybeUninit<u8>; N]]
//! ```
//!
//! - Total size: `PFX + N` bytes, alignment 1.
//! - `data[..len]` contains valid UTF-8 bytes.
//! - `data[len..N]` is uninitialized (MaybeUninit).
//!
//! # Usage in account structs
//!
//! **As `PodString<N>` directly (or via `fixed_capacity`):**
//! The full `PFX + N` bytes are always in account data — no realloc ever. Best
//! when the worst-case rent cost is acceptable.
//!
//! ```ignore
//! #[account(discriminator = 1)]
//! pub struct Config {
//!     pub label: PodString<32>,   // always 33 bytes on-chain (PFX=1 default)
//!     pub owner: Address,
//! }
//!
//! // Direct zero-copy write — no guard needed:
//! let ok = ctx.accounts.config.label.set("my-label");
//! ```
//!
//! **As `String<N>` or `String<N, u16>` in `#[account]` structs (dynamic
//! sizing):** The derive macro generates a `DynGuard` RAII wrapper. Account
//! data stores only the active bytes (`[len: PFX bytes LE][active bytes]`), so
//! rent scales with content. `PodString` is used as the stack-local copy —
//! loaded on guard creation, flushed back (with one realloc CPI if size
//! changes) on drop.

use {
    crate::prefix::{cap_check, decode_prefix_len, encode_prefix_len},
    core::mem::MaybeUninit,
};

/// Fixed-capacity inline string stored in account data.
///
/// `PFX` is the byte width of the on-disk length prefix (`1`, `2`, `4`, or
/// `8`). It defaults to `1`, preserving backward compatibility with existing
/// `PodString<N>` usage. Use `PodString<N, 2>` for strings up to 65 535 bytes,
/// or `PodString<N, 4>` for up to 4 GiB.
///
/// # Safety invariants
///
/// - `data[..len]` contains valid UTF-8, written by the program's own `set()`.
/// - Only the owning program can modify account data (SVM invariant).
/// - `create_account` zeros the buffer, so a fresh `PodString` has `len=0`.
/// - Reads clamp `len` to `min(len, N)` to prevent panics on corrupted data.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PodString<const N: usize, const PFX: usize = 1> {
    len: [u8; PFX],
    data: [MaybeUninit<u8>; N],
}

// Compile-time: PFX must be in {1,2,4,8} and N must fit in the prefix.
impl<const N: usize, const PFX: usize> PodString<N, PFX> {
    const _CAP_CHECK: () = cap_check!("PodString<N, PFX>", N, PFX);

    /// Compile-time validity check. Reference this in a `const` context to
    /// verify that `N` and `PFX` are in range at the call site.
    ///
    /// ```ignore
    /// const _: () = PodString::<256, 1>::VALID; // compile error: N exceeds prefix range
    /// ```
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
    /// Decode the on-disk length prefix into a `usize`.
    #[inline(always)]
    fn decode_len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        decode_prefix_len(&self.len)
    }

    /// Encode `n` as a `PFX`-byte little-endian prefix into `self.len`.
    #[inline(always)]
    fn encode_len(&mut self, n: usize) {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        encode_prefix_len(&mut self.len, n);
    }

    /// Number of active bytes in the string.
    #[inline(always)]
    pub fn len(&self) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        // Clamp to N to prevent out-of-bounds on corrupted account data.
        self.decode_len().min(N)
    }

    /// Returns `true` if the string is empty.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.decode_len() == 0
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
    #[must_use = "returns false if value exceeds capacity — unhandled means the write was silently \
                  skipped"]
    #[inline(always)]
    pub fn set(&mut self, value: &str) -> bool {
        let vlen = value.len();
        if vlen > N {
            return false;
        }
        // SAFETY: `vlen <= N` checked above. The source is valid UTF-8
        // (Rust `&str` invariant). Writing to MaybeUninit is always safe.
        unsafe {
            core::ptr::copy_nonoverlapping(value.as_ptr(), self.data.as_mut_ptr() as *mut u8, vlen);
        }
        self.encode_len(vlen);
        true
    }

    /// Append `value` to the string. Returns `false` if remaining capacity
    /// is insufficient.
    #[must_use = "returns false if appending would exceed capacity — unhandled means the append \
                  was silently skipped"]
    #[inline(always)]
    pub fn push_str(&mut self, value: &str) -> bool {
        let cur = self.len();
        let vlen = value.len();
        // Overflow-safe: `cur <= N` is a struct invariant, so `N - cur` cannot
        // wrap.
        if vlen > N - cur {
            return false;
        }
        let new_len = cur + vlen;
        // SAFETY: `new_len <= N` verified above. The destination range
        // `data[cur..new_len]` is within the N-byte capacity. Source and
        // destination are in different allocations (stack vs str), so they
        // cannot overlap.
        unsafe {
            core::ptr::copy_nonoverlapping(
                value.as_ptr(),
                (self.data.as_mut_ptr() as *mut u8).add(cur),
                vlen,
            );
        }
        self.encode_len(new_len);
        true
    }

    /// Shorten the string to `new_len` bytes. No-op if `new_len >= len()`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `new_len` is not on a UTF-8 character
    /// boundary.
    #[inline(always)]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            debug_assert!(
                self.as_str().is_char_boundary(new_len),
                "truncate: new_len is not on a UTF-8 character boundary"
            );
            self.encode_len(new_len);
        }
    }

    /// Clear the string (set length to 0).
    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = [0u8; PFX];
    }

    /// Load from a byte slice containing `[len: PFX bytes LE][utf8 bytes...]`.
    ///
    /// Copies `min(len, N)` bytes into self. Returns the number of bytes
    /// consumed from the source slice (prefix + data).
    ///
    /// The caller must ensure `bytes.len() >= PFX + min(encoded_len, N)`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if the slice is shorter than the encoded length.
    #[inline(always)]
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> usize {
        #[allow(clippy::let_unit_value)]
        let _ = Self::_CAP_CHECK;
        debug_assert!(
            bytes.len() >= PFX,
            "load_from_bytes: slice must have at least PFX bytes"
        );
        // SAFETY: debug_assert above ensures bytes.len() >= PFX. We need a
        // [u8; PFX] reference for decode_prefix_len; try_into is infallible
        // given the length check.
        let pfx_bytes: &[u8; PFX] = bytes[..PFX].try_into().unwrap();
        let slen = decode_prefix_len(pfx_bytes).min(N);
        debug_assert!(
            bytes.len() >= PFX + slen,
            "load_from_bytes: slice too short for encoded length"
        );
        // SAFETY: `slen` is clamped to N, so we write at most N bytes
        // into `self.data`, which has exactly N capacity. Source (account
        // data) and destination (stack) are different allocations, so
        // they cannot overlap.
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

    /// Write `[len: PFX bytes LE][utf8 bytes...]` to a byte slice.
    ///
    /// Returns the number of bytes written (prefix + data).
    ///
    /// The caller must ensure `dest.len() >= PFX + self.len()`.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `dest` is shorter than the encoded length.
    #[inline(always)]
    pub fn write_to_bytes(&self, dest: &mut [u8]) -> usize {
        let slen = self.len();
        debug_assert!(
            dest.len() >= PFX + slen,
            "write_to_bytes: dest too short for encoded length"
        );
        // Write the (possibly clamped) length as PFX LE bytes.
        dest[..PFX].copy_from_slice(&(slen as u64).to_le_bytes()[..PFX]);
        // SAFETY: `slen` is clamped to N via `len()`, so we read at
        // most N bytes from `self.data`. Source (stack) and destination
        // (account data) are different allocations, so they cannot overlap.
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.data.as_ptr() as *const u8,
                dest[PFX..].as_mut_ptr(),
                slen,
            );
        }
        PFX + slen
    }

    /// Total bytes this field occupies when serialized: `PFX + len`.
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
        write!(f, "PodString<{}, {}>(\"{}\")", N, PFX, self.as_str())
    }
}

impl<const N: usize, const PFX: usize> core::fmt::Display for PodString<N, PFX> {
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
        assert!(s.set("abcd")); // initialize all 4 bytes so no MaybeUninit read after corruption
                                // Simulate corrupted len > N (PFX=1, so [u8; 1])
        s.len = [255];
        // Should NOT panic — len is clamped to N
        assert_eq!(s.len(), 4);
        // as_bytes() is over fully-initialized data — no UB
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
        // PFX=1 (default)
        assert_eq!(core::mem::size_of::<PodString<32>>(), 33);
        assert_eq!(core::mem::align_of::<PodString<32>>(), 1);
        assert_eq!(core::mem::size_of::<PodString<0>>(), 1);
        assert_eq!(core::mem::align_of::<PodString<0>>(), 1);
        // PFX=2
        assert_eq!(core::mem::size_of::<PodString<32, 2>>(), 34);
        assert_eq!(core::mem::align_of::<PodString<32, 2>>(), 1);
        // PFX=4
        assert_eq!(core::mem::size_of::<PodString<32, 4>>(), 36);
        assert_eq!(core::mem::align_of::<PodString<32, 4>>(), 1);
    }

    #[test]
    fn deref_to_str() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello"));
        let r: &str = &s;
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

    #[test]
    fn load_from_bytes_empty() {
        let mut s = PodString::<32>::default();
        let bytes = [0u8]; // len=0
        let consumed = s.load_from_bytes(&bytes);
        assert_eq!(consumed, 1);
        assert!(s.is_empty());
        assert_eq!(s.as_str(), "");
    }

    #[test]
    fn load_from_bytes_hello() {
        let mut s = PodString::<32>::default();
        let bytes = [5u8, b'h', b'e', b'l', b'l', b'o'];
        let consumed = s.load_from_bytes(&bytes);
        assert_eq!(consumed, 6);
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn load_from_bytes_clamps_to_n() {
        let mut s = PodString::<3>::default();
        // Source says len=10 but N=3, should clamp
        let bytes = [
            10u8, b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        ];
        let consumed = s.load_from_bytes(&bytes);
        assert_eq!(consumed, 4); // 1 + 3
        assert_eq!(s.len(), 3);
        assert_eq!(s.as_str(), "abc");
    }

    #[test]
    fn write_to_bytes_empty() {
        let s = PodString::<32>::default();
        let mut buf = [0xFFu8; 33];
        let written = s.write_to_bytes(&mut buf);
        assert_eq!(written, 1);
        assert_eq!(buf[0], 0); // len=0
    }

    #[test]
    fn write_to_bytes_with_data() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello"));
        let mut buf = [0u8; 33];
        let written = s.write_to_bytes(&mut buf);
        assert_eq!(written, 6);
        assert_eq!(buf[0], 5); // len=5
        assert_eq!(&buf[1..6], b"hello");
    }

    #[test]
    fn load_write_roundtrip() {
        let mut s = PodString::<32>::default();
        assert!(s.set("test string"));

        let mut buf = [0u8; 33];
        let written = s.write_to_bytes(&mut buf);
        assert_eq!(written, 12); // 1 + 11

        let mut s2 = PodString::<32>::default();
        let consumed = s2.load_from_bytes(&buf);
        assert_eq!(consumed, 12);
        assert_eq!(s2.as_str(), "test string");
    }

    #[test]
    fn serialized_len_string() {
        let mut s = PodString::<32>::default();
        assert_eq!(s.serialized_len(), 1); // empty: just prefix
        assert!(s.set("hi"));
        assert_eq!(s.serialized_len(), 3); // 1 + 2
        assert!(s.set("hello world"));
        assert_eq!(s.serialized_len(), 12); // 1 + 11
    }

    #[test]
    fn load_mutate_write_roundtrip() {
        // Simulate the stack-cache pattern: load → mutate → write back
        let original = [5u8, b'h', b'e', b'l', b'l', b'o'];

        let mut s = PodString::<32>::default();
        s.load_from_bytes(&original);
        assert_eq!(s.as_str(), "hello");

        // Mutate on "stack"
        assert!(s.set("world!"));

        // Write back
        let mut buf = [0u8; 33];
        let written = s.write_to_bytes(&mut buf);
        assert_eq!(written, 7); // 1 + 6
        assert_eq!(buf[0], 6);
        assert_eq!(&buf[1..7], b"world!");
    }

    #[test]
    fn load_from_bytes_utf8_multibyte() {
        let mut s = PodString::<32>::default();
        let cafe = "café"; // 5 bytes in UTF-8
        let mut bytes = [0u8; 6];
        bytes[0] = 5;
        bytes[1..6].copy_from_slice(cafe.as_bytes());
        let consumed = s.load_from_bytes(&bytes);
        assert_eq!(consumed, 6);
        assert_eq!(s.as_str(), "café");
    }

    #[test]
    fn push_str_basic() {
        let mut s = PodString::<10>::default();
        assert!(s.set("hello"));
        assert!(s.push_str(" world"[..5].as_ref())); // " worl" — fits exactly
                                                     // "hello" (5) + " worl" (5) = 10 = N
        assert_eq!(s.len(), 10);
        assert_eq!(s.as_str(), "hello worl");
    }

    #[test]
    fn push_str_exceeds_capacity() {
        let mut s = PodString::<8>::default();
        assert!(s.set("hello"));
        // "hello" (5) + " world" (6) = 11 > 8
        assert!(!s.push_str(" world"));
        // Original content unchanged
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn push_str_empty() {
        let mut s = PodString::<10>::default();
        assert!(s.set("hi"));
        assert!(s.push_str(""));
        assert_eq!(s.as_str(), "hi");
    }

    #[test]
    fn truncate_basic() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello world"));
        s.truncate(5);
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn truncate_noop_when_longer() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello"));
        s.truncate(10); // new_len > len() — no-op
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn truncate_to_zero() {
        let mut s = PodString::<32>::default();
        assert!(s.set("hello"));
        s.truncate(0);
        assert!(s.is_empty());
    }

    // --- PFX=2 tests ---

    #[test]
    fn pfx2_empty() {
        let s = PodString::<100, 2>::default();
        assert!(s.is_empty());
        assert_eq!(s.serialized_len(), 2); // just the 2-byte prefix
    }

    #[test]
    fn pfx2_roundtrip() {
        let mut s = PodString::<200, 2>::default();
        assert!(s.set("hello world"));

        let mut buf = [0u8; 213]; // 2 + 200 + 1 slack
        let written = s.write_to_bytes(&mut buf);
        assert_eq!(written, 13); // 2 + 11
        assert_eq!(&buf[..2], &[11u8, 0]); // len=11 in LE u16

        let mut s2 = PodString::<200, 2>::default();
        let consumed = s2.load_from_bytes(&buf);
        assert_eq!(consumed, 13);
        assert_eq!(s2.as_str(), "hello world");
    }

    #[test]
    fn pfx2_corrupted_len_clamped() {
        let mut s = PodString::<4, 2>::default();
        assert!(s.set("abcd"));
        // Simulate corrupted len > N (PFX=2 → [u8; 2])
        s.len = [0xFF, 0xFF]; // 65535
        assert_eq!(s.len(), 4); // clamped to N
    }

    #[test]
    fn pfx2_serialized_len() {
        let mut s = PodString::<100, 2>::default();
        assert_eq!(s.serialized_len(), 2);
        assert!(s.set("hi"));
        assert_eq!(s.serialized_len(), 4); // 2 + 2
    }

    // --- PFX=4 tests ---

    #[test]
    fn pfx4_roundtrip() {
        let mut s = PodString::<200, 4>::default();
        assert!(s.set("hello world"));

        let mut buf = [0u8; 215]; // 4 + 200 + 1 slack
        let written = s.write_to_bytes(&mut buf);
        assert_eq!(written, 15); // 4 + 11
        assert_eq!(&buf[..4], &[11u8, 0, 0, 0]); // 11 in LE u32

        let mut s2 = PodString::<200, 4>::default();
        let consumed = s2.load_from_bytes(&buf);
        assert_eq!(consumed, 15);
        assert_eq!(s2.as_str(), "hello world");
    }

    #[test]
    fn pfx4_serialized_len() {
        let mut s = PodString::<100, 4>::default();
        assert_eq!(s.serialized_len(), 4);
        assert!(s.set("hi"));
        assert_eq!(s.serialized_len(), 6); // 4 + 2
    }
}
