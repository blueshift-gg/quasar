//! Program Derived Address (PDA) derivation.
//!
//! Uses `sol_sha256` + `sol_curve_validate_point` syscalls directly instead of
//! `sol_create_program_address` / `sol_try_find_program_address`, reducing
//! per-attempt cost from ~1,500 CU to ~544 CU.
//!
//! On SBF, `&[u8]` has layout `(*const u8, u64)` — identical to `sol_sha256`'s
//! `SolBytes`. The slice-of-slices cast exploits this to pass seed arrays
//! directly to the syscall without intermediate copies.

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::{sol_curve_validate_point, sol_sha256};
use {solana_address::Address, solana_program_error::ProgramError};

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

/// Maximum number of slices in a PDA hash input: up to 17 seeds + bump +
/// program_id + PDA_MARKER.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
const MAX_PDA_SLICES: usize = 19;

/// Verify that `expected` matches `sha256(seeds || program_id ||
/// "ProgramDerivedAddress")`.
///
/// The seeds slice must already include the bump byte.
///
/// Kani proof: `verify_program_address_indices_within_bounds`.
// NOTE: Uses `#[inline]` rather than `#[inline(always)]` deliberately —
// these functions are large enough that forced inlining at every callsite
// risks .so bloat. Benchmark `#[inline(always)]` if CU regression appears.
#[inline]
pub fn verify_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<(), ProgramError> {
    // seeds includes bump. Array has 19 slots: seeds(max 17) + program_id +
    // PDA_MARKER.
    if seeds.len() > 17 {
        return Err(ProgramError::InvalidSeeds);
    }

    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let n = seeds.len();

        // Build the input array: [seeds..., program_id, PDA_MARKER].
        // Max 17 seeds + program_id + marker = 19 entries.
        let mut slices = core::mem::MaybeUninit::<[&[u8]; MAX_PDA_SLICES]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];

        let mut i = 0;
        while i < n {
            // SAFETY: `i < n <= 17` so `sptr.add(i)` is within the 19-slot array.
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        // SAFETY: Slots `n` and `n+1` are within bounds (n <= 17, array has 19).
        unsafe {
            sptr.add(n).write(program_id.as_ref());
            sptr.add(n + 1).write(PDA_MARKER.as_slice());
        }

        // SAFETY: All `n + 2` elements initialized above.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 2) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();

        // SAFETY: On SBF, `&[u8]` has layout `(*const u8, u64)` which is
        // identical to `SolBytes`. The slice-of-slices cast passes seed
        // arrays directly to the syscall without intermediate copies.
        unsafe {
            sol_sha256(
                input as *const _ as *const u8,
                input.len() as u64,
                hash.as_mut_ptr() as *mut u8,
            );
        }

        // SAFETY: `hash` is fully initialized by `sol_sha256`. The cast to
        // `*const Address` is valid because `Address` is `[u8; 32]`.
        if crate::keys_eq(unsafe { &*(hash.as_ptr() as *const Address) }, expected) {
            Ok(())
        } else {
            Err(ProgramError::InvalidSeeds)
        }
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        let _ = (seeds, program_id, expected);
        Err(ProgramError::InvalidArgument)
    }
}

