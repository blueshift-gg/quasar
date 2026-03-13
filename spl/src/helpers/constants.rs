use solana_address::Address;

/// Raw bytes of the SPL Token program address
/// (`TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`).
pub(crate) const SPL_TOKEN_BYTES: [u8; 32] = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
];

/// Raw bytes of the Token-2022 program address
/// (`TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`).
pub(crate) const TOKEN_2022_BYTES: [u8; 32] = [
    6, 221, 246, 225, 238, 130, 236, 193, 200, 168, 65, 2, 106, 93, 64, 59, 117, 155, 197, 130,
    200, 159, 250, 31, 239, 205, 35, 168, 238, 94, 220, 87,
];

/// SPL Token program address.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static SPL_TOKEN_ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const SPL_TOKEN_ID: Address = Address::new_from_array(SPL_TOKEN_BYTES);

/// Token-2022 program address.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static TOKEN_2022_ID: Address = Address::new_from_array(TOKEN_2022_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const TOKEN_2022_ID: Address = Address::new_from_array(TOKEN_2022_BYTES);

/// Raw bytes of the ATA program address
/// (`ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`).
pub(crate) const ATA_PROGRAM_BYTES: [u8; 32] = [
    140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19, 153, 218,
    255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
];

/// Associated Token Account program address.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static ATA_PROGRAM_ID: Address = Address::new_from_array(ATA_PROGRAM_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const ATA_PROGRAM_ID: Address = Address::new_from_array(ATA_PROGRAM_BYTES);
