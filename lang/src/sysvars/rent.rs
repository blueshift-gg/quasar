use {
    crate::{impl_sysvar_get, pod::PodU64, sysvars::Sysvar, utils::hint::unlikely},
    solana_address::Address,
    solana_program_error::ProgramError,
};

/// The address of the Rent sysvar.
const RENT_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

/// Maximum permitted size of account data (10 MiB).
const MAX_PERMITTED_DATA_LENGTH: u64 = 10 * 1024 * 1024;

/// The `f64::to_le_bytes` representation of `2.0` (current default threshold).
pub const CURRENT_EXEMPTION_THRESHOLD: u64 = u64::from_le_bytes([0, 0, 0, 0, 0, 0, 0, 64]);

/// The `f64::to_le_bytes` representation of `1.0` (SIMD-0194 threshold).
pub const SIMD0194_EXEMPTION_THRESHOLD: u64 = u64::from_le_bytes([0, 0, 0, 0, 0, 0, 240, 63]);

/// Maximum lamports/byte that avoids overflow with SIMD-0194 threshold.
const SIMD0194_MAX_LAMPORTS_PER_BYTE: u64 = 1_759_197_129_867;

/// Maximum lamports/byte that avoids overflow with current threshold.
const CURRENT_MAX_LAMPORTS_PER_BYTE: u64 = 879_598_564_933;

/// Account storage overhead for rent-exemption calculation.
///
/// This is the number of bytes required to store an account with no
/// data. It is added to an account's data length when calculating
/// the minimum balance.
pub const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;

/// Rent sysvar data (first 16 bytes only).
///
/// The full Rent sysvar includes `burn_percent: u8` at offset 16. Quasar does
/// not consume it, so this struct and the syscall read only the first 16 bytes.
///
/// Uses `PodU64` for `lamports_per_byte` to guarantee alignment 1, making
/// `from_bytes_unchecked` sound on all targets (not just SBF).
#[repr(C)]
#[derive(Clone, Debug)]
pub struct Rent {
    /// Rental rate in lamports per byte.
    lamports_per_byte: PodU64,

    /// Exemption threshold as `f64::to_le_bytes`.
    ///
    /// Stored as raw bytes to avoid floating-point operations on-chain.
    /// Compared bitwise against known threshold constants.
    exemption_threshold: [u8; 8],
}

const _: () = assert!(core::mem::size_of::<Rent>() == 16);
const _: () = assert!(core::mem::align_of::<Rent>() == 1);

impl Rent {
    /// Returns the lamports-per-byte rental rate.
    #[inline(always)]
    pub fn lamports_per_byte(&self) -> u64 {
        self.lamports_per_byte.get()
    }

    /// Returns the raw exemption threshold as a `u64` (bit representation
    /// of the f64 threshold). Compare against [`CURRENT_EXEMPTION_THRESHOLD`]
    /// or [`SIMD0194_EXEMPTION_THRESHOLD`].
    ///
    /// # Safety (internal)
    ///
    /// `exemption_threshold` is a `[u8; 8]`; reading it as u64 via
    /// `read_unaligned` is always valid. The f64 threshold lives in the
    /// sysvar but is reinterpreted as u64 for bit-exact comparison.
    #[inline(always)]
    pub fn exemption_threshold_raw(&self) -> u64 {
        unsafe { core::ptr::read_unaligned(self.exemption_threshold.as_ptr() as *const u64) }
    }

    /// Return the minimum lamport balance for rent exemption.
    ///
    /// Performs no overflow or length validation; prefer
    /// [`try_minimum_balance`](Self::try_minimum_balance) unless you have
    /// already verified that `data_len <= 10 MiB` and the sysvar's
    /// `lamports_per_byte` is within safe bounds.
    #[inline(always)]
    pub fn minimum_balance_unchecked(&self, data_len: usize) -> u64 {
        let lamports_per_byte = self.lamports_per_byte.get();
        let threshold = self.exemption_threshold_raw();
        Self::minimum_balance_inner(data_len, lamports_per_byte, threshold)
    }

