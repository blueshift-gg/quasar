//! The single owner of the SVM account-buffer walk.
//!
//! The Solana loader serializes the transaction's accounts into the input
//! buffer as a sequence of two entry shapes (see [`crate::svm_abi`] for the
//! authoritative ABI description):
//!
//! - a **non-duplicate** entry: a [`RuntimeAccount`] header followed by its
//!   data and realloc padding, occupying [`account_stride`] bytes;
//! - a **duplicate** entry: an 8-byte marker whose first byte is the index of
//!   the source account it aliases, occupying [`DUP_ENTRY_SIZE`] bytes.
//!
//! A non-duplicate entry is distinguished by its `borrow_state` byte being
//! [`NOT_BORROWED`] (`0xFF`); any other value is a duplicate marker carrying
//! the source index in that byte.
//!
//! [`Cursor`] is the *only* place in the crate that decodes this distinction
//! and advances past an entry — i.e. the only caller of [`account_stride`] and
//! [`DUP_ENTRY_SIZE`], and the only reader of `borrow_state` for a walk step.
//! Every buffer walk (`parse_all_accounts_unchecked`, the remaining-account
//! iterator and parsers, one-off `get`/dup resolution) drives a `Cursor`, so
//! the walk semantics are defined once and covered once by Miri and Kani.

use {
    crate::__internal::{account_stride, ACCOUNT_HEADER, DUP_ENTRY_SIZE},
    solana_account_view::{AccountView, RuntimeAccount, NOT_BORROWED},
};

/// Advance a buffer pointer past the non-duplicate account whose header starts
/// at `ptr`, given its `data_len`, using the pointer-rounding form.
///
/// This is **value-equal** to `ptr.add(account_stride(data_len))` —
/// `ACCOUNT_HEADER` is a multiple of 8, so header+data rounded up to the next
/// 8-byte boundary lands on the same address either way — but it is retained as
/// a distinct form on purpose: the two expressions lower to different SBF
/// instruction schedules, and the hot single-field parsers
/// (`__internal::parse_account`/`parse_account_dup`) are size/CU-sensitive.
/// Empirically, rewriting them to call `account_stride` moved `.so` size in
/// both directions across the example programs (escrow -1 KiB, multisig +176
/// B), so those parsers keep this tuned form, de-duplicated here into one
/// `#[inline(always)]` definition. [`Cursor`], which owns the full walk decode,
/// uses `account_stride` directly (its walk sites were already on that form, so
/// routing them through `Cursor` is byte-neutral).
///
/// # Safety
///
/// `ptr` must be the 8-byte-aligned start of a non-duplicate account entry with
/// `data_len` bytes of data, within the SVM input allocation. The returned
/// pointer is at most one-past-the-end of that entry.
#[inline(always)]
pub(crate) unsafe fn advance_account_data(ptr: *mut u8, data_len: usize) -> *mut u8 {
    // SAFETY: header + data stays within the account entry.
    let ptr = unsafe { ptr.add(ACCOUNT_HEADER.wrapping_add(data_len)) };
    // SAFETY: rounding up to the next 8-byte boundary stays within the entry's
    // trailing alignment padding.
    unsafe { ptr.add((ptr as usize).wrapping_neg() & 7) }
}

/// Classify an account entry's `borrow_state` byte (byte 0 of its header).
///
/// This is the **sole** `NOT_BORROWED` comparison in the crate — the one place
/// the non-duplicate/duplicate distinction is decoded. [`Cursor::next`] uses it
/// for the full walk (reading `borrow_state` off a `RuntimeAccount`); the hot
/// single-field parser [`crate::__internal::parse_account_dup`] uses it on the
/// low byte of the `u32` header it already loaded (that parser keeps its tuned
/// `advance_account_data` stride form rather than driving a `Cursor`, so it
/// cannot go through `next()` without a size/CU delta — but it shares this one
/// decode).
///
/// Returns `None` for a non-duplicate entry (`borrow_state == NOT_BORROWED`,
/// `0xFF`) and `Some(idx)` for a duplicate marker aliasing source index `idx`.
#[inline(always)]
pub(crate) fn classify_borrow_state(borrow_state: u8) -> Option<u8> {
    if borrow_state == NOT_BORROWED {
        None
    } else {
        Some(borrow_state)
    }
}

/// One decoded entry from the SVM account buffer, produced by
/// [`Cursor::next`].
///
/// The cursor has already advanced past the entry by the time this is
/// returned; the payload identifies what the entry *was*.
pub(crate) enum RawEntry {
    /// A non-duplicate account: a pointer to its [`RuntimeAccount`] header.
    Account(*mut RuntimeAccount),
    /// A duplicate marker carrying the source account index (`borrow_state`).
    Dup(u8),
}

