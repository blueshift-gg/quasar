use {
    crate::{
        bpf_loader::{
            self, BPF_LOADER_UPGRADEABLE_ID, SYSTEM_PROGRAM_ID, SYSVAR_CLOCK_ID, SYSVAR_RENT_ID,
            programdata_pda,
        },
        rpc::{get_account_data, get_latest_blockhash, send_transaction, Keypair},
        style,
    },
    sha2::{Digest, Sha256},
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::path::Path,
};

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
pub fn short_addr(addr: &Address) -> String {
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
    priority_fee: u64,
) -> crate::error::CliResult {
    let keypair = Keypair::read_from_file(keypair_path)?;
    let member = keypair.address();

    // 1. Upload buffer
    let sp = style::spinner("Uploading program buffer...");
    let buffer = bpf_loader::write_buffer(so_path, &keypair, rpc_url, priority_fee)?;
    sp.finish_and_clear();
    println!(
        "  {} Buffer: {}",
        style::dim("✓"),
        bs58::encode(buffer).into_string()
    );

    // 2. Transfer buffer authority to the vault so Squads can use it
    let (vault, _) = vault_pda(multisig, vault_index);
    let sp = style::spinner("Transferring buffer authority to vault...");
    bpf_loader::set_authority(&buffer, &keypair, Some(&vault), rpc_url, priority_fee)?;
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

    let mut ixs = vec![];
    if priority_fee > 0 {
        ixs.push(bpf_loader::set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(ix_create);
    ixs.push(ix_propose);
    ixs.push(ix_approve);

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
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
    priority_fee: u64,
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
    let multisig_short = short_addr(multisig);
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
        let addr = short_addr(&member.key);
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
                priority_fee,
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
    priority_fee: u64,
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

    let mut ixs = vec![];
    if priority_fee > 0 {
        ixs.push(bpf_loader::set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(ix);

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
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

    // VaultTransaction layout (Squads v4):
    //  0: [u8; 8]  Anchor discriminator
    //  8: Pubkey    multisig          (32 bytes)
    // 40: Pubkey    creator           (32 bytes)
    // 72: u64       index             ( 8 bytes)
    // 80: u8        bump              ( 1 byte)
    // 81: u8        vault_index       ( 1 byte)
    // 82: u8        vault_bump        ( 1 byte)
    // 83: Vec<u8>   ephemeral_signer_bumps (4 byte len + N bytes)
    // 87+N: message (VaultTransactionMessage, variable)
    if vault_tx_data.len() < 87 {
        return Err(anyhow::anyhow!("vault transaction data too short").into());
    }

    let vault_index = vault_tx_data[81];
    let (vault, _) = vault_pda(multisig, vault_index);

    // Parse ephemeral_signer_bumps Vec<u8> to skip past it
    let eph_bumps_len =
        u32::from_le_bytes(vault_tx_data[83..87].try_into().unwrap()) as usize;
    let msg_offset = 87 + eph_bumps_len;

    // The message field is serialized as a Borsh Vec<u8>: u32 LE length + bytes
    if vault_tx_data.len() < msg_offset + 4 {
        return Err(
            anyhow::anyhow!("vault transaction data too short for message length").into(),
        );
    }
    let msg_len = u32::from_le_bytes(
        vault_tx_data[msg_offset..msg_offset + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let msg_start = msg_offset + 4;
    if vault_tx_data.len() < msg_start + msg_len {
        return Err(
            anyhow::anyhow!("vault transaction data too short for message bytes").into(),
        );
    }
    let msg = &vault_tx_data[msg_start..msg_start + msg_len];

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
    fn short_addr_formatting() {
        // Use a known address
        let addr = Address::from([
            0x06, 0x81, 0xc4, 0xce, 0x47, 0xe2, 0x23, 0x68, 0xb8, 0xb1, 0x55, 0x5e, 0xc8, 0x87,
            0xaf, 0x09, 0x2e, 0xfc, 0x7e, 0xfb, 0xb6, 0x6c, 0xa3, 0xf5, 0x2f, 0xbf, 0x68, 0xd4,
            0xac, 0x9c, 0xb7, 0xa8,
        ]);
        let short = short_addr(&addr);
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

    #[test]
    fn vault_transaction_execute_ix_parses_correct_offsets() {
        // Build a synthetic VaultTransaction account matching Squads v4 layout:
        //  0: discriminator (8)
        //  8: multisig (32)
        // 40: creator (32)
        // 72: index (8)
        // 80: bump (1)
        // 81: vault_index (1)
        // 82: vault_bump (1)
        // 83: ephemeral_signer_bumps vec len (4) + data (0)
        // 87: message vec len (4) + message data

        let multisig_addr = Address::from([0xAA; 32]);

        // Build an inner TransactionMessage with 2 account keys (vault + 1 extra)
        let vault_index: u8 = 0;
        let (vault, _) = vault_pda(&multisig_addr, vault_index);
        let extra_key = Address::from([0xBB; 32]);

        let mut inner_msg = vec![
            1u8, // num_signers
            1,   // num_writable_signers
            0,   // num_writable_non_signers
            2,   // num_keys
        ];
        inner_msg.extend_from_slice(vault.as_ref());
        inner_msg.extend_from_slice(extra_key.as_ref());
        // 1 compiled instruction
        inner_msg.push(1); // num_instructions
        inner_msg.push(0); // program_id_index
        inner_msg.push(0); // account_indexes len
        inner_msg.extend_from_slice(&0u16.to_le_bytes()); // data len
        inner_msg.push(0); // address_table_lookups len

        // Build full account data
        let mut data = vec![0u8; 87];
        // discriminator at 0..8 (zeroes fine)
        data[8..40].copy_from_slice(&[0xAA; 32]); // multisig
        data[40..72].copy_from_slice(&[0xCC; 32]); // creator
        data[72..80].copy_from_slice(&1u64.to_le_bytes()); // index
        data[80] = 255; // bump
        data[81] = vault_index;
        data[82] = 254; // vault_bump
        // ephemeral_signer_bumps: empty vec (len=0)
        data[83..87].copy_from_slice(&0u32.to_le_bytes());
        // message: Vec<u8>
        data.extend_from_slice(&(inner_msg.len() as u32).to_le_bytes());
        data.extend_from_slice(&inner_msg);

        let ms = MultisigState {
            threshold: 1,
            transaction_index: 1,
            members: vec![MultisigMember {
                key: Address::from([0xDD; 32]),
                permissions: 0x07,
            }],
        };

        let transaction_addr = Address::from([0xEE; 32]);
        let proposal_addr = Address::from([0xFF; 32]);
        let member_addr = Address::from([0xDD; 32]);

        let ix = vault_transaction_execute_ix(
            &multisig_addr,
            &transaction_addr,
            &proposal_addr,
            &member_addr,
            &data,
            &ms,
        )
        .unwrap();

        // Verify the instruction was built with correct accounts:
        // [multisig, transaction, proposal, member, ...members, vault, ...remaining]
        assert_eq!(ix.program_id, SQUADS_PROGRAM_ID);
        assert_eq!(ix.accounts[0].pubkey, multisig_addr);
        assert_eq!(ix.accounts[1].pubkey, transaction_addr);
        assert_eq!(ix.accounts[2].pubkey, proposal_addr);
        assert_eq!(ix.accounts[3].pubkey, member_addr);
        // member list (1 member)
        assert_eq!(ix.accounts[4].pubkey, Address::from([0xDD; 32]));
        // vault PDA
        assert_eq!(ix.accounts[5].pubkey, vault);
        // remaining accounts from inner message (skip index 0 which is vault)
        assert_eq!(ix.accounts[6].pubkey, extra_key);
        assert_eq!(ix.accounts.len(), 7);
    }

    #[test]
    fn vault_transaction_execute_ix_with_ephemeral_bumps() {
        // Same as above but with 2 ephemeral signer bumps to verify offset math
        let multisig_addr = Address::from([0xAA; 32]);
        let vault_index: u8 = 0;
        let (vault, _) = vault_pda(&multisig_addr, vault_index);

        // Minimal inner message: just the vault key, no extra accounts
        let mut inner_msg = vec![1, 1, 0, 1]; // 1 key (vault only)
        inner_msg.extend_from_slice(vault.as_ref());
        inner_msg.push(0); // 0 instructions
        inner_msg.push(0); // 0 lookups

        let mut data = vec![0u8; 87];
        data[8..40].copy_from_slice(&[0xAA; 32]); // multisig
        data[40..72].copy_from_slice(&[0xCC; 32]); // creator
        data[72..80].copy_from_slice(&1u64.to_le_bytes());
        data[80] = 255;
        data[81] = vault_index;
        data[82] = 254;
        // ephemeral_signer_bumps: 2 bumps
        data[83..87].copy_from_slice(&2u32.to_le_bytes());
        data.push(200); // bump 0
        data.push(201); // bump 1
        // message comes after bumps
        data.extend_from_slice(&(inner_msg.len() as u32).to_le_bytes());
        data.extend_from_slice(&inner_msg);

        let ms = MultisigState {
            threshold: 1,
            transaction_index: 1,
            members: vec![],
        };

        let ix = vault_transaction_execute_ix(
            &multisig_addr,
            &Address::from([0xEE; 32]),
            &Address::from([0xFF; 32]),
            &Address::from([0xDD; 32]),
            &data,
            &ms,
        )
        .unwrap();

        // Should still find the vault correctly despite the bump offset
        assert_eq!(ix.program_id, SQUADS_PROGRAM_ID);
        // accounts: multisig, transaction, proposal, member, vault (no remaining, no members)
        assert_eq!(ix.accounts[4].pubkey, vault);
    }
}
