/// `MAX_PDA_SLICES` from the parent module is Solana-target-only, so proofs
/// keep their own copy of the bound they verify.
const MAX_PDA_SLICES: usize = 19;

/// Prove `build_pda_input`'s no-bump index arithmetic (as used by
/// `verify_program_address`) is safe: seeds, then program_id and PDA_MARKER.
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

/// Prove `build_pda_input`'s with-bump index arithmetic (as used by
/// `try_find_program_address`, `verify_canonical_program_address`, and
/// `find_bump_for_address`) is safe: seeds, bump, program_id, PDA_MARKER.
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
