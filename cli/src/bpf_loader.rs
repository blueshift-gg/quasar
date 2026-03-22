use crate::rpc::{
    self, confirm_transaction, get_latest_blockhash, get_minimum_balance_for_rent_exemption,
    send_transaction, Keypair,
};
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
// Orchestrators
// ---------------------------------------------------------------------------

/// Read the upgrade authority from a programdata account's raw bytes.
///
/// Returns `None` if the program is immutable (authority option tag is 0).
pub fn parse_programdata_authority(data: &[u8]) -> Result<Option<Address>, crate::error::CliError> {
    if data.len() < 45 {
        return Err(anyhow::anyhow!(
            "programdata account too short ({} bytes, need at least 45)",
            data.len()
        )
        .into());
    }
    match data[12] {
        0 => Ok(None),
        1 => {
            let pubkey: [u8; 32] = data[13..45]
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid authority pubkey slice"))?;
            Ok(Some(Address::from(pubkey)))
        }
        other => Err(anyhow::anyhow!("invalid authority option tag: {other}").into()),
    }
}

/// Build a `SystemProgram::CreateAccount` instruction.
pub fn create_account_ix(
    payer: &Address,
    new_account: &Address,
    lamports: u64,
    space: u64,
    owner: &Address,
) -> Instruction {
    let mut data = Vec::with_capacity(52);
    data.extend_from_slice(&0u32.to_le_bytes()); // discriminant
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());
    data.extend_from_slice(owner.as_ref());
    Instruction {
        program_id: SYSTEM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*new_account, true),
        ],
        data,
    }
}

/// Verify that the on-chain upgrade authority matches `expected_authority`.
///
/// Errors if the program is immutable or if the authority doesn't match.
pub fn verify_upgrade_authority(
    rpc_url: &str,
    program_id: &Address,
    expected_authority: &Address,
) -> Result<(), crate::error::CliError> {
    let (programdata, _) = programdata_pda(program_id);
    let data = rpc::get_account_data(rpc_url, &programdata)?
        .ok_or_else(|| anyhow::anyhow!("programdata account not found"))?;
    let authority = parse_programdata_authority(&data)?;
    match authority {
        None => Err(anyhow::anyhow!("program is immutable (no upgrade authority)").into()),
        Some(on_chain) if on_chain != *expected_authority => Err(anyhow::anyhow!(
            "upgrade authority mismatch: on-chain is {}, your keypair is {}",
            bs58::encode(on_chain).into_string(),
            bs58::encode(expected_authority).into_string()
        )
        .into()),
        Some(_) => Ok(()),
    }
}

