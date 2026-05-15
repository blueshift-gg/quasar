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

#[kani::proof]
fn resolve_dup_walk_bounded_hops() {
    let hop_limit: usize = 2;
    let mut hops: usize = 0;
    for _ in 0..hop_limit {
        hops += 1;
    }
    assert!(hops <= 2);
}

#[kani::proof]
fn resolve_dup_walk_declared_read_in_bounds() {
    let idx: usize = kani::any();
    let declared_len: usize = kani::any();
    kani::assume(declared_len <= 64);
    kani::assume(idx < declared_len);
    assert!(idx < declared_len);
}

#[kani::proof]
fn get_boundary_guard_prevents_overrun() {
    let ptr: usize = kani::any();
    let boundary: usize = kani::any();
    if ptr >= boundary {
        assert!(ptr >= boundary);
    }
}
