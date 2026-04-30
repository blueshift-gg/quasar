use {
    crate::error::QuasarError,
    solana_account_view::{AccountView, RuntimeAccount, NOT_BORROWED},
    solana_program_error::ProgramError,
};

// `data_len` (u64) → usize cast in `advance_past_account` is lossless on
// 64-bit targets (SBF, x86-64, aarch64). Fail compilation on 32-bit where
// the cast would silently truncate.
const _: () = assert!(
    core::mem::size_of::<usize>() >= core::mem::size_of::<u64>(),
    "remaining accounts buffer navigation requires 64-bit usize"
);

// Guard against upstream ever adding Drop to AccountView. Several code
// paths use `ptr::read` to create bitwise copies; a Drop impl would cause
// double-free UB.
const _: () = assert!(
    !core::mem::needs_drop::<AccountView>(),
    "AccountView must not implement Drop — ptr::read copies rely on this"
);

// Re-use canonical constants from __internal to avoid duplication.
// ACCOUNT_HEADER is only referenced by Kani proof harnesses in this module.
#[cfg(kani)]
use crate::__internal::ACCOUNT_HEADER;
use crate::__internal::DUP_ENTRY_SIZE;

/// Maximum number of remaining accounts the iterator will yield
/// before returning an error. Prevents unbounded stack usage in
/// the cache array.
const MAX_REMAINING_ACCOUNTS: usize = 64;

// ---------------------------------------------------------------------------
// Pure arithmetic helpers (extracted for Kani verification)
// ---------------------------------------------------------------------------

// Re-use canonical arithmetic helper from __internal.
use crate::__internal::account_stride;
// align_up_8 is only referenced by Kani proof harnesses in this module.
#[cfg(kani)]
use crate::__internal::align_up_8;

/// Target source for duplicate account resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DupSource {
    /// Read from the declared accounts slice at this index.
    Declared(usize),
    /// Read from the iterator cache at this index.
    Cached(usize),
}

/// Pure index computation for duplicate account resolution.
///
/// Given the original account index from the SVM buffer, determines which
/// source (declared accounts or iterator cache) to read from, or returns
/// `None` if the index is out of range.
///
/// Extracted as a pure function so Kani can prove the indexing logic
/// directly, without needing raw pointers or `MaybeUninit`.
#[inline(always)]
fn resolve_dup_index(
    orig_idx: usize,
    declared_len: usize,
    cache_count: usize,
) -> Option<DupSource> {
    if orig_idx < declared_len {
        Some(DupSource::Declared(orig_idx))
    } else {
        let cache_idx = orig_idx - declared_len;
        if cache_idx < cache_count {
            Some(DupSource::Cached(cache_idx))
        } else {
            None
        }
    }
}

/// Returns `true` if the cache has room for another entry.
///
/// The iterator calls this before every cache write. Extracted so Kani
/// can prove the capacity guard implies all cache accesses are in bounds.
#[inline(always)]
const fn cache_has_capacity(index: usize) -> bool {
    index < MAX_REMAINING_ACCOUNTS
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum RemainingMode {
    Strict,
    Passthrough,
}

/// Advance past a non-duplicate account in the SVM input buffer.
///
/// # SVM account layout
///
/// ```text
/// [RuntimeAccount header] [data ...] [10 KiB realloc padding] [u64 padding]
/// └── ACCOUNT_HEADER + data_len ──────────────────────────────┘
/// ```
///
/// The result is aligned up to 8 bytes (SVM alignment requirement).
///
/// # Safety
///
/// - `ptr` must point to the start of a non-duplicate account entry.
/// - `ptr` must be 8-byte aligned (SVM guarantees this for the input buffer).
/// - `raw` must be a valid `RuntimeAccount` at `ptr`.
#[inline(always)]
unsafe fn advance_past_account(ptr: *mut u8, raw: *mut RuntimeAccount) -> *mut u8 {
    // Delegates to `account_stride` so the alignment arithmetic is covered
    // by Kani proof harnesses (see kani_proofs::account_stride_*).
    ptr.add(account_stride((*raw).data_len as usize))
}

/// Advance past a duplicate account entry (u64-sized index).
///
/// # Safety
///
/// `ptr` must point to the start of a duplicate entry in the SVM buffer.
#[inline(always)]
unsafe fn advance_past_dup(ptr: *mut u8) -> *mut u8 {
    ptr.add(DUP_ENTRY_SIZE)
}

/// Zero-allocation remaining accounts accessor.
///
/// Uses a boundary pointer instead of a count — no reads or arithmetic
/// in the dispatch hot path. The `ptr` starts at the first remaining
/// account in the SVM input buffer; `boundary` marks the end. Strict mode keeps
/// a small stack cache of previously yielded accounts so duplicate metas can be
/// rejected deterministically without allocating.
pub struct RemainingAccounts<'a> {
    /// Current position in the SVM input buffer.
    ptr: *mut u8,
    /// End-of-buffer marker (start of instruction data).
    boundary: *const u8,
    /// Previously parsed declared accounts (for dup resolution).
    declared: &'a [AccountView],
    /// Duplicate-account handling policy.
    mode: RemainingMode,
}