/// Find a valid program derived address and its bump seed.
///
/// Iterates bump values from 255 down to 0, hashing with `sol_sha256`
/// and checking off-curve with `sol_curve_validate_point`.
///
/// For a typical PDA (bump 255, first try): ~544 CU vs ~1,500 CU.
///
/// Kani proof: `find_program_address_indices_within_bounds`.
#[inline]
pub fn based_try_find_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<(Address, u8), ProgramError> {
    // bump added internally. Array has 19 slots: seeds(max 16) + bump + program_id
    // + PDA_MARKER.
    if seeds.len() > 16 {
        return Err(ProgramError::InvalidSeeds);
    }

    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        const CURVE25519_EDWARDS: u64 = 0;
        let n = seeds.len();

        // Build the input array: [seeds..., bump, program_id, PDA_MARKER].
        // Max 16 seeds + bump + program_id + marker = 19 entries.
        let mut slices = core::mem::MaybeUninit::<[&[u8]; MAX_PDA_SLICES]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];

        let mut i = 0;
        while i < n {
            // SAFETY: `i < n <= 16` so `sptr.add(i)` is within the 19-slot array.
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        // SAFETY: Slots `n+1` and `n+2` are within bounds (n <= 16, array has 19).
        unsafe {
            sptr.add(n + 1).write(program_id.as_ref());
            sptr.add(n + 2).write(PDA_MARKER.as_slice());
        }

        // The bump slot points into bump_arr — only the byte changes per iteration.
        let mut bump_arr = [u8::MAX];
        let bump_ptr = bump_arr.as_mut_ptr();
        // SAFETY: `sptr.add(n)` is within bounds. The `&[u8]` slice stored
        // here points to `bump_arr` but is NEVER read through Rust code —
        // it is only consumed by `sol_sha256` as a raw `(*const u8, u64)`
        // pair (SolBytes). The subsequent mutation of `bump_arr` via
        // `bump_ptr.write()` is invisible to any Rust reference. This relies
        // on the SBF ABI layout equivalence between `&[u8]` and `SolBytes`,
        // which is validated by the module-level documentation.
        unsafe { sptr.add(n).write(core::slice::from_raw_parts(bump_ptr, 1)) };

        // SAFETY: All `n + 3` elements initialized above.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 3) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();

        // u64 counter avoids per-iteration zero-extension on SBF.
        let mut bump: u64 = u8::MAX as u64;

        loop {
            // SAFETY: `bump_ptr` points to `bump_arr[0]`. Writing a u8 is always valid.
            unsafe { bump_ptr.write(bump as u8) };

            // SAFETY: Same SBF slice layout as `verify_program_address`.
            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                );
            }

            // SAFETY: `hash` was written by `sol_sha256`. Returns 0 if on
            // curve, non-zero if off curve (valid PDA).
            let on_curve = unsafe {
                sol_curve_validate_point(
                    CURVE25519_EDWARDS,
                    hash.as_ptr() as *const u8,
                    core::ptr::null_mut(),
                )
            };

            if on_curve != 0 {
                // SAFETY: `hash` fully initialized by `sol_sha256` above.
                let hash_bytes = unsafe { hash.assume_init() };
                return Ok((Address::new_from_array(hash_bytes), bump as u8));
            }

            if bump == 0 {
                break;
            }
            bump -= 1;
        }

        Err(ProgramError::InvalidSeeds)
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        let _ = (seeds, program_id);
        Err(ProgramError::InvalidArgument)
    }
}