/// A forward-only walk over the SVM account region.
///
/// `ptr` is the current position; `boundary` is one-past-the-end of the
/// account region (the start of the instruction-data length word). The cursor
/// never reads `boundary` — it only compares against it in [`Cursor::at_end`];
/// callers that walk a known number of entries (e.g. the hot-path single-field
/// parsers) may ignore the boundary entirely.
pub(crate) struct Cursor {
    ptr: *mut u8,
    boundary: *const u8,
}

impl Cursor {
    /// Create a cursor over `[ptr, boundary)`.
    ///
    /// # Safety
    ///
    /// - `ptr` must be 8-byte aligned and point at the start of an account
    ///   entry (or at `boundary` for an empty region), within the same live SVM
    ///   input allocation as `boundary`.
    /// - `boundary` must be one-past-the-end of the account region (the SVM
    ///   guarantees `ptr <= boundary`).
    #[inline(always)]
    pub unsafe fn new(ptr: *mut u8, boundary: *const u8) -> Self {
        debug_assert!(
            ptr as usize & 7 == 0,
            "Cursor::new: ptr is not 8-byte aligned"
        );
        Self { ptr, boundary }
    }

    /// Returns `true` once the cursor has reached the account boundary.
    #[inline(always)]
    pub fn at_end(&self) -> bool {
        self.ptr as *const u8 >= self.boundary
    }

    /// The current buffer position.
    #[inline(always)]
    pub fn ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Decode the entry at the current position and advance past it.
    ///
    /// This is the sole walk step: it reads `borrow_state` to classify the
    /// entry, advances `self.ptr` by [`account_stride`] (non-duplicate) or
    /// [`DUP_ENTRY_SIZE`] (duplicate), and returns the classification.
    ///
    /// # Safety
    ///
    /// The cursor must not be [`at_end`](Self::at_end): `self.ptr` must point
    /// at a valid account entry within the region. For a non-duplicate entry
    /// the full [`RuntimeAccount`] header (including `data_len`) must be
    /// readable at `self.ptr`.
    #[inline(always)]
    pub unsafe fn next(&mut self) -> RawEntry {
        let raw = self.ptr as *mut RuntimeAccount;
        // SAFETY: `self.ptr` points at a valid `RuntimeAccount` header
        // (caller contract); `borrow_state` is byte 0 of the `#[repr(C)]`
        // struct.
        let borrow = unsafe { (*raw).borrow_state };

        match classify_borrow_state(borrow) {
            None => {
                // SAFETY: a non-duplicate entry has a full header; `data_len` is
                // valid to read.
                let data_len = unsafe { (*raw).data_len as usize };
                // SAFETY: a non-duplicate entry occupies exactly
                // `account_stride(data_len)` bytes (header + data + padding,
                // rounded up to 8), so the new pointer is at most `boundary`.
                self.ptr = unsafe { self.ptr.add(account_stride(data_len)) };
                RawEntry::Account(raw)
            }
            Some(idx) => {
                // SAFETY: a duplicate entry occupies exactly `DUP_ENTRY_SIZE`
                // (8) bytes.
                self.ptr = unsafe { self.ptr.add(DUP_ENTRY_SIZE) };
                RawEntry::Dup(idx)
            }
        }
    }
}

/// Where a duplicate marker's source index (`idx`) is resolved from.
///
/// A duplicate marker carries the loader's **global** account index — the
/// position of the account it aliases in the flat, in-order sequence of every
/// account the SVM serialized for the instruction. Each variant materializes a
/// prefix of that global sequence differently, so `idx`'s meaning ("index
/// space") differs per variant even though it always denotes the same global
/// position. Each variant documents its space; [`resolve_dup`] is the single
/// reader of all three.
pub(crate) enum DupSources<'a> {
    /// **Flat buffer space.** `idx` indexes `base[0..count]` directly: the
    /// declared/raw parsers write each parsed [`AccountView`] to the slot equal
    /// to its global index, so buffer slot == global index. `Some` iff
    /// `idx < count` (a dup must alias an *earlier* slot). Used by
    /// `parse_all_accounts_unchecked` and `parse_account_dup`.
    Buffer {
        /// Start of the output slot array (slots `0..count` initialized).
        base: *const AccountView,
        /// Number of accounts parsed so far == the current slot's global index.
        count: usize,
    },
    /// **Split declared++cache space.** `idx` indexes the concatenation
    /// `[declared ++ cache]`: `idx < declared.len()` aliases a declared account
    /// (a dup pointing back before the remaining region), otherwise
    /// `idx - declared.len()` indexes `cache` — the remaining accounts already
    /// yielded, in yield order (yield order == buffer order, so cache slot ==
    /// remaining-region-local global index). `None` iff `idx` is past both.
    /// Used by the remaining-account iterator (`RemainingIterImpl`).
    Cache {
        /// The instruction's declared accounts (the global-index prefix).
        declared: &'a [AccountView],
        /// Remaining accounts yielded so far, in order.
        cache: &'a [AccountView],
    },
    /// **Declared-only prefix** of the split space: `idx < declared.len()`
    /// aliases a declared account; any larger `idx` returns `None` because it
    /// is not resolvable from a slice — the caller walks the remaining
    /// region for it (see `resolve_dup_walk`). Used for the first hop of
    /// the one-off `get`/`parse_single` walk resolver.
    Declared(&'a [AccountView]),
}