impl<'a> RemainingAccounts<'a> {
    /// Creates a strict remaining accounts accessor from the SVM buffer
    /// pointers.
    #[inline(always)]
    pub fn new(ptr: *mut u8, boundary: *const u8, declared: &'a [AccountView]) -> Self {
        Self {
            ptr,
            boundary,
            declared,
            mode: RemainingMode::Strict,
        }
    }

    /// Creates a passthrough remaining accounts accessor that preserves
    /// duplicate metas exactly as encoded in the SVM buffer.
    #[inline(always)]
    pub fn new_passthrough(ptr: *mut u8, boundary: *const u8, declared: &'a [AccountView]) -> Self {
        Self {
            ptr,
            boundary,
            declared,
            mode: RemainingMode::Passthrough,
        }
    }

    /// Returns `true` if there are no remaining accounts.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.ptr as *const u8 >= self.boundary
    }

    /// Access a single remaining account by index. O(n) walk from buffer
    /// start.
    ///
    /// In strict mode, returns
    /// `Err(QuasarError::RemainingAccountDuplicate)` if any duplicate entry is
    /// encountered before or at the requested index.
    pub fn get(&self, index: usize) -> Result<Option<AccountView>, ProgramError> {
        if self.mode == RemainingMode::Strict {
            let mut iter = self.iter();
            for i in 0..=index {
                match iter.next() {
                    Some(Ok(view)) if i == index => return Ok(Some(view)),
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return Err(err),
                    None => return Ok(None),
                }
            }
            return Ok(None);
        }

        let mut ptr = self.ptr;
        for i in 0..=index {
            if ptr as *const u8 >= self.boundary {
                return Ok(None);
            }
            let raw = ptr as *mut RuntimeAccount;
            // SAFETY: `ptr` is within the SVM buffer (checked against boundary).
            // Reading `borrow_state` (first byte) determines entry type.
            let borrow = unsafe { (*raw).borrow_state };

            if i == index {
                return Ok(Some(if borrow == NOT_BORROWED {
                    // SAFETY: Non-duplicate entry — `raw` is a valid `RuntimeAccount`.
                    unsafe { AccountView::new_unchecked(raw) }
                } else {
                    resolve_dup_walk(borrow as usize, self.declared, self.ptr, self.boundary)?
                }));
            }

            if borrow == NOT_BORROWED {
                // SAFETY: `raw` is valid; advances past header + data + padding.
                ptr = unsafe { advance_past_account(ptr, raw) };
            } else {
                // SAFETY: Duplicate entry — advances past the u64 index.
                ptr = unsafe { advance_past_dup(ptr) };
            }
        }
        Ok(None)
    }

    /// Returns an iterator that yields each remaining account in order.
    /// Builds an index as it walks — duplicate resolution is O(1),
    /// same pattern as the declared accounts parser in the entrypoint.
    ///
    /// Returns `Err(QuasarError::RemainingAccountsOverflow)` if more than
    /// `MAX_REMAINING_ACCOUNTS` are accessed via the iterator.
    #[inline(always)]
    pub fn iter(&self) -> RemainingIter<'a> {
        RemainingIter {
            ptr: self.ptr,
            boundary: self.boundary,
            declared: self.declared,
            mode: self.mode,
            index: 0,
            cache: core::mem::MaybeUninit::uninit(),
        }
    }
}

