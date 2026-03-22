use {
    crate::style,
    ed25519_dalek::SigningKey,
    sha2::{Digest, Sha256},
    solana_address::Address,
    solana_hash::Hash,
    solana_instruction::AccountMeta,
    solana_signature::Signature,
    solana_signer::{Signer, SignerError},
    std::{
        fs,
        path::Path,
        process::{Command, Stdio},
    },
};

// ---------------------------------------------------------------------------
// Solana CLI config
// ---------------------------------------------------------------------------

/// Read the Solana CLI config to get RPC URL and keypair path.
/// Falls back to defaults if config is missing.
pub fn solana_rpc_url(url_override: Option<&str>) -> String {
    if let Some(url) = url_override {
        return url.to_string();
    }
    read_config_field("json_rpc_url")
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string())
}

pub fn solana_keypair_path(keypair_override: Option<&Path>) -> std::path::PathBuf {
    if let Some(p) = keypair_override {
        return p.to_path_buf();
    }
    read_config_field("keypair_path")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".config/solana/id.json")
        })
}

fn read_config_field(field: &str) -> Option<String> {
    let config_path = dirs::home_dir()?.join(".config/solana/cli/config.yml");
    let contents = fs::read_to_string(config_path).ok()?;
    // Simple YAML parsing — find "field: value" line
    contents.lines().find_map(|line| {
        let line = line.trim();
        let prefix = format!("{field}:");
        if line.starts_with(&prefix) {
            let value = line[prefix.len()..]
                .trim()
                .trim_matches('\'')
                .trim_matches('"')
                .to_string();
            Some(expand_tilde(&value))
        } else {
            None
        }
    })
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{rest}", home.display());
        }
    }
    path.to_string()
}

// ---------------------------------------------------------------------------
// Keypair
// ---------------------------------------------------------------------------

/// Thin wrapper around ed25519-dalek SigningKey that implements solana Signer.
pub struct Keypair(SigningKey);

impl Keypair {
    /// Read a Solana keypair JSON file (array of 64 bytes).
    pub fn read_from_file(path: &Path) -> Result<Self, crate::error::CliError> {
        let contents = fs::read_to_string(path)?;
        let bytes: Vec<u8> = serde_json::from_str(&contents).map_err(anyhow::Error::from)?;
        if bytes.len() != 64 {
            return Err(anyhow::anyhow!(
                "keypair file must contain exactly 64 bytes, got {}",
                bytes.len()
            )
            .into());
        }
        let secret: [u8; 32] = bytes[..32].try_into().unwrap();
        Ok(Self(SigningKey::from_bytes(&secret)))
    }

    pub fn address(&self) -> Address {
        Address::from(self.0.verifying_key().to_bytes())
    }
}

impl Signer for Keypair {
    fn try_pubkey(&self) -> Result<Address, SignerError> {
        Ok(self.address())
    }

    fn try_sign_message(&self, message: &[u8]) -> Result<Signature, SignerError> {
        use ed25519_dalek::Signer as _;
        Ok(Signature::from(self.0.sign(message).to_bytes()))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// RPC (raw JSON-RPC via ureq)
// ---------------------------------------------------------------------------

/// Fetch the latest blockhash from the RPC.
pub fn get_latest_blockhash(rpc_url: &str) -> Result<Hash, crate::error::CliError> {
    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestBlockhash",
            "params": [{"commitment": "confirmed"}]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", err).into());
    }

    let hash_str = resp["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing blockhash in RPC response"))?;

    let bytes: [u8; 32] = bs58::decode(hash_str)
        .into_vec()
        .map_err(|e| anyhow::anyhow!("invalid blockhash: {e}"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("blockhash wrong length"))?;

    Ok(Hash::from(bytes))
}

/// Send a signed transaction to the RPC. Returns the signature string.
pub fn send_transaction(rpc_url: &str, tx_bytes: &[u8]) -> Result<String, crate::error::CliError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let encoded = STANDARD.encode(tx_bytes);

    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [encoded, {"encoding": "base64", "skipPreflight": false}]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", err).into());
    }

    resp["result"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("missing signature in RPC response").into())
}

/// Fetch account data as raw bytes. Returns None if account doesn't exist.
pub fn get_account_data(
    rpc_url: &str,
    address: &Address,
) -> Result<Option<Vec<u8>>, crate::error::CliError> {
    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [bs58::encode(address).into_string(), {"encoding": "base64", "commitment": "confirmed"}]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", err).into());
    }

    let value = &resp["result"]["value"];
    if value.is_null() {
        return Ok(None);
    }

    let data_str = value["data"][0]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing account data"))?;

    use base64::{engine::general_purpose::STANDARD, Engine};
    Ok(Some(
        STANDARD.decode(data_str).map_err(anyhow::Error::from)?,
    ))
}

/// Check whether a program exists on-chain at the given address.
/// Returns `true` if the account exists and is owned by the BPF Loader Upgradeable.
pub fn program_exists_on_chain(
    rpc_url: &str,
    program_id: &Address,
) -> Result<bool, crate::error::CliError> {
    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                bs58::encode(program_id).into_string(),
                {"encoding": "base64", "commitment": "confirmed"}
            ]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", err).into());
    }

    let value = &resp["result"]["value"];
    if value.is_null() {
        return Ok(false);
    }

    // Check if owned by BPF Loader Upgradeable
    let owner = value["owner"].as_str().unwrap_or_default();
    Ok(owner == "BPFLoaderUpgradeab1e11111111111111111111111")
}