/// Resolve a duplicate marker's source index to the aliased [`AccountView`], or
/// `None` when `idx` is outside the range addressable from `sources`.
///
/// This is the single dup-index resolver: every duplicate-account copy in the
/// crate (declared/raw parsing in `lib.rs`, the remaining-account iterator and
/// the one-off walk resolver in `remaining.rs`) routes its index arithmetic
/// through here. The [`DupSources`] variant selects the index space; all three
/// ultimately address the loader's global account index (see the variant docs).
///
/// The returned value is a bitwise copy of the source entry.
/// [`AccountView`] is not `Copy` (to make aliasing explicit) but implements no
/// `Drop`, so `ptr::read` is sound — the copy simply aliases the same
/// [`RuntimeAccount`]; the caller is responsible for borrow discipline across
/// aliases, exactly as before this was unified.
#[inline(always)]
pub(crate) fn resolve_dup(idx: usize, sources: DupSources<'_>) -> Option<AccountView> {
    match sources {
        DupSources::Buffer { base, count } => {
            if idx < count {
                // SAFETY: `idx < count`, and slots `0..count` were initialized
                // by the caller as it parsed earlier accounts.
                Some(unsafe { core::ptr::read(base.add(idx)) })
            } else {
                None
            }
        }
        DupSources::Declared(declared) => resolve_from_slice(idx, declared),
        DupSources::Cache { declared, cache } => match resolve_from_slice(idx, declared) {
            some @ Some(_) => some,
            None => resolve_from_slice(idx - declared.len(), cache),
        },
    }
}