    #[inline(always)]
    fn minimum_balance_inner(data_len: usize, lamports_per_byte: u64, threshold: u64) -> u64 {
        let total_bytes = ACCOUNT_STORAGE_OVERHEAD.wrapping_add(data_len as u64);

        if threshold == SIMD0194_EXEMPTION_THRESHOLD {
            total_bytes.wrapping_mul(lamports_per_byte)
        } else if threshold == CURRENT_EXEMPTION_THRESHOLD {
            total_bytes.wrapping_mul(lamports_per_byte).wrapping_mul(2)
        } else {
            #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
            {
                (total_bytes.wrapping_mul(lamports_per_byte) as f64 * f64::from_bits(threshold))
                    as u64
            }
            #[cfg(any(target_os = "solana", target_arch = "bpf"))]
            {
                total_bytes.wrapping_mul(lamports_per_byte).wrapping_mul(2)
            }
        }
    }

    /// Return the minimum lamport balance for rent exemption, with overflow
    /// protection.
    ///
    /// # Errors
    ///
    /// Returns `InvalidArgument` if:
    /// - `data_len` exceeds the 10 MiB maximum permitted account size.
    /// - `lamports_per_byte` would overflow the multiplication for the current
    ///   exemption threshold.
    #[inline(always)]
    pub fn try_minimum_balance(&self, data_len: usize) -> Result<u64, ProgramError> {
        if unlikely(data_len as u64 > MAX_PERMITTED_DATA_LENGTH) {
            return Err(ProgramError::InvalidArgument);
        }

        let lamports_per_byte = self.lamports_per_byte.get();
        let threshold = self.exemption_threshold_raw();
        if unlikely(lamports_per_byte_exceeds_bound(
            lamports_per_byte,
            threshold,
        )) {
            return Err(ProgramError::InvalidArgument);
        }

        Ok(Self::minimum_balance_inner(
            data_len,
            lamports_per_byte,
            threshold,
        ))
    }
}

#[inline(always)]
fn lamports_per_byte_exceeds_bound(lamports_per_byte: u64, threshold: u64) -> bool {
    let max = if threshold == SIMD0194_EXEMPTION_THRESHOLD {
        SIMD0194_MAX_LAMPORTS_PER_BYTE
    } else {
        CURRENT_MAX_LAMPORTS_PER_BYTE
    };

    lamports_per_byte > max
}

/// Compute the rent-exempt minimum balance from raw values.
///
/// Standalone function for use in codegen where the full `Rent` struct
/// is destructured into its `u64` components.
///
/// Assumes only two known thresholds exist on mainnet:
/// `CURRENT_EXEMPTION_THRESHOLD` (2.0) and `SIMD0194_EXEMPTION_THRESHOLD`
/// (1.0). The else branch defaults to `2x` (current threshold behavior). If a
/// third threshold is ever introduced, this function must be updated.
#[inline(always)]
pub fn minimum_balance_raw(
    lamports_per_byte: u64,
    threshold: u64,
    space: u64,
) -> Result<u64, ProgramError> {
    if unlikely(space > MAX_PERMITTED_DATA_LENGTH) {
        return Err(ProgramError::InvalidArgument);
    }
    if unlikely(lamports_per_byte_exceeds_bound(
        lamports_per_byte,
        threshold,
    )) {
        return Err(ProgramError::InvalidArgument);
    }

    let total_bytes = ACCOUNT_STORAGE_OVERHEAD.wrapping_add(space);
    if threshold == SIMD0194_EXEMPTION_THRESHOLD {
        Ok(total_bytes.wrapping_mul(lamports_per_byte))
    } else {
        debug_assert!(
            threshold == CURRENT_EXEMPTION_THRESHOLD,
            "minimum_balance_raw: unknown exemption threshold"
        );
        Ok(total_bytes.wrapping_mul(lamports_per_byte).wrapping_mul(2))
    }
}

impl Sysvar for Rent {
    impl_sysvar_get!(RENT_ID, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimum_balance_raw_rejects_current_threshold_overflow() {
        assert_eq!(
            minimum_balance_raw(
                CURRENT_MAX_LAMPORTS_PER_BYTE + 1,
                CURRENT_EXEMPTION_THRESHOLD,
                0
            ),
            Err(ProgramError::InvalidArgument)
        );
    }

    #[test]
    fn minimum_balance_raw_rejects_simd_threshold_overflow() {
        let lamports_per_byte = SIMD0194_MAX_LAMPORTS_PER_BYTE + 1;
        assert!(lamports_per_byte > CURRENT_MAX_LAMPORTS_PER_BYTE);

        assert_eq!(
            minimum_balance_raw(lamports_per_byte, SIMD0194_EXEMPTION_THRESHOLD, 0),
            Err(ProgramError::InvalidArgument)
        );
    }
}

#[cfg(kani)]
#[path = "../../kani/sysvars/rent.rs"]
mod kani_proofs;
