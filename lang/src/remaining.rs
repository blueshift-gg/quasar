use {
    crate::{
        account_load::AccountLoad,
        error::QuasarError,
        svm::{Cursor, RawEntry},
    },
    solana_account_view::{AccountView, Ref, RefMut},
    solana_address::Address,
    solana_program_error::ProgramError,
};

// `data_len` (u64) -> usize cast in `advance_past_account` is lossless on
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
    "AccountView must not implement Drop; ptr::read copies rely on this"
);

/// Maximum number of remaining accounts the iterator will yield
/// before returning an error. Prevents unbounded stack usage in
/// the cache array.
const MAX_REMAINING_ACCOUNTS: usize = 64;

/// Returns `true` if the cache has room for another entry.
///
/// The iterator calls this before every cache write. Extracted so Kani
/// can prove the capacity guard implies all cache accesses are in bounds.
#[inline(always)]
const fn cache_has_capacity(index: usize) -> bool {
    index < MAX_REMAINING_ACCOUNTS
}

/// Safe handle for one remaining account entry.
///
/// Duplicate entries resolve to their canonical runtime account, but this type
/// does not expose raw unchecked account access through safe methods. Data
/// borrows use the runtime borrow state, so duplicate entries remain safe.
#[repr(transparent)]
pub struct RemainingAccount {
    view: AccountView,
}

impl RemainingAccount {
    #[inline(always)]
    fn new(view: AccountView) -> Self {
        Self { view }
    }

    /// Returns the account address.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }

    /// Returns whether the transaction signed for this account.
    #[inline(always)]
    pub fn is_signer(&self) -> bool {
        self.view.is_signer()
    }

    /// Returns the account owner.
    #[inline(always)]
    pub fn owner(&self) -> &Address {
        self.view.owner()
    }

    /// Returns whether the account was provided as writable.
    #[inline(always)]
    pub fn is_writable(&self) -> bool {
        self.view.is_writable()
    }

    /// Returns whether the account is executable.
    #[inline(always)]
    pub fn executable(&self) -> bool {
        self.view.executable()
    }

    /// Returns the current lamport balance.
    #[inline(always)]
    pub fn lamports(&self) -> u64 {
        self.view.lamports()
    }

    /// Returns the account-data length in bytes.
    #[inline(always)]
    pub fn data_len(&self) -> usize {
        self.view.data_len()
    }

    /// Immutably borrows the account data through runtime borrow tracking.
    #[inline(always)]
    pub fn try_borrow_data(&self) -> Result<Ref<'_, [u8]>, ProgramError> {
        self.view.try_borrow()
    }

    /// Mutably borrows the account data through runtime borrow tracking.
    #[inline(always)]
    pub fn try_borrow_data_mut(&mut self) -> Result<RefMut<'_, [u8]>, ProgramError> {
        self.view.try_borrow_mut()
    }

    /// # Safety
    ///
    /// The returned view may alias declared accounts or other remaining account
    /// entries. Callers must uphold all aliasing and borrow invariants before
    /// using unchecked account access through the raw view.
    #[inline(always)]
    pub unsafe fn as_account_view_unchecked(&self) -> &AccountView {
        &self.view
    }

    /// # Safety
    ///
    /// Same requirements as [`Self::as_account_view_unchecked`], plus callers
    /// must ensure no aliases are used while mutating through the returned
    /// view.
    #[inline(always)]
    pub unsafe fn as_account_view_unchecked_mut(&mut self) -> &mut AccountView {
        &mut self.view
    }
}

/// Zero-allocation remaining accounts accessor.
///
/// Uses a boundary pointer instead of a count, with no reads or arithmetic
/// in the dispatch hot path. The `ptr` starts at the first remaining
/// account in the SVM input buffer; `boundary` marks the end. The iterator
/// keeps a small stack cache of previously yielded accounts so duplicate metas
/// can be resolved without allocating.
pub struct RemainingAccounts<'a> {
    /// Current position in the SVM input buffer.
    ptr: *mut u8,
    /// End-of-buffer marker (start of instruction data).
    boundary: *const u8,
    /// Previously parsed declared accounts (for dup resolution).
    declared: &'a [AccountView],
    /// Program ID for typed account-group parsing.
    program_id: Option<&'a Address>,
    /// Instruction data for typed account-group parsing.
    data: &'a [u8],
}