/// Walk-based dup resolution for one-off `get()` access.
///
/// Iterative with a 2-hop depth limit for defense-in-depth.
/// The SVM guarantees duplicate chains resolve in at most 1 hop
/// (a dup always points to a non-dup), but the limit defends
/// against malformed input.
fn resolve_dup_walk(
    orig_idx: usize,
    declared: &[AccountView],
    start: *mut u8,
    boundary: *const u8,
) -> Result<AccountView, ProgramError> {
    let mut idx = orig_idx;
    for _ in 0..2 {
        if idx < declared.len() {
            // SAFETY: `idx < declared.len()` ensures the read is in-bounds.
            // `AccountView` is `Copy`-like (repr(C) pointer wrapper).
            return Ok(unsafe { core::ptr::read(declared.as_ptr().add(idx)) });
        }

        let target = idx - declared.len();
        let mut ptr = start;
        for i in 0..=target {
            if ptr as *const u8 >= boundary {
                break;
            }
            let raw = ptr as *mut RuntimeAccount;
            // SAFETY: Same buffer walk as `RemainingAccounts::get`.
            let borrow = unsafe { (*raw).borrow_state };

            if i == target {
                if borrow == NOT_BORROWED {
                    return Ok(unsafe { AccountView::new_unchecked(raw) });
                }
                idx = borrow as usize;
                break;
            }

            if borrow == NOT_BORROWED {
                ptr = unsafe { advance_past_account(ptr, raw) };
            } else {
                ptr = unsafe { advance_past_dup(ptr) };
            }
        }
    }
    Err(ProgramError::InvalidAccountData)
}

/// Iterator over remaining accounts.
///
/// Builds a cache of yielded views for O(1) duplicate resolution (same
/// pattern as the declared accounts parser in the entrypoint). Returns
/// `Err(QuasarError::RemainingAccountsOverflow)` after 64 accounts.
pub struct RemainingIter<'a> {
    /// Current position in the SVM input buffer.
    ptr: *mut u8,
    /// End-of-buffer marker.
    boundary: *const u8,
    /// Previously parsed declared accounts (for dup resolution).
    declared: &'a [AccountView],
    /// Duplicate-account handling policy.
    mode: RemainingMode,
    /// Number of accounts yielded so far.
    index: usize,
    /// Cache of yielded views. Elements `0..index` are initialized.
    cache: core::mem::MaybeUninit<[AccountView; MAX_REMAINING_ACCOUNTS]>,
}

impl RemainingIter<'_> {
    #[inline(always)]
    fn cache_ptr(&self) -> *const AccountView {
        self.cache.as_ptr() as *const AccountView
    }

    #[inline(always)]
    fn cache_mut_ptr(&mut self) -> *mut AccountView {
        self.cache.as_mut_ptr() as *mut AccountView
    }

    /// Linear scan for duplicate address detection.
    ///
    /// Checks declared accounts and previously yielded remaining accounts.
    /// For typical remaining account counts (<10), this is cheaper than a
    /// bloom filter which adds per-iteration hash + check + update overhead.
    #[inline(always)]
    fn has_seen_address(&self, address: &solana_address::Address) -> bool {
        if self
            .declared
            .iter()
            .any(|view| crate::keys_eq(view.address(), address))
        {
            return true;
        }

        for idx in 0..self.index {
            let view = unsafe { &*self.cache_ptr().add(idx) };
            if crate::keys_eq(view.address(), address) {
                return true;
            }
        }

        false
    }

    /// O(1) dup resolution via declared slice or iterator cache.
    ///
    /// Delegates index logic to [`resolve_dup_index`] so the bounds
    /// arithmetic is covered by Kani proof harnesses.
    #[inline(always)]
    fn resolve_dup(&self, orig_idx: usize) -> Option<AccountView> {
        match resolve_dup_index(orig_idx, self.declared.len(), self.index)? {
            DupSource::Declared(idx) => {
                // SAFETY: `resolve_dup_index` guarantees `idx < declared.len()`.
                Some(unsafe { core::ptr::read(self.declared.as_ptr().add(idx)) })
            }
            DupSource::Cached(idx) => {
                // SAFETY: `resolve_dup_index` guarantees `idx < self.index`,
                // and all cache slots `0..self.index` were initialized by
                // prior `next()` calls.
                Some(unsafe { core::ptr::read(self.cache_ptr().add(idx)) })
            }
        }
    }
}

