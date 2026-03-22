use solana_address::Address;

// ---------------------------------------------------------------------------
// Well-known program & sysvar addresses
// ---------------------------------------------------------------------------

/// BPF Loader Upgradeable — BPFLoaderUpgradeab1e11111111111111111111111.
pub const BPF_LOADER_UPGRADEABLE_ID: Address = Address::new_from_array([
    0x02, 0xa8, 0xf6, 0x91, 0x4e, 0x88, 0xa1, 0xb0, 0xe2, 0x10, 0x15, 0x3e, 0xf7, 0x63, 0xae,
    0x2b, 0x00, 0xc2, 0xb9, 0x3d, 0x16, 0xc1, 0x24, 0xd2, 0xc0, 0x53, 0x7a, 0x10, 0x04, 0x80,
    0x00, 0x00,
]);

/// System program ID — 11111111111111111111111111111111.
pub const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Sysvar Rent — SysvarRent111111111111111111111111111111111.
pub const SYSVAR_RENT_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155,
    161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

/// Sysvar Clock — SysvarC1ock11111111111111111111111111111111.
pub const SYSVAR_CLOCK_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163,
    155, 75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

/// Compute Budget program — ComputeBudget111111111111111111111111111111.
pub const COMPUTE_BUDGET_PROGRAM_ID: Address = Address::new_from_array([
    3, 6, 70, 111, 229, 33, 23, 50, 255, 236, 173, 186, 114, 195, 155, 231, 188, 140, 229, 187,
    197, 247, 18, 107, 44, 67, 155, 58, 64, 0, 0, 0,
]);

// ---------------------------------------------------------------------------
// BPF Loader constants
// ---------------------------------------------------------------------------

/// Maximum payload per `Write` instruction (keeps transactions under the
/// 1232-byte packet limit with room for signatures and accounts).
pub const CHUNK_SIZE: usize = 950;

/// Size of the `Buffer` account header: 4-byte enum tag + 1-byte Option
/// discriminant + 32-byte authority pubkey.
pub const BUFFER_HEADER_SIZE: usize = 37;

// ---------------------------------------------------------------------------
// PDA helpers
// ---------------------------------------------------------------------------

/// Derive the program-data account address for a given program.
pub fn programdata_pda(program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE_ID)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_bpf_loader_id() {
        let expected = bs58::decode("BPFLoaderUpgradeab1e11111111111111111111111")
            .into_vec()
            .unwrap();
        assert_eq!(BPF_LOADER_UPGRADEABLE_ID.as_ref(), &expected[..]);
    }

    #[test]
    fn verify_sysvar_rent_id() {
        let expected = bs58::decode("SysvarRent111111111111111111111111111111111")
            .into_vec()
            .unwrap();
        assert_eq!(SYSVAR_RENT_ID.as_ref(), &expected[..]);
    }

    #[test]
    fn verify_sysvar_clock_id() {
        let expected = bs58::decode("SysvarC1ock11111111111111111111111111111111")
            .into_vec()
            .unwrap();
        assert_eq!(SYSVAR_CLOCK_ID.as_ref(), &expected[..]);
    }

    #[test]
    fn verify_compute_budget_program_id() {
        let expected = bs58::decode("ComputeBudget111111111111111111111111111111")
            .into_vec()
            .unwrap();
        assert_eq!(COMPUTE_BUDGET_PROGRAM_ID.as_ref(), &expected[..]);
    }

    #[test]
    fn buffer_header_size() {
        assert_eq!(BUFFER_HEADER_SIZE, 4 + 1 + 32);
    }
}
