use {
    super::*,
    crate::{
        __internal::{account_stride, align_up_8, ACCOUNT_HEADER, DUP_ENTRY_SIZE},
        svm::{resolve_dup, Cursor, DupSources, RawEntry},
    },
    solana_account_view::{RuntimeAccount, NOT_BORROWED},
};

#[kani::proof]
fn align_up_8_always_aligned() {
    let n: usize = kani::any();
    kani::assume(n <= usize::MAX - 7);
    assert!(align_up_8(n) % 8 == 0);
}

#[kani::proof]
fn align_up_8_never_rounds_down() {
    let n: usize = kani::any();
    kani::assume(n <= usize::MAX - 7);
    assert!(align_up_8(n) >= n);
}

#[kani::proof]
fn align_up_8_overshoot_bounded() {
    let n: usize = kani::any();
    kani::assume(n <= usize::MAX - 7);
    assert!(align_up_8(n) - n < 8);
}

#[kani::proof]
fn align_up_8_idempotent() {
    let n: usize = kani::any();
    kani::assume(n <= usize::MAX - 7);
    assert!(align_up_8(align_up_8(n)) == align_up_8(n));
}

#[kani::proof]
fn account_stride_aligned() {
    let data_len: usize = kani::any();
    kani::assume(data_len <= 10 * 1024 * 1024);
    assert!(account_stride(data_len) % 8 == 0);
}

#[kani::proof]
fn account_stride_covers_data() {
    let data_len: usize = kani::any();
    kani::assume(data_len <= 10 * 1024 * 1024);
    assert!(account_stride(data_len) >= ACCOUNT_HEADER + data_len);
}

#[kani::proof]
fn account_stride_overshoot_bounded() {
    let data_len: usize = kani::any();
    kani::assume(data_len <= 10 * 1024 * 1024);
    assert!(account_stride(data_len) - (ACCOUNT_HEADER + data_len) < 8);
}

#[kani::proof]
fn account_stride_monotone() {
    let a: usize = kani::any();
    let b: usize = kani::any();
    kani::assume(a <= 10 * 1024 * 1024);
    kani::assume(b <= 10 * 1024 * 1024);
    kani::assume(a <= b);
    assert!(account_stride(a) <= account_stride(b));
}

#[kani::proof]
fn dup_entry_size_is_8() {
    assert!(DUP_ENTRY_SIZE == 8);
}

#[kani::proof]
fn cache_has_capacity_implies_write_in_bounds() {
    let index: usize = kani::any();
    if cache_has_capacity(index) {
        assert!(index < MAX_REMAINING_ACCOUNTS);
        assert!(index + 1 <= MAX_REMAINING_ACCOUNTS);
    }
}

#[kani::proof]
fn cache_capacity_implies_scan_in_bounds() {
    let index: usize = kani::any();
    kani::assume(index <= MAX_REMAINING_ACCOUNTS);
    let scan_idx: usize = kani::any();
    kani::assume(scan_idx < index);
    assert!(scan_idx < MAX_REMAINING_ACCOUNTS);
}

// ---------------------------------------------------------------------------
// Unified dup-resolver (`svm::resolve_dup`) proofs.
//
// These replace the former pure-arithmetic `resolve_dup_index` proofs: the
// index logic now lives in `svm::resolve_dup`. Each proof drives the real
// resolver over a symbolic index and asserts its resolution BOUNDARY — `Some`
// iff the index is within the source's addressable range — which is exactly
// what the old `resolve_dup_index_{declared,cached}_in_bounds` /
// `_none_iff_out_of_range` proofs pinned. The resolved views' *in-buffer* /
// exact-account correctness is proven at the real call sites by
// `remaining_iter_next_in_buffer`, `resolve_dup_walk_reads_in_buffer`, and
// `remaining_parse_single_in_buffer` below. One proof per `DupSources`
// variant's documented index space.