impl<'a> RemainingAccounts<'a> {
    /// Creates a remaining accounts accessor from the SVM buffer
    /// pointers.
    ///
    /// # Safety
    ///
    /// `ptr` and `boundary` must delimit a valid remaining-account region of
    /// the SVM input buffer (`ptr <= boundary`, both within the same live
    /// allocation), and `declared` must be the declared-account slice parsed
    /// from that same buffer so duplicate resolution stays in bounds. Safe
    /// callers should obtain a `RemainingAccounts` via
    /// [`CtxWithRemaining::remaining_accounts`](crate::context::CtxWithRemaining::remaining_accounts)
    /// instead of constructing one directly.
    #[inline(always)]
    pub unsafe fn new(ptr: *mut u8, boundary: *const u8, declared: &'a [AccountView]) -> Self {
        Self {
            ptr,
            boundary,
            declared,
            program_id: None,
            data: &[],
        }
    }

    /// Creates a remaining accounts accessor that can parse typed account
    /// groups requiring program ID and instruction data.
    ///
    /// # Safety
    ///
    /// Same contract as [`new`](Self::new): `ptr`/`boundary` must delimit a
    /// valid remaining-account region of the SVM input buffer and `declared`
    /// must be the matching declared-account slice.
    #[inline(always)]
    pub unsafe fn new_with_context(
        ptr: *mut u8,
        boundary: *const u8,
        declared: &'a [AccountView],
        program_id: &'a Address,
        data: &'a [u8],
    ) -> Self {
        Self {
            ptr,
            boundary,
            declared,
            program_id: Some(program_id),
            data,
        }
    }
    /// Returns `true` if there are no remaining accounts.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.ptr as *const u8 >= self.boundary
    }

    /// Access a single remaining account by index. O(n) walk from buffer
    /// start.
    pub fn get(&self, index: usize) -> Result<Option<RemainingAccount>, ProgramError> {
        // SAFETY: `self.ptr`/`self.boundary` delimit the remaining region.
        let mut cursor = unsafe { Cursor::new(self.ptr, self.boundary) };
        for i in 0..=index {
            if cursor.at_end() {
                return Ok(None);
            }
            // SAFETY: not at end (checked above). Advancing at `i == index` is
            // harmless: the entry is returned and the cursor discarded.
            match unsafe { cursor.next() } {
                // SAFETY: Non-duplicate entry; `raw` is a valid `RuntimeAccount`.
                RawEntry::Account(raw) if i == index => {
                    return Ok(Some(RemainingAccount::new(unsafe {
                        AccountView::new_unchecked(raw)
                    })));
                }
                RawEntry::Dup(borrow) if i == index => {
                    return Ok(Some(RemainingAccount::new(resolve_dup_walk(
                        borrow as usize,
                        self.declared,
                        self.ptr,
                        self.boundary,
                    )?)));
                }
                _ => {}
            }
        }
        Ok(None)
    }

    /// Returns an iterator that yields each remaining account in order.
    /// Builds an index as it walks; duplicate resolution is O(1),
    /// same pattern as the declared accounts parser in the entrypoint.
    ///
    /// Returns `Err(QuasarError::RemainingAccountsOverflow)` if more than
    /// `MAX_REMAINING_ACCOUNTS` are accessed via the iterator.
    #[inline(always)]
    pub fn iter(&self) -> RemainingIter<'a> {
        RemainingIterImpl {
            ptr: self.ptr,
            boundary: self.boundary,
            declared: self.declared,
            index: 0,
            cache: core::mem::MaybeUninit::uninit(),
        }
    }

    /// Parses all remaining entries into at most `N` typed items.
    ///
    /// Composite item types may consume more than one raw account per item;
    /// `N` always bounds the number of resulting `T` values.
    #[inline(always)]
    pub fn parse<T, const N: usize>(&self) -> Result<Remaining<T, N>, ProgramError>
    where
        T: RemainingItem<'a>,
    {
        Remaining::parse(Self {
            ptr: self.ptr,
            boundary: self.boundary,
            declared: self.declared,
            program_id: self.program_id,
            data: self.data,
        })
    }
}