/// Upload a .so binary to a new buffer account.
///
/// Returns the buffer account address on success.
pub fn write_buffer(
    so_path: &std::path::Path,
    payer: &Keypair,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<Address, crate::error::CliError> {
    let program_data = std::fs::read(so_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", so_path.display()))?;
    let buffer_keypair = Keypair::generate();
    let buffer_addr = buffer_keypair.address();

    let total_size = program_data.len() + BUFFER_HEADER_SIZE;
    let lamports = get_minimum_balance_for_rent_exemption(rpc_url, total_size)?;

    // 1. Create buffer account + initialize in one transaction
    let mut ixs = Vec::new();
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(create_account_ix(
        &payer.address(),
        &buffer_addr,
        lamports,
        total_size as u64,
        &BPF_LOADER_UPGRADEABLE_ID,
    ));
    ixs.push(initialize_buffer_ix(&buffer_addr, &payer.address()));

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
        Some(&payer.address()),
        &[payer, &buffer_keypair],
        blockhash,
    );
    let tx_bytes =
        bincode::serialize(&tx).map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;
    let confirmed = confirm_transaction(rpc_url, &sig, 30)?;
    if !confirmed {
        return Err(anyhow::anyhow!(
            "buffer creation timed out (buffer: {})",
            bs58::encode(buffer_addr).into_string()
        )
        .into());
    }

    // 2. Write chunks sequentially with a progress bar
    let chunks = num_chunks(program_data.len());
    let bar = indicatif::ProgressBar::new(program_data.len() as u64);
    bar.set_style(
        indicatif::ProgressStyle::with_template("  {bar:40.cyan/dim} {bytes}/{total_bytes} ({eta})")
            .unwrap(),
    );

    for i in 0..chunks {
        let offset = i * CHUNK_SIZE;
        let end = (offset + CHUNK_SIZE).min(program_data.len());
        let chunk = &program_data[offset..end];

        let mut write_ixs = Vec::new();
        if priority_fee > 0 {
            write_ixs.push(set_compute_unit_price_ix(priority_fee));
        }
        write_ixs.push(write_ix(
            &buffer_addr,
            &payer.address(),
            offset as u32,
            chunk,
        ));

        let bh = get_latest_blockhash(rpc_url)?;
        let write_tx = solana_transaction::Transaction::new_signed_with_payer(
            &write_ixs,
            Some(&payer.address()),
            &[payer],
            bh,
        );
        let write_tx_bytes = bincode::serialize(&write_tx)
            .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
        let write_sig = send_transaction(rpc_url, &write_tx_bytes)?;
        let write_confirmed = confirm_transaction(rpc_url, &write_sig, 30)?;
        if !write_confirmed {
            return Err(anyhow::anyhow!(
                "write chunk {i} timed out (buffer: {})",
                bs58::encode(buffer_addr).into_string()
            )
            .into());
        }
        bar.set_position(end as u64);
    }

    bar.finish_and_clear();
    Ok(buffer_addr)
}

/// Deploy a new program.
///
/// Uploads the .so to a buffer, creates the program account, and deploys.
/// Returns the program address.
pub fn deploy_program(
    so_path: &std::path::Path,
    program_keypair: &Keypair,
    payer: &Keypair,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<Address, crate::error::CliError> {
    let so_len = std::fs::metadata(so_path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", so_path.display()))?
        .len() as usize;

    let buffer_addr = write_buffer(so_path, payer, rpc_url, priority_fee)?;

    let program_addr = program_keypair.address();
    let (programdata, _) = programdata_pda(&program_addr);
    let max_data_len = (so_len * 2) as u64;

    // Program account is 36 bytes
    let program_lamports = get_minimum_balance_for_rent_exemption(rpc_url, 36)?;

    let mut ixs = Vec::new();
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(create_account_ix(
        &payer.address(),
        &program_addr,
        program_lamports,
        36,
        &BPF_LOADER_UPGRADEABLE_ID,
    ));
    ixs.push(deploy_with_max_data_len_ix(
        &payer.address(),
        &programdata,
        &program_addr,
        &buffer_addr,
        &payer.address(),
        max_data_len,
    ));

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
        Some(&payer.address()),
        &[payer, program_keypair],
        blockhash,
    );
    let tx_bytes =
        bincode::serialize(&tx).map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;
    let confirmed = confirm_transaction(rpc_url, &sig, 30)?;
    if !confirmed {
        return Err(anyhow::anyhow!("deploy transaction timed out").into());
    }

    Ok(program_addr)
}

/// Upgrade an existing program with a new .so binary.
pub fn upgrade_program(
    so_path: &std::path::Path,
    program_id: &Address,
    authority: &Keypair,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<(), crate::error::CliError> {
    let buffer_addr = write_buffer(so_path, authority, rpc_url, priority_fee)?;

    let (programdata, _) = programdata_pda(program_id);
    let authority_addr = authority.address();

    let mut ixs = Vec::new();
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(upgrade_ix(
        &programdata,
        program_id,
        &buffer_addr,
        &authority_addr,
        &authority_addr,
    ));

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority_addr),
        &[authority],
        blockhash,
    );
    let tx_bytes =
        bincode::serialize(&tx).map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;
    let confirmed = confirm_transaction(rpc_url, &sig, 30)?;
    if !confirmed {
        return Err(anyhow::anyhow!("upgrade transaction timed out").into());
    }

    Ok(())
}

/// Transfer or revoke upgrade authority on an account.
///
/// Pass `None` for `new_authority` to make the program immutable.
pub fn set_authority(
    account: &Address,
    current_authority: &Keypair,
    new_authority: Option<&Address>,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<(), crate::error::CliError> {
    let mut ixs = Vec::new();
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(set_authority_ix(
        account,
        &current_authority.address(),
        new_authority,
    ));

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
        Some(&current_authority.address()),
        &[current_authority],
        blockhash,
    );
    let tx_bytes =
        bincode::serialize(&tx).map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;
    let confirmed = confirm_transaction(rpc_url, &sig, 30)?;
    if !confirmed {
        return Err(anyhow::anyhow!("set authority transaction timed out").into());
    }

    Ok(())
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

    #[test]
    fn parse_programdata_authority_some() {
        let mut data = vec![0u8; 45];
        data[0..4].copy_from_slice(&3u32.to_le_bytes());
        data[4..12].copy_from_slice(&100u64.to_le_bytes());
        data[12] = 1;
        data[13..45].copy_from_slice(&[0xAA; 32]);
        let authority = parse_programdata_authority(&data).unwrap();
        assert_eq!(authority, Some(Address::from([0xAA; 32])));
    }

    #[test]
    fn parse_programdata_authority_none() {
        let mut data = vec![0u8; 45];
        data[0..4].copy_from_slice(&3u32.to_le_bytes());
        data[4..12].copy_from_slice(&100u64.to_le_bytes());
        data[12] = 0;
        let authority = parse_programdata_authority(&data).unwrap();
        assert!(authority.is_none());
    }

    #[test]
    fn create_account_ix_serialization() {
        let payer = Address::from([1u8; 32]);
        let new_account = Address::from([2u8; 32]);
        let owner = Address::from([3u8; 32]);
        let ix = create_account_ix(&payer, &new_account, 1_000_000, 100, &owner);
        assert_eq!(ix.program_id, SYSTEM_PROGRAM_ID);
        assert_eq!(&ix.data[..4], &[0, 0, 0, 0]);
        assert_eq!(&ix.data[4..12], &1_000_000u64.to_le_bytes());
        assert_eq!(&ix.data[12..20], &100u64.to_le_bytes());
        assert_eq!(&ix.data[20..52], owner.as_ref());
        assert_eq!(ix.data.len(), 52);
        assert!(ix.accounts[0].is_signer);
        assert!(ix.accounts[1].is_signer);
    }
}
