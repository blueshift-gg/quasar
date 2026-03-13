use solana_address::Address;

pub(crate) const METADATA_PROGRAM_BYTES: [u8; 32] = [
    11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205, 88, 184, 108, 115,
    26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
];

/// Metaplex Token Metadata program address
/// (`metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`).
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub static METADATA_PROGRAM_ID: Address = Address::new_from_array(METADATA_PROGRAM_BYTES);
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub const METADATA_PROGRAM_ID: Address = Address::new_from_array(METADATA_PROGRAM_BYTES);
