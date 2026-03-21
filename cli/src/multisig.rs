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
            Some(line[prefix.len()..].trim().trim_matches('\'').trim_matches('"').to_string())
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// Keypair
// ---------------------------------------------------------------------------

/// Thin wrapper around ed25519-dalek SigningKey that implements solana Signer.
pub struct Keypair(pub SigningKey);

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
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestBlockhash",
            "params": [{"commitment": "confirmed"}]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

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
pub fn send_transaction(
    rpc_url: &str,
    tx_bytes: &[u8],
) -> Result<String, crate::error::CliError> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    let encoded = STANDARD.encode(tx_bytes);

    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(&serde_json::json!({
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
pub fn get_account_data(rpc_url: &str, address: &Address) -> Result<Option<Vec<u8>>, crate::error::CliError> {
    let resp: serde_json::Value = ureq::post(rpc_url)
        .send_json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [bs58::encode(address).into_string(), {"encoding": "base64", "commitment": "confirmed"}]
        }))
        .map_err(anyhow::Error::from)?
        .body_mut()
        .read_json()
        .map_err(anyhow::Error::from)?;

    let value = &resp["result"]["value"];
    if value.is_null() {
        return Ok(None);
    }

    let data_str = value["data"][0]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing account data"))?;

    use base64::{Engine, engine::general_purpose::STANDARD};
    Ok(Some(STANDARD.decode(data_str).map_err(anyhow::Error::from)?))
}

// ---------------------------------------------------------------------------
// Squads v4 PDAs
// ---------------------------------------------------------------------------

/// Squads v4 program ID — SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf.
/// Verify with: `bs58::decode("SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf").into_vec()`
/// These bytes MUST be verified at implementation time via the test in Task 8.
const SQUADS_PROGRAM_ID: Address = Address::new_from_array([
    0x06, 0x81, 0xc4, 0xce, 0x47, 0xe2, 0x23, 0x68,
    0xb8, 0xb1, 0x55, 0x5e, 0xc8, 0x87, 0xaf, 0x09,
    0x2e, 0xfc, 0x7e, 0xfb, 0xb6, 0x6c, 0xa3, 0xf5,
    0x2f, 0xbf, 0x68, 0xd4, 0xac, 0x9c, 0xb7, 0xa8,
]);

/// BPF Loader Upgradeable — BPFLoaderUpgradeab1e11111111111111111111111.
/// Verify with: `bs58::decode("BPFLoaderUpgradeab1e11111111111111111111111").into_vec()`
const BPF_LOADER_UPGRADEABLE_ID: Address = Address::new_from_array([
    0x02, 0xa8, 0xf6, 0x91, 0x4e, 0x88, 0xa1, 0xb0,
    0xe2, 0x10, 0x15, 0x3e, 0xf7, 0x63, 0xae, 0x2b,
    0x00, 0xc2, 0xb9, 0x3d, 0x16, 0xc1, 0x24, 0xd2,
    0xc0, 0x53, 0x7a, 0x10, 0x04, 0x80, 0x00, 0x00,
]);

/// System program ID.
const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Sysvar Rent — SysvarRent111111111111111111111111111111111.
/// Matches `lang/src/sysvars/rent.rs` RENT_ID.
const SYSVAR_RENT_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76,
    61, 74, 241, 127, 88, 218, 238, 8, 155, 161, 253, 68,
    227, 219, 217, 138, 0, 0, 0, 0,
]);

/// Sysvar Clock — SysvarC1ock11111111111111111111111111111111.
/// Matches `lang/src/sysvars/clock.rs` CLOCK_ID.
const SYSVAR_CLOCK_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152,
    105, 29, 94, 182, 139, 94, 184, 163, 155, 75, 109, 92,
    115, 85, 91, 33, 0, 0, 0, 0,
]);

pub fn vault_pda(multisig: &Address, vault_index: u8) -> (Address, u8) {
    Address::find_program_address(
        &[b"multisig", multisig.as_ref(), b"vault", &[vault_index]],
        &SQUADS_PROGRAM_ID,
    )
}

pub fn transaction_pda(multisig: &Address, transaction_index: u64) -> (Address, u8) {
    Address::find_program_address(
        &[b"multisig", multisig.as_ref(), b"transaction", &transaction_index.to_le_bytes()],
        &SQUADS_PROGRAM_ID,
    )
}

pub fn proposal_pda(multisig: &Address, transaction_index: u64) -> (Address, u8) {
    Address::find_program_address(
        &[
            b"multisig", multisig.as_ref(), b"transaction",
            &transaction_index.to_le_bytes(), b"proposal",
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
        return Err(anyhow::anyhow!("multisig account data too short ({} bytes)", account_data.len()).into());
    }
    let bytes: [u8; 8] = account_data[78..86].try_into().unwrap();
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_pda_derivation() {
        let multisig = Address::from([1u8; 32]);
        let (vault, bump) = vault_pda(&multisig, 0);
        assert_ne!(vault, Address::default());
        assert!(bump <= 255);
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
}
