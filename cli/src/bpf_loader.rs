use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

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
// Instruction builders
// ---------------------------------------------------------------------------

/// Build an `InitializeBuffer` instruction for the BPF Loader Upgradeable.
pub fn initialize_buffer_ix(buffer: &Address, authority: &Address) -> Instruction {
    let data = 0u32.to_le_bytes().to_vec();
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts: vec![
            AccountMeta::new(*buffer, false),
            AccountMeta::new_readonly(*authority, false),
        ],
        data,
    }
}

/// Build a `Write` instruction for the BPF Loader Upgradeable.
pub fn write_ix(buffer: &Address, authority: &Address, offset: u32, bytes: &[u8]) -> Instruction {
    let mut data = Vec::with_capacity(4 + 4 + 4 + bytes.len());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&offset.to_le_bytes());
    data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(bytes);
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts: vec![
            AccountMeta::new(*buffer, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

/// Build a `DeployWithMaxDataLen` instruction for the BPF Loader Upgradeable.
pub fn deploy_with_max_data_len_ix(
    payer: &Address,
    programdata: &Address,
    program: &Address,
    buffer: &Address,
    authority: &Address,
    max_data_len: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes());
    data.extend_from_slice(&max_data_len.to_le_bytes());
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*programdata, false),
            AccountMeta::new(*program, false),
            AccountMeta::new(*buffer, false),
            AccountMeta::new_readonly(SYSVAR_RENT_ID, false),
            AccountMeta::new_readonly(SYSVAR_CLOCK_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

/// Build an `Upgrade` instruction for the BPF Loader Upgradeable.
pub fn upgrade_ix(
    programdata: &Address,
    program: &Address,
    buffer: &Address,
    spill: &Address,
    authority: &Address,
) -> Instruction {
    let data = 3u32.to_le_bytes().to_vec();
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts: vec![
            AccountMeta::new(*programdata, false),
            AccountMeta::new(*program, false),
            AccountMeta::new(*buffer, false),
            AccountMeta::new(*spill, false),
            AccountMeta::new_readonly(SYSVAR_RENT_ID, false),
            AccountMeta::new_readonly(SYSVAR_CLOCK_ID, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

/// Build a `SetAuthority` instruction for the BPF Loader Upgradeable.
///
/// When `new_authority` is `None` the program is made immutable.
pub fn set_authority_ix(
    account: &Address,
    current_authority: &Address,
    new_authority: Option<&Address>,
) -> Instruction {
    let data = 4u32.to_le_bytes().to_vec();
    let mut accounts = vec![
        AccountMeta::new(*account, false),
        AccountMeta::new_readonly(*current_authority, true),
    ];
    if let Some(new_auth) = new_authority {
        accounts.push(AccountMeta::new_readonly(*new_auth, false));
    }
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts,
        data,
    }
}

/// Build a `SetComputeUnitPrice` instruction for the Compute Budget program.
pub fn set_compute_unit_price_ix(micro_lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(3u8);
    data.extend_from_slice(&micro_lamports.to_le_bytes());
    Instruction {
        program_id: COMPUTE_BUDGET_PROGRAM_ID,
        accounts: vec![],
        data,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the number of `CHUNK_SIZE` chunks needed to upload `file_size` bytes.
pub fn num_chunks(file_size: usize) -> usize {
    if file_size == 0 {
        0
    } else {
        file_size.div_ceil(CHUNK_SIZE)
    }
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

    #[test]
    fn initialize_buffer_ix_serialization() {
        let buffer = Address::from([1u8; 32]);
        let authority = Address::from([2u8; 32]);
        let ix = initialize_buffer_ix(&buffer, &authority);
        assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
        assert_eq!(&ix.data[..4], &[0, 0, 0, 0]);
        assert_eq!(ix.data.len(), 4);
        assert_eq!(ix.accounts.len(), 2);
        assert!(ix.accounts[0].is_writable);
        assert!(!ix.accounts[1].is_writable);
    }

    #[test]
    fn write_ix_serialization() {
        let buffer = Address::from([1u8; 32]);
        let authority = Address::from([2u8; 32]);
        let chunk = vec![0xAA; 100];
        let ix = write_ix(&buffer, &authority, 500, &chunk);
        assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
        assert_eq!(&ix.data[..4], &[1, 0, 0, 0]);
        assert_eq!(&ix.data[4..8], &500u32.to_le_bytes());
        assert_eq!(&ix.data[8..12], &100u32.to_le_bytes());
        assert_eq!(&ix.data[12..], &chunk[..]);
        assert_eq!(ix.accounts.len(), 2);
        assert!(ix.accounts[0].is_writable);
        assert!(ix.accounts[1].is_signer);
    }

    #[test]
    fn deploy_with_max_data_len_ix_serialization() {
        let payer = Address::from([1u8; 32]);
        let programdata = Address::from([2u8; 32]);
        let program = Address::from([3u8; 32]);
        let buffer = Address::from([4u8; 32]);
        let authority = Address::from([5u8; 32]);
        let ix =
            deploy_with_max_data_len_ix(&payer, &programdata, &program, &buffer, &authority, 10000);
        assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
        assert_eq!(&ix.data[..4], &[2, 0, 0, 0]);
        assert_eq!(&ix.data[4..12], &10000u64.to_le_bytes());
        assert_eq!(ix.data.len(), 12);
        assert_eq!(ix.accounts.len(), 8);
        // Verify account ordering
        assert_eq!(ix.accounts[0].pubkey, payer);
        assert_eq!(ix.accounts[1].pubkey, programdata);
        assert_eq!(ix.accounts[2].pubkey, program);
        assert_eq!(ix.accounts[3].pubkey, buffer);
        assert_eq!(ix.accounts[4].pubkey, SYSVAR_RENT_ID);
        assert_eq!(ix.accounts[5].pubkey, SYSVAR_CLOCK_ID);
        assert_eq!(ix.accounts[6].pubkey, SYSTEM_PROGRAM_ID);
        assert_eq!(ix.accounts[7].pubkey, authority);
        assert!(ix.accounts[7].is_signer);
    }

    #[test]
    fn upgrade_ix_serialization() {
        let programdata = Address::from([1u8; 32]);
        let program = Address::from([2u8; 32]);
        let buffer = Address::from([3u8; 32]);
        let spill = Address::from([4u8; 32]);
        let authority = Address::from([5u8; 32]);
        let ix = upgrade_ix(&programdata, &program, &buffer, &spill, &authority);
        assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
        assert_eq!(&ix.data[..4], &[3, 0, 0, 0]);
        assert_eq!(ix.data.len(), 4);
        assert_eq!(ix.accounts.len(), 7);
        assert!(ix.accounts[6].is_signer);
    }

    #[test]
    fn set_authority_ix_serialization() {
        let account = Address::from([1u8; 32]);
        let current = Address::from([2u8; 32]);
        let new_auth = Address::from([3u8; 32]);
        let ix = set_authority_ix(&account, &current, Some(&new_auth));
        assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
        assert_eq!(&ix.data[..4], &[4, 0, 0, 0]);
        assert_eq!(ix.data.len(), 4);
        assert_eq!(ix.accounts.len(), 3);

        let ix2 = set_authority_ix(&account, &current, None);
        assert_eq!(ix2.accounts.len(), 2);
    }

    #[test]
    fn set_compute_unit_price_ix_serialization() {
        let ix = set_compute_unit_price_ix(1000);
        assert_eq!(ix.program_id, COMPUTE_BUDGET_PROGRAM_ID);
        assert_eq!(ix.data[0], 3);
        assert_eq!(&ix.data[1..9], &1000u64.to_le_bytes());
        assert_eq!(ix.data.len(), 9);
        assert!(ix.accounts.is_empty());
    }

    #[test]
    fn chunk_count_calculation() {
        assert_eq!(num_chunks(0), 0);
        assert_eq!(num_chunks(1), 1);
        assert_eq!(num_chunks(CHUNK_SIZE), 1);
        assert_eq!(num_chunks(CHUNK_SIZE + 1), 2);
        assert_eq!(num_chunks(CHUNK_SIZE * 3), 3);
        assert_eq!(num_chunks(CHUNK_SIZE * 3 + 1), 4);
    }
}