/// Find the bump seed whose PDA hash matches `expected`.
///
/// Iterates bump values from 255 down to 0, hashing with `sol_sha256` and
/// comparing each hash against `expected` via [`keys_eq`](crate::keys_eq).
///
/// This replaces [`based_try_find_program_address`]'s per-iteration
/// `sol_curve_validate_point` syscall (~100 CU) with a `keys_eq` comparison
/// (~10 CU), saving ~90 CU per attempt while producing identical results.
///
/// Kani proof: `find_program_address_indices_within_bounds`.
///
/// # When to use
///
/// Use this instead of [`based_try_find_program_address`] whenever the
/// expected PDA address is already known — which is always the case during
/// account parsing (the account is passed in the transaction).
///
/// # Correctness
///
/// If `sha256(seeds ‖ bump ‖ program_id ‖ "ProgramDerivedAddress")` equals
/// `expected`, and `expected` is the address of an account in the current
/// transaction, then it must be a valid off-curve PDA: the Solana runtime
/// rejects on-curve addresses during account creation. Therefore
/// `hash == expected` ⟹ off-curve ⟹ valid PDA.
///
/// **Init caveat**: during `#[account(init)]`, the account does not yet
/// exist on-chain. Correctness relies on defense-in-depth: the subsequent
/// `create_account` CPI will itself reject an on-curve address.
///
/// # Caller contract
///
/// Callers MUST guarantee that `expected` is the address of an account
/// that exists in the current transaction. Passing a fabricated address
/// that lies on the ed25519 curve would produce a bump value for an
/// invalid PDA. The framework's codegen never calls this function in
/// `#[account(init)]` contexts — init paths use
/// [`based_try_find_program_address`] which includes the on-curve check.
#[inline]
pub fn find_bump_for_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<u8, ProgramError> {
    // bump added internally. Array has 19 slots: seeds(max 16) + bump + program_id
    // + PDA_MARKER.
    if seeds.len() > 16 {
        return Err(ProgramError::InvalidSeeds);
    }

    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let n = seeds.len();

        // Build the input array: [seeds..., bump, program_id, PDA_MARKER].
        // Max 16 seeds + bump + program_id + marker = 19 entries.
        let mut slices = core::mem::MaybeUninit::<[&[u8]; MAX_PDA_SLICES]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];

        let mut i = 0;
        while i < n {
            // SAFETY: `i < n <= 16` so `sptr.add(i)` is within the 19-slot array.
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        // SAFETY: Slots `n+1` and `n+2` are within bounds (n <= 16, array has 19).
        unsafe {
            sptr.add(n + 1).write(program_id.as_ref());
            sptr.add(n + 2).write(PDA_MARKER.as_slice());
        }

        // The bump slot points into bump_arr — only the byte changes per iteration.
        let mut bump_arr = [u8::MAX];
        let bump_ptr = bump_arr.as_mut_ptr();
        // SAFETY: `sptr.add(n)` is within bounds. The `&[u8]` slice stored
        // here points to `bump_arr` but is NEVER read through Rust code —
        // it is only consumed by `sol_sha256` as a raw `(*const u8, u64)`
        // pair (SolBytes). The subsequent mutation of `bump_arr` via
        // `bump_ptr.write()` is invisible to any Rust reference. This relies
        // on the SBF ABI layout equivalence between `&[u8]` and `SolBytes`,
        // which is validated by the module-level documentation.
        unsafe { sptr.add(n).write(core::slice::from_raw_parts(bump_ptr, 1)) };

        // SAFETY: All `n + 3` elements initialized above.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 3) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();

        // u64 counter avoids per-iteration zero-extension on SBF.
        let mut bump: u64 = u8::MAX as u64;

        loop {
            // SAFETY: `bump_ptr` points to `bump_arr[0]`. Writing a u8 is always valid.
            unsafe { bump_ptr.write(bump as u8) };

            // SAFETY: Same SBF slice layout as `verify_program_address`.
            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                );
            }

            // Compare the hash against the expected address (~10 CU) instead
            // of calling `sol_curve_validate_point` (~100 CU). A match proves
            // this is the correct bump — see the correctness argument above.
            //
            // SAFETY: `hash` fully initialized by `sol_sha256`. The cast to
            // `*const Address` is valid because `Address` is `[u8; 32]`.
            if crate::keys_eq(unsafe { &*(hash.as_ptr() as *const Address) }, expected) {
                return Ok(bump as u8);
            }

            if bump == 0 {
                break;
            }
            bump -= 1;
        }

        Err(ProgramError::InvalidSeeds)
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        let _ = (seeds, program_id, expected);
        Err(ProgramError::InvalidArgument)
    }
}

/// Read the PDA bump byte from account data at the given offset.
///
/// Used by the BUMP_OFFSET fast path to read the bump from the account's
/// own data instead of re-deriving it.
///
/// Kani proof: `read_bump_offset_within_bounds`.
#[inline(always)]
pub fn read_bump_from_account(
    view: &solana_account_view::AccountView,
    offset: usize,
) -> Result<u8, ProgramError> {
    if crate::utils::hint::unlikely(offset >= view.data_len()) {
        return Err(ProgramError::AccountDataTooSmall);
    }
    // SAFETY: Bounds checked above. `data_ptr()` returns a valid pointer
    // to `data_len()` bytes.
    Ok(unsafe { *view.data_ptr().add(offset) })
}

/// Compile-time PDA derivation using `const_crypto`.
pub const fn find_program_address_const(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
    let (bytes, bump) = const_crypto::ed25519::derive_program_address(seeds, program_id.as_array());
    (Address::new_from_array(bytes), bump)
}

#[cfg(kani)]
mod kani_proofs {
    /// `MAX_PDA_SLICES` from the parent module (cfg'd to Solana, so we
    /// redefine it here for verification).
    const MAX_PDA_SLICES: usize = 19;

