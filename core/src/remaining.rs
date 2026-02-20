use solana_account_view::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED};

const ACCOUNT_HEADER: usize = core::mem::size_of::<RuntimeAccount>()
    + MAX_PERMITTED_DATA_INCREASE
    + core::mem::size_of::<u64>();

const MAX_REMAINING_ACCOUNTS: usize = 64;

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
    #[inline(always)]
    pub fn new(ptr: *mut u8, boundary: *const u8, declared: &'a [AccountView]) -> Self {
        Self {
            ptr,
            boundary,
            declared,
        }
    }

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
            let borrow = unsafe { (*raw).borrow_state };

            if i == index {
                return Some(if borrow == NOT_BORROWED {
                    unsafe { AccountView::new_unchecked(raw) }
                } else {
                    resolve_dup_walk(borrow as usize, self.declared, self.ptr, self.boundary)
                });
            }

            if borrow == NOT_BORROWED {
                unsafe {
                    ptr = ptr.add(ACCOUNT_HEADER + (*raw).data_len as usize);
                    ptr = ((ptr as usize + 7) & !7) as *mut u8;
                }
            } else {
                unsafe {
                    ptr = ptr.add(core::mem::size_of::<u64>());
                }
            }
        }
        None
    }

    /// Returns an iterator that yields each remaining account in order.
    /// Builds an index as it walks — duplicate resolution is O(1),
    /// same pattern as the declared accounts parser in the entrypoint.
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
fn resolve_dup_walk(
    orig_idx: usize,
    declared: &[AccountView],
    start: *mut u8,
    boundary: *const u8,
) -> AccountView {
    if orig_idx < declared.len() {
        unsafe { core::ptr::read(declared.as_ptr().add(orig_idx)) }
    } else {
        let target = orig_idx - declared.len();
        let mut ptr = start;
        for i in 0..=target {
            if ptr as *const u8 >= boundary {
                break;
            }
            let raw = ptr as *mut RuntimeAccount;
            let borrow = unsafe { (*raw).borrow_state };

            if i == target {
                return if borrow == NOT_BORROWED {
                    unsafe { AccountView::new_unchecked(raw) }
                } else {
                    resolve_dup_walk(borrow as usize, declared, start, boundary)
                };
            }

            if borrow == NOT_BORROWED {
                unsafe {
                    ptr = ptr.add(ACCOUNT_HEADER + (*raw).data_len as usize);
                    ptr = ((ptr as usize + 7) & !7) as *mut u8;
                }
            } else {
                unsafe {
                    ptr = ptr.add(core::mem::size_of::<u64>());
                }
            }
        }
        unreachable!()
    }
}

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
    fn resolve_dup(&self, orig_idx: usize) -> AccountView {
        if orig_idx < self.declared.len() {
            unsafe { core::ptr::read(self.declared.as_ptr().add(orig_idx)) }
        } else {
            let remaining_idx = orig_idx - self.declared.len();
            // SAFETY: SVM duplicates always reference earlier accounts,
            // so remaining_idx < self.index (already cached).
            debug_assert!(
                remaining_idx < self.index,
                "forward-reference in remaining accounts"
            );
            unsafe { core::ptr::read(self.cache_ptr().add(remaining_idx)) }
        }
    }
}

impl Iterator for RemainingIter<'_> {
    type Item = AccountView;

    fn next(&mut self) -> Option<AccountView> {
        if self.ptr as *const u8 >= self.boundary || self.index >= MAX_REMAINING_ACCOUNTS {
            return None;
        }

        let raw = self.ptr as *mut RuntimeAccount;
        let borrow = unsafe { (*raw).borrow_state };

        let view = if borrow == NOT_BORROWED {
            let view = unsafe { AccountView::new_unchecked(raw) };
            unsafe {
                self.ptr = self.ptr.add(ACCOUNT_HEADER + (*raw).data_len as usize);
                self.ptr = ((self.ptr as usize + 7) & !7) as *mut u8;
            }
            view
        } else {
            unsafe {
                self.ptr = self.ptr.add(core::mem::size_of::<u64>());
            }
            self.resolve_dup(borrow as usize)
        };

        // Cache for future dup resolution — same pattern as the entrypoint.
        // SAFETY: AccountView is a thin pointer wrapper; ptr::read is a bitwise copy.
        unsafe {
            let copy = core::ptr::read(&view);
            core::ptr::write(self.cache_mut_ptr().add(self.index), copy);
        }
        self.index += 1;
        Some(view)
    }
}
