//! PDA derivation proofs.
//!
//! `read_bump_offset_within_bounds` calls production code directly
//! (`super::read_bump_from_account`, host-reachable).
//!
//! The two `*_indices_within_bounds` proofs drive the real
//! `super::build_pda_input` over a symbolic seed count. That function is pure
//! slice arithmetic and is compiled under `kani` (see its `#[cfg]`), so the
//! proofs exercise the production slot-array construction directly instead of a
//! restated copy: Kani's pointer-bounds checks turn "every
//! `out.add(i).write(..)` stays inside the `MAX_PDA_SLICES`-slot array" into an
//! automatic verification condition, and the explicit `count` assertions pin
//! the seed-count decomposition. `pda_slice_capacity_is_exact` pins the
//! capacity constant.

use {crate::svm_abi::SolBytes, solana_address::Address};

/// Drive `build_pda_input`'s no-bump path (as used by
/// `verify_program_address`): seeds, then program_id and PDA_MARKER. Kani
/// proves every slot write stays inside the `MAX_PDA_SLICES`-slot array and the
/// returned count is `n + 2`.
#[kani::proof]
#[kani::unwind(18)]
fn verify_program_address_indices_within_bounds() {
    let n: usize = kani::any();
    kani::assume(n <= 17);

    let seed_storage: [&[u8]; 17] = [&[] as &[u8]; 17];
    let seeds = &seed_storage[..n];
    let program_id = Address::new_from_array([0u8; 32]);

    let mut slots = core::mem::MaybeUninit::<[SolBytes; super::MAX_PDA_SLICES]>::uninit();
    let sptr = slots.as_mut_ptr() as *mut SolBytes;
    // SAFETY: `seeds.len() == n <= 17`, so `build_pda_input` writes at most
    // `n + 2 <= 19` slots into the `MAX_PDA_SLICES`-slot array. Kani checks the
    // writes stay in bounds.
    let count = unsafe { super::build_pda_input(seeds, &program_id, None, sptr) };

    assert!(count == n + 2, "no-bump slot count");
    assert!(count <= super::MAX_PDA_SLICES, "slice length exceeds array");
}

/// Drive `build_pda_input`'s with-bump path (as used by
/// `try_find_program_address`, `verify_canonical_program_address`, and
/// `find_bump_for_address`): seeds, bump, program_id, PDA_MARKER. Kani proves
/// every slot write stays in bounds and the returned count is `n + 3`.
#[kani::proof]
#[kani::unwind(17)]
fn find_program_address_indices_within_bounds() {
    let n: usize = kani::any();
    kani::assume(n <= 16);

    let seed_storage: [&[u8]; 16] = [&[] as &[u8]; 16];
    let seeds = &seed_storage[..n];
    let program_id = Address::new_from_array([0u8; 32]);
    let bump: u8 = kani::any();

    let mut slots = core::mem::MaybeUninit::<[SolBytes; super::MAX_PDA_SLICES]>::uninit();
    let sptr = slots.as_mut_ptr() as *mut SolBytes;
    // SAFETY: `seeds.len() == n <= 16`, so `build_pda_input` writes at most
    // `n + 3 <= 19` slots; the bump slot stores a `SolBytes` pointing at the
    // live `bump` local. Kani checks the writes stay in bounds.
    let count =
        unsafe { super::build_pda_input(seeds, &program_id, Some(&bump as *const u8), sptr) };

    assert!(count == n + 3, "with-bump slot count");
    assert!(count <= super::MAX_PDA_SLICES, "slice length exceeds array");
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
    assert!(17 + 1 + 1 == super::MAX_PDA_SLICES);
    assert!(16 + 1 + 1 + 1 == super::MAX_PDA_SLICES);
}
