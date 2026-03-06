use solana_account_view::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED};
use solana_program_error::ProgramError;

use crate::error::QuasarError;

const ACCOUNT_HEADER: usize = core::mem::size_of::<RuntimeAccount>()
    + MAX_PERMITTED_DATA_INCREASE
    + core::mem::size_of::<u64>();

const DUP_ENTRY_SIZE: usize = core::mem::size_of::<u64>();

const MAX_REMAINING_ACCOUNTS: usize = 64;

/// Advance pointer past a non-duplicate account.
///
/// # Safety
/// `raw` must point to a valid RuntimeAccount within the SVM buffer.
/// The returned pointer is aligned to 8 bytes and points to the next
/// account slot (or past the buffer end).
#[inline(always)]
unsafe fn advance_past_account(ptr: *mut u8, raw: *mut RuntimeAccount) -> *mut u8 {
    let next = ptr.add(ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
    ((next as usize).wrapping_add(7) & !7) as *mut u8
}

/// Advance pointer past a duplicate account entry (u64-sized).
#[inline(always)]
unsafe fn advance_past_dup(ptr: *mut u8) -> *mut u8 {
    ptr.add(DUP_ENTRY_SIZE)
}

/// Zero-allocation remaining accounts accessor.
///
/// Uses a boundary pointer (end of accounts region in the SVM buffer) instead
/// of a count. No reads, no arithmetic in the dispatch hot path — the struct
/// is constructed only when `Ctx::remaining_accounts()` is called.
pub struct RemainingAccounts<'a> {
    ptr: *mut u8,
    boundary: *const u8,
    declared: &'a [AccountView],
}

impl<'a> RemainingAccounts<'a> {
    /// Creates a new remaining accounts accessor from the SVM buffer pointers.
    #[inline(always)]
    pub fn new(ptr: *mut u8, boundary: *const u8, declared: &'a [AccountView]) -> Self {
        Self {
            ptr,
            boundary,
            declared,
        }
    }

    /// Returns `true` if there are no remaining accounts.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.ptr as *const u8 >= self.boundary
    }

    /// Access a single remaining account by index. O(n) — walks from the
    /// start of the buffer. Use `iter()` for sequential access.
    pub fn get(&self, index: usize) -> Option<AccountView> {
        let mut ptr = self.ptr;
        for i in 0..=index {
            if ptr as *const u8 >= self.boundary {
                return None;
            }
            let raw = ptr as *mut RuntimeAccount;
            // SAFETY: ptr is within the SVM accounts buffer (bounded by self.boundary).
            // borrow_state is the first field of RuntimeAccount.
            let borrow = unsafe { (*raw).borrow_state };

            if i == index {
                return Some(if borrow == NOT_BORROWED {
                    // SAFETY: raw points to a valid, non-duplicate RuntimeAccount.
                    unsafe { AccountView::new_unchecked(raw) }
                } else {
                    resolve_dup_walk(borrow as usize, self.declared, self.ptr, self.boundary)
                });
            }

            if borrow == NOT_BORROWED {
                // SAFETY: raw is a valid RuntimeAccount; data_len is set by the SVM.
                ptr = unsafe { advance_past_account(ptr, raw) };
            } else {
                // SAFETY: duplicate entries are u64-sized in the SVM buffer.
                ptr = unsafe { advance_past_dup(ptr) };
            }
        }
        None
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
            index: 0,
            cache: core::mem::MaybeUninit::uninit(),
        }
    }
}