// ---------------------------------------------------------------------------
// Squads v4 PDAs
// ---------------------------------------------------------------------------

/// Squads v4 program ID — SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf.
/// Verify with:
/// `bs58::decode("SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf").into_vec()`
/// These bytes MUST be verified at implementation time via the test in Task 8.
const SQUADS_PROGRAM_ID: Address = Address::new_from_array([
    0x06, 0x81, 0xc4, 0xce, 0x47, 0xe2, 0x23, 0x68, 0xb8, 0xb1, 0x55, 0x5e, 0xc8, 0x87, 0xaf, 0x09,
    0x2e, 0xfc, 0x7e, 0xfb, 0xb6, 0x6c, 0xa3, 0xf5, 0x2f, 0xbf, 0x68, 0xd4, 0xac, 0x9c, 0xb7, 0xa8,
]);

/// BPF Loader Upgradeable — BPFLoaderUpgradeab1e11111111111111111111111.
/// Verify with:
/// `bs58::decode("BPFLoaderUpgradeab1e11111111111111111111111").into_vec()`
const BPF_LOADER_UPGRADEABLE_ID: Address = Address::new_from_array([
    0x02, 0xa8, 0xf6, 0x91, 0x4e, 0x88, 0xa1, 0xb0, 0xe2, 0x10, 0x15, 0x3e, 0xf7, 0x63, 0xae, 0x2b,
    0x00, 0xc2, 0xb9, 0x3d, 0x16, 0xc1, 0x24, 0xd2, 0xc0, 0x53, 0x7a, 0x10, 0x04, 0x80, 0x00, 0x00,
]);

/// System program ID.
const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Sysvar Rent — SysvarRent111111111111111111111111111111111.
/// Matches `lang/src/sysvars/rent.rs` RENT_ID.
const SYSVAR_RENT_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

/// Sysvar Clock — SysvarC1ock11111111111111111111111111111111.
/// Matches `lang/src/sysvars/clock.rs` CLOCK_ID.
const SYSVAR_CLOCK_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

pub fn vault_pda(multisig: &Address, vault_index: u8) -> (Address, u8) {
    Address::find_program_address(
        &[b"multisig", multisig.as_ref(), b"vault", &[vault_index]],
        &SQUADS_PROGRAM_ID,
    )
}

pub fn transaction_pda(multisig: &Address, transaction_index: u64) -> (Address, u8) {
    Address::find_program_address(
        &[
            b"multisig",
            multisig.as_ref(),
            b"transaction",
            &transaction_index.to_le_bytes(),
        ],
        &SQUADS_PROGRAM_ID,
    )
}

pub fn proposal_pda(multisig: &Address, transaction_index: u64) -> (Address, u8) {
    Address::find_program_address(
        &[
            b"multisig",
            multisig.as_ref(),
            b"transaction",
            &transaction_index.to_le_bytes(),
            b"proposal",
        ],
        &SQUADS_PROGRAM_ID,
    )
}

pub fn programdata_pda(program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE_ID)
}

/// Read the current transaction_index from a multisig account's on-chain data.
/// The field is at byte offset 78, u64 LE.
pub fn read_transaction_index(account_data: &[u8]) -> Result<u64, crate::error::CliError> {
    if account_data.len() < 86 {
        return Err(anyhow::anyhow!(
            "multisig account data too short ({} bytes)",
            account_data.len()
        )
        .into());
    }
    let bytes: [u8; 8] = account_data[78..86].try_into().unwrap();
    Ok(u64::from_le_bytes(bytes))
}

/// A multisig member with their public key and permissions bitmask.
pub struct MultisigMember {
    pub key: Address,
    pub permissions: u8,
}

impl MultisigMember {
    /// Whether the member has Vote permission (bit 1).
    pub fn can_vote(&self) -> bool {
        self.permissions & 0x02 != 0
    }
}

/// Parsed state from a multisig account.
pub struct MultisigState {
    pub threshold: u16,
    pub transaction_index: u64,
    pub members: Vec<MultisigMember>,
}

/// Parse a multisig account's threshold, transaction_index, and members.
pub fn parse_multisig_account(data: &[u8]) -> Result<MultisigState, crate::error::CliError> {
    // Offsets: 8 disc + 32 create_key + 32 config_authority = 72 -> threshold (u16)
    //          74 time_lock (u32), 78 transaction_index (u64), 86 stale_tx_index (u64)
    //          94 rent_collector (33), 127 bump (1), 128 members vec len (u32)
    if data.len() < 132 {
        return Err(anyhow::anyhow!(
            "multisig account data too short ({} bytes)",
            data.len()
        )
        .into());
    }

    let threshold = u16::from_le_bytes(data[72..74].try_into().unwrap());
    let transaction_index = u64::from_le_bytes(data[78..86].try_into().unwrap());
    let num_members = u32::from_le_bytes(data[128..132].try_into().unwrap()) as usize;

    let required_len = 132 + num_members * 33;
    if data.len() < required_len {
        return Err(anyhow::anyhow!(
            "multisig account data too short for {} members ({} < {} bytes)",
            num_members,
            data.len(),
            required_len,
        )
        .into());
    }

    let mut members = Vec::with_capacity(num_members);
    for i in 0..num_members {
        let offset = 132 + i * 33;
        let key = Address::from(<[u8; 32]>::try_from(&data[offset..offset + 32]).unwrap());
        let permissions = data[offset + 32];
        members.push(MultisigMember { key, permissions });
    }

    Ok(MultisigState {
        threshold,
        transaction_index,
        members,
    })
}

