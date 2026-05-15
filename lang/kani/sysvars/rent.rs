use {
    super::*,
    core::mem::{align_of, size_of},
};

/// Prove alignment is 1 and size is 16 bytes.
/// Mirrors the compile-time assertions but makes the property explicit
/// in the verification suite.
#[kani::proof]
fn rent_struct_layout() {
    assert!(align_of::<Rent>() == 1);
    assert!(size_of::<Rent>() == 16);
}

/// Prove: any u64 written via `to_le_bytes` then read back through
/// `read_unaligned` produces the input value. This is the exact
/// pattern `exemption_threshold_raw()` uses.
#[kani::proof]
fn exemption_threshold_raw_roundtrip() {
    let value: u64 = kani::any();
    let bytes = value.to_le_bytes();
    let recovered = unsafe { core::ptr::read_unaligned(bytes.as_ptr() as *const u64) };
    assert!(recovered == value);
}

/// Prove: when `data_len <= MAX_PERMITTED_DATA_LENGTH` and
/// `lamports_per_byte <= CURRENT_MAX_LAMPORTS_PER_BYTE`, the
/// multiplication `2 * (ACCOUNT_STORAGE_OVERHEAD + data_len) *
/// lamports_per_byte` does not overflow u64.
#[kani::proof]
fn try_minimum_balance_no_overflow_current_threshold() {
    let data_len: u64 = kani::any();
    let lamports_per_byte: u64 = kani::any();

    kani::assume(data_len <= MAX_PERMITTED_DATA_LENGTH);
    kani::assume(lamports_per_byte <= CURRENT_MAX_LAMPORTS_PER_BYTE);

    let total_bytes = ACCOUNT_STORAGE_OVERHEAD + data_len;
    // Prove each intermediate step does not overflow.
    let step1 = total_bytes.checked_mul(lamports_per_byte);
    assert!(step1.is_some());
    let step2 = 2u64.checked_mul(step1.unwrap());
    assert!(step2.is_some());
}

/// Prove: when `data_len <= MAX_PERMITTED_DATA_LENGTH` and
/// `lamports_per_byte <= SIMD0194_MAX_LAMPORTS_PER_BYTE`, the
/// multiplication `(ACCOUNT_STORAGE_OVERHEAD + data_len) *
/// lamports_per_byte` does not overflow u64.
#[kani::proof]
fn try_minimum_balance_no_overflow_simd0194_threshold() {
    let data_len: u64 = kani::any();
    let lamports_per_byte: u64 = kani::any();

    kani::assume(data_len <= MAX_PERMITTED_DATA_LENGTH);
    kani::assume(lamports_per_byte <= SIMD0194_MAX_LAMPORTS_PER_BYTE);

    let total_bytes = ACCOUNT_STORAGE_OVERHEAD + data_len;
    let result = total_bytes.checked_mul(lamports_per_byte);
    assert!(result.is_some());
}

/// Prove: `minimum_balance_raw` with the current exemption threshold
/// returns `Ok` and the inner `2 * total_bytes * lamports_per_byte`
/// does not overflow, for all in-range inputs.
#[kani::proof]
fn minimum_balance_raw_no_overflow_current_threshold() {
    let space: u64 = kani::any();
    let lamports_per_byte: u64 = kani::any();

    kani::assume(space <= MAX_PERMITTED_DATA_LENGTH);
    kani::assume(lamports_per_byte <= CURRENT_MAX_LAMPORTS_PER_BYTE);

    let result = minimum_balance_raw(lamports_per_byte, CURRENT_EXEMPTION_THRESHOLD, space);
    assert!(result.is_ok());
}

/// Prove: `minimum_balance_raw` with the SIMD-0194 exemption threshold
/// returns `Ok` and the inner `total_bytes * lamports_per_byte` does not
/// overflow, for all in-range inputs.
#[kani::proof]
fn minimum_balance_raw_no_overflow_simd0194_threshold() {
    let space: u64 = kani::any();
    let lamports_per_byte: u64 = kani::any();

    kani::assume(space <= MAX_PERMITTED_DATA_LENGTH);
    kani::assume(lamports_per_byte <= SIMD0194_MAX_LAMPORTS_PER_BYTE);

    let result = minimum_balance_raw(lamports_per_byte, SIMD0194_EXEMPTION_THRESHOLD, space);
    assert!(result.is_ok());
}

/// Prove: `minimum_balance_raw` rejects any `space >
/// MAX_PERMITTED_DATA_LENGTH` regardless of other inputs.
#[kani::proof]
fn minimum_balance_raw_rejects_oversized_data() {
    let space: u64 = kani::any();
    let lamports_per_byte: u64 = kani::any();
    let threshold: u64 = kani::any();

    kani::assume(space > MAX_PERMITTED_DATA_LENGTH);

    let result = minimum_balance_raw(lamports_per_byte, threshold, space);
    assert!(result.is_err());
}

/// Prove: `minimum_balance_raw` with the current threshold rejects
/// `lamports_per_byte > CURRENT_MAX_LAMPORTS_PER_BYTE`.
#[kani::proof]
fn minimum_balance_raw_rejects_excess_lamports_current() {
    let space: u64 = kani::any();
    let lamports_per_byte: u64 = kani::any();

    kani::assume(space <= MAX_PERMITTED_DATA_LENGTH);
    kani::assume(lamports_per_byte > CURRENT_MAX_LAMPORTS_PER_BYTE);

    let result = minimum_balance_raw(lamports_per_byte, CURRENT_EXEMPTION_THRESHOLD, space);
    assert!(result.is_err());
}