/// Fill `views[0..count]` with `AccountView`s over `count` zero-data accounts
/// written into `buf`, so the resolver's `ptr::read` copies read initialized
/// slots.
///
/// # Safety
/// `buf` holds `>= count * ZERO_ACCT_STRIDE` bytes and `views` has `>= count`
/// slots.
unsafe fn fill_store<const NB: usize>(buf: &mut SvmBuf<NB>, views: *mut AccountView, count: usize) {
    let mut k = 0;
    while k < count {
        // SAFETY: slot `k` fits (caller contract).
        unsafe {
            buf.write_account(k * ZERO_ACCT_STRIDE, NOT_BORROWED, (k as u8) + 1, 0);
            let raw = buf.base().add(k * ZERO_ACCT_STRIDE) as *mut RuntimeAccount;
            views.add(k).write(AccountView::new_unchecked(raw));
        }
        k += 1;
    }
}

/// `DupSources::Buffer`: `idx` indexes `base[0..count]`; resolves iff
/// `idx < count`.
#[kani::proof]
#[kani::unwind(6)]
fn resolve_dup_buffer_bounds() {
    const TOTAL: usize = 4;
    const NB: usize = TOTAL * ZERO_ACCT_STRIDE;

    let mut buf = SvmBuf::<NB>::zeroed();
    // SAFETY: an array of `MaybeUninit` is valid uninitialized.
    let mut views: [core::mem::MaybeUninit<AccountView>; TOTAL] =
        unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    // SAFETY: `buf` holds `TOTAL` accounts; `views` has `TOTAL` slots.
    unsafe { fill_store(&mut buf, views.as_mut_ptr() as *mut AccountView, TOTAL) };
    // SAFETY: `[MaybeUninit<AccountView>; TOTAL]` shares layout with
    // `[AccountView; TOTAL]` and every slot was initialized above.
    let base = views.as_ptr() as *const AccountView;

    let count: usize = kani::any();
    kani::assume(count <= TOTAL);
    let idx: usize = kani::any();
    kani::assume(idx <= TOTAL + 1);

    assert!(resolve_dup(idx, DupSources::Buffer { base, count }).is_some() == (idx < count));
}

/// `DupSources::Declared`: `idx` indexes the declared slice; resolves iff
/// `idx < declared.len()` (a larger index is left to the caller's walk).
#[kani::proof]
#[kani::unwind(6)]
fn resolve_dup_declared_bounds() {
    const D: usize = 4;
    const NB: usize = D * ZERO_ACCT_STRIDE;

    let mut buf = SvmBuf::<NB>::zeroed();
    // SAFETY: an array of `MaybeUninit` is valid uninitialized.
    let mut views: [core::mem::MaybeUninit<AccountView>; D] =
        unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    // SAFETY: `buf` holds `D` accounts; `views` has `D` slots.
    unsafe { fill_store(&mut buf, views.as_mut_ptr() as *mut AccountView, D) };
    // SAFETY: all `D` slots initialized above; layout matches `[AccountView; D]`.
    let declared = unsafe { core::slice::from_raw_parts(views.as_ptr() as *const AccountView, D) };

    let idx: usize = kani::any();
    kani::assume(idx <= D + 1);

    assert!(resolve_dup(idx, DupSources::Declared(declared)).is_some() == (idx < D));
}

