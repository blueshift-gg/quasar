//! Well-known SPL program addresses.

use solana_address::Address;

pub(crate) const SPL_TOKEN_BYTES: [u8; 32] = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];

/// SPL Token program address.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static SPL_TOKEN_ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const SPL_TOKEN_ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);

pub(crate) const TOKEN_2022_BYTES: [u8; 32] = [
    6, 221, 246, 225, 238, 117, 143, 222, 24, 66, 93, 188, 228, 108, 205, 218, 182, 26, 252, 77,
    131, 185, 13, 39, 254, 189, 249, 40, 216, 161, 139, 252,
];

/// Token-2022 program address.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static TOKEN_2022_ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const TOKEN_2022_ID: Address = Address::new_from_array(TOKEN_2022_BYTES);

pub(crate) const ATA_PROGRAM_BYTES: [u8; 32] = [
    140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19, 153, 218,
    255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
];

/// Associated Token Account program address.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static ATA_PROGRAM_ID: Address = Address::new_from_array(ATA_PROGRAM_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const ATA_PROGRAM_ID: Address = Address::new_from_array(ATA_PROGRAM_BYTES);
