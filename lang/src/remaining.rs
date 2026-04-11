use {
    crate::error::QuasarError,
    solana_account_view::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED},
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

/// Size of a non-duplicate account entry in the SVM input buffer:
/// `RuntimeAccount` header + 10 KiB realloc region + u64 padding.
const ACCOUNT_HEADER: usize = core::mem::size_of::<RuntimeAccount>()
    + MAX_PERMITTED_DATA_INCREASE
    + core::mem::size_of::<u64>();

/// Size of a duplicate account entry in the SVM input buffer.
const DUP_ENTRY_SIZE: usize = core::mem::size_of::<u64>();

/// Maximum number of remaining accounts the iterator will yield
/// before returning an error. Prevents unbounded stack usage in
/// the cache array.
const MAX_REMAINING_ACCOUNTS: usize = 64;

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
/// - `raw` must be a valid `RuntimeAccount` at `ptr`.
#[inline(always)]
unsafe fn advance_past_account(ptr: *mut u8, raw: *mut RuntimeAccount) -> *mut u8 {
    let next = ptr.add(ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
    next.add((next as usize).wrapping_neg() & 7)
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
        // Bloom filter starts empty — seeded lazily on first `next()` call.
        // This avoids the O(declared.len()) seeding cost when the iterator
        // is constructed but never consumed (e.g. early-return after
        // `is_empty()` check).
        RemainingIter {
            ptr: self.ptr,
            boundary: self.boundary,
            declared: self.declared,
            mode: self.mode,
            index: 0,
            cache: core::mem::MaybeUninit::uninit(),
            bloom: [0u64; 4],
            bloom_seeded: false,
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
    /// 256-bit bloom filter for fast-reject in `has_seen_address`. Seeded
    /// lazily on first `next()` with declared account addresses, then updated
    /// on each yield. False positives fall through to the exact `keys_eq`
    /// scan; false negatives are impossible (all inserted addresses set
    /// their bit).
    bloom: [u64; 4],
    /// Whether the bloom filter has been seeded with declared accounts.
    /// Lazy initialization avoids the O(declared.len()) seeding cost when
    /// the iterator is never consumed.
    bloom_seeded: bool,
}

/// Hash an address into a (bucket, bit) pair for the 256-bit bloom filter.
/// Uses XOR of the first two bytes for the 8-bit hash — high entropy for
/// Solana pubkeys which are uniformly distributed.
#[inline(always)]
fn bloom_hash(addr: &solana_address::Address) -> (usize, u64) {
    let b = addr.as_array();
    let h = (b[0] as usize) ^ (b[16] as usize);
    (h >> 6, 1u64 << (h & 63))
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

    #[inline(always)]
    fn has_seen_address(&self, address: &solana_address::Address) -> bool {
        // Fast-reject via bloom filter: if the bit is not set, the address
        // has definitely not been seen — skip the O(n) linear scan.
        let (idx, bit) = bloom_hash(address);
        if self.bloom[idx] & bit == 0 {
            return false;
        }

        // Bloom hit — could be a false positive. Do the exact scan.
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
    #[inline(always)]
    fn resolve_dup(&self, orig_idx: usize) -> Option<AccountView> {
        if orig_idx < self.declared.len() {
            // SAFETY: Index is within bounds of the declared accounts slice.
            Some(unsafe { core::ptr::read(self.declared.as_ptr().add(orig_idx)) })
        } else {
            let remaining_idx = orig_idx - self.declared.len();
            if remaining_idx >= self.index {
                return None;
            }
            // SAFETY: `remaining_idx < self.index` guarantees this cache slot
            // was initialized by a prior `next()` call.
            Some(unsafe { core::ptr::read(self.cache_ptr().add(remaining_idx)) })
        }
    }
}

impl Iterator for RemainingIter<'_> {
    type Item = Result<AccountView, ProgramError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr as *const u8 >= self.boundary {
            return None;
        }
        if crate::utils::hint::unlikely(self.index >= MAX_REMAINING_ACCOUNTS) {
            self.ptr = self.boundary as *mut u8;
            return Some(Err(QuasarError::RemainingAccountsOverflow.into()));
        }

        // Lazy bloom seeding: deferred from iter() to first next() call.
        // Saves ~14 CU per declared account when the iterator is created
        // but never consumed. The bool check is ~1 CU per subsequent call
        // (branch-predicted after first call).
        if !self.bloom_seeded {
            if self.mode == RemainingMode::Strict {
                for view in self.declared.iter() {
                    let (idx, bit) = bloom_hash(view.address());
                    self.bloom[idx] |= bit;
                }
            }
            self.bloom_seeded = true;
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

        // Update bloom filter for strict-mode duplicate detection.
        if self.mode == RemainingMode::Strict {
            let (bidx, bit) = bloom_hash(view.address());
            self.bloom[bidx] |= bit;
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
