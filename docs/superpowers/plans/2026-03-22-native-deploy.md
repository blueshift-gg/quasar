# Native Deploy with Priority Fees — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all Solana CLI shell-outs in the deploy pipeline with native Rust RPC calls, add priority fee support, and add pre-deploy validation.

**Architecture:** Extract shared RPC/keypair code from `multisig.rs` into `rpc.rs`. Create `bpf_loader.rs` for BPF Loader Upgradeable interactions. Update `deploy.rs` to call native orchestrators instead of shelling out. Add `--priority-fee` CLI flag with auto-calculation fallback.

**Tech Stack:** Rust, ureq (JSON-RPC), solana-transaction/instruction/signer/address crates, ed25519-dalek, bincode, indicatif (progress bar)

**Spec:** `docs/superpowers/specs/2026-03-22-native-deploy-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `cli/src/rpc.rs` | **Create** | RPC client, Keypair struct, Solana CLI config helpers, priority fee calculation, transaction confirmation |
| `cli/src/bpf_loader.rs` | **Create** | BPF Loader Upgradeable constants, instruction builders, buffer upload, deploy/upgrade orchestrators, authority validation |
| `cli/src/multisig.rs` | **Modify** | Remove all code moved to rpc.rs/bpf_loader.rs, import from new modules, replace shell-outs with native calls, add priority_fee params |
| `cli/src/deploy.rs` | **Modify** | Remove solana_deploy() shell-out, call native orchestrators, add reverse --upgrade check, add authority validation, propagate priority fee |
| `cli/src/lib.rs` | **Modify** | Add `pub mod rpc; pub mod bpf_loader;`, add `priority_fee: Option<u64>` to DeployCommand and DeployOpts |

---

### Task 1: Create `rpc.rs` — Extract RPC & Config Code

Move all RPC helpers, Keypair struct, and Solana CLI config functions from `multisig.rs` into a new `rpc.rs` module. This is a refactor — the only new code is `Keypair::generate()` (needed by Task 5). All existing tests must pass.

**Files:**
- Create: `cli/src/rpc.rs`
- Modify: `cli/src/lib.rs` (add module declaration)
- Modify: `cli/src/multisig.rs` (remove moved code, add imports from rpc)
- Modify: `cli/src/deploy.rs` (update import paths)

**What moves from `multisig.rs` to `rpc.rs`:**

The entire "Solana CLI config" section (lines 17-84):
- `solana_rpc_url(url_override) -> String`
- `resolve_cluster(input) -> String` (make `pub` — needed by tests)
- `solana_keypair_path(keypair_override) -> PathBuf`
- `read_config_field(field) -> Option<String>`
- `expand_tilde(path) -> String` (make `pub(crate)` — needed by tests)

The entire "Keypair" section (lines 86-127):
- `pub struct Keypair(SigningKey)` + all impls

The entire "RPC" section (lines 129-260):
- `get_latest_blockhash(rpc_url) -> Result<Hash>`
- `send_transaction(rpc_url, tx_bytes) -> Result<String>`
- `get_account_data(rpc_url, address) -> Result<Option<Vec<u8>>>`
- `program_exists_on_chain(rpc_url, program_id) -> Result<bool>`

The `read_program_id_from_keypair` function (lines 807-829).

**Tests that move to `rpc.rs`:**
- `tilde_expansion` (line 1311)
- `cluster_name_resolution` (line 1322)

- [ ] **Step 1: Create `cli/src/rpc.rs` with moved code**

```rust
use {
    ed25519_dalek::SigningKey,
    solana_address::Address,
    solana_hash::Hash,
    solana_signature::Signature,
    solana_signer::{Signer, SignerError},
    std::{fs, path::Path},
};

// ---------------------------------------------------------------------------
// Solana CLI config
// ---------------------------------------------------------------------------

/// Resolve a cluster name or URL to a full RPC endpoint.
pub fn solana_rpc_url(url_override: Option<&str>) -> String {
    if let Some(url) = url_override {
        return resolve_cluster(url);
    }
    read_config_field("json_rpc_url")
        .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string())
}

