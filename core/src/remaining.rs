use solana_account_view::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED};
use solana_program_error::ProgramError;

use crate::error::QuasarError;

const ACCOUNT_HEADER: usize = core::mem::size_of::<RuntimeAccount>()
    + MAX_PERMITTED_DATA_INCREASE
    + core::mem::size_of::<u64>();

const DUP_ENTRY_SIZE: usize = core::mem::size_of::<u64>();

const MAX_REMAINING_ACCOUNTS: usize = 64;

/// Advance past a non-duplicate account, aligning to 8 bytes.
#[inline(always)]
unsafe fn advance_past_account(ptr: *mut u8, raw: *mut RuntimeAccount) -> *mut u8 {
    let next = ptr.add(ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
    next.add((next as usize).wrapping_neg() & 7)
}

/// Advance past a duplicate account entry (u64-sized).
#[inline(always)]
unsafe fn advance_past_dup(ptr: *mut u8) -> *mut u8 {
    ptr.add(DUP_ENTRY_SIZE)
}

/// Zero-allocation remaining accounts accessor.
///
/// Uses a boundary pointer instead of a count — no reads or arithmetic
/// in the dispatch hot path.
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

    /// Access a single remaining account by index. O(n) walk from buffer start.
    pub fn get(&self, index: usize) -> Option<AccountView> {
        let mut ptr = self.ptr;
        for i in 0..=index {
            if ptr as *const u8 >= self.boundary {
                return None;
            }
            let raw = ptr as *mut RuntimeAccount;
            let borrow = unsafe { (*raw).borrow_state };

            if i == index {
                return Some(if borrow == NOT_BORROWED {
                    unsafe { AccountView::new_unchecked(raw) }
                } else {
                    resolve_dup_walk(borrow as usize, self.declared, self.ptr, self.boundary)
                });
            }

            if borrow == NOT_BORROWED {
                ptr = unsafe { advance_past_account(ptr, raw) };
            } else {
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
/// Iterative with a 2-hop depth limit for defense-in-depth.
fn resolve_dup_walk(
    orig_idx: usize,
    declared: &[AccountView],
    start: *mut u8,
    boundary: *const u8,
) -> AccountView {
    let mut idx = orig_idx;
    for _ in 0..2 {
        if idx < declared.len() {
            return unsafe { core::ptr::read(declared.as_ptr().add(idx)) };
        }

        let target = idx - declared.len();
        let mut ptr = start;
        for i in 0..=target {
            if ptr as *const u8 >= boundary {
                break;
            }
            let raw = ptr as *mut RuntimeAccount;
            let borrow = unsafe { (*raw).borrow_state };

            if i == target {
                if borrow == NOT_BORROWED {
                    return unsafe { AccountView::new_unchecked(raw) };
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
    unreachable!("duplicate chain exceeded maximum depth")
}

/// Iterator over remaining accounts. Builds a cache of yielded views
/// for O(1) duplicate resolution. Errors after 64 accounts.
pub struct RemainingIter<'a> {
    ptr: *mut u8,
    boundary: *const u8,
    declared: &'a [AccountView],
    index: usize,
    /// Elements 0..index are initialized.
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

    /// O(1) dup resolution via declared slice or iterator cache.
    #[inline(always)]
    fn resolve_dup(&self, orig_idx: usize) -> Option<AccountView> {
        if orig_idx < self.declared.len() {
            Some(unsafe { core::ptr::read(self.declared.as_ptr().add(orig_idx)) })
        } else {
            let remaining_idx = orig_idx - self.declared.len();
            if remaining_idx >= self.index {
                return None;
            }
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
        let borrow = unsafe { (*raw).borrow_state };

        let view = if borrow == NOT_BORROWED {
            let view = unsafe { AccountView::new_unchecked(raw) };
            self.ptr = unsafe { advance_past_account(self.ptr, raw) };
            view
        } else {
            self.ptr = unsafe { advance_past_dup(self.ptr) };
            self.resolve_dup(borrow as usize)?
        };

        unsafe {
            let copy = core::ptr::read(&view);
            core::ptr::write(self.cache_mut_ptr().add(self.index), copy);
        }
        self.index = self.index.wrapping_add(1);
        Some(Ok(view))
    }
}
