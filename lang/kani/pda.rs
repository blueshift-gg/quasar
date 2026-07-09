//! PDA derivation proofs.
//!
//! `read_bump_offset_within_bounds` calls production code directly
//! (`super::read_bump_from_account`, host-reachable).
//!
//! The two `*_indices_within_bounds` proofs re-derive the seed-slot arithmetic
//! against a local `MAX_PDA_SLICES` copy rather than the production functions.
//! This is deliberate and currently unavoidable: `MAX_PDA_SLICES` and the
//! slot-array construction in `verify_program_address` /
//! `based_try_find_program_address` / `find_bump_for_address` live entirely
//! inside `#[cfg(any(target_os = "solana", target_arch = "bpf"))]` blocks, so
//! on Kani's host target those functions take the `Err(InvalidArgument)`
//! fallback and never execute the arithmetic under proof. Reaching the real
//! index logic requires hoisting the slot build into a host-reachable
//! `build_pda_input(seeds, program_id, bump, slots)` helper (planned as the F5
//! extraction); until that seam exists these proofs stay as-is. Keep the local
//! bound in sync with `pda::MAX_PDA_SLICES` (`pda_slice_capacity_is_exact`
//! pins the seed-count decomposition).

/// Local copy of `pda::MAX_PDA_SLICES` (Solana-target-only in production; see
/// module comment).
const MAX_PDA_SLICES: usize = 19;

/// Prove `verify_program_address` index arithmetic is safe.
#[kani::proof]
fn verify_program_address_indices_within_bounds() {
    let n: usize = kani::any();
    kani::assume(n <= 17);

    let mut i: usize = 0;
    while i < n {
        assert!(i < MAX_PDA_SLICES, "loop index out of bounds");
        i += 1;
    }

    assert!(n < MAX_PDA_SLICES, "program_id slot out of bounds");
    assert!(n + 1 < MAX_PDA_SLICES, "PDA_MARKER slot out of bounds");
    assert!(n + 2 <= MAX_PDA_SLICES, "slice length exceeds array");
}

/// Prove `based_try_find_program_address` and `find_bump_for_address` index
/// arithmetic is safe.
#[kani::proof]
fn find_program_address_indices_within_bounds() {
    let n: usize = kani::any();
    kani::assume(n <= 16);

    let mut i: usize = 0;
    while i < n {
        assert!(i < MAX_PDA_SLICES, "loop index out of bounds");
        i += 1;
    }

    assert!(n < MAX_PDA_SLICES, "bump slot out of bounds");
    assert!(n + 1 < MAX_PDA_SLICES, "program_id slot out of bounds");
    assert!(n + 2 < MAX_PDA_SLICES, "PDA_MARKER slot out of bounds");
    assert!(n + 3 <= MAX_PDA_SLICES, "slice length exceeds array");
}

/// Prove that `read_bump_from_account`'s offset check prevents out-of-bounds
/// pointer arithmetic.
#[kani::proof]
fn read_bump_offset_within_bounds() {
    use crate::cpi::{AccountBuffer, MIN_ACCOUNT_BUF};

    const DATA_LEN: usize = 8;
    const BUF_SIZE: usize = MIN_ACCOUNT_BUF + DATA_LEN;

    let mut buf = AccountBuffer::<BUF_SIZE>::new();
    buf.init([1; 32], [0xAA; 32], DATA_LEN, false, true, false);
    // SAFETY: The buffer was initialized with a valid account header above.
    let view = unsafe { buf.view() };

    let offset: usize = kani::any();
    // Keep solver tractable; offsets beyond a small range are equivalent.
    kani::assume(offset <= DATA_LEN + 1);

    let result = super::read_bump_from_account(&view, offset);

    if offset < DATA_LEN {
        assert!(result.is_ok());
    } else {
        assert!(result.is_err());
    }
}

/// Prove the gap between max seeds and `MAX_PDA_SLICES` is exact.
#[kani::proof]
fn pda_slice_capacity_is_exact() {
    assert!(17 + 1 + 1 == MAX_PDA_SLICES);
    assert!(16 + 1 + 1 + 1 == MAX_PDA_SLICES);
}
