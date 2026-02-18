#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::{sol_create_program_address, sol_try_find_program_address};
use {
    solana_address::Address,
    solana_instruction_view::cpi::Seed,
    solana_program_error::ProgramError,
};

/// Create a program derived address from seeds.
///
/// Accepts `&[Seed]` directly — on SBF, `Seed`'s `#[repr(C)]` layout
/// (`*const u8, u64`) matches the `&[u8]` fat pointer layout (`*const u8, usize`)
/// expected by the syscall, so the slice passes through with zero conversion.
#[inline(always)]
pub fn create_program_address(
    seeds: &[Seed],
    program_id: &Address,
) -> Result<Address, ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let mut bytes = core::mem::MaybeUninit::<Address>::uninit();
        let result = unsafe {
            sol_create_program_address(
                seeds.as_ptr() as *const u8,
                seeds.len() as u64,
                program_id as *const _ as *const u8,
                bytes.as_mut_ptr() as *mut u8,
            )
        };
        match result {
            0 => Ok(unsafe { bytes.assume_init() }),
            _ => Err(ProgramError::InvalidSeeds),
        }
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        core::hint::black_box((seeds, program_id));
        Ok(Address::default())
    }
}

/// Find a valid program derived address and its bump seed.
///
/// Same `Seed`-native approach as `create_program_address`. On SBF, the
/// seed slice passes directly to the `sol_try_find_program_address` syscall.
#[inline(always)]
pub fn find_program_address(
    seeds: &[Seed],
    program_id: &Address,
) -> (Address, u8) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let mut bytes = core::mem::MaybeUninit::<Address>::uninit();
        let mut bump = u8::MAX;
        let result = unsafe {
            sol_try_find_program_address(
                seeds.as_ptr() as *const u8,
                seeds.len() as u64,
                program_id as *const _ as *const u8,
                bytes.as_mut_ptr() as *mut u8,
                &mut bump as *mut u8,
            )
        };
        match result {
            0 => (unsafe { bytes.assume_init() }, bump),
            _ => panic!("Unable to find a viable program address bump seed"),
        }
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        core::hint::black_box((seeds, program_id));
        (Address::default(), u8::MAX)
    }
}