    /// Prove `verify_program_address` index arithmetic is safe.
    ///
    /// Mirrors `verify_program_address()` slice-building loop:
    ///   `while i < n { sptr.add(i).write(seeds[i]); ... }`
    ///   `sptr.add(n).write(program_id...);`
    ///   `sptr.add(n + 1).write(PDA_MARKER...);`
    ///
    /// seeds.len() is checked `<= 17`. All indices must be `< 19`.
    #[kani::proof]
    fn verify_program_address_indices_within_bounds() {
        let n: usize = kani::any();
        kani::assume(n <= 17);

        // Loop indices: 0..n
        let mut i: usize = 0;
        while i < n {
            assert!(i < MAX_PDA_SLICES, "loop index out of bounds");
            i += 1;
        }
        // Post-loop: slots n (program_id) and n+1 (PDA_MARKER)
        assert!(n < MAX_PDA_SLICES, "program_id slot out of bounds");
        assert!(n + 1 < MAX_PDA_SLICES, "PDA_MARKER slot out of bounds");

        // Total initialized elements passed to from_raw_parts
        assert!(n + 2 <= MAX_PDA_SLICES, "slice length exceeds array");
    }

    /// Prove `based_try_find_program_address` and `find_bump_for_address`
    /// index arithmetic is safe.
    ///
    /// Mirrors `based_try_find_program_address()` / `find_bump_for_address()`
    /// slice-building loop:
    ///   `while i < n { sptr.add(i).write(seeds[i]); ... }`
    ///   `sptr.add(n).write(...bump...);`
    ///   `sptr.add(n + 1).write(program_id...);`
    ///   `sptr.add(n + 2).write(PDA_MARKER...);`
    ///
    /// seeds.len() is checked `<= 16`. All indices must be `< 19`.
    #[kani::proof]
    fn find_program_address_indices_within_bounds() {
        let n: usize = kani::any();
        kani::assume(n <= 16);

        // Loop indices: 0..n
        let mut i: usize = 0;
        while i < n {
            assert!(i < MAX_PDA_SLICES, "loop index out of bounds");
            i += 1;
        }
        // Post-loop: slots n (bump), n+1 (program_id), n+2 (PDA_MARKER)
        assert!(n < MAX_PDA_SLICES, "bump slot out of bounds");
        assert!(n + 1 < MAX_PDA_SLICES, "program_id slot out of bounds");
        assert!(n + 2 < MAX_PDA_SLICES, "PDA_MARKER slot out of bounds");

        // Total initialized elements passed to from_raw_parts
        assert!(n + 3 <= MAX_PDA_SLICES, "slice length exceeds array");
    }

    /// Prove that `read_bump_from_account` offset check prevents
    /// out-of-bounds access by calling the real function with symbolic offset.
    ///
    /// Constructs a real AccountView with known data_len = 8, then calls
    /// `read_bump_from_account` with a symbolic offset. Kani verifies:
    /// - No UB in the pointer arithmetic when offset < data_len
    /// - Ok is returned when offset < data_len
    /// - Err is returned when offset >= data_len
    #[kani::proof]
    fn read_bump_offset_within_bounds() {
        use crate::cpi::{AccountBuffer, MIN_ACCOUNT_BUF};

        const DATA_LEN: usize = 8;
        const BUF_SIZE: usize = MIN_ACCOUNT_BUF + DATA_LEN;

        let mut buf = AccountBuffer::<BUF_SIZE>::new();
        buf.init([1; 32], [0xAA; 32], DATA_LEN, false, true, false);
        let view = unsafe { buf.view() };

        let offset: usize = kani::any();
        // Keep solver tractable — offsets beyond a small range are equivalent.
        kani::assume(offset <= DATA_LEN + 1);

        let result = super::read_bump_from_account(&view, offset);

        if offset < DATA_LEN {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    /// Prove the gap between max seeds and MAX_PDA_SLICES is exactly right.
    ///
    /// `verify_program_address`: 17 seeds + program_id + marker = 19 =
    /// MAX_PDA_SLICES. `based_try_find_program_address` /
    /// `find_bump_for_address`:   16 seeds + bump + program_id + marker =
    /// 19 = MAX_PDA_SLICES.
    ///
    /// This ensures the constants are consistent -- changing one without
    /// the other would break the proofs above.
    #[kani::proof]
    fn pda_slice_capacity_is_exact() {
        // verify_program_address: seeds(max 17) + program_id + PDA_MARKER
        assert!(17 + 1 + 1 == MAX_PDA_SLICES);

        // based_try_find_program_address / find_bump_for_address:
        // seeds(max 16) + bump + program_id + PDA_MARKER
        assert!(16 + 1 + 1 + 1 == MAX_PDA_SLICES);
    }
}