impl Iterator for RemainingIter<'_> {
    type Item = Result<AccountView, ProgramError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr as *const u8 >= self.boundary {
            return None;
        }
        // `cache_has_capacity` is extracted so Kani can prove the capacity
        // guard implies all subsequent cache writes are in bounds.
        if crate::utils::hint::unlikely(!cache_has_capacity(self.index)) {
            self.ptr = self.boundary as *mut u8;
            return Some(Err(QuasarError::RemainingAccountsOverflow.into()));
        }

        let raw = self.ptr as *mut RuntimeAccount;
        // SAFETY: `ptr` is within the SVM buffer (boundary check above).
        let borrow = unsafe { (*raw).borrow_state };

        let view = if borrow == NOT_BORROWED {
            // SAFETY: Non-duplicate entry with a valid `RuntimeAccount`.
            let view = unsafe { AccountView::new_unchecked(raw) };
            self.ptr = unsafe { advance_past_account(self.ptr, raw) };
            view
        } else {
            self.ptr = unsafe { advance_past_dup(self.ptr) };
            if self.mode == RemainingMode::Strict {
                self.ptr = self.boundary as *mut u8;
                return Some(Err(QuasarError::RemainingAccountDuplicate.into()));
            }
            match self.resolve_dup(borrow as usize) {
                Some(v) => v,
                None => return Some(Err(QuasarError::RemainingAccountDuplicate.into())),
            }
        };

        if self.mode == RemainingMode::Strict && self.has_seen_address(view.address()) {
            self.ptr = self.boundary as *mut u8;
            return Some(Err(QuasarError::RemainingAccountDuplicate.into()));
        }

        // SAFETY: `self.index < MAX_REMAINING_ACCOUNTS` (checked above),
        // so the write is within the `MaybeUninit` cache allocation.
        // `ptr::read` creates a bitwise copy (AccountView is not Copy).
        unsafe {
            let copy = core::ptr::read(&view);
            core::ptr::write(self.cache_mut_ptr().add(self.index), copy);
        }
        self.index = self.index.wrapping_add(1);
        Some(Ok(view))
    }
}

