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

/// Verify that `expected` matches `sha256(seeds || program_id || "ProgramDerivedAddress")`.
///
/// The seeds slice must already include the bump byte.
#[inline(always)]
pub fn verify_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<(), ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let n = seeds.len();

        // Build the input array: [seeds..., program_id, PDA_MARKER].
        let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];

        let mut i = 0;
        while i < n {
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        unsafe {
            sptr.add(n).write(program_id.as_ref());
            sptr.add(n + 1).write(PDA_MARKER.as_slice());
        }

        // SAFETY: all n + 2 elements initialized above.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 2) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();

        unsafe {
            sol_sha256(
                input as *const _ as *const u8,
                input.len() as u64,
                hash.as_mut_ptr() as *mut u8,
            );
        }

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
#[inline(always)]
pub fn based_try_find_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<(Address, u8), ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        const CURVE25519_EDWARDS: u64 = 0;
        let n = seeds.len();

        // Build the input array: [seeds..., bump, program_id, PDA_MARKER].
        let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];

        let mut i = 0;
        while i < n {
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        unsafe {
            sptr.add(n + 1).write(program_id.as_ref());
            sptr.add(n + 2).write(PDA_MARKER.as_slice());
        }

        // The bump slot points into bump_arr — only the byte changes per iteration.
        let mut bump_arr = [u8::MAX];
        let bump_ptr = bump_arr.as_mut_ptr();
        unsafe { sptr.add(n).write(core::slice::from_raw_parts(bump_ptr, 1)) };

        // SAFETY: all n + 3 elements initialized above.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 3) };
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();

        // u64 counter avoids per-iteration zero-extension on SBF.
        let mut bump: u64 = u8::MAX as u64;

        loop {
            unsafe { bump_ptr.write(bump as u8) };

            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                );
            }

            // Returns 0 if on curve, non-zero if off curve (valid PDA).
            let on_curve = unsafe {
                sol_curve_validate_point(
                    CURVE25519_EDWARDS,
                    hash.as_ptr() as *const u8,
                    core::ptr::null_mut(),
                )
            };

            if on_curve != 0 {
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

/// Compile-time PDA derivation using `const_crypto`.
pub const fn find_program_address_const(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
    let (bytes, bump) = const_crypto::ed25519::derive_program_address(seeds, program_id.as_array());
    (Address::new_from_array(bytes), bump)
}
