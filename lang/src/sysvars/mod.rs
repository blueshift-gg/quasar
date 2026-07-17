//! Sysvar access via the `sol_get_sysvar` syscall.
//!
//! Provides the `Sysvar` trait for zero-copy sysvar access and the
//! `impl_sysvar_get!` macro for implementing it. Concrete implementations
//! live in the `clock` and `rent` submodules.

use {solana_address::Address, solana_program_error::ProgramError};

/// Clock sysvar representation and access.
pub mod clock;
/// Rent sysvar representation and access.
pub mod rent;

const OFFSET_LENGTH_EXCEEDS_SYSVAR: u64 = 1;

/// A zero-copy Solana sysvar that can be loaded through `sol_get_sysvar`.
pub trait Sysvar: Sized {
    /// Address of the sysvar account.
    const ID: Address;

    /// # Safety
    /// `bytes.len()` must be `>= size_of::<Self>()` with valid sysvar data.
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self;

    /// Loads the current sysvar value.
    fn get() -> Result<Self, ProgramError> {
        Err(ProgramError::UnsupportedSysvar)
    }
}

#[macro_export]
/// Implements [`Sysvar`](crate::sysvars::Sysvar) access for a POD sysvar type.
///
/// The padding argument is the number of trailing bytes absent from the
/// syscall representation and therefore zero-initialized by the implementation.
macro_rules! impl_sysvar_get {
    ($syscall_id:expr, $padding:literal) => {
        const ID: solana_address::Address = $syscall_id;

        /// # Safety
        ///
        /// The caller must ensure `bytes` holds at least
        /// `size_of::<Self>()` bytes of valid sysvar data.
        #[inline(always)]
        unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
            // SAFETY: Caller guarantees `bytes` contains valid sysvar data
            // with length >= size_of::<Self>(). The struct is `#[repr(C)]`
            // with alignment 1, so the pointer cast is always valid.
            unsafe { &*(bytes.as_ptr() as *const Self) }
        }

        #[inline(always)]
        fn get() -> Result<Self, solana_program_error::ProgramError> {
            const {
                let padding = $padding;
                assert!(
                    padding <= core::mem::size_of::<Self>(),
                    "impl_sysvar_get! padding exceeds struct size"
                )
            };

            let mut var = core::mem::MaybeUninit::<Self>::uninit();
            let var_addr = var.as_mut_ptr() as *mut _ as *mut u8;

            #[cfg(any(target_os = "solana", target_arch = "bpf"))]
            // SAFETY: `var_addr` is backed by `MaybeUninit<Self>` (enough
            // space); the syscall fills `length` bytes and the trailing
            // `$padding` bytes are zeroed, initializing the whole struct.
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

            #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
            let result = {
                // SAFETY: `var_addr` is backed by `MaybeUninit<Self>`, valid
                // for a full `size_of::<Self>()` zero-fill.
                unsafe { var_addr.write_bytes(0, core::mem::size_of::<Self>()) };
                core::hint::black_box(var_addr as *const _ as u64)
            };

            match result {
                // SAFETY: On success (result == 0), the syscall has written
                // valid sysvar data and the padding was zeroed; all bytes
                // of `MaybeUninit<Self>` are initialized.
                0 => Ok(unsafe { var.assume_init() }),
                $crate::sysvars::OFFSET_LENGTH_EXCEEDS_SYSVAR => {
                    Err(solana_program_error::ProgramError::InvalidArgument)
                }
                // Any other nonzero code (including SYSVAR_NOT_FOUND == 2)
                // maps to `UnsupportedSysvar`.
                _ => Err(solana_program_error::ProgramError::UnsupportedSysvar),
            }
        }
    };
}