#[doc(hidden)]
pub trait RemainingItem<'input>: Sized {
    const COUNT: usize;
    const REJECT_DUPLICATES: bool = true;

    /// # Safety
    ///
    /// `account` must be an initialized account view already checked against
    /// declared/remaining duplicates.
    unsafe fn parse_remaining_one(
        account: AccountView,
        program_id: Option<&Address>,
        data: &[u8],
    ) -> Result<Self, ProgramError> {
        let mut account = core::mem::MaybeUninit::new(account);
        // SAFETY: `account` was initialized above and the temporary slice has
        // exactly one initialized `AccountView`.
        let accounts = unsafe { core::slice::from_raw_parts_mut(account.as_mut_ptr(), 1) };
        // SAFETY: The one-element slice satisfies the default single-account
        // chunk contract.
        unsafe { Self::parse_remaining_chunk(accounts, program_id, data) }
    }

    /// # Safety
    ///
    /// `accounts` must contain exactly `Self::COUNT` initialized account
    /// views, already checked against declared/remaining duplicates.
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        program_id: Option<&Address>,
        data: &[u8],
    ) -> Result<Self, ProgramError>;
}

#[doc(hidden)]
#[inline(always)]
pub fn parse_remaining_view<T: AccountLoad>(view: &AccountView) -> Result<T, ProgramError> {
    if T::IS_SIGNER && !view.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if T::IS_EXECUTABLE && !view.executable() {
        return Err(ProgramError::InvalidAccountData);
    }
    T::load_checked(view)
}

#[doc(hidden)]
#[inline(always)]
pub fn parse_remaining_account<T: AccountLoad>(
    accounts: &[AccountView],
) -> Result<T, ProgramError> {
    let view = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    parse_remaining_view::<T>(view)
}

impl<'input, T> RemainingItem<'input> for crate::accounts::Account<T>
where
    T: crate::traits::AsAccountView
        + crate::account_load::AccountLoad
        + crate::traits::CheckOwner
        + crate::traits::StaticView,
{
    const COUNT: usize = 1;

    #[inline(always)]
    unsafe fn parse_remaining_one(
        account: AccountView,
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_view::<Self>(&account)
    }

    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_account::<Self>(accounts)
    }
}

impl<'input, T> RemainingItem<'input> for crate::accounts::InterfaceAccount<T>
where
    T: crate::traits::Owners + crate::account_load::AccountLoad,
{
    const COUNT: usize = 1;

    #[inline(always)]
    unsafe fn parse_remaining_one(
        account: AccountView,
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_view::<Self>(&account)
    }

    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_account::<Self>(accounts)
    }
}

impl<'input, T> RemainingItem<'input> for crate::accounts::Program<T>
where
    T: crate::traits::Id,
{
    const COUNT: usize = 1;

    #[inline(always)]
    unsafe fn parse_remaining_one(
        account: AccountView,
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_view::<Self>(&account)
    }

    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_account::<Self>(accounts)
    }
}

impl<'input, T> RemainingItem<'input> for crate::accounts::Interface<T>
where
    T: crate::traits::ProgramInterface,
{
    const COUNT: usize = 1;

    #[inline(always)]
    unsafe fn parse_remaining_one(
        account: AccountView,
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_view::<Self>(&account)
    }

    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_account::<Self>(accounts)
    }
}