/// `DupSources::Cache`: `idx` indexes the split `[declared ++ cache]` space;
/// resolves iff `idx < declared.len() + cache.len()`.
#[kani::proof]
#[kani::unwind(6)]
fn resolve_dup_cache_bounds() {
    const D: usize = 2;
    const C: usize = 2;
    const TOTAL: usize = D + C;
    const NB: usize = TOTAL * ZERO_ACCT_STRIDE;

    let mut buf = SvmBuf::<NB>::zeroed();
    // SAFETY: an array of `MaybeUninit` is valid uninitialized.
    let mut views: [core::mem::MaybeUninit<AccountView>; TOTAL] =
        unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    // SAFETY: `buf` holds `TOTAL` accounts; `views` has `TOTAL` slots.
    unsafe { fill_store(&mut buf, views.as_mut_ptr() as *mut AccountView, TOTAL) };
    // SAFETY: all `TOTAL` slots initialized above; layout matches
    // `[AccountView; TOTAL]`, so the two sub-slices are sound.
    let base = views.as_ptr() as *const AccountView;
    let declared = unsafe { core::slice::from_raw_parts(base, D) };
    let cache = unsafe { core::slice::from_raw_parts(base.add(D), C) };

    let idx: usize = kani::any();
    kani::assume(idx <= TOTAL + 1);

    assert!(resolve_dup(idx, DupSources::Cache { declared, cache }).is_some() == (idx < TOTAL));
}

// ---------------------------------------------------------------------------
// Real SVM-buffer walk proofs.
//
// The proofs below drive the *actual* unsafe navigation functions
// (`svm::Cursor::next`, `resolve_dup_walk`, `RemainingIterImpl::next`,
// `Remaining::parse_single`) over a symbolic-but-well-formed SVM input buffer,
// rather than restating arithmetic about local variables. Kani's built-in
// pointer-bounds checks turn "the walk never reads outside the buffer" into a
// verification condition automatically; the explicit assertions add the
// higher-level invariants the plan calls for (produced views stay in-buffer).
//
// A zero-data account occupies exactly `account_stride(0)` bytes in the buffer;
// a duplicate entry occupies `DUP_ENTRY_SIZE` (8) bytes. `ACCOUNT_HEADER` is
// already 8-aligned, so `account_stride(0) == ACCOUNT_HEADER`.
const ZERO_ACCT_STRIDE: usize = ACCOUNT_HEADER;
const _: () = assert!(account_stride(0) == ZERO_ACCT_STRIDE);

/// 8-byte-aligned stack buffer standing in for the SVM account region.
#[repr(C, align(8))]
struct SvmBuf<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> SvmBuf<N> {
    fn zeroed() -> Self {
        Self { bytes: [0u8; N] }
    }

    fn base(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }

    /// One-past-the-end pointer for the first `len` bytes: the walk boundary.
    fn boundary(&self, len: usize) -> *const u8 {
        // SAFETY: `len <= N`, so this is at most a one-past-the-end pointer.
        unsafe { (self.bytes.as_ptr()).add(len) }
    }

    /// Write a non-duplicate `RuntimeAccount` header at `offset`.
    ///
    /// # Safety
    /// `offset` must be 8-aligned and leave room for a header plus `data_len`
    /// data bytes within the buffer.
    unsafe fn write_account(&mut self, offset: usize, borrow: u8, addr_byte: u8, data_len: u64) {
        // SAFETY: caller guarantees the header fits at `offset`.
        let raw = unsafe { self.base().add(offset) as *mut RuntimeAccount };
        unsafe {
            (*raw).borrow_state = borrow;
            (*raw).is_signer = 0;
            (*raw).is_writable = 1;
            (*raw).executable = 0;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array([addr_byte; 32]);
            (*raw).owner = Address::new_from_array([0xAA; 32]);
            (*raw).lamports = 100;
            (*raw).data_len = data_len;
        }
    }

    /// Write a duplicate entry (borrow marker = source index) at `offset`.
    ///
    /// # Safety
    /// `offset` must be 8-aligned and leave room for `DUP_ENTRY_SIZE` bytes.
    unsafe fn write_dup(&mut self, offset: usize, idx: u8) {
        // SAFETY: caller guarantees the 8-byte entry fits at `offset`.
        unsafe { *self.base().add(offset) = idx };
    }
}

