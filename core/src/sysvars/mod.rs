#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
use core::hint::black_box;
use solana_address::Address;
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use solana_define_syscall::definitions::sol_get_sysvar;
use solana_program_error::ProgramError;

pub mod rent;
pub mod clock;

const OFFSET_LENGTH_EXCEEDS_SYSVAR: u64 = 1;
const SYSVAR_NOT_FOUND: u64 = 2;

pub trait Sysvar: Sized {
    const ID: Address;

    /// # Safety
    ///
    /// Caller must ensure `bytes.len() >= size_of::<Self>()` and that the data
    /// represents a valid sysvar. The pointer cast may be misaligned on non-SBF
    /// targets, but SBF handles unaligned access natively.
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self;

    fn get() -> Result<Self, ProgramError> {
        Err(ProgramError::UnsupportedSysvar)
    }
}

#[macro_export]
macro_rules! impl_sysvar_get {
    ($syscall_id:expr, $padding:literal) => {
        const ID: Address = $syscall_id;

        #[inline(always)]
        unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
            unsafe { &*(bytes.as_ptr() as *const Self) }
        }

        #[inline(always)]
        fn get() -> Result<Self, ProgramError> {
            let mut var = core::mem::MaybeUninit::<Self>::uninit();
            let var_addr = var.as_mut_ptr() as *mut _ as *mut u8;

            #[cfg(target_os = "solana")]
            let result = unsafe {
                let length = core::mem::size_of::<Self>() - $padding;
                var_addr.add(length).write_bytes(0, $padding);

                solana_define_syscall::definitions::sol_get_sysvar(
                    &$syscall_id as *const _ as *const u8,
                    var_addr,
                    0,
                    length as u64,
                )
            };

            #[cfg(not(target_os = "solana"))]
            let result = {
                unsafe { var_addr.write_bytes(0, core::mem::size_of::<Self>()) };
                core::hint::black_box(var_addr as *const _ as u64)
            };

            match result {
                0 => Ok(unsafe { var.assume_init() }),
                $crate::sysvars::OFFSET_LENGTH_EXCEEDS_SYSVAR => Err(ProgramError::InvalidArgument),
                $crate::sysvars::SYSVAR_NOT_FOUND => Err(ProgramError::UnsupportedSysvar),
                _ => Err(ProgramError::UnsupportedSysvar),
            }
        }
    };
}

/// # Safety
///
/// `dst` must point to a buffer of at least `len` bytes. `sysvar_id` must be
/// a valid sysvar address. The caller is responsible for ensuring the buffer
/// is large enough to hold the requested sysvar data.
#[inline]
pub unsafe fn get_sysvar_unchecked(
    dst: *mut u8,
    sysvar_id: &Address,
    offset: usize,
    len: usize,
) -> Result<(), ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        let result = unsafe {
            sol_get_sysvar(
                sysvar_id as *const _ as *const u8,
                dst,
                offset as u64,
                len as u64,
            )
        };

        match result {
            0 => Ok(()),
            OFFSET_LENGTH_EXCEEDS_SYSVAR => Err(ProgramError::InvalidArgument),
            SYSVAR_NOT_FOUND => Err(ProgramError::UnsupportedSysvar),
            _ => Err(ProgramError::UnsupportedSysvar),
        }
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        black_box((dst, sysvar_id, offset, len));
        Ok(())
    }
}