impl<'input, T> RemainingItem<'input> for crate::accounts::Sysvar<T>
where
    T: crate::sysvars::Sysvar,
{
    const COUNT: usize = 1;

    #[inline(always)]
    unsafe fn parse_remaining_one(
        account: AccountView,
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_view::<Self>(&account)
    }

    #[inline(always)]
    unsafe fn parse_remaining_chunk(
        accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        parse_remaining_account::<Self>(accounts)
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
        // First hop: a dup pointing back into the declared accounts resolves
        // directly (the declared-only prefix of the split index space).
        if let Some(view) = crate::svm::resolve_dup(idx, crate::svm::DupSources::Declared(declared))
        {
            return Ok(view);
        }

        // Otherwise `idx >= declared.len()`; index the remaining region.
        let target = match idx.checked_sub(declared.len()) {
            Some(target) => target,
            None => return Err(ProgramError::InvalidAccountData),
        };
        // SAFETY: `start`/`boundary` delimit the remaining region (same buffer
        // walk as `RemainingAccounts::get`).
        let mut cursor = unsafe { Cursor::new(start, boundary) };
        for i in 0..=target {
            if cursor.at_end() {
                break;
            }
            // SAFETY: not at end (checked above). At `i == target` the cursor
            // advances one entry too far, but it is discarded on return/break.
            match unsafe { cursor.next() } {
                RawEntry::Account(raw) if i == target => {
                    // SAFETY: The target entry is a full runtime account.
                    return Ok(unsafe { AccountView::new_unchecked(raw) });
                }
                RawEntry::Dup(borrow) if i == target => {
                    idx = borrow as usize;
                    break;
                }
                _ => {}
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
pub type RemainingIter<'a> = RemainingIterImpl<'a>;

/// Bounded typed view over a remaining-account tail.
///
/// `Remaining<T, N>` accepts any number of typed remaining items up to `N`.
/// For single account wrappers, one item consumes one raw remaining account.
/// For `#[derive(Accounts)]` groups, one item consumes the group's fixed
/// account count. Use raw [`RemainingAccounts`] when the tail is intentionally
/// uncapped or forwarded without local validation.
pub struct Remaining<T, const N: usize> {
    items: [core::mem::MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> Remaining<T, N> {
    /// Parse up to `N` typed remaining items.
    ///
    /// # Cost
    ///
    /// When `T::REJECT_DUPLICATES` is set, every parsed account is compared for
    /// address equality against every account already seen (and every declared
    /// account), so the duplicate scan is **O(n²)** in the number of remaining
    /// accounts — each new account walks the already-consumed prefix. The raw
    /// account count remains bounded by `MAX_REMAINING_ACCOUNTS`, while `N`
    /// always means typed items (including when one item consumes multiple raw
    /// accounts).
    #[inline(always)]
    pub fn parse<'input>(accounts: RemainingAccounts<'input>) -> Result<Self, ProgramError>
    where
        T: RemainingItem<'input>,
    {
        let mut out = Self {
            // SAFETY: An uninitialized `[MaybeUninit<T>; N]` is valid.
            items: unsafe {
                core::mem::MaybeUninit::<[core::mem::MaybeUninit<T>; N]>::uninit().assume_init()
            },
            len: 0,
        };
        // SAFETY: An uninitialized
        // `[MaybeUninit<AccountView>; MAX_REMAINING_ACCOUNTS]` is valid.
        let mut chunk = unsafe {
            core::mem::MaybeUninit::<
                [core::mem::MaybeUninit<AccountView>; MAX_REMAINING_ACCOUNTS],
            >::uninit()
            .assume_init()
        };
        let mut raw_len = 0usize;
        let mut chunk_len = 0usize;
        let chunk_count = T::COUNT;

        if chunk_count == 0 || chunk_count > MAX_REMAINING_ACCOUNTS {
            return Err(ProgramError::InvalidAccountData);
        }
        if chunk_count == 1 {
            return Self::parse_single(accounts);
        }

        // SAFETY: `accounts.ptr`/`accounts.boundary` delimit the region.
        let mut cursor = unsafe { Cursor::new(accounts.ptr, accounts.boundary) };
        while !cursor.at_end() {
            if out.len >= N {
                return Err(QuasarError::RemainingAccountsOverflow.into());
            }
            if raw_len >= MAX_REMAINING_ACCOUNTS {
                return Err(QuasarError::RemainingAccountsOverflow.into());
            }

            // The multi-account chunk parser never resolves duplicates: a dup
            // marker in a fixed-count group is always an error.
            let entry_start = cursor.ptr();
            // SAFETY: not at end (checked above).
            let view = match unsafe { cursor.next() } {
                // SAFETY: Non-duplicate entry with a valid `RuntimeAccount`.
                RawEntry::Account(raw) => unsafe { AccountView::new_unchecked(raw) },
                RawEntry::Dup(_) => {
                    return Err(QuasarError::RemainingAccountDuplicate.into());
                }
            };

            if T::REJECT_DUPLICATES {
                if accounts
                    .declared
                    .iter()
                    .any(|declared| crate::keys_eq(declared.address(), view.address()))
                {
                    return Err(QuasarError::RemainingAccountDuplicate.into());
                }
                // Re-scan the already-consumed raw prefix instead of reserving
                // `N * T::COUNT` addresses on the 4 KiB SBF stack. This keeps
                // `N` as the typed-item capacity and preserves the existing
                // O(n^2) duplicate-checking cost model.
                // SAFETY: `accounts.ptr..entry_start` is the valid prefix
                // consumed by this cursor from the same remaining region.
                if unsafe { prefix_contains_address(accounts.ptr, entry_start, view.address()) } {
                    return Err(QuasarError::RemainingAccountDuplicate.into());
                }
            }

            chunk[chunk_len].write(view);
            chunk_len += 1;
            raw_len += 1;

            if chunk_len == chunk_count {
                let chunk_ptr = chunk.as_mut_ptr() as *mut AccountView;
                // SAFETY: `chunk_len == chunk_count`, so the first
                // `chunk_count` entries are initialized.
                let chunk_slice =
                    unsafe { core::slice::from_raw_parts_mut(chunk_ptr, chunk_count) };
                // SAFETY: The slice contains exactly `T::COUNT` initialized
                // account views and duplicate checks have already run.
                let item = unsafe {
                    T::parse_remaining_chunk(chunk_slice, accounts.program_id, accounts.data)?
                };
                out.items[out.len].write(item);
                out.len += 1;
                chunk_len = 0;
            }
        }

        if chunk_len != 0 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        Ok(out)
    }

    #[inline(always)]
    fn parse_single<'input>(accounts: RemainingAccounts<'input>) -> Result<Self, ProgramError>
    where
        T: RemainingItem<'input>,
    {
        let mut out = Self {
            // SAFETY: An uninitialized `[MaybeUninit<T>; N]` is valid.
            items: unsafe {
                core::mem::MaybeUninit::<[core::mem::MaybeUninit<T>; N]>::uninit().assume_init()
            },
            len: 0,
        };
        // SAFETY: An uninitialized `[MaybeUninit<Address>; N]` is valid.
        let mut seen = unsafe {
            core::mem::MaybeUninit::<[core::mem::MaybeUninit<Address>; N]>::uninit().assume_init()
        };
        // SAFETY: `accounts.ptr`/`accounts.boundary` delimit the region.
        let mut cursor = unsafe { Cursor::new(accounts.ptr, accounts.boundary) };
        while !cursor.at_end() {
            if out.len >= N {
                return Err(QuasarError::RemainingAccountsOverflow.into());
            }

            // SAFETY: not at end (checked above).
            let view = match unsafe { cursor.next() } {
                // SAFETY: Non-duplicate entry with a valid `RuntimeAccount`.
                RawEntry::Account(raw) => unsafe { AccountView::new_unchecked(raw) },
                RawEntry::Dup(borrow) => {
                    if T::REJECT_DUPLICATES {
                        return Err(QuasarError::RemainingAccountDuplicate.into());
                    }
                    resolve_dup_walk(
                        borrow as usize,
                        accounts.declared,
                        accounts.ptr,
                        accounts.boundary,
                    )?
                }
            };

            let address = *view.address();
            if T::REJECT_DUPLICATES {
                if accounts
                    .declared
                    .iter()
                    .any(|declared| crate::keys_eq(declared.address(), &address))
                {
                    return Err(QuasarError::RemainingAccountDuplicate.into());
                }
                let mut i = 0usize;
                while i < out.len {
                    // SAFETY: The first `out.len` seen-address slots were
                    // initialized alongside parsed output items.
                    let seen_address = unsafe { seen[i].assume_init_ref() };
                    if crate::keys_eq(seen_address, &address) {
                        return Err(QuasarError::RemainingAccountDuplicate.into());
                    }
                    i += 1;
                }
                seen[out.len].write(address);
            }

            // SAFETY: `view` is initialized and duplicate policy checks have
            // already run for this remaining item.
            let item = unsafe { T::parse_remaining_one(view, accounts.program_id, accounts.data)? };
            out.items[out.len].write(item);
            out.len += 1;
        }

        Ok(out)
    }
}

/// Return whether a previously consumed remaining-account prefix contains an
/// address (or a duplicate marker, which is invalid for typed account groups).
///
/// # Safety
///
/// `start..boundary` must delimit a valid, fully consumed prefix of one SVM
/// remaining-account region.
#[inline(always)]
unsafe fn prefix_contains_address(start: *mut u8, boundary: *const u8, address: &Address) -> bool {
    // SAFETY: upheld by the caller.
    let mut cursor = unsafe { Cursor::new(start, boundary) };
    while !cursor.at_end() {
        // SAFETY: the cursor is not at the prefix boundary.
        match unsafe { cursor.next() } {
            RawEntry::Account(raw) => {
                // SAFETY: a non-duplicate entry is a valid RuntimeAccount.
                let view = unsafe { AccountView::new_unchecked(raw) };
                if crate::keys_eq(view.address(), address) {
                    return true;
                }
            }
            RawEntry::Dup(_) => return true,
        }
    }
    false
}

impl<T, const N: usize> Remaining<T, N> {
    /// Returns the successfully parsed typed items.
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: Only the first `self.len` entries are initialized, and `len`
        // is incremented after each successful write.
        unsafe { core::slice::from_raw_parts(self.items.as_ptr() as *const T, self.len) }
    }

    /// Iterates over the successfully parsed typed items.
    #[inline(always)]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Returns the number of successfully parsed items.
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the compile-time typed-item capacity.
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns whether no typed items were parsed.
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T, const N: usize> Drop for Remaining<T, N> {
    fn drop(&mut self) {
        if !core::mem::needs_drop::<T>() {
            return;
        }
        let mut i = 0usize;
        while i < self.len {
            // SAFETY: Only slots below `self.len` are initialized.
            unsafe { self.items[i].assume_init_drop() };
            i += 1;
        }
    }
}

impl<T, const N: usize> AsRef<[T]> for Remaining<T, N> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

#[doc(hidden)]
pub struct RemainingIterImpl<'a> {
    /// Current position in the SVM input buffer.
    ptr: *mut u8,
    /// End-of-buffer marker.
    boundary: *const u8,
    /// Previously parsed declared accounts (for dup resolution).
    declared: &'a [AccountView],
    /// Number of accounts yielded so far.
    index: usize,
    /// Cache of yielded views. Elements `0..index` are initialized.
    cache: core::mem::MaybeUninit<[AccountView; MAX_REMAINING_ACCOUNTS]>,
}

impl RemainingIterImpl<'_> {
    #[inline(always)]
    fn cache_ptr(&self) -> *const AccountView {
        self.cache.as_ptr() as *const AccountView
    }

    #[inline(always)]
    fn cache_mut_ptr(&mut self) -> *mut AccountView {
        self.cache.as_mut_ptr() as *mut AccountView
    }

    /// O(1) dup resolution via declared slice or iterator cache.
    ///
    /// Delegates to the single [`crate::svm::resolve_dup`] over the split
    /// `[declared ++ cache]` index space; the cache is the prefix of yielded
    /// views (`0..self.index`).
    #[inline(always)]
    fn resolve_dup(&self, orig_idx: usize) -> Option<AccountView> {
        // SAFETY: cache slots `0..self.index` were initialized by prior
        // `next()` calls, so this slice is fully initialized.
        let cache = unsafe { core::slice::from_raw_parts(self.cache_ptr(), self.index) };
        crate::svm::resolve_dup(
            orig_idx,
            crate::svm::DupSources::Cache {
                declared: self.declared,
                cache,
            },
        )
    }
}

impl Iterator for RemainingIterImpl<'_> {
    type Item = Result<RemainingAccount, ProgramError>;

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

        // SAFETY: `self.ptr` is within the SVM buffer (boundary check above),
        // 8-aligned, and delimited by `self.boundary`.
        let mut cursor = unsafe { Cursor::new(self.ptr, self.boundary) };
        // SAFETY: the cursor is not at end (boundary check above).
        let view = match unsafe { cursor.next() } {
            // SAFETY: Non-duplicate entry with a valid `RuntimeAccount`.
            RawEntry::Account(raw) => unsafe { AccountView::new_unchecked(raw) },
            RawEntry::Dup(borrow) => match self.resolve_dup(borrow as usize) {
                Some(v) => v,
                None => {
                    // Fuse: an unresolvable dup has advanced the cursor past the
                    // entry but skips the cache-index increment below, so the
                    // cache would desync from the buffer position. Jump `ptr`
                    // to `boundary` so the next `next()` returns `None` and
                    // iteration terminates instead of yielding mis-cached
                    // entries.
                    self.ptr = self.boundary as *mut u8;
                    return Some(Err(QuasarError::RemainingAccountDuplicate.into()));
                }
            },
        };
        self.ptr = cursor.ptr();

        // SAFETY: `self.index < MAX_REMAINING_ACCOUNTS` (checked above),
        // so the write is within the `MaybeUninit` cache allocation.
        // `ptr::read` creates a bitwise copy (AccountView is not Copy).
        unsafe {
            let copy = core::ptr::read(&view);
            core::ptr::write(self.cache_mut_ptr().add(self.index), copy);
        }
        self.index += 1;
        Some(Ok(RemainingAccount::new(view)))
    }
}

#[cfg(kani)]
#[path = "../kani/remaining.rs"]
mod kani_proofs;
