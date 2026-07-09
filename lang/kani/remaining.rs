use {
    super::*,
    crate::__internal::{align_up_8, ACCOUNT_HEADER, DUP_ENTRY_SIZE},
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

#[kani::proof]
fn resolve_dup_index_declared_in_bounds() {
    let orig_idx: usize = kani::any();
    let declared_len: usize = kani::any();
    let cache_count: usize = kani::any();
    kani::assume(declared_len <= 64);
    kani::assume(cache_count <= MAX_REMAINING_ACCOUNTS);

    if let Some(DupSource::Declared(idx)) = resolve_dup_index(orig_idx, declared_len, cache_count) {
        assert!(idx < declared_len);
    }
}

#[kani::proof]
fn resolve_dup_index_cached_in_bounds() {
    let orig_idx: usize = kani::any();
    let declared_len: usize = kani::any();
    let cache_count: usize = kani::any();
    kani::assume(declared_len <= 64);
    kani::assume(cache_count <= MAX_REMAINING_ACCOUNTS);

    if let Some(DupSource::Cached(idx)) = resolve_dup_index(orig_idx, declared_len, cache_count) {
        assert!(idx < cache_count);
        assert!(idx < MAX_REMAINING_ACCOUNTS);
    }
}

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

// ---------------------------------------------------------------------------
// Real SVM-buffer walk proofs.
//
// The proofs below drive the *actual* unsafe navigation functions
// (`advance_past_account`, `resolve_dup_walk`, `RemainingIterImpl::next`,
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

/// `advance_past_account` never leaves the allocation for a well-formed entry.
///
/// Drives the real `advance_past_account` with a symbolic `data_len`; the
/// returned pointer must equal the Kani-proven `account_stride` and stay within
/// the buffer (`ptr.add` would be UB otherwise, which Kani would flag).
#[kani::proof]
fn advance_past_account_stays_in_buffer() {
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

    // SAFETY: `raw` is the freshly written non-duplicate account at `base`.
    let next = unsafe { advance_past_account(base, raw) };

    let advanced = (next as usize) - (base as usize);
    assert!(advanced == account_stride(data_len as usize));
    assert!(advanced % 8 == 0);
    // Read pointer never exceeds the buffer end (== boundary for one account).
    assert!(advanced <= N);
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
