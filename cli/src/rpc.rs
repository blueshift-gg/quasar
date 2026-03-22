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
///
/// Accepts `mainnet-beta`, `devnet`, `testnet`, `localnet`, or a full URL.
/// Falls back to the Solana CLI config if no override is provided.
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

    /// Generate a random keypair using the OS random number generator.
    pub fn generate() -> Self {
        let mut rng = rand::rngs::OsRng;
        Self(SigningKey::generate(&mut rng))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn cluster_name_resolution() {
        assert_eq!(
            resolve_cluster("mainnet-beta"),
            "https://api.mainnet-beta.solana.com"
        );
        assert_eq!(
            resolve_cluster("devnet"),
            "https://api.devnet.solana.com"
        );
        assert_eq!(
            resolve_cluster("testnet"),
            "https://api.testnet.solana.com"
        );
        assert_eq!(resolve_cluster("localnet"), "http://localhost:8899");
        assert_eq!(
            resolve_cluster("https://my-rpc.example.com"),
            "https://my-rpc.example.com"
        );
    }
}