/// `Cursor::next` never leaves the allocation for a well-formed account entry.
///
/// Drives the real `svm::Cursor::next` (the single owner of the walk step) over
/// a symbolic-data non-duplicate account. The advanced pointer must equal the
/// Kani-proven `account_stride` and stay within the buffer (`ptr.add` would be
/// UB otherwise, which Kani would flag), and the decoded entry must classify as
/// an account.
#[kani::proof]
fn cursor_next_account_stays_in_buffer() {
    const D: usize = 24;
    const N: usize = account_stride(D);

    let mut buf = SvmBuf::<N>::zeroed();
    let base = buf.base();
    let raw = base as *mut RuntimeAccount;

    let data_len: u64 = kani::any();
    kani::assume(data_len <= D as u64);
    // SAFETY: `base` is 8-aligned and the buffer holds a full header + data.
    unsafe {
        (*raw).borrow_state = NOT_BORROWED;
        (*raw).data_len = data_len;
    }

    // SAFETY: `base`/`boundary` delimit the one-account region built above.
    let mut cursor = unsafe { Cursor::new(base, buf.boundary(N)) };
    // SAFETY: the cursor is not at end (a full account is present).
    let entry = unsafe { cursor.next() };
    assert!(matches!(entry, RawEntry::Account(_)));

    let advanced = (cursor.ptr() as usize) - (base as usize);
    assert!(advanced == account_stride(data_len as usize));
    assert!(advanced % 8 == 0);
    // Read pointer never exceeds the buffer end (== boundary for one account).
    assert!(advanced <= N);
}

/// `Cursor::next` decodes a duplicate marker and advances by `DUP_ENTRY_SIZE`.
///
/// Drives the real `svm::Cursor::next` over a single duplicate entry with a
/// symbolic source index; the entry must classify as `Dup` carrying that index
/// and the cursor must advance exactly `DUP_ENTRY_SIZE` bytes.
#[kani::proof]
fn cursor_next_dup_advances_by_dup_size() {
    const N: usize = DUP_ENTRY_SIZE;

    let mut buf = SvmBuf::<N>::zeroed();
    let dup_idx: u8 = kani::any();
    kani::assume(dup_idx != NOT_BORROWED);
    // SAFETY: the dup marker fits in the 8-byte buffer.
    unsafe {
        buf.write_dup(0, dup_idx);
    }
    let base = buf.base();

    // SAFETY: `base`/`boundary` delimit the one-entry region built above.
    let mut cursor = unsafe { Cursor::new(base, buf.boundary(N)) };
    // SAFETY: the cursor is not at end (a dup marker is present).
    match unsafe { cursor.next() } {
        RawEntry::Dup(idx) => assert!(idx == dup_idx),
        RawEntry::Account(_) => unreachable!(),
    }
    assert!((cursor.ptr() as usize) - (base as usize) == DUP_ENTRY_SIZE);
    assert!(cursor.at_end());
}

/// `resolve_dup_walk` only ever produces an in-buffer view (or an error).
///
/// Buffer: one non-dup account followed by one dup entry with a *symbolic*
/// source index. For any caller-supplied original index, the walk must either
/// fail or return a view whose address bytes lie inside `[base, boundary)`.
#[kani::proof]
#[kani::unwind(4)]
fn resolve_dup_walk_reads_in_buffer() {
    const N: usize = ZERO_ACCT_STRIDE + DUP_ENTRY_SIZE;

    let mut buf = SvmBuf::<N>::zeroed();
    // SAFETY: offsets are 8-aligned and inside the buffer.
    unsafe {
        buf.write_account(0, NOT_BORROWED, 0x01, 0);
    }
    let dup_idx: u8 = kani::any();
    kani::assume(dup_idx != NOT_BORROWED);
    // SAFETY: the dup entry fits in the trailing 8 bytes.
    unsafe {
        buf.write_dup(ZERO_ACCT_STRIDE, dup_idx);
    }

    let base = buf.base();
    let boundary = buf.boundary(N);

    let orig: usize = kani::any();
    kani::assume(orig <= 2);

    if let Ok(view) = resolve_dup_walk(orig, &[], base, boundary) {
        let addr = view.address() as *const Address as usize;
        assert!(addr >= base as usize);
        assert!(addr < boundary as usize);
    }
}

