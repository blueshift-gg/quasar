//! Program Derived Address (PDA) derivation.
//!
//! Uses `sol_sha256` + `sol_curve_validate_point` syscalls directly instead of
//! the higher-level `sol_create_program_address` / `sol_try_find_program_address`
//! syscalls, reducing per-attempt cost from ~1,500 CU to ~544 CU.
//!
//! Also provides `find_program_address_const` for compile-time PDA derivation
//! using `const_crypto` — useful for declaring static PDAs in `const` contexts.

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
/// then compares the result against `expected` via `read_unaligned` u64 chunks.
#[inline(always)]
pub fn verify_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
    expected: &Address,
) -> Result<(), ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];
        let n = seeds.len();
        let mut i = 0;
        while i < n {
            // SAFETY: i < n <= 17 (max seeds). sptr[i] is within the 19-element array.
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        // SAFETY: sptr[n] and sptr[n+1] are within bounds (n <= 17, array has 19 slots).
        unsafe {
            sptr.add(n).write(program_id.as_ref());
            sptr.add(n + 1).write(PDA_MARKER.as_slice());
        }
        // SAFETY: Elements 0..n+2 are initialized by the loop and two writes above.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 2) };
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
        // SAFETY: sol_sha256 writes exactly 32 bytes to the output buffer,
        // fully initializing hash.
        let hash = unsafe { hash.assume_init() };
        let h = hash.as_ptr() as *const u64;
        let e = expected.as_array().as_ptr() as *const u64;
        // SAFETY: Both hash and expected are [u8; 32] — 32 contiguous bytes.
        // read_unaligned at offsets 0,8,16,24 stays within bounds.
        let eq = unsafe {
            core::ptr::read_unaligned(h) == core::ptr::read_unaligned(e)
                && core::ptr::read_unaligned(h.add(1)) == core::ptr::read_unaligned(e.add(1))
                && core::ptr::read_unaligned(h.add(2)) == core::ptr::read_unaligned(e.add(2))
                && core::ptr::read_unaligned(h.add(3)) == core::ptr::read_unaligned(e.add(3))
        };
        if eq {
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

        let mut slices = core::mem::MaybeUninit::<[&[u8]; 19]>::uninit();
        let sptr = slices.as_mut_ptr() as *mut &[u8];
        let mut i = 0;
        while i < n {
            // SAFETY: i < n <= 16 (max seeds). sptr[i] is within the 19-element array.
            unsafe { sptr.add(i).write(seeds[i]) };
            i += 1;
        }
        // SAFETY: sptr[n+1] and sptr[n+2] are within bounds (n <= 16, array has 19 slots).
        unsafe {
            sptr.add(n + 1).write(program_id.as_ref());
            sptr.add(n + 2).write(PDA_MARKER.as_slice());
        }

        let mut bump_arr = [u8::MAX];
        let bump_ptr = bump_arr.as_mut_ptr();
        // SAFETY: sptr[n] is within bounds (n <= 16, array has 19 slots).
        // bump_arr lives for the entire block. The fat pointer (ptr, len=1)
        // stored in sptr[n] points to bump_arr for the duration.
        unsafe { sptr.add(n).write(core::slice::from_raw_parts(bump_ptr, 1)) };

        // Use u64 for the loop counter to avoid per-iteration `and64 r, 0xff`
        // zero-extension the compiler emits for u8 arithmetic on SBF.
        let mut bump: u64 = u8::MAX as u64;
        // Pre-build the input slice once — only the bump byte changes per iteration.
        // SAFETY: Elements 0..n+3 are initialized: 0..n by the loop above,
        // n by bump write, n+1 and n+2 by program_id/marker writes.
        let input = unsafe { core::slice::from_raw_parts(sptr, n + 3) };
        // Allocate the hash buffer once outside the loop.
        let mut hash = core::mem::MaybeUninit::<[u8; 32]>::uninit();
        loop {
            // SAFETY: bump is in [0, 255], the cast is lossless.
            unsafe { bump_ptr.write(bump as u8) };
            // SAFETY: On SBF, &[u8] has layout (*const u8, u64) — identical to
            // sol_sha256's SolBytes. The cast reinterprets the slice-of-fat-pointers
            // as the byte array the syscall expects. Technique from Dean Little's
            // solana-nostd-sha256.
            unsafe {
                sol_sha256(
                    input as *const _ as *const u8,
                    input.len() as u64,
                    hash.as_mut_ptr() as *mut u8,
                );
            }
            // SAFETY: sol_sha256 writes exactly 32 bytes to hash. We pass the
            // pointer directly to sol_curve_validate_point which only reads it.
            // Returns 0 if on curve, non-zero if off curve (valid PDA).
            let on_curve = unsafe {
                sol_curve_validate_point(
                    CURVE25519_EDWARDS,
                    hash.as_ptr() as *const u8,
                    core::ptr::null_mut(),
                )
            };
            if on_curve != 0 {
                // SAFETY: sol_sha256 fully initialized the 32-byte buffer.
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

/// Find a valid program derived address and its bump seed at compile time.
///
/// Uses `const_crypto` for const-compatible SHA-256 hashing and Ed25519
/// off-curve evaluation, making this suitable for `const` contexts.
pub const fn find_program_address_const(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
    let (bytes, bump) = const_crypto::ed25519::derive_program_address(seeds, program_id.as_array());
    (Address::new_from_array(bytes), bump)
}