/// Proposal status variants from the on-chain enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalStatus {
    Draft,
    Active,
    Rejected,
    Approved,
    Executing,
    Executed,
    Cancelled,
}

impl ProposalStatus {
    fn from_discriminant(d: u8) -> Result<Self, crate::error::CliError> {
        match d {
            0 => Ok(Self::Draft),
            1 => Ok(Self::Active),
            2 => Ok(Self::Rejected),
            3 => Ok(Self::Approved),
            4 => Ok(Self::Executing),
            5 => Ok(Self::Executed),
            6 => Ok(Self::Cancelled),
            _ => Err(anyhow::anyhow!("unknown proposal status: {d}").into()),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Active => "Active",
            Self::Rejected => "Rejected",
            Self::Approved => "Approved",
            Self::Executing => "Executing",
            Self::Executed => "Executed",
            Self::Cancelled => "Cancelled",
        }
    }
}

/// Parsed state from a proposal account.
pub struct ProposalState {
    pub transaction_index: u64,
    pub status: ProposalStatus,
    pub approved: Vec<Address>,
}

/// Parse a proposal account's status and approval list.
pub fn parse_proposal_account(data: &[u8]) -> Result<ProposalState, crate::error::CliError> {
    // 8 disc + 32 multisig + 8 tx_index = 48 -> status (1 byte variant)
    if data.len() < 62 {
        return Err(anyhow::anyhow!(
            "proposal account data too short ({} bytes)",
            data.len()
        )
        .into());
    }

    let transaction_index = u64::from_le_bytes(data[40..48].try_into().unwrap());
    let status = ProposalStatus::from_discriminant(data[48])?;

    // Status payload: all variants except Executing (4) have an i64 timestamp (8 bytes)
    let status_size = if status == ProposalStatus::Executing {
        1
    } else {
        9
    };
    let bump_offset = 48 + status_size;
    let approved_len_offset = bump_offset + 1; // skip bump byte

    if data.len() < approved_len_offset + 4 {
        return Err(anyhow::anyhow!("proposal data too short for approved vec").into());
    }

    let num_approved =
        u32::from_le_bytes(data[approved_len_offset..approved_len_offset + 4].try_into().unwrap())
            as usize;
    let approved_start = approved_len_offset + 4;
    let required = approved_start + num_approved * 32;
    if data.len() < required {
        return Err(anyhow::anyhow!("proposal data too short for {} approvals", num_approved).into());
    }

    let mut approved = Vec::with_capacity(num_approved);
    for i in 0..num_approved {
        let offset = approved_start + i * 32;
        approved.push(Address::from(
            <[u8; 32]>::try_from(&data[offset..offset + 32]).unwrap(),
        ));
    }

    Ok(ProposalState {
        transaction_index,
        status,
        approved,
    })
}

/// Format an address as "1234...5678".
fn short_address(addr: &Address) -> String {
    let s = bs58::encode(addr).into_string();
    if s.len() <= 8 {
        return s;
    }
    format!("{}...{}", &s[..4], &s[s.len() - 4..])
}

// ---------------------------------------------------------------------------
// Instruction building
// ---------------------------------------------------------------------------

/// Compute an Anchor instruction discriminator: first 8 bytes of
/// sha256("global:<name>").
fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}").as_bytes());
    let hash = hasher.finalize();
    hash[..8].try_into().unwrap()
}