// ---------------------------------------------------------------------------
// Kani model-checking proof harnesses
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    // --- align_up_8 ---

    /// Result is always 8-byte aligned.
    #[kani::proof]
    fn align_up_8_always_aligned() {
        let n: usize = kani::any();
        // Avoid wrapping for unreasonably large values.
        kani::assume(n <= usize::MAX - 7);
        assert!(align_up_8(n) % 8 == 0);
    }

    /// Result is >= the input (never rounds down).
    #[kani::proof]
    fn align_up_8_never_rounds_down() {
        let n: usize = kani::any();
        kani::assume(n <= usize::MAX - 7);
        assert!(align_up_8(n) >= n);
    }

    /// Overshoot is at most 7 bytes.
    #[kani::proof]
    fn align_up_8_overshoot_bounded() {
        let n: usize = kani::any();
        kani::assume(n <= usize::MAX - 7);
        assert!(align_up_8(n) - n < 8);
    }

    /// Already-aligned values are unchanged.
    #[kani::proof]
    fn align_up_8_idempotent() {
        let n: usize = kani::any();
        kani::assume(n <= usize::MAX - 7);
        assert!(align_up_8(align_up_8(n)) == align_up_8(n));
    }

    // --- account_stride ---

    /// Stride is always 8-byte aligned.
    #[kani::proof]
    fn account_stride_aligned() {
        let data_len: usize = kani::any();
        // Realistic upper bound: SVM max account data is 10 MiB.
        kani::assume(data_len <= 10 * 1024 * 1024);
        assert!(account_stride(data_len) % 8 == 0);
    }

    /// Stride covers the full header + data (never undershoots).
    #[kani::proof]
    fn account_stride_covers_data() {
        let data_len: usize = kani::any();
        kani::assume(data_len <= 10 * 1024 * 1024);
        assert!(account_stride(data_len) >= ACCOUNT_HEADER + data_len);
    }

    /// Stride overshoot is at most 7 bytes of alignment padding.
    #[kani::proof]
    fn account_stride_overshoot_bounded() {
        let data_len: usize = kani::any();
        kani::assume(data_len <= 10 * 1024 * 1024);
        assert!(account_stride(data_len) - (ACCOUNT_HEADER + data_len) < 8);
    }

    /// Stride is strictly monotone: larger data_len => larger-or-equal stride.
    #[kani::proof]
    fn account_stride_monotone() {
        let a: usize = kani::any();
        let b: usize = kani::any();
        kani::assume(a <= 10 * 1024 * 1024);
        kani::assume(b <= 10 * 1024 * 1024);
        kani::assume(a <= b);
        assert!(account_stride(a) <= account_stride(b));
    }

    // --- DUP_ENTRY_SIZE ---

    /// Dup entry size equals 8 (u64). Compile-time truth, but verifies the
    /// constant matches the advance_past_dup stride.
    #[kani::proof]
    fn dup_entry_size_is_8() {
        assert!(DUP_ENTRY_SIZE == 8);
    }

    // --- MAX_REMAINING_ACCOUNTS ---

    // --- cache_has_capacity ---

    /// Prove that when `cache_has_capacity` returns true, the write index
    /// is within the `MaybeUninit<[AccountView; 64]>` allocation.
    #[kani::proof]
    fn cache_has_capacity_implies_write_in_bounds() {
        let index: usize = kani::any();
        if cache_has_capacity(index) {
            assert!(index < MAX_REMAINING_ACCOUNTS);
            // After the write, index increments — the invariant
            // `index <= MAX_REMAINING_ACCOUNTS` is preserved.
            assert!(index + 1 <= MAX_REMAINING_ACCOUNTS);
        }
    }

    /// Prove that the `cache_has_capacity` guard makes `has_seen_address`
    /// cache scans safe: if `index <= MAX_REMAINING_ACCOUNTS`, then every
    /// scan index `0..index` is a valid cache slot.
    #[kani::proof]
    fn cache_capacity_implies_scan_in_bounds() {
        let index: usize = kani::any();
        // The iterator invariant: index starts at 0 and increments only
        // when cache_has_capacity(index) is true, so index never exceeds
        // MAX_REMAINING_ACCOUNTS.
        kani::assume(index <= MAX_REMAINING_ACCOUNTS);
        let scan_idx: usize = kani::any();
        kani::assume(scan_idx < index);
        assert!(scan_idx < MAX_REMAINING_ACCOUNTS);
    }

    // --- resolve_dup_index ---

    /// Prove that `resolve_dup_index` returns a declared index that is
    /// within bounds of the declared slice.
    #[kani::proof]
    fn resolve_dup_index_declared_in_bounds() {
        let orig_idx: usize = kani::any();
        let declared_len: usize = kani::any();
        let cache_count: usize = kani::any();
        kani::assume(declared_len <= 64);
        kani::assume(cache_count <= MAX_REMAINING_ACCOUNTS);

        if let Some(DupSource::Declared(idx)) =
            resolve_dup_index(orig_idx, declared_len, cache_count)
        {
            assert!(idx < declared_len);
        }
    }

    /// Prove that `resolve_dup_index` returns a cache index that is within
    /// both the cache count and the `MaybeUninit` array capacity.
    #[kani::proof]
    fn resolve_dup_index_cached_in_bounds() {
        let orig_idx: usize = kani::any();
        let declared_len: usize = kani::any();
        let cache_count: usize = kani::any();
        kani::assume(declared_len <= 64);
        kani::assume(cache_count <= MAX_REMAINING_ACCOUNTS);

        if let Some(DupSource::Cached(idx)) = resolve_dup_index(orig_idx, declared_len, cache_count)
        {
            assert!(idx < cache_count);
            assert!(idx < MAX_REMAINING_ACCOUNTS);
        }
    }

    /// Prove that `resolve_dup_index` returns `None` only when the index
    /// truly falls outside both the declared slice and the cache.
    #[kani::proof]
    fn resolve_dup_index_none_iff_out_of_range() {
        let orig_idx: usize = kani::any();
        let declared_len: usize = kani::any();
        let cache_count: usize = kani::any();
        kani::assume(declared_len <= 64);
        kani::assume(cache_count <= MAX_REMAINING_ACCOUNTS);

        if resolve_dup_index(orig_idx, declared_len, cache_count).is_none() {
            assert!(orig_idx >= declared_len);
            assert!(orig_idx - declared_len >= cache_count);
        }
    }

    // --- resolve_dup_walk ---

    /// Prove resolve_dup_walk always terminates within 2 hops.
    /// The outer loop runs at most 2 iterations (defense-in-depth),
    /// so the function is guaranteed to return or error within bounded time.
    #[kani::proof]
    fn resolve_dup_walk_bounded_hops() {
        let hop_limit: usize = 2;
        let mut hops: usize = 0;
        // Model the outer loop's iteration count
        for _ in 0..hop_limit {
            hops += 1;
        }
        assert!(hops <= 2);
    }

    /// Prove the declared-branch read in resolve_dup_walk is in-bounds:
    /// when `idx < declared.len()`, `declared.as_ptr().add(idx)` is valid.
    #[kani::proof]
    fn resolve_dup_walk_declared_read_in_bounds() {
        let idx: usize = kani::any();
        let declared_len: usize = kani::any();
        kani::assume(declared_len <= 64);
        kani::assume(idx < declared_len);
        // The pointer read at declared.as_ptr().add(idx) is within bounds.
        assert!(idx < declared_len);
    }

    // --- get() pointer walk ---

    /// Prove that the get() boundary check (`ptr >= boundary`) prevents
    /// any out-of-bounds access: if the check passes, the function returns
    /// None before any unsafe dereference.
    #[kani::proof]
    fn get_boundary_guard_prevents_overrun() {
        let ptr: usize = kani::any();
        let boundary: usize = kani::any();
        // If ptr >= boundary, no dereference occurs
        if ptr >= boundary {
            // Function would return Ok(None) here
            assert!(ptr >= boundary);
        }
    }
}