pub fn resolve_cluster(input: &str) -> String {
    match input {
        "mainnet-beta" => "https://api.mainnet-beta.solana.com".to_string(),
        "devnet" => "https://api.devnet.solana.com".to_string(),
        "testnet" => "https://api.testnet.solana.com".to_string(),
        "localnet" => "http://localhost:8899".to_string(),
        url => url.to_string(),
    }
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

pub(crate) fn expand_tilde(path: &str) -> String {
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

    /// Create a random keypair (for buffer accounts).
    pub fn generate() -> Self {
        use rand::rngs::OsRng;
        Self(SigningKey::generate(&mut OsRng))
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

pub fn get_latest_blockhash(rpc_url: &str) -> Result<Hash, crate::error::CliError> {
    // ... exact copy from multisig.rs lines 134-162
}

pub fn send_transaction(rpc_url: &str, tx_bytes: &[u8]) -> Result<String, crate::error::CliError> {
    // ... exact copy from multisig.rs lines 165-189
}

pub fn get_account_data(
    rpc_url: &str,
    address: &Address,
) -> Result<Option<Vec<u8>>, crate::error::CliError> {
    // ... exact copy from multisig.rs lines 192-225
}

pub fn program_exists_on_chain(
    rpc_url: &str,
    program_id: &Address,
) -> Result<bool, crate::error::CliError> {
    // ... exact copy from multisig.rs lines 229-260
}

pub fn read_program_id_from_keypair(path: &Path) -> Result<Address, crate::error::CliError> {
    // ... exact copy from multisig.rs lines 809-829
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expansion() {
        let expanded = expand_tilde("~/foo/bar");
        assert!(!expanded.starts_with('~'), "tilde should be expanded");
        assert!(expanded.ends_with("/foo/bar"));
        assert_eq!(expand_tilde("/absolute/path"), "/absolute/path");
        assert_eq!(expand_tilde("relative/path"), "relative/path");
    }

    #[test]
    fn cluster_name_resolution() {
        assert_eq!(resolve_cluster("mainnet-beta"), "https://api.mainnet-beta.solana.com");
        assert_eq!(resolve_cluster("devnet"), "https://api.devnet.solana.com");
        assert_eq!(resolve_cluster("testnet"), "https://api.testnet.solana.com");
        assert_eq!(resolve_cluster("localnet"), "http://localhost:8899");
        assert_eq!(resolve_cluster("https://my-rpc.example.com"), "https://my-rpc.example.com");
    }
}
```

Note: Copy the full function bodies from `multisig.rs` — the `// ...` comments above are shorthand for the plan, not actual code.

- [ ] **Step 2: Add `pub mod rpc;` to `lib.rs`**

In `cli/src/lib.rs`, add after `pub mod multisig;` (line 16):

```rust
pub mod rpc;
```

- [ ] **Step 3: Update `multisig.rs` — remove moved code, add imports**

Remove the following sections from `multisig.rs`:
- Lines 1-15: Replace the `use` block imports (remove `ed25519_dalek::SigningKey`, `solana_hash::Hash`, `solana_signature::Signature`, `solana_signer::{Signer, SignerError}`, `std::fs`)
- Lines 17-84: Remove `solana_rpc_url`, `resolve_cluster`, `solana_keypair_path`, `read_config_field`, `expand_tilde`
- Lines 86-127: Remove `Keypair` struct and impl
- Lines 129-260: Remove `get_latest_blockhash`, `send_transaction`, `get_account_data`, `program_exists_on_chain`
- Lines 807-829: Remove `read_program_id_from_keypair`

Replace with imports at the top:

```rust
use {
    crate::{
        rpc::{
            self, get_account_data, get_latest_blockhash, send_transaction, Keypair,
        },
        style,
    },
    sha2::{Digest, Sha256},
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::{
        path::Path,
        process::{Command, Stdio},
    },
};
```

All functions in multisig.rs that previously called `Keypair::read_from_file`, `get_latest_blockhash`, `send_transaction`, `get_account_data`, `read_program_id_from_keypair` now use the imported versions from `crate::rpc`.

Remove moved tests from `multisig.rs::tests`:
- `tilde_expansion`
- `cluster_name_resolution`

- [ ] **Step 4: Update `deploy.rs` import paths**

In `deploy.rs`, change all `crate::multisig::` references for moved functions:
- `crate::multisig::solana_keypair_path` → `crate::rpc::solana_keypair_path`
- `crate::multisig::solana_rpc_url` → `crate::rpc::solana_rpc_url`
- `crate::multisig::read_program_id_from_keypair` → `crate::rpc::read_program_id_from_keypair`
- `crate::multisig::program_exists_on_chain` → `crate::rpc::program_exists_on_chain`

Keep `crate::multisig::` for: `vault_pda`, `set_upgrade_authority`, `short_addr`, `propose_upgrade`, `show_proposal_status`.

- [ ] **Step 5: Run tests to verify refactor**

Run: `cargo test -p quasar-cli`
Expected: All 30 tests pass. Zero functionality changed.

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 6: Commit**

```bash
git add cli/src/rpc.rs cli/src/lib.rs cli/src/multisig.rs cli/src/deploy.rs
git commit -m "refactor: extract rpc.rs from multisig.rs"
```

---

### Task 2: Add New RPC Functions to `rpc.rs`

Add priority fee calculation, transaction confirmation, and rent exemption query.

**Files:**
- Modify: `cli/src/rpc.rs`

- [ ] **Step 1: Write tests for priority fee median calculation**

Add to the `tests` module in `rpc.rs`:

```rust
#[test]
fn priority_fee_median_odd() {
    assert_eq!(median_fee(&mut vec![100, 300, 200]), 200);
}

#[test]
fn priority_fee_median_even() {
    assert_eq!(median_fee(&mut vec![100, 200, 300, 400]), 250);
}

#[test]
fn priority_fee_median_empty() {
    assert_eq!(median_fee(&mut vec![]), 0);
}

#[test]
fn priority_fee_median_single() {
    assert_eq!(median_fee(&mut vec![500]), 500);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p quasar-cli median`
Expected: FAIL — `median_fee` not found.

- [ ] **Step 3: Implement `median_fee` helper**

Add to `rpc.rs` (private helper):

```rust
fn median_fee(fees: &mut Vec<u64>) -> u64 {
    if fees.is_empty() {
        return 0;
    }
    fees.sort_unstable();
    let mid = fees.len() / 2;
    if fees.len() % 2 == 0 {
        (fees[mid - 1] + fees[mid]) / 2
    } else {
        fees[mid]
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p quasar-cli median`
Expected: All 4 pass.

- [ ] **Step 5: Implement `get_recent_prioritization_fees`**

```rust
/// Query recent prioritization fees and return the median in micro-lamports.
/// Returns 0 if no recent fees are available.
pub fn get_recent_prioritization_fees(rpc_url: &str) -> Result<u64, crate::error::CliError> {
    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getRecentPrioritizationFees",
            "params": []
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", err).into());
    }

    let entries = resp["result"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut fees: Vec<u64> = entries
        .iter()
        .filter_map(|e| e["prioritizationFee"].as_u64())
        .filter(|&f| f > 0)
        .collect();

    Ok(median_fee(&mut fees))
}
```

- [ ] **Step 6: Implement `confirm_transaction`**

```rust
/// Poll `getSignatureStatuses` until the transaction reaches `confirmed`
/// commitment or the timeout expires. Returns true if confirmed.
pub fn confirm_transaction(
    rpc_url: &str,
    signature: &str,
    timeout_secs: u64,
) -> Result<bool, crate::error::CliError> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() >= timeout {
            return Ok(false);
        }

        let resp: serde_json::Value = ureq::post(rpc_url)
            .send_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getSignatureStatuses",
                "params": [[signature]]
            }))
            .map_err(anyhow::Error::from)?
            .body_mut()
            .read_json()
            .map_err(anyhow::Error::from)?;

        if let Some(status) = resp["result"]["value"][0].as_object() {
            if status.get("err").is_some() && !status["err"].is_null() {
                return Err(anyhow::anyhow!(
                    "transaction failed: {}",
                    status["err"]
                ).into());
            }
            let confirmation = status
                .get("confirmationStatus")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if confirmation == "confirmed" || confirmation == "finalized" {
                return Ok(true);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
```

- [ ] **Step 7: Implement `get_minimum_balance_for_rent_exemption`**

```rust
/// Query the minimum balance for rent exemption for a given data length.
pub fn get_minimum_balance_for_rent_exemption(
    rpc_url: &str,
    data_len: usize,
) -> Result<u64, crate::error::CliError> {
    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMinimumBalanceForRentExemption",
            "params": [data_len]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow::anyhow!("RPC error: {}", err).into());
    }

    resp["result"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("missing rent exemption in RPC response").into())
}
```

- [ ] **Step 8: Run all tests**

Run: `cargo test -p quasar-cli`
Expected: All tests pass (30 existing + 4 new median tests = 34).

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 9: Commit**

```bash
git add cli/src/rpc.rs
git commit -m "feat: add priority fee, confirm_transaction, rent exemption RPC helpers"
```

---

### Task 3: Create `bpf_loader.rs` — Constants & PDA

Move BPF Loader constants and `programdata_pda` from `multisig.rs`. Add new constants.

**Files:**
- Create: `cli/src/bpf_loader.rs`
- Modify: `cli/src/lib.rs` (add module declaration)
- Modify: `cli/src/multisig.rs` (remove moved constants, import from bpf_loader)

- [ ] **Step 1: Create `cli/src/bpf_loader.rs` with constants and PDA**

```rust
use solana_address::Address;

// ---------------------------------------------------------------------------
// Program IDs & Sysvars
// ---------------------------------------------------------------------------

/// BPF Loader Upgradeable — BPFLoaderUpgradeab1e11111111111111111111111.
pub const BPF_LOADER_UPGRADEABLE_ID: Address = Address::new_from_array([
    0x02, 0xa8, 0xf6, 0x91, 0x4e, 0x88, 0xa1, 0xb0, 0xe2, 0x10, 0x15, 0x3e, 0xf7, 0x63, 0xae, 0x2b,
    0x00, 0xc2, 0xb9, 0x3d, 0x16, 0xc1, 0x24, 0xd2, 0xc0, 0x53, 0x7a, 0x10, 0x04, 0x80, 0x00, 0x00,
]);

/// System program ID.
pub const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Sysvar Rent — SysvarRent111111111111111111111111111111111.
pub const SYSVAR_RENT_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);

/// Sysvar Clock — SysvarC1ock11111111111111111111111111111111.
pub const SYSVAR_CLOCK_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

/// ComputeBudget program — ComputeBudget111111111111111111111111111111.
pub const COMPUTE_BUDGET_PROGRAM_ID: Address = Address::new_from_array([
    3, 6, 70, 111, 229, 33, 23, 50, 255, 236, 173, 186, 114, 195, 155, 231,
    188, 140, 229, 187, 197, 247, 18, 107, 44, 67, 155, 58, 64, 0, 0, 0,
]);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Bytes per Write transaction chunk. Accounts for tx overhead (~212 bytes)
/// plus ComputeBudget::SetComputeUnitPrice instruction (~45 bytes) within
/// the 1232-byte transaction limit.
pub const CHUNK_SIZE: usize = 950;

/// Buffer account header size: 4 bytes (discriminant u32 LE = 1) +
/// 1 byte (Option tag) + 32 bytes (authority pubkey).
pub const BUFFER_HEADER_SIZE: usize = 37;

// ---------------------------------------------------------------------------
// PDA derivation
// ---------------------------------------------------------------------------

/// Derive the programdata PDA for a given program address.
pub fn programdata_pda(program_id: &Address) -> (Address, u8) {
    Address::find_program_address(&[program_id.as_ref()], &BPF_LOADER_UPGRADEABLE_ID)
}

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
        // 4 (discriminant) + 1 (Option tag) + 32 (authority) = 37
        assert_eq!(BUFFER_HEADER_SIZE, 4 + 1 + 32);
    }
}
```

- [ ] **Step 2: Add `pub mod bpf_loader;` to `lib.rs`**

In `cli/src/lib.rs`, add after the `pub mod rpc;` line added in Task 1:

```rust
pub mod bpf_loader;
```

- [ ] **Step 3: Update `multisig.rs` — remove moved constants, import from bpf_loader**

Remove from `multisig.rs`:
- `BPF_LOADER_UPGRADEABLE_ID` constant (lines 275-281)
- `SYSTEM_PROGRAM_ID` constant (line 284)
- `SYSVAR_RENT_ID` constant (lines 286-291)
- `SYSVAR_CLOCK_ID` constant (lines 293-298)
- `programdata_pda` function (lines 332-334)

Add to the `use` block at the top of `multisig.rs`:

```rust
use crate::bpf_loader::{
    BPF_LOADER_UPGRADEABLE_ID, SYSTEM_PROGRAM_ID, SYSVAR_CLOCK_ID, SYSVAR_RENT_ID,
    programdata_pda,
};
```

Remove moved tests from `multisig.rs::tests`:
- `verify_bpf_loader_id`
- `verify_sysvar_rent_id`
- `verify_sysvar_clock_id`

- [ ] **Step 4: Run tests**

Run: `cargo test -p quasar-cli`
Expected: All tests pass (moved tests now in rpc.rs + bpf_loader.rs).

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add cli/src/bpf_loader.rs cli/src/lib.rs cli/src/multisig.rs
git commit -m "refactor: extract bpf_loader.rs constants and PDA from multisig.rs"
```

---

### Task 4: Add BPF Instruction Builders to `bpf_loader.rs`

Add all 5 BPF Loader Upgradeable instruction builders and the ComputeBudget SetComputeUnitPrice instruction.

**Files:**
- Modify: `cli/src/bpf_loader.rs`

- [ ] **Step 1: Write tests for instruction serialization**

Add to the `tests` module in `bpf_loader.rs`:

```rust
#[test]
fn initialize_buffer_ix_serialization() {
    let buffer = Address::from([1u8; 32]);
    let authority = Address::from([2u8; 32]);
    let ix = initialize_buffer_ix(&buffer, &authority);
    assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
    // Discriminant: 0u32 LE
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
    // Discriminant: 1u32 LE
    assert_eq!(&ix.data[..4], &[1, 0, 0, 0]);
    // Offset: 500u32 LE
    assert_eq!(&ix.data[4..8], &500u32.to_le_bytes());
    // Length: 100u32 LE
    assert_eq!(&ix.data[8..12], &100u32.to_le_bytes());
    // Data
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
    let ix = deploy_with_max_data_len_ix(&payer, &programdata, &program, &buffer, &authority, 10000);
    assert_eq!(ix.program_id, BPF_LOADER_UPGRADEABLE_ID);
    // Discriminant: 2u32 LE
    assert_eq!(&ix.data[..4], &[2, 0, 0, 0]);
    // max_data_len: 10000u64 LE
    assert_eq!(&ix.data[4..12], &10000u64.to_le_bytes());
    assert_eq!(ix.data.len(), 12);
    assert_eq!(ix.accounts.len(), 8);
    // Verify account ordering: payer, programdata, program, buffer, rent, clock, system, authority
    assert_eq!(ix.accounts[0].pubkey, payer);
    assert_eq!(ix.accounts[1].pubkey, programdata);
    assert_eq!(ix.accounts[2].pubkey, program);
    assert_eq!(ix.accounts[3].pubkey, buffer);
    assert_eq!(ix.accounts[4].pubkey, SYSVAR_RENT_ID);
    assert_eq!(ix.accounts[5].pubkey, SYSVAR_CLOCK_ID);
    assert_eq!(ix.accounts[6].pubkey, SYSTEM_PROGRAM_ID);
    assert_eq!(ix.accounts[7].pubkey, authority);
    assert!(ix.accounts[7].is_signer); // authority must sign
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
    assert!(ix.accounts[6].is_signer); // authority
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

    // Without new authority (make immutable)
    let ix2 = set_authority_ix(&account, &current, None);
    assert_eq!(ix2.accounts.len(), 2);
}

#[test]
fn set_compute_unit_price_ix_serialization() {
    let ix = set_compute_unit_price_ix(1000);
    assert_eq!(ix.program_id, COMPUTE_BUDGET_PROGRAM_ID);
    // Discriminant: 3u8
    assert_eq!(ix.data[0], 3);
    // micro_lamports: 1000u64 LE
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p quasar-cli bpf_loader`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement instruction builders**

Add to `bpf_loader.rs`:

```rust
use solana_instruction::{AccountMeta, Instruction};

// ---------------------------------------------------------------------------
// BPF Loader Upgradeable instructions
// ---------------------------------------------------------------------------

/// InitializeBuffer (discriminant 0). Accounts: [buffer (writable), authority (readonly)].
pub fn initialize_buffer_ix(buffer: &Address, authority: &Address) -> Instruction {
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts: vec![
            AccountMeta::new(*buffer, false),
            AccountMeta::new_readonly(*authority, false),
        ],
        data: 0u32.to_le_bytes().to_vec(),
    }
}

/// Write (discriminant 1). Accounts: [buffer (writable), authority (signer)].
pub fn write_ix(buffer: &Address, authority: &Address, offset: u32, data: &[u8]) -> Instruction {
    let mut ix_data = Vec::with_capacity(12 + data.len());
    ix_data.extend_from_slice(&1u32.to_le_bytes());
    ix_data.extend_from_slice(&offset.to_le_bytes());
    ix_data.extend_from_slice(&(data.len() as u32).to_le_bytes());
    ix_data.extend_from_slice(data);
    Instruction {
        program_id: BPF_LOADER_UPGRADEABLE_ID,
        accounts: vec![
            AccountMeta::new(*buffer, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data: ix_data,
    }
}

/// DeployWithMaxDataLen (discriminant 2).
/// Accounts: [payer, programdata, program, buffer, rent, clock, system, authority].
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
            AccountMeta::new(*programdata, true),
            AccountMeta::new(*program, true),
            AccountMeta::new(*buffer, true),
            AccountMeta::new_readonly(SYSVAR_RENT_ID, false),
            AccountMeta::new_readonly(SYSVAR_CLOCK_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

/// Upgrade (discriminant 3).
/// Accounts: [programdata, program, buffer, spill, rent, clock, authority].
pub fn upgrade_ix(
    programdata: &Address,
    program: &Address,
    buffer: &Address,
    spill: &Address,
    authority: &Address,
) -> Instruction {
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
        data: 3u32.to_le_bytes().to_vec(),
    }
}

/// SetAuthority (discriminant 4).
/// Accounts: [account (writable), current_authority (signer), new_authority (optional readonly)].
pub fn set_authority_ix(
    account: &Address,
    current_authority: &Address,
    new_authority: Option<&Address>,
) -> Instruction {
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
        data: 4u32.to_le_bytes().to_vec(),
    }
}

// ---------------------------------------------------------------------------
// ComputeBudget
// ---------------------------------------------------------------------------

/// SetComputeUnitPrice instruction. Discriminant: 3u8.
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

/// Calculate the number of chunks needed for a given file size.
pub fn num_chunks(file_size: usize) -> usize {
    if file_size == 0 {
        return 0;
    }
    (file_size + CHUNK_SIZE - 1) / CHUNK_SIZE
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p quasar-cli bpf_loader`
Expected: All new tests pass + existing constant tests pass.

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 5: Commit**

```bash
git add cli/src/bpf_loader.rs
git commit -m "feat: add BPF Loader instruction builders and ComputeBudget instruction"
```

---

### Task 5: Add Orchestrators to `bpf_loader.rs`

Add buffer upload, deploy, upgrade, authority validation, and SystemProgram::CreateAccount.

**Files:**
- Modify: `cli/src/bpf_loader.rs`

- [ ] **Step 1: Write tests for programdata authority parsing**

Add to `bpf_loader.rs::tests`:

```rust
#[test]
fn parse_programdata_authority_some() {
    let mut data = vec![0u8; 45];
    // discriminant = 3 (ProgramData)
    data[0..4].copy_from_slice(&3u32.to_le_bytes());
    // slot
    data[4..12].copy_from_slice(&100u64.to_le_bytes());
    // Option tag = 1 (Some)
    data[12] = 1;
    // authority pubkey
    data[13..45].copy_from_slice(&[0xAA; 32]);

    let authority = parse_programdata_authority(&data).unwrap();
    assert_eq!(authority, Some(Address::from([0xAA; 32])));
}

#[test]
fn parse_programdata_authority_none() {
    let mut data = vec![0u8; 45];
    data[0..4].copy_from_slice(&3u32.to_le_bytes());
    data[4..12].copy_from_slice(&100u64.to_le_bytes());
    data[12] = 0; // None — immutable

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
    // SystemProgram::CreateAccount discriminant = 0u32 LE
    assert_eq!(&ix.data[..4], &[0, 0, 0, 0]);
    // lamports
    assert_eq!(&ix.data[4..12], &1_000_000u64.to_le_bytes());
    // space
    assert_eq!(&ix.data[12..20], &100u64.to_le_bytes());
    // owner
    assert_eq!(&ix.data[20..52], owner.as_ref());
    assert_eq!(ix.data.len(), 52);
    assert!(ix.accounts[0].is_signer);
    assert!(ix.accounts[1].is_signer);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p quasar-cli parse_programdata`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement helper functions**

Add to `bpf_loader.rs`:

```rust
use crate::rpc::{
    self, confirm_transaction, get_latest_blockhash, get_minimum_balance_for_rent_exemption,
    send_transaction, Keypair,
};

/// Parse the authority from a programdata account.
/// Returns Some(address) if authority is set, None if immutable.
pub fn parse_programdata_authority(
    data: &[u8],
) -> Result<Option<Address>, crate::error::CliError> {
    if data.len() < 45 {
        return Err(anyhow::anyhow!("programdata account too short").into());
    }
    if data[12] == 0 {
        Ok(None) // immutable
    } else {
        let bytes: [u8; 32] = data[13..45].try_into().unwrap();
        Ok(Some(Address::from(bytes)))
    }
}

/// SystemProgram::CreateAccount instruction.
pub fn create_account_ix(
    payer: &Address,
    new_account: &Address,
    lamports: u64,
    space: u64,
    owner: &Address,
) -> Instruction {
    let mut data = Vec::with_capacity(52);
    data.extend_from_slice(&0u32.to_le_bytes()); // CreateAccount discriminant
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
```

- [ ] **Step 4: Run tests to verify helpers pass**

Run: `cargo test -p quasar-cli parse_programdata create_account_ix`
Expected: All 3 pass.

- [ ] **Step 5: Implement `verify_upgrade_authority`**

```rust
/// Verify the on-chain upgrade authority matches the expected authority.
/// Errors if the program is immutable or the authority doesn't match.
pub fn verify_upgrade_authority(
    rpc_url: &str,
    program_id: &Address,
    expected_authority: &Address,
) -> Result<(), crate::error::CliError> {
    let (programdata_addr, _) = programdata_pda(program_id);
    let data = rpc::get_account_data(rpc_url, &programdata_addr)?
        .ok_or_else(|| anyhow::anyhow!("programdata account not found"))?;

    match parse_programdata_authority(&data)? {
        None => Err(anyhow::anyhow!("program is immutable (no upgrade authority)").into()),
        Some(authority) if authority != *expected_authority => {
            Err(anyhow::anyhow!(
                "upgrade authority mismatch: on-chain is {}, your keypair is {}",
                bs58::encode(authority).into_string(),
                bs58::encode(expected_authority).into_string(),
            ).into())
        }
        Some(_) => Ok(()),
    }
}
```

- [ ] **Step 6: Implement `write_buffer`**

```rust
/// Upload a .so binary to a new buffer account. Returns the buffer address.
pub fn write_buffer(
    so_path: &std::path::Path,
    payer: &Keypair,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<Address, crate::error::CliError> {
    let program_data = std::fs::read(so_path)?;
    let buffer_size = program_data.len() + BUFFER_HEADER_SIZE;

    // Generate random buffer keypair
    let buffer_keypair = Keypair::generate();
    let buffer_addr = buffer_keypair.address();

    // Get rent exemption
    let lamports = get_minimum_balance_for_rent_exemption(rpc_url, buffer_size)?;

    // 1. Create buffer account + initialize
    let mut ixs = vec![];
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(create_account_ix(
        &payer.address(),
        &buffer_addr,
        lamports,
        buffer_size as u64,
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
    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;

    if !confirm_transaction(rpc_url, &sig, 30)? {
        return Err(anyhow::anyhow!(
            "buffer creation not confirmed within 30s (buffer: {})",
            bs58::encode(&buffer_addr).into_string()
        ).into());
    }

    // 2. Write chunks sequentially
    let total_chunks = num_chunks(program_data.len());
    let pb = indicatif::ProgressBar::new(program_data.len() as u64);
    pb.set_style(
        indicatif::ProgressStyle::with_template(
            "  {bar:40.cyan/dim} {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("█▓░"),
    );

    for (i, chunk) in program_data.chunks(CHUNK_SIZE).enumerate() {
        let offset = (i * CHUNK_SIZE) as u32;
        let mut ixs = vec![];
        if priority_fee > 0 {
            ixs.push(set_compute_unit_price_ix(priority_fee));
        }
        ixs.push(write_ix(&buffer_addr, &payer.address(), offset, chunk));

        let blockhash = get_latest_blockhash(rpc_url)?;
        let tx = solana_transaction::Transaction::new_signed_with_payer(
            &ixs,
            Some(&payer.address()),
            &[payer],
            blockhash,
        );
        let tx_bytes = bincode::serialize(&tx)
            .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
        let sig = send_transaction(rpc_url, &tx_bytes)?;

        if !confirm_transaction(rpc_url, &sig, 30)? {
            return Err(anyhow::anyhow!(
                "chunk {}/{} not confirmed within 30s (buffer: {})",
                i + 1,
                total_chunks,
                bs58::encode(&buffer_addr).into_string()
            ).into());
        }

        pb.set_position((offset as u64) + chunk.len() as u64);
    }

    pb.finish_and_clear();
    Ok(buffer_addr)
}
```

- [ ] **Step 7: Implement `deploy_program`**

```rust
/// Deploy a new program. Creates the program account and calls DeployWithMaxDataLen.
/// `program_keypair` is a Keypair because DeployWithMaxDataLen requires the
/// program account to sign its own CreateAccount.
pub fn deploy_program(
    so_path: &std::path::Path,
    program_keypair: &Keypair,
    payer: &Keypair,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<Address, crate::error::CliError> {
    let program_addr = program_keypair.address();

    // 1. Upload buffer
    let buffer = write_buffer(so_path, payer, rpc_url, priority_fee)?;

    // 2. Derive programdata PDA
    let (programdata, _) = programdata_pda(&program_addr);
    let so_len = std::fs::metadata(so_path)?.len() as usize;
    let max_data_len = so_len * 2; // double size for future upgrades

    // Program account is 36 bytes: 4-byte discriminant + 32-byte programdata address
    let program_account_size: u64 = 36;
    let program_lamports =
        get_minimum_balance_for_rent_exemption(rpc_url, program_account_size as usize)?;

    // 3. Create program account + deploy in one transaction
    let mut ixs = vec![];
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(create_account_ix(
        &payer.address(),
        &program_addr,
        program_lamports,
        program_account_size,
        &BPF_LOADER_UPGRADEABLE_ID,
    ));
    ixs.push(deploy_with_max_data_len_ix(
        &payer.address(),
        &programdata,
        &program_addr,
        &buffer,
        &payer.address(),
        max_data_len as u64,
    ));

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
        Some(&payer.address()),
        &[payer, program_keypair],
        blockhash,
    );
    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;

    if !confirm_transaction(rpc_url, &sig, 30)? {
        return Err(anyhow::anyhow!("deploy transaction not confirmed within 30s").into());
    }

    Ok(program_addr)
}
```

- [ ] **Step 8: Implement `upgrade_program`**

```rust
/// Upgrade an existing program. Uploads buffer then calls Upgrade.
pub fn upgrade_program(
    so_path: &std::path::Path,
    program_id: &Address,
    authority: &Keypair,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<(), crate::error::CliError> {
    // 1. Upload buffer
    let buffer = write_buffer(so_path, authority, rpc_url, priority_fee)?;

    // 2. Upgrade
    let (programdata, _) = programdata_pda(program_id);
    let authority_addr = authority.address();

    let mut ixs = vec![];
    if priority_fee > 0 {
        ixs.push(set_compute_unit_price_ix(priority_fee));
    }
    ixs.push(upgrade_ix(
        &programdata,
        program_id,
        &buffer,
        &authority_addr, // spill = authority (reclaim buffer rent)
        &authority_addr,
    ));

    let blockhash = get_latest_blockhash(rpc_url)?;
    let tx = solana_transaction::Transaction::new_signed_with_payer(
        &ixs,
        Some(&authority_addr),
        &[authority],
        blockhash,
    );
    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;

    if !confirm_transaction(rpc_url, &sig, 30)? {
        return Err(anyhow::anyhow!("upgrade transaction not confirmed within 30s").into());
    }

    Ok(())
}
```

- [ ] **Step 9: Implement `set_authority` convenience wrapper**

```rust
/// Transfer authority of a buffer or program to a new address (or revoke it).
pub fn set_authority(
    account: &Address,
    current_authority: &Keypair,
    new_authority: Option<&Address>,
    rpc_url: &str,
    priority_fee: u64,
) -> Result<(), crate::error::CliError> {
    let mut ixs = vec![];
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
    let tx_bytes = bincode::serialize(&tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;
    let sig = send_transaction(rpc_url, &tx_bytes)?;

    if !confirm_transaction(rpc_url, &sig, 30)? {
        return Err(anyhow::anyhow!("set-authority transaction not confirmed within 30s").into());
    }

    Ok(())
}
```

- [ ] **Step 10: Run tests**

Run: `cargo test -p quasar-cli`
Expected: All tests pass.

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 11: Commit**

```bash
git add cli/src/bpf_loader.rs
git commit -m "feat: add native buffer upload, deploy, upgrade, and authority orchestrators"
```

---

### Task 6: Update `multisig.rs` — Replace Shell-outs + Priority Fee

Replace the three shell-out functions with native `bpf_loader` calls. Add `priority_fee` parameter to `propose_upgrade` and `execute_approved_proposal`.

**Files:**
- Modify: `cli/src/multisig.rs`
- Modify: `cli/src/deploy.rs` (update `set_upgrade_authority` call site to keep code compiling)

- [ ] **Step 1: Remove shell-out functions**

Delete these three functions from `multisig.rs`:
- `write_buffer()` (lines 694-737) — the shell-out that calls `solana program write-buffer`
- `set_buffer_authority()` (lines 741-770) — the shell-out that calls `solana program set-buffer-authority`
- `set_upgrade_authority()` (lines 774-805) — the shell-out that calls `solana program set-upgrade-authority`

**IMPORTANT:** Since `deploy.rs` currently calls `crate::multisig::set_upgrade_authority(...)` on line 268, you must also update that call site in this task to keep the code compiling. Change it to:

```rust
let authority_keypair = crate::rpc::Keypair::read_from_file(&payer_path)?;
crate::bpf_loader::set_authority(
    &crate::bpf_loader::programdata_pda(&program_id).0,
    &authority_keypair,
    Some(&vault),
    &rpc_url,
    0, // priority fee not yet wired through; Task 7 adds it
)?;
```

This is a temporary bridge — Task 7 will rewrite this entire section of deploy.rs.

- [ ] **Step 2: Remove `std::process` imports**

Remove `Command` and `Stdio` from the `use` block since there are no more shell-outs:

```rust
// Remove these from the use block:
//     std::process::{Command, Stdio},
```

The `use` block should now look like:

```rust
use {
    crate::{
        bpf_loader::{
            self, BPF_LOADER_UPGRADEABLE_ID, SYSTEM_PROGRAM_ID, SYSVAR_CLOCK_ID,
            SYSVAR_RENT_ID, programdata_pda,
        },
        rpc::{self, get_account_data, get_latest_blockhash, send_transaction, Keypair},
        style,
    },
    sha2::{Digest, Sha256},
    solana_address::Address,
    solana_instruction::AccountMeta,
    std::path::Path,
};
```

- [ ] **Step 3: Update `propose_upgrade` signature and body**

Change the function signature to accept `priority_fee`:

```rust
pub fn propose_upgrade(
    so_path: &Path,
    program_id: &Address,
    multisig: &Address,
    keypair_path: &Path,
    rpc_url: &str,
    vault_index: u8,
    priority_fee: u64,
) -> crate::error::CliResult {
```

Replace the buffer upload (step 1) — was:
```rust
let buffer = write_buffer(so_path, keypair_path, rpc_url)?;
```
becomes:
```rust
let buffer = bpf_loader::write_buffer(so_path, &keypair, rpc_url, priority_fee)?;
```

Note: `keypair` is already loaded 2 lines above. Remove the `keypair_path` usage for write_buffer.

Replace the buffer authority transfer (step 2) — was:
```rust
set_buffer_authority(&buffer, &vault, keypair_path, rpc_url)?;
```
becomes:
```rust
bpf_loader::set_authority(&buffer, &keypair, Some(&vault), rpc_url, priority_fee)?;
```

In step 7 (build transaction), prepend priority fee instruction if non-zero:

```rust
let mut ixs = vec![];
if priority_fee > 0 {
    ixs.push(bpf_loader::set_compute_unit_price_ix(priority_fee));
}
ixs.push(ix_create);
ixs.push(ix_propose);
ixs.push(ix_approve);

let tx = solana_transaction::Transaction::new_signed_with_payer(
    &ixs,
    Some(&member),
    &[&keypair],
    blockhash,
);
```

- [ ] **Step 4: Update `execute_approved_proposal` to accept priority fee**

Change signature:

```rust
fn execute_approved_proposal(
    multisig: &Address,
    ms: &MultisigState,
    proposal: &ProposalState,
    keypair_path: &Path,
    rpc_url: &str,
    priority_fee: u64,
) -> crate::error::CliResult {
```

Prepend priority fee instruction:

```rust
let mut ixs = vec![];
if priority_fee > 0 {
    ixs.push(bpf_loader::set_compute_unit_price_ix(priority_fee));
}
ixs.push(ix);

let tx = solana_transaction::Transaction::new_signed_with_payer(
    &ixs,
    Some(&member),
    &[&keypair],
    blockhash,
);
```

- [ ] **Step 5: Update `show_proposal_status` to accept and pass priority fee**

Change signature:

```rust
pub fn show_proposal_status(
    multisig: &Address,
    keypair_path: &Path,
    rpc_url: &str,
    priority_fee: u64,
) -> crate::error::CliResult {
```

Update the call to `execute_approved_proposal`:

```rust
execute_approved_proposal(
    multisig,
    &ms,
    &proposal,
    keypair_path,
    rpc_url,
    priority_fee,
)?;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p quasar-cli`
Expected: All tests pass. (Note: `multisig.rs` tests don't call the removed shell-out functions — they test parsing/building which is unchanged.)

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 7: Commit**

```bash
git add cli/src/multisig.rs cli/src/deploy.rs
git commit -m "refactor: replace multisig shell-outs with native bpf_loader calls, add priority fee"
```

---

### Task 7: Update `deploy.rs` + `lib.rs` — Native Calls + Validation + Priority Fee

Replace `solana_deploy()` shell-out with native calls. Add the `--priority-fee` flag, reverse `--upgrade` check, and authority validation.

**Files:**
- Modify: `cli/src/lib.rs` (add priority_fee field)
- Modify: `cli/src/deploy.rs` (replace shell-outs, add validation)

- [ ] **Step 1: Add `priority_fee` to `DeployCommand` in `lib.rs`**

In `cli/src/lib.rs`, add a new field to `DeployCommand` after the `status` field (line 187):

```rust
    /// Priority fee in micro-lamports (auto-calculated if omitted)
    #[arg(long, value_name = "MICRO_LAMPORTS")]
    pub priority_fee: Option<u64>,
```

Update the `deploy::run(deploy::DeployOpts { ... })` call (around line 337) to include:

```rust
Command::Deploy(cmd) => deploy::run(deploy::DeployOpts {
    program_keypair: cmd.program_keypair,
    upgrade_authority: cmd.upgrade_authority,
    keypair: cmd.keypair,
    url: cmd.url,
    skip_build: cmd.skip_build,
    multisig: cmd.multisig,
    status: cmd.status,
    upgrade: cmd.upgrade,
    priority_fee: cmd.priority_fee,
}),
```

- [ ] **Step 2: Add `priority_fee` to `DeployOpts` in `deploy.rs`**

In `cli/src/deploy.rs`, add to the `DeployOpts` struct:

```rust
pub struct DeployOpts {
    pub program_keypair: Option<PathBuf>,
    pub upgrade_authority: Option<PathBuf>,
    pub keypair: Option<PathBuf>,
    pub url: Option<String>,
    pub skip_build: bool,
    pub multisig: Option<String>,
    pub status: bool,
    pub upgrade: bool,
    pub priority_fee: Option<u64>,
}
```

- [ ] **Step 3: Rewrite `deploy.rs` — remove shell-out, add native calls**

Replace the entire `use` block at the top:

```rust
use {
    crate::{config::QuasarConfig, error::CliResult, style, utils},
    std::path::PathBuf,
};
```

(Remove `std::process::{Command, Stdio}` — no more shell-outs. Note: `bs58` is used via fully qualified `bs58::encode(...)` calls throughout, which doesn't require a `use` import.)

Delete the `solana_deploy()` function entirely (lines 60-147).

Rewrite the `run()` function body. Here's the complete new version:

```rust
pub fn run(opts: DeployOpts) -> CliResult {
    let DeployOpts {
        program_keypair,
        upgrade_authority,
        keypair,
        url,
        skip_build,
        multisig,
        status,
        upgrade,
        priority_fee,
    } = opts;
    let config = QuasarConfig::load()?;
    let name = &config.project.name;

    // Resolve cluster URL once
    let rpc_url = crate::rpc::solana_rpc_url(url.as_deref());

    // Resolve priority fee: use override or auto-calculate
    let fee = match priority_fee {
        Some(f) => f,
        None => {
            let auto = crate::rpc::get_recent_prioritization_fees(&rpc_url).unwrap_or(0);
            if auto > 0 {
                println!(
                    "  {} Auto priority fee: {} micro-lamports",
                    style::dim("ℹ"),
                    auto
                );
            }
            auto
        }
    };

    // --upgrade --multisig: Squads proposal flow
    if upgrade {
        if let Some(multisig_addr) = &multisig {
            let multisig_key = parse_multisig_address(multisig_addr)?;
            let payer_path = crate::rpc::solana_keypair_path(keypair.as_deref());

            if status {
                return crate::multisig::show_proposal_status(
                    &multisig_key,
                    &payer_path,
                    &rpc_url,
                    fee,
                );
            }

            let so_path = build_and_find_so(&config, name, skip_build)?;
            let prog_keypair_path = resolve_program_keypair(&config, program_keypair);
            let program_id =
                crate::rpc::read_program_id_from_keypair(&prog_keypair_path)?;

            return crate::multisig::propose_upgrade(
                &so_path,
                &program_id,
                &multisig_key,
                &payer_path,
                &rpc_url,
                0,
                fee,
            );
        }
    }

    // Everything below needs a build and a .so
    let so_path = build_and_find_so(&config, name, skip_build)?;
    let keypair_path = resolve_program_keypair(&config, program_keypair);

    if !keypair_path.exists() {
        eprintln!(
            "\n  {}",
            style::fail(&format!(
                "program keypair not found: {}",
                keypair_path.display()
            ))
        );
        eprintln!();
        eprintln!(
            "  Run {} to generate one, or pass {} explicitly.",
            style::bold("quasar keys new"),
            style::bold("--program-keypair")
        );
        eprintln!();
        std::process::exit(1);
    }

    // Read program ID from the keypair for on-chain check
    let program_id = crate::rpc::read_program_id_from_keypair(&keypair_path)?;
    let exists = crate::rpc::program_exists_on_chain(&rpc_url, &program_id)?;

    // Forward check: deploy on existing program
    if !upgrade && exists {
        eprintln!(
            "\n  {}",
            style::fail(&format!(
                "program already deployed at {}",
                bs58::encode(program_id).into_string()
            ))
        );
        eprintln!();
        eprintln!(
            "  Use {} to upgrade an existing program.",
            style::bold("quasar deploy --upgrade")
        );
        eprintln!();
        std::process::exit(1);
    }

    // Reverse check: --upgrade on non-existent program
    if upgrade && !exists {
        eprintln!(
            "\n  {}",
            style::fail(&format!(
                "program not found at {}",
                bs58::encode(program_id).into_string()
            ))
        );
        eprintln!();
        eprintln!(
            "  Drop {} for a fresh deploy.",
            style::bold("--upgrade")
        );
        eprintln!();
        std::process::exit(1);
    }

    // Load the payer keypair
    let payer_path = crate::rpc::solana_keypair_path(keypair.as_deref());
    let payer = crate::rpc::Keypair::read_from_file(&payer_path)?;

    if upgrade {
        // Authority validation before buffer upload
        let authority_keypair = if let Some(ref auth_path) = upgrade_authority {
            crate::rpc::Keypair::read_from_file(auth_path)?
        } else {
            crate::rpc::Keypair::read_from_file(&payer_path)?
        };

        let sp = style::spinner("Verifying upgrade authority...");
        crate::bpf_loader::verify_upgrade_authority(
            &rpc_url,
            &program_id,
            &authority_keypair.address(),
        )?;
        sp.finish_and_clear();

        // Upgrade
        let sp = style::spinner("Uploading and upgrading...");
        crate::bpf_loader::upgrade_program(
            &so_path,
            &program_id,
            &authority_keypair,
            &rpc_url,
            fee,
        )?;
        sp.finish_and_clear();

        println!(
            "\n  {}",
            style::success(&format!(
                "Upgraded {}",
                style::bold(&bs58::encode(program_id).into_string())
            ))
        );
    } else {
        // Fresh deploy
        let program_kp = crate::rpc::Keypair::read_from_file(&keypair_path)?;

        let sp = style::spinner("Deploying...");
        let addr = crate::bpf_loader::deploy_program(
            &so_path,
            &program_kp,
            &payer,
            &rpc_url,
            fee,
        )?;
        sp.finish_and_clear();

        println!(
            "\n  {}",
            style::success(&format!(
                "Deployed to {}",
                style::bold(&bs58::encode(addr).into_string())
            ))
        );
    }

    // --multisig without --upgrade: transfer authority to vault after deploy
    if let Some(multisig_addr) = &multisig {
        let multisig_key = parse_multisig_address(multisig_addr)?;
        let (vault, _) = crate::multisig::vault_pda(&multisig_key, 0);

        let authority_keypair = if let Some(ref auth_path) = upgrade_authority {
            crate::rpc::Keypair::read_from_file(auth_path)?
        } else {
            crate::rpc::Keypair::read_from_file(&payer_path)?
        };

        let sp = style::spinner("Transferring upgrade authority to multisig vault...");
        crate::bpf_loader::set_authority(
            &crate::bpf_loader::programdata_pda(&program_id).0,
            &authority_keypair,
            Some(&vault),
            &rpc_url,
            fee,
        )?;
        sp.finish_and_clear();

        println!(
            "  {}",
            style::success(&format!(
                "Upgrade authority transferred to vault {}",
                style::bold(&crate::multisig::short_addr(&vault))
            ))
        );
        println!();
        println!(
            "  Future upgrades: {}",
            style::dim(&format!(
                "quasar deploy --upgrade --multisig {multisig_addr}"
            ))
        );
    }

    println!();
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p quasar-cli`
Expected: All tests pass.

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 5: Update help text in `print_help()`**

In `lib.rs`, update the deploy help line to mention priority fee:

```rust
print_cmd(
    "deploy  [-u url] [-k keypair] [--upgrade] [--multisig addr] [--priority-fee n]",
    "Deploy or upgrade a program",
);
```

- [ ] **Step 6: Run full test suite one final time**

Run: `cargo test -p quasar-cli`
Expected: All tests pass.

Run: `cargo clippy -p quasar-cli -- -D warnings`
Expected: Clean.

- [ ] **Step 7: Commit**

```bash
git add cli/src/deploy.rs cli/src/lib.rs
git commit -m "feat: native deploy/upgrade with priority fees and pre-deploy validation"
```