/// `RemainingIterImpl::next` yields only in-buffer views and terminates.
///
/// Same buffer shape as above. Each `Ok` account must point into the buffer,
/// the dup cache resolution (real `resolve_dup`/`resolve_dup_index`) must stay
/// sound for a symbolic dup index, and iteration must stop after at most two
/// steps (the fuse guarantees termination even for an unresolvable dup).
#[kani::proof]
#[kani::unwind(4)]
fn remaining_iter_next_in_buffer() {
    const N: usize = ZERO_ACCT_STRIDE + DUP_ENTRY_SIZE;

    let mut buf = SvmBuf::<N>::zeroed();
    // SAFETY: offsets are 8-aligned and inside the buffer.
    unsafe {
        buf.write_account(0, NOT_BORROWED, 0x01, 0);
    }
    let dup_idx: u8 = kani::any();
    kani::assume(dup_idx != NOT_BORROWED);
    // SAFETY: the dup entry fits in the trailing 8 bytes.
    unsafe {
        buf.write_dup(ZERO_ACCT_STRIDE, dup_idx);
    }

    let base = buf.base();
    let boundary = buf.boundary(N);
    let base_u = base as usize;
    let bound_u = boundary as usize;

    // SAFETY: `base`/`boundary` delimit the well-formed region built above and
    // the declared slice is empty.
    let remaining = unsafe { RemainingAccounts::new(base, boundary, &[]) };
    let mut iter = remaining.iter();

    let mut steps = 0usize;
    while let Some(item) = iter.next() {
        steps += 1;
        assert!(steps <= 2);
        if let Ok(account) = item {
            let addr = account.address() as *const Address as usize;
            assert!(addr >= base_u);
            assert!(addr < bound_u);
        }
    }
}

/// Minimal `RemainingItem` used to drive `Remaining::parse_single`. Accepts
/// duplicates so the dup-resolution (`resolve_dup_walk`) branch is exercised.
struct Probe;

impl<'input> RemainingItem<'input> for Probe {
    const COUNT: usize = 1;
    const REJECT_DUPLICATES: bool = false;

    unsafe fn parse_remaining_chunk(
        _accounts: &'input mut [AccountView],
        _program_id: Option<&Address>,
        _data: &[u8],
    ) -> Result<Self, ProgramError> {
        Ok(Probe)
    }
}

/// `Remaining::parse_single` walks the buffer without reading out of bounds.
///
/// Drives the real single-item parse (which calls `resolve_dup_walk` for the
/// dup entry, since `Probe` accepts duplicates). Kani proves the whole walk is
/// memory-safe for a symbolic dup index; the parse either succeeds or errors.
#[kani::proof]
#[kani::unwind(5)]
fn remaining_parse_single_in_buffer() {
    const N: usize = ZERO_ACCT_STRIDE + DUP_ENTRY_SIZE;

    let mut buf = SvmBuf::<N>::zeroed();
    // SAFETY: offsets are 8-aligned and inside the buffer.
    unsafe {
        buf.write_account(0, NOT_BORROWED, 0x01, 0);
    }
    let dup_idx: u8 = kani::any();
    kani::assume(dup_idx != NOT_BORROWED);
    // SAFETY: the dup entry fits in the trailing 8 bytes.
    unsafe {
        buf.write_dup(ZERO_ACCT_STRIDE, dup_idx);
    }

    let base = buf.base();
    let boundary = buf.boundary(N);

    // SAFETY: `base`/`boundary` delimit the region built above; declared empty.
    let remaining = unsafe { RemainingAccounts::new(base, boundary, &[]) };
    let _ = remaining.parse::<Probe, 4>();
}