/// Build the inner TransactionMessage bytes for a BPF upgrade.
/// This is the Squads SmallVec-encoded message that gets stored on-chain.
///
/// The inner instruction upgrades `program_id` using `buffer` with the
/// `vault` PDA as the upgrade authority.
pub fn build_upgrade_message(
    vault: &Address,
    program_id: &Address,
    buffer: &Address,
    spill: &Address,
) -> Vec<u8> {
    let (programdata, _) = programdata_pda(program_id);

    // Account keys ordering:
    // [0] vault (writable signer — the authority)
    // [1] programdata (writable)
    // [2] program_id (writable)
    // [3] buffer (writable)
    // [4] spill (writable)
    // [5] rent sysvar (readonly)
    // [6] clock sysvar (readonly)
    // [7] BPF loader upgradeable (readonly, program)
    let account_keys: Vec<&Address> = vec![
        vault,
        &programdata,
        program_id,
        buffer,
        spill,
        &SYSVAR_RENT_ID,
        &SYSVAR_CLOCK_ID,
        &BPF_LOADER_UPGRADEABLE_ID,
    ];

    let num_signers: u8 = 1; // vault
    let num_writable_signers: u8 = 1; // vault is writable
    let num_writable_non_signers: u8 = 4; // programdata, program, buffer, spill

    // BPF upgrade instruction data: u32 LE = 3 (Upgrade variant)
    let ix_data: [u8; 4] = [0x03, 0x00, 0x00, 0x00];

    // Compiled instruction: program_id_index=7 (BPF loader),
    // accounts=[1,2,3,4,5,6,0] Account order for upgrade(): programdata,
    // program, buffer, spill, rent, clock, authority
    let account_indexes: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 0];

    // Serialize TransactionMessage with SmallVec encoding
    let mut msg = vec![
        num_signers,
        num_writable_signers,
        num_writable_non_signers,
        // account_keys: SmallVec<u8, Pubkey>
        account_keys.len() as u8,
    ];
    for key in &account_keys {
        msg.extend_from_slice(key.as_ref());
    }

    // instructions: SmallVec<u8, CompiledInstruction>
    msg.push(1u8); // 1 instruction
                   // CompiledInstruction:
    msg.push(7u8); // program_id_index
                   // account_indexes: SmallVec<u8, u8>
    msg.push(account_indexes.len() as u8);
    msg.extend_from_slice(&account_indexes);
    // data: SmallVec<u16, u8>
    msg.extend_from_slice(&(ix_data.len() as u16).to_le_bytes());
    msg.extend_from_slice(&ix_data);

    // address_table_lookups: SmallVec<u8, _> — empty
    msg.push(0u8);

    msg
}

