#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::{sol_curve_validate_point, sol_sha256};
use {solana_address::Address, solana_program_error::ProgramError};

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
const PDA_MARKER: &[u8; 21] = b"ProgramDerivedAddress";

/// Verify that `expected` is the PDA derived from `seeds` and `program_id`.
///
/// Uses `sol_sha256` (~150-250 CU) instead of `sol_create_program_address`
/// (1,500 CU). The seeds slice must already include the bump byte.
///
/// Hashes `seeds || program_id || "ProgramDerivedAddress"` with SHA-256,
/// then compares the result against `expected` using `keys_eq`.
#[inline(always)]
pub fn verify_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<(), ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let mut slices = [&[] as &[u8]; 19];
        let n = seeds.len();
        let mut i = 0;
        while i < n {
            slices[i] = seeds[i];
            i += 1;
        }
        slices[n] = program_id.as_ref();
        slices[n + 1] = PDA_MARKER.as_slice();
        let input = &slices[..n + 2];
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();
        // SAFETY: On SBF, &[u8] has layout (*const u8, u64) — identical to sol_sha256's
        // SolBytes. The cast reinterprets the slice-of-fat-pointers as the byte array
        // the syscall expects. Technique from Dean Little's solana-nostd-sha256.
        unsafe {
            sol_sha256(
                input as *const _ as *const u8,
                input.len() as u64,
                hash.as_mut_ptr() as *mut u8,
            );
        }
        let hash = unsafe { hash.assume_init() };
        if crate::keys_eq(&Address::new_from_array(hash), expected) {
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
/// Uses `sol_sha256` (~285 CU) + `sol_curve_validate_point` (~259 CU) per
/// bump attempt instead of `sol_try_find_program_address` which charges
/// `create_program_address` cost (1,500 CU) per attempt internally.
///
/// For a typical PDA (bump=255, found on first try): ~544 CU vs ~1,500 CU.
#[inline(always)]
pub fn based_try_find_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<(Address, u8), ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        const CURVE25519_EDWARDS: u64 = 0;
        let n = seeds.len();
        let mut bump = u8::MAX;
        loop {
            let bump_arr = [bump];
            let mut slices = [&[] as &[u8]; 19];
            let mut i = 0;
            while i < n {
                slices[i] = seeds[i];
                i += 1;
            }
            slices[n] = &bump_arr;
            slices[n + 1] = program_id.as_ref();
            slices[n + 2] = PDA_MARKER.as_slice();
            let input = &slices[..n + 3];
            let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();
            // SAFETY: Same Dean Little cast as verify_program_address.
            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                );
            }
            let hash_bytes = unsafe { hash.assume_init() };
            // SAFETY: hash_bytes is a valid 32-byte array. sol_curve_validate_point
            // reads 32 bytes from the pointer. Returns 0 if on curve, non-zero if not.
            let on_curve = unsafe {
                sol_curve_validate_point(
                    CURVE25519_EDWARDS,
                    hash_bytes.as_ptr(),
                    core::ptr::null_mut(),
                )
            };
            if on_curve != 0 {
                return Ok((Address::new_from_array(hash_bytes), bump));
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

/// Find a valid program derived address and its bump seed at compile time.
///
/// Uses `const_crypto` for const-compatible SHA-256 hashing and Ed25519
/// off-curve evaluation, making this suitable for `const` contexts.
pub const fn find_program_address_const(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
    let (bytes, bump) = const_crypto::ed25519::derive_program_address(seeds, program_id.as_array());
    (Address::new_from_array(bytes), bump)
}