/// Read `slice[idx]` as a bitwise [`AccountView`] copy, or `None` if out of
/// range. Shared by the [`DupSources::Declared`] and [`DupSources::Cache`]
/// arms.
#[inline(always)]
fn resolve_from_slice(idx: usize, slice: &[AccountView]) -> Option<AccountView> {
    if idx < slice.len() {
        // SAFETY: `idx < slice.len()`; `AccountView` has no `Drop`, so the
        // bitwise copy aliases the same `RuntimeAccount` soundly.
        Some(unsafe { core::ptr::read(slice.as_ptr().add(idx)) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::__internal::account_stride,
        solana_account_view::{AccountView, MAX_PERMITTED_DATA_INCREASE},
        solana_address::Address,
    };

    /// Stride of a zero-data account (`ACCOUNT_HEADER`, already 8-aligned).
    const ZERO: usize = account_stride(0);

    /// 8-byte-aligned backing store for a synthetic account region, big enough
    /// for any case here.
    #[repr(C, align(8))]
    struct SvmBuf {
        bytes: [u8; 4 * (ZERO + MAX_PERMITTED_DATA_INCREASE)],
    }

    impl SvmBuf {
        fn new() -> Self {
            Self {
                bytes: [0u8; 4 * (ZERO + MAX_PERMITTED_DATA_INCREASE)],
            }
        }

        fn base(&mut self) -> *mut u8 {
            self.bytes.as_mut_ptr()
        }

        fn boundary(&self, len: usize) -> *const u8 {
            // SAFETY: `len` is within the buffer.
            unsafe { self.bytes.as_ptr().add(len) }
        }

        /// Write a non-duplicate header at `offset`; returns the stride.
        ///
        /// # Safety
        /// `offset` must be 8-aligned and leave room for the header + data.
        unsafe fn write_account(&mut self, offset: usize, addr_byte: u8, data_len: u64) -> usize {
            // SAFETY: caller guarantees room at `offset`.
            let raw = unsafe { self.base().add(offset) as *mut RuntimeAccount };
            unsafe {
                (*raw).borrow_state = NOT_BORROWED;
                (*raw).is_signer = 0;
                (*raw).is_writable = 1;
                (*raw).executable = 0;
                (*raw).padding = [0u8; 4];
                (*raw).address = Address::new_from_array([addr_byte; 32]);
                (*raw).owner = Address::new_from_array([0xAA; 32]);
                (*raw).lamports = 100;
                (*raw).data_len = data_len;
            }
            account_stride(data_len as usize)
        }

        /// Write a duplicate marker (source index `idx`) at `offset`.
        ///
        /// # Safety
        /// `offset` must be 8-aligned and leave room for `DUP_ENTRY_SIZE`.
        unsafe fn write_dup(&mut self, offset: usize, idx: u8) -> usize {
            // SAFETY: caller guarantees the 8-byte entry fits at `offset`.
            unsafe { *self.base().add(offset) = idx };
            DUP_ENTRY_SIZE
        }
    }

    #[test]
    fn empty_region_is_at_end() {
        let mut buf = SvmBuf::new();
        let base = buf.base();
        // SAFETY: empty region: ptr == boundary.
        let cursor = unsafe { Cursor::new(base, base as *const u8) };
        assert!(cursor.at_end());
        assert_eq!(cursor.ptr(), base);
    }

    #[test]
    fn single_account() {
        let mut buf = SvmBuf::new();
        // SAFETY: fresh buffer, offset 0 fits a zero-data account.
        let stride = unsafe { buf.write_account(0, 0x01, 0) };
        let base = buf.base();
        let boundary = buf.boundary(stride);
        // SAFETY: base/boundary delimit the one-account region built above.
        let mut cursor = unsafe { Cursor::new(base, boundary) };

        assert!(!cursor.at_end());
        // SAFETY: cursor is not at end.
        match unsafe { cursor.next() } {
            RawEntry::Account(raw) => {
                // SAFETY: `raw` is the account just written.
                let view = unsafe { AccountView::new_unchecked(raw) };
                assert_eq!(view.address().as_array()[0], 0x01);
            }
            RawEntry::Dup(_) => panic!("expected account"),
        }
        assert_eq!(cursor.ptr() as usize, base as usize + stride);
        assert!(cursor.at_end());
    }

    #[test]
    fn account_then_dup() {
        let mut buf = SvmBuf::new();
        // SAFETY: offsets are 8-aligned and in range.
        let s0 = unsafe { buf.write_account(0, 0x01, 0) };
        let s1 = unsafe { buf.write_dup(s0, 0) };
        let base = buf.base();
        let boundary = buf.boundary(s0 + s1);
        // SAFETY: region built above.
        let mut cursor = unsafe { Cursor::new(base, boundary) };

        // SAFETY: not at end.
        assert!(matches!(unsafe { cursor.next() }, RawEntry::Account(_)));
        assert_eq!(cursor.ptr() as usize, base as usize + s0);
        assert!(!cursor.at_end());
        // SAFETY: not at end.
        match unsafe { cursor.next() } {
            RawEntry::Dup(idx) => assert_eq!(idx, 0),
            RawEntry::Account(_) => panic!("expected dup"),
        }
        assert_eq!(cursor.ptr() as usize, base as usize + s0 + s1);
        assert!(cursor.at_end());
    }

    #[test]
    fn dup_first() {
        let mut buf = SvmBuf::new();
        // A dup marker as the very first entry (source index 7).
        // SAFETY: offset 0 fits an 8-byte marker.
        let s0 = unsafe { buf.write_dup(0, 7) };
        let s1 = unsafe { buf.write_account(s0, 0x02, 0) };
        let base = buf.base();
        let boundary = buf.boundary(s0 + s1);
        // SAFETY: region built above.
        let mut cursor = unsafe { Cursor::new(base, boundary) };

        // SAFETY: not at end.
        match unsafe { cursor.next() } {
            RawEntry::Dup(idx) => assert_eq!(idx, 7),
            RawEntry::Account(_) => panic!("expected dup"),
        }
        assert_eq!(cursor.ptr() as usize, base as usize + s0);
        // SAFETY: not at end.
        assert!(matches!(unsafe { cursor.next() }, RawEntry::Account(_)));
        assert!(cursor.at_end());
    }

    #[test]
    fn boundary_exact_after_data_account() {
        let mut buf = SvmBuf::new();
        // A non-zero data length forces the stride to round up; the cursor
        // must land exactly on the boundary.
        // SAFETY: offset 0 fits a header + 24 data bytes.
        let stride = unsafe { buf.write_account(0, 0x03, 24) };
        assert_eq!(stride % 8, 0);
        let base = buf.base();
        let boundary = buf.boundary(stride);
        // SAFETY: region built above.
        let mut cursor = unsafe { Cursor::new(base, boundary) };

        // SAFETY: not at end.
        assert!(matches!(unsafe { cursor.next() }, RawEntry::Account(_)));
        assert_eq!(cursor.ptr() as *const u8, boundary);
        assert!(cursor.at_end());
    }
}