/// Walk-based dup resolution for one-off `get()` access.
/// Uses iterative resolution with a depth limit to prevent stack overflow
/// from malformed duplicate chains in the SVM input buffer.
fn resolve_dup_walk(
    orig_idx: usize,
    declared: &[AccountView],
    start: *mut u8,
    boundary: *const u8,
) -> AccountView {
    // Iterative resolution: follow duplicate chains up to 2 hops.
    // The SVM guarantees duplicates resolve in one hop, but we allow
    // a second for defense-in-depth without risking stack overflow.
    let mut idx = orig_idx;
    for _ in 0..2 {
        if idx < declared.len() {
            // SAFETY: idx is within bounds of the declared accounts slice.
            return unsafe { core::ptr::read(declared.as_ptr().add(idx)) };
        }

        let target = idx - declared.len();
        let mut ptr = start;
        for i in 0..=target {
            if ptr as *const u8 >= boundary {
                break;
            }
            let raw = ptr as *mut RuntimeAccount;
            // SAFETY: ptr is within the SVM buffer (bounded by boundary check above).
            let borrow = unsafe { (*raw).borrow_state };

            if i == target {
                if borrow == NOT_BORROWED {
                    // SAFETY: raw points to a valid, non-duplicate RuntimeAccount.
                    return unsafe { AccountView::new_unchecked(raw) };
                }
                // Follow the chain iteratively instead of recursing
                idx = borrow as usize;
                break;
            }

            if borrow == NOT_BORROWED {
                // SAFETY: raw is a valid RuntimeAccount within the SVM buffer.
                ptr = unsafe { advance_past_account(ptr, raw) };
            } else {
                // SAFETY: duplicate entries are u64-sized in the SVM buffer.
                ptr = unsafe { advance_past_dup(ptr) };
            }
        }
    }
    unreachable!("duplicate chain exceeded maximum depth")
}

/// Iterator over remaining accounts in the SVM input buffer.
///
/// Builds a cache of yielded [`AccountView`]s for O(1) duplicate resolution.
/// Returns `Err(QuasarError::RemainingAccountsOverflow)` if more than 64
/// accounts are yielded.
pub struct RemainingIter<'a> {
    ptr: *mut u8,
    boundary: *const u8,
    declared: &'a [AccountView],
    index: usize,
    // SAFETY: Elements 0..index are initialized. Only allocated on the stack
    // when iter() is called — zero cost when remaining accounts aren't used.
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

    /// O(1) dup resolution: declared accounts via slice, previously-yielded
    /// remaining accounts via the iterator's own cache.
    #[inline(always)]
    fn resolve_dup(&self, orig_idx: usize) -> Option<AccountView> {
        if orig_idx < self.declared.len() {
            // SAFETY: orig_idx < declared.len() bounds-checked above.
            Some(unsafe { core::ptr::read(self.declared.as_ptr().add(orig_idx)) })
        } else {
            let remaining_idx = orig_idx - self.declared.len();
            // Hard bounds check: SVM duplicates always reference earlier accounts.
            // A forward-reference would read uninitialized cache memory.
            if remaining_idx >= self.index {
                return None;
            }
            // SAFETY: remaining_idx < self.index, and elements 0..index are initialized
            // by previous next() calls.
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
        if self.index >= MAX_REMAINING_ACCOUNTS {
            self.ptr = self.boundary as *mut u8;
            return Some(Err(QuasarError::RemainingAccountsOverflow.into()));
        }

        let raw = self.ptr as *mut RuntimeAccount;
        // SAFETY: ptr is within the SVM buffer (boundary check above).
        let borrow = unsafe { (*raw).borrow_state };

        let view = if borrow == NOT_BORROWED {
            // SAFETY: raw points to a valid, non-duplicate RuntimeAccount.
            let view = unsafe { AccountView::new_unchecked(raw) };
            // SAFETY: raw is a valid RuntimeAccount within the SVM buffer.
            self.ptr = unsafe { advance_past_account(self.ptr, raw) };
            view
        } else {
            // SAFETY: duplicate entries are u64-sized in the SVM buffer.
            self.ptr = unsafe { advance_past_dup(self.ptr) };
            self.resolve_dup(borrow as usize)?
        };

        // SAFETY: view is a valid AccountView yielded from either new_unchecked or
        // resolve_dup. ptr::write copies into the cache at index self.index, which
        // is < MAX_REMAINING_ACCOUNTS (checked on line above boundary check).
        unsafe {
            let copy = core::ptr::read(&view);
            core::ptr::write(self.cache_mut_ptr().add(self.index), copy);
        }
        self.index += 1;
        Some(Ok(view))
    }
}