/// Build the VaultTransactionCreate instruction.
pub fn vault_transaction_create_ix(
    multisig: &Address,
    transaction: &Address,
    creator: &Address,
    rent_payer: &Address,
    vault_index: u8,
    transaction_message: Vec<u8>,
) -> solana_instruction::Instruction {
    let discriminator = anchor_discriminator("vault_transaction_create");

    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    data.push(vault_index);
    data.push(0u8); // ephemeral_signers = 0
                    // transaction_message: Borsh Vec<u8> = u32 LE length + bytes
    data.extend_from_slice(&(transaction_message.len() as u32).to_le_bytes());
    data.extend_from_slice(&transaction_message);
    data.push(0u8); // memo: Option<String> = None

    solana_instruction::Instruction {
        program_id: SQUADS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*multisig, false),
            AccountMeta::new(*transaction, false),
            AccountMeta::new_readonly(*creator, true),
            AccountMeta::new(*rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}

/// Build the ProposalCreate instruction.
pub fn proposal_create_ix(
    multisig: &Address,
    proposal: &Address,
    creator: &Address,
    rent_payer: &Address,
    transaction_index: u64,
) -> solana_instruction::Instruction {
    let discriminator = anchor_discriminator("proposal_create");

    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&transaction_index.to_le_bytes());
    data.push(0u8); // draft = false (start as Active)

    solana_instruction::Instruction {
        program_id: SQUADS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(*multisig, false),
            AccountMeta::new(*proposal, false),
            AccountMeta::new_readonly(*creator, true),
            AccountMeta::new(*rent_payer, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}

/// Build the ProposalApprove instruction.
pub fn proposal_approve_ix(
    multisig: &Address,
    member: &Address,
    proposal: &Address,
) -> solana_instruction::Instruction {
    let discriminator = anchor_discriminator("proposal_approve");

    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    data.push(0u8); // memo: Option<String> = None

    solana_instruction::Instruction {
        program_id: SQUADS_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(*multisig, false),
            AccountMeta::new(*member, true),
            AccountMeta::new(*proposal, false),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// Buffer upload
// ---------------------------------------------------------------------------

/// Upload a program binary as a buffer via the Solana CLI.
/// Returns the buffer address.
pub fn write_buffer(
    so_path: &Path,
    keypair_path: &Path,
    rpc_url: &str,
) -> Result<Address, crate::error::CliError> {
    let output = Command::new("solana")
        .args([
            "program",
            "write-buffer",
            so_path.to_str().unwrap_or_default(),
            "--keypair",
            keypair_path.to_str().unwrap_or_default(),
            "--url",
            rpc_url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run solana program write-buffer: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("write-buffer failed: {stderr}").into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output format: "Buffer: <ADDRESS>"
    let addr_str = stdout
        .lines()
        .find(|l| l.contains("Buffer:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim())
        .ok_or_else(|| anyhow::anyhow!("could not parse buffer address from: {stdout}"))?;

    let bytes: [u8; 32] = bs58::decode(addr_str)
        .into_vec()
        .map_err(|e| anyhow::anyhow!("invalid buffer address: {e}"))?
        .try_into()
        .map_err(|_| anyhow::anyhow!("buffer address wrong length"))?;

    Ok(Address::from(bytes))
}

/// Transfer buffer authority to a new address (the vault PDA) so Squads
/// can execute the upgrade.
fn set_buffer_authority(
    buffer: &Address,
    new_authority: &Address,
    keypair_path: &Path,
    rpc_url: &str,
) -> Result<(), crate::error::CliError> {
    let output = Command::new("solana")
        .args([
            "program",
            "set-buffer-authority",
            &bs58::encode(buffer).into_string(),
            "--new-buffer-authority",
            &bs58::encode(new_authority).into_string(),
            "--keypair",
            keypair_path.to_str().unwrap_or_default(),
            "--url",
            rpc_url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run solana program set-buffer-authority: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("set-buffer-authority failed: {stderr}").into());
    }

    Ok(())
}

/// Transfer a program's upgrade authority to a new address.
/// Used after initial deploy to hand authority to the Squads vault PDA.
pub fn set_upgrade_authority(
    program_id: &Address,
    new_authority: &Address,
    keypair_path: &Path,
    rpc_url: &str,
) -> Result<(), crate::error::CliError> {
    let output = Command::new("solana")
        .args([
            "program",
            "set-upgrade-authority",
            &bs58::encode(program_id).into_string(),
            "--new-upgrade-authority",
            &bs58::encode(new_authority).into_string(),
            "--keypair",
            keypair_path.to_str().unwrap_or_default(),
            "--url",
            rpc_url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            anyhow::anyhow!("failed to run solana program set-upgrade-authority: {e}")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("set-upgrade-authority failed: {stderr}").into());
    }

    Ok(())
}

/// Read a program ID (public key) from a Solana keypair file.
/// Public key is bytes 32..64 of the 64-byte keypair.
pub fn read_program_id_from_keypair(path: &Path) -> Result<Address, crate::error::CliError> {
    if !path.exists() {
        return Err(anyhow::anyhow!(
            "program keypair not found: {}",
            path.display()
        )
        .into());
    }
    let contents = fs::read_to_string(path)?;
    let bytes: Vec<u8> = serde_json::from_str(&contents).map_err(anyhow::Error::from)?;
    if bytes.len() != 64 {
        return Err(anyhow::anyhow!(
            "program keypair must contain exactly 64 bytes, got {}",
            bytes.len()
        )
        .into());
    }
    Ok(Address::from(
        <[u8; 32]>::try_from(&bytes[32..64]).unwrap(),
    ))
}

// ---------------------------------------------------------------------------
// Top-level orchestrator
// ---------------------------------------------------------------------------

/// Propose a program upgrade through a Squads multisig.
///
/// 1. Uploads the .so as a buffer
/// 2. Builds the Squads vault transaction + proposal + approve
/// 3. Signs and sends the transaction
pub fn propose_upgrade(
    so_path: &Path,
    program_id: &Address,
    multisig: &Address,
    keypair_path: &Path,
    rpc_url: &str,
    vault_index: u8,
) -> crate::error::CliResult {
    let keypair = Keypair::read_from_file(keypair_path)?;
    let member = keypair.address();

    // 1. Upload buffer
    let sp = style::spinner("Uploading program buffer...");
    let buffer = write_buffer(so_path, keypair_path, rpc_url)?;
    sp.finish_and_clear();
    println!(
        "  {} Buffer: {}",
        style::dim("✓"),
        bs58::encode(buffer).into_string()
    );

    // 2. Transfer buffer authority to the vault so Squads can use it
    let (vault, _) = vault_pda(multisig, vault_index);
    let sp = style::spinner("Transferring buffer authority to vault...");
    set_buffer_authority(&buffer, &vault, keypair_path, rpc_url)?;
    sp.finish_and_clear();
    println!("  {} Buffer authority transferred to vault", style::dim("✓"));

    // 3. Read multisig state to get next transaction index
    let account_data = get_account_data(rpc_url, multisig)?.ok_or_else(|| {
        anyhow::anyhow!(
            "multisig account not found: {}",
            bs58::encode(multisig).into_string()
        )
    })?;
    let current_index = read_transaction_index(&account_data)?;
    let next_index = current_index
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("transaction index overflow"))?;

    // 4. Derive remaining PDAs
    let (transaction, _) = transaction_pda(multisig, next_index);
    let (proposal, _) = proposal_pda(multisig, next_index);

    // 5. Build inner upgrade message
    let upgrade_msg = build_upgrade_message(&vault, program_id, &buffer, &member);

    // 6. Build Squads instructions
    let ix_create = vault_transaction_create_ix(
        multisig,
        &transaction,
        &member,
        &member,
        vault_index,
        upgrade_msg,
    );
    let ix_propose = proposal_create_ix(multisig, &proposal, &member, &member, next_index);
    let ix_approve = proposal_approve_ix(multisig, &member, &proposal);

    // 7. Build, sign, send transaction
    let sp = style::spinner("Submitting proposal...");

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix_create, ix_propose, ix_approve],
        Some(&member),
        &[&keypair],
        blockhash,
    );

    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;

    let sig = send_transaction(rpc_url, &tx_bytes)?;

    sp.finish_and_clear();

    println!(
        "\n  {}",
        style::success(&format!(
            "Upgrade proposed (tx #{})",
            style::bold(&next_index.to_string())
        ))
    );
    println!("  {} {sig}", style::dim("Signature:"));
    println!(
        "  {} https://v4.squads.so/transactions/{}/tx/{}",
        style::dim("Squads:"),
        bs58::encode(multisig).into_string(),
        next_index
    );
    println!();

    Ok(())
}

/// Show the approval status of the latest multisig proposal.
///
/// Displays each member's vote status with colored indicators, and prompts
/// the user to execute if the threshold has been reached.
pub fn show_proposal_status(
    multisig: &Address,
    keypair_path: &Path,
    rpc_url: &str,
) -> crate::error::CliResult {
    // 1. Fetch and parse the multisig account
    let sp = style::spinner("Fetching multisig state...");
    let ms_data = get_account_data(rpc_url, multisig)?.ok_or_else(|| {
        anyhow::anyhow!(
            "multisig account not found: {}",
            bs58::encode(multisig).into_string()
        )
    })?;
    let ms = parse_multisig_account(&ms_data)?;
    sp.finish_and_clear();

    if ms.transaction_index == 0 {
        println!("\n  {} No proposals found for this multisig.\n", style::dim("·"));
        return Ok(());
    }

    // 2. Fetch the latest proposal
    let (proposal_addr, _) = proposal_pda(multisig, ms.transaction_index);
    let sp = style::spinner("Fetching proposal...");
    let prop_data = get_account_data(rpc_url, &proposal_addr)?.ok_or_else(|| {
        anyhow::anyhow!(
            "proposal account not found for tx #{}",
            ms.transaction_index
        )
    })?;
    let proposal = parse_proposal_account(&prop_data)?;
    sp.finish_and_clear();

    // 3. Display header
    let multisig_short = short_address(multisig);
    println!();
    println!(
        "  {} Multisig {} — Transaction #{}",
        style::bold("▸"),
        style::color(45, &multisig_short),
        style::bold(&ms.transaction_index.to_string()),
    );
    println!(
        "  {} Proposal status: {}",
        style::dim("│"),
        match proposal.status {
            ProposalStatus::Active => style::color(220, proposal.status.label()),
            ProposalStatus::Approved => style::color(83, proposal.status.label()),
            ProposalStatus::Executed => style::color(83, proposal.status.label()),
            ProposalStatus::Rejected => style::color(196, proposal.status.label()),
            ProposalStatus::Cancelled => style::color(196, proposal.status.label()),
            _ => style::dim(proposal.status.label()),
        },
    );
    println!("  {}", style::dim("│"));

    // 4. Show each voting member's status
    let voters: Vec<&MultisigMember> = ms.members.iter().filter(|m| m.can_vote()).collect();
    let approved_count = proposal.approved.len();

    for member in &voters {
        let addr = short_address(&member.key);
        let voted = proposal.approved.contains(&member.key);
        if voted {
            // Green checkmark
            println!(
                "  {}  {} {}",
                style::dim("│"),
                style::color(83, "✔"),
                style::color(83, &addr),
            );
        } else {
            // Dim pending dot
            println!(
                "  {}  {} {}",
                style::dim("│"),
                style::dim("·"),
                style::dim(&addr),
            );
        }
    }

    println!("  {}", style::dim("│"));

    // 5. Show threshold status
    let threshold = ms.threshold as usize;
    let remaining = threshold.saturating_sub(approved_count);

    if approved_count >= threshold {
        println!(
            "  {} Status: {}/{} signed — {}",
            style::dim("╰"),
            style::color(83, &approved_count.to_string()),
            threshold,
            style::color(83, "ready to execute"),
        );
        println!();

        // Prompt user to execute
        print!(
            "  {} Execute this transaction? [y/N] ",
            style::color(45, "?"),
        );
        use std::io::Write;
        std::io::stdout().flush().ok();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        let input = input.trim().to_lowercase();

        if input == "y" || input == "yes" {
            execute_approved_proposal(
                multisig,
                &ms,
                &proposal,
                keypair_path,
                rpc_url,
            )?;
        } else {
            println!();
        }
    } else {
        println!(
            "  {} Status: {}/{} signed — awaiting {} {}",
            style::dim("╰"),
            style::color(220, &approved_count.to_string()),
            threshold,
            style::bold(&remaining.to_string()),
            if remaining == 1 { "signature" } else { "signatures" },
        );
        println!();
    }

    Ok(())
}

/// Execute an approved proposal by calling VaultTransactionExecute.
fn execute_approved_proposal(
    multisig: &Address,
    ms: &MultisigState,
    proposal: &ProposalState,
    keypair_path: &Path,
    rpc_url: &str,
) -> crate::error::CliResult {
    let keypair = Keypair::read_from_file(keypair_path)?;
    let member = keypair.address();

    let tx_index = proposal.transaction_index;
    let (transaction_pda, _) = transaction_pda(multisig, tx_index);
    let (proposal_pda, _) = proposal_pda(multisig, tx_index);

    // Fetch the VaultTransaction to get inner accounts for execute
    let tx_data = get_account_data(rpc_url, &transaction_pda)?.ok_or_else(|| {
        anyhow::anyhow!("vault transaction account not found for tx #{tx_index}")
    })?;

    let sp = style::spinner("Executing transaction...");

    // Build VaultTransactionExecute instruction
    let ix = vault_transaction_execute_ix(
        multisig,
        &transaction_pda,
        &proposal_pda,
        &member,
        &tx_data,
        ms,
    )?;

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&member),
        &[&keypair],
        blockhash,
    );

    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;

    let sig = send_transaction(rpc_url, &tx_bytes)?;
    sp.finish_and_clear();

    println!(
        "\n  {}",
        style::success(&format!(
            "Transaction #{} executed",
            style::bold(&tx_index.to_string())
        ))
    );
    println!("  {} {sig}", style::dim("Signature:"));
    println!();

    Ok(())
}

/// Build the VaultTransactionExecute instruction.
///
/// Parses the VaultTransaction account data to extract inner account keys
/// needed for the execute instruction's remaining accounts.
fn vault_transaction_execute_ix(
    multisig: &Address,
    transaction: &Address,
    proposal: &Address,
    member: &Address,
    vault_tx_data: &[u8],
    ms: &MultisigState,
) -> Result<solana_instruction::Instruction, crate::error::CliError> {
    let discriminator = anchor_discriminator("vault_transaction_execute");

    // VaultTransaction layout:
    // 8 disc + 32 multisig + 8 creator + 8 tx_index + 1 bump + 1 vault_index
    // + 1 ephemeral_signers + 4 message_len + message_bytes
    // We need the vault_index to derive vault PDA, and the message's account_keys
    // to pass as remaining accounts.
    if vault_tx_data.len() < 59 {
        return Err(anyhow::anyhow!("vault transaction data too short").into());
    }

    let vault_index = vault_tx_data[49];
    let (vault, _) = vault_pda(multisig, vault_index);

    // Parse inner message account keys
    // Offset 50: ephemeral_signers (u8), 51: message (Borsh Vec<u8>: u32 len + bytes)
    let _ephemeral_signers = vault_tx_data[50];
    if vault_tx_data.len() < 55 {
        return Err(anyhow::anyhow!("vault transaction data too short for message").into());
    }
    let msg_len =
        u32::from_le_bytes(vault_tx_data[51..55].try_into().unwrap()) as usize;
    if vault_tx_data.len() < 55 + msg_len {
        return Err(anyhow::anyhow!("vault transaction data too short for message bytes").into());
    }
    let msg = &vault_tx_data[55..55 + msg_len];

    // TransactionMessage layout (SmallVec):
    // 3 header bytes, then u8 num_keys, then num_keys * 32 bytes of account keys
    if msg.len() < 4 {
        return Err(anyhow::anyhow!("inner message too short").into());
    }
    let num_keys = msg[3] as usize;
    if msg.len() < 4 + num_keys * 32 {
        return Err(anyhow::anyhow!("inner message too short for account keys").into());
    }

    // Collect the inner account keys (skip index 0 which is the vault/signer)
    let mut remaining_accounts = Vec::new();
    for i in 1..num_keys {
        let offset = 4 + i * 32;
        let key = Address::from(<[u8; 32]>::try_from(&msg[offset..offset + 32]).unwrap());
        // All non-vault accounts are passed as writable non-signers
        remaining_accounts.push(AccountMeta::new(key, false));
    }

    // Also add program IDs from the inner instructions
    // The compiled instructions reference program_id_index into the account_keys array,
    // so those are already covered above.

    // Build the main accounts
    let mut accounts = vec![
        AccountMeta::new(*multisig, false),
        AccountMeta::new_readonly(*transaction, false),
        AccountMeta::new(*proposal, false),
        AccountMeta::new_readonly(*member, true),
    ];

    // Add all multisig members as non-signer readonly (required by Squads)
    for m in &ms.members {
        accounts.push(AccountMeta::new_readonly(m.key, false));
    }

    // Vault (ephemeral signer)
    accounts.push(AccountMeta::new(vault, false));

    // Remaining accounts from the inner transaction
    accounts.extend(remaining_accounts);

    Ok(solana_instruction::Instruction {
        program_id: SQUADS_PROGRAM_ID,
        accounts,
        data: discriminator.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_pda_derivation() {
        let multisig = Address::from([1u8; 32]);
        let (vault, _bump) = vault_pda(&multisig, 0);
        assert_ne!(vault, Address::default());
    }

    #[test]
    fn transaction_index_parsing() {
        let mut data = vec![0u8; 128];
        data[78..86].copy_from_slice(&42u64.to_le_bytes());
        assert_eq!(read_transaction_index(&data).unwrap(), 42);
    }

    #[test]
    fn transaction_index_too_short() {
        let data = vec![0u8; 50];
        assert!(read_transaction_index(&data).is_err());
    }

    #[test]
    fn anchor_discriminators() {
        assert_eq!(
            anchor_discriminator("vault_transaction_create"),
            [0x30, 0xfa, 0x4e, 0xa8, 0xd0, 0xe2, 0xda, 0xd3]
        );
        assert_eq!(
            anchor_discriminator("proposal_create"),
            [0xdc, 0x3c, 0x49, 0xe0, 0x1e, 0x6c, 0x4f, 0x9f]
        );
        assert_eq!(
            anchor_discriminator("proposal_approve"),
            [0x90, 0x25, 0xa4, 0x88, 0xbc, 0xd8, 0x2a, 0xf8]
        );
    }

    #[test]
    fn verify_squads_program_id() {
        let expected = bs58::decode("SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf")
            .into_vec()
            .unwrap();
        assert_eq!(SQUADS_PROGRAM_ID.as_ref(), &expected[..]);
    }

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
    fn tilde_expansion() {
        let expanded = expand_tilde("~/foo/bar");
        assert!(!expanded.starts_with('~'), "tilde should be expanded");
        assert!(expanded.ends_with("/foo/bar"));

        // Non-tilde paths are unchanged
        assert_eq!(expand_tilde("/absolute/path"), "/absolute/path");
        assert_eq!(expand_tilde("relative/path"), "relative/path");
    }

    #[test]
    fn short_address_formatting() {
        // Use a known address
        let addr = Address::from([
            0x06, 0x81, 0xc4, 0xce, 0x47, 0xe2, 0x23, 0x68, 0xb8, 0xb1, 0x55, 0x5e, 0xc8, 0x87,
            0xaf, 0x09, 0x2e, 0xfc, 0x7e, 0xfb, 0xb6, 0x6c, 0xa3, 0xf5, 0x2f, 0xbf, 0x68, 0xd4,
            0xac, 0x9c, 0xb7, 0xa8,
        ]);
        let short = short_address(&addr);
        assert!(short.contains("..."), "should contain ellipsis");
        assert_eq!(&short[..4], &bs58::encode(addr).into_string()[..4]);
    }

    #[test]
    fn parse_multisig_account_roundtrip() {
        // Build a fake multisig account with 2 members
        let mut data = vec![0u8; 132 + 2 * 33];
        // threshold at offset 72
        data[72..74].copy_from_slice(&3u16.to_le_bytes());
        // transaction_index at offset 78
        data[78..86].copy_from_slice(&5u64.to_le_bytes());
        // num_members at offset 128
        data[128..132].copy_from_slice(&2u32.to_le_bytes());
        // member 0: all 1s, permissions = 7 (all)
        data[132..164].copy_from_slice(&[1u8; 32]);
        data[164] = 0x07;
        // member 1: all 2s, permissions = 4 (execute only)
        data[165..197].copy_from_slice(&[2u8; 32]);
        data[197] = 0x04;

        let ms = parse_multisig_account(&data).unwrap();
        assert_eq!(ms.threshold, 3);
        assert_eq!(ms.transaction_index, 5);
        assert_eq!(ms.members.len(), 2);
        assert!(ms.members[0].can_vote());
        assert!(!ms.members[1].can_vote()); // execute-only can't vote
    }

    #[test]
    fn parse_proposal_account_active() {
        // Build a fake proposal with 1 approval
        let mut data = vec![0u8; 62 + 32]; // enough for 1 approval
        // transaction_index at offset 40
        data[40..48].copy_from_slice(&7u64.to_le_bytes());
        // status = Active (1) at offset 48
        data[48] = 1;
        // timestamp at 49..57 (don't care about value)
        // bump at 57
        // approved vec len at 58
        data[58..62].copy_from_slice(&1u32.to_le_bytes());
        // approved[0] = [3u8; 32]
        data[62..94].copy_from_slice(&[3u8; 32]);

        let prop = parse_proposal_account(&data).unwrap();
        assert_eq!(prop.transaction_index, 7);
        assert_eq!(prop.status, ProposalStatus::Active);
        assert_eq!(prop.approved.len(), 1);
        assert_eq!(prop.approved[0], Address::from([3u8; 32]));
    }

    #[test]
    fn parse_proposal_account_no_approvals() {
        let mut data = vec![0u8; 62];
        data[40..48].copy_from_slice(&1u64.to_le_bytes());
        data[48] = 0; // Draft
        // approved vec len = 0
        data[58..62].copy_from_slice(&0u32.to_le_bytes());

        let prop = parse_proposal_account(&data).unwrap();
        assert_eq!(prop.status, ProposalStatus::Draft);
        assert!(prop.approved.is_empty());
    }

    #[test]
    fn proposal_status_labels() {
        assert_eq!(ProposalStatus::Active.label(), "Active");
        assert_eq!(ProposalStatus::Approved.label(), "Approved");
        assert_eq!(ProposalStatus::Executed.label(), "Executed");
    }

    #[test]
    fn upgrade_message_is_valid() {
        let vault = Address::from([1u8; 32]);
        let program = Address::from([2u8; 32]);
        let buffer = Address::from([3u8; 32]);
        let spill = Address::from([4u8; 32]);
        let msg = build_upgrade_message(&vault, &program, &buffer, &spill);

        // Check header
        assert_eq!(msg[0], 1); // num_signers
        assert_eq!(msg[1], 1); // num_writable_signers
        assert_eq!(msg[2], 4); // num_writable_non_signers

        // Check 8 account keys
        assert_eq!(msg[3], 8);

        // Total size: 3 header + 1 len + 8*32 keys + 1 ix_count
        //   + 1 program_id_index + 1 acct_idx_len + 7 acct_idxs
        //   + 2 data_len + 4 data + 1 lookups_len
        assert_eq!(msg.len(), 3 + 1 + 256 + 1 + 1 + 1 + 7 + 2 + 4 + 1);
    }
}
