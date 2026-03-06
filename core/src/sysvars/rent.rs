use crate::impl_sysvar_get;
use crate::pod::PodU64;
use {
    crate::sysvars::Sysvar,
    core::mem::{align_of, size_of},
    solana_address::Address,
    solana_program_error::ProgramError,
};

const RENT_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

const MAX_PERMITTED_DATA_LENGTH: u64 = 10 * 1024 * 1024;
const CURRENT_EXEMPTION_THRESHOLD: u64 = u64::from_le_bytes([0, 0, 0, 0, 0, 0, 0, 64]);
const SIMD0194_EXEMPTION_THRESHOLD: u64 = u64::from_le_bytes([0, 0, 0, 0, 0, 0, 240, 63]);
const SIMD0194_MAX_LAMPORTS_PER_BYTE: u64 = 1_759_197_129_867;
const CURRENT_MAX_LAMPORTS_PER_BYTE: u64 = 879_598_564_933;
pub const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;

/// Rent sysvar data (first 16 bytes only).
///
/// The full Rent sysvar is 17 bytes (includes `burn_percent: u8` at offset
/// 16), but `burn_percent` is unused so only the first 16 bytes are read
/// via `impl_sysvar_get` with padding = 0.
///
/// Uses `PodU64` for `lamports_per_byte` to guarantee alignment 1, making
/// `from_bytes_unchecked` sound on all targets (not just SBF).
#[repr(C)]
#[derive(Clone, Debug)]
pub struct Rent {
    lamports_per_byte: PodU64,
    exemption_threshold: [u8; 8],
}

const _ASSERT_STRUCT_LEN: () = assert!(size_of::<Rent>() == 16);
const _ASSERT_STRUCT_ALIGN: () = assert!(align_of::<Rent>() == 1);

impl Rent {
    #[inline(always)]
    fn exemption_threshold_u64(&self) -> u64 {
        u64::from_le_bytes(self.exemption_threshold)
    }

    #[inline(always)]
    pub fn minimum_balance_unchecked(&self, data_len: usize) -> u64 {
        self.minimum_balance_inner(data_len, self.lamports_per_byte.get())
    }

    #[inline(always)]
    fn minimum_balance_inner(&self, data_len: usize, lamports_per_byte: u64) -> u64 {
        let bytes = data_len as u64;
        let threshold = self.exemption_threshold_u64();

        if threshold == SIMD0194_EXEMPTION_THRESHOLD {
            (ACCOUNT_STORAGE_OVERHEAD + bytes) * lamports_per_byte
        } else if threshold == CURRENT_EXEMPTION_THRESHOLD {
            2 * (ACCOUNT_STORAGE_OVERHEAD + bytes) * lamports_per_byte
        } else {
            #[cfg(not(target_arch = "bpf"))]
            {
                (((ACCOUNT_STORAGE_OVERHEAD + bytes) * lamports_per_byte) as f64
                    * f64::from_le_bytes(self.exemption_threshold)) as u64
            }
            #[cfg(target_arch = "bpf")]
            {
                2 * (ACCOUNT_STORAGE_OVERHEAD + bytes) * lamports_per_byte
            }
        }
    }

    #[allow(clippy::collapsible_if)]
    #[inline(always)]
    pub fn try_minimum_balance(&self, data_len: usize) -> Result<u64, ProgramError> {
        if data_len as u64 > MAX_PERMITTED_DATA_LENGTH {
            return Err(ProgramError::InvalidArgument);
        }

        let lamports_per_byte = self.lamports_per_byte.get();
        let threshold = self.exemption_threshold_u64();
        if lamports_per_byte > CURRENT_MAX_LAMPORTS_PER_BYTE {
            if threshold == CURRENT_EXEMPTION_THRESHOLD {
                return Err(ProgramError::InvalidArgument);
            }
        } else if lamports_per_byte > SIMD0194_MAX_LAMPORTS_PER_BYTE {
            if threshold == SIMD0194_EXEMPTION_THRESHOLD {
                return Err(ProgramError::InvalidArgument);
            }
        }

        Ok(self.minimum_balance_inner(data_len, lamports_per_byte))
    }
}

impl Sysvar for Rent {
    impl_sysvar_get!(RENT_ID, 0);
}
