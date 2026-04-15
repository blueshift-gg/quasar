use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        style,
    },
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Locate the program keypair in target/deploy/.
fn keypair_path(config: &QuasarConfig) -> PathBuf {
    let name = &config.project.name;
    let module = config.module_name();

    let default = PathBuf::from("target/deploy").join(format!("{name}-keypair.json"));
    if default.exists() {
        return default;
    }
    let alt = PathBuf::from("target/deploy").join(format!("{module}-keypair.json"));
    if alt.exists() {
        return alt;
    }
    default
}

/// Read the public key (program ID) from a Solana CLI-compatible keypair file.
/// The file contains a 64-byte JSON array: [secret(32) | public(32)].
fn read_program_id(path: &Path) -> Result<String, crate::error::CliError> {
    let json = fs::read_to_string(path)?;
    let bytes: Vec<u8> = serde_json::from_str(&json)?;
    Ok(bs58::encode(&bytes[32..64]).into_string())
}

/// Find the current `declare_id!("...")` value in src/lib.rs using the IDL
/// parser.
fn current_program_id() -> Option<String> {
    let source = fs::read_to_string("src/lib.rs").ok()?;
    let file = syn::parse_file(&source).ok()?;
    quasar_idl::parser::program::extract_program_id(&file)
}

/// Replace the address inside `declare_id!("...")` in src/lib.rs.
fn replace_program_id(old_id: &str, new_id: &str) -> Result<(), crate::error::CliError> {
    let source = fs::read_to_string("src/lib.rs")?;
    let updated = source.replace(
        &format!("declare_id!(\"{old_id}\")"),
        &format!("declare_id!(\"{new_id}\")"),
    );
    fs::write("src/lib.rs", updated)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Print the program ID from the keypair file.
pub fn list() -> CliResult {
    let config = QuasarConfig::load()?;
    let path = keypair_path(&config);

    if !path.exists() {
        return Err(CliError::message(format!(
            "keypair not found: {}\n  Run quasar keys new first.",
            path.display()
        )));
    }

    let id = read_program_id(&path)?;
    println!("  {}", style::bold(&id));
    Ok(())
}

/// Update declare_id!() in src/lib.rs to match the keypair file.
pub fn sync() -> CliResult {
    let config = QuasarConfig::load()?;
    let path = keypair_path(&config);

    if !path.exists() {
        return Err(CliError::message(format!(
            "keypair not found: {}\n  Run quasar keys new first.",
            path.display()
        )));
    }

    let keypair_id = read_program_id(&path)?;

    let current_id = match current_program_id() {
        Some(id) => id,
        None => return Err(CliError::message("declare_id!() not found in src/lib.rs")),
    };

    if current_id == keypair_id {
        println!(
            "  {} {}",
            style::success("Already in sync:"),
            style::bold(&keypair_id)
        );
        return Ok(());
    }

    replace_program_id(&current_id, &keypair_id)?;

    println!(
        "  {} {}",
        style::success("Synced program ID:"),
        style::bold(&keypair_id)
    );
    Ok(())
}

/// Generate a new keypair and update declare_id!().
pub fn new(force: bool) -> CliResult {
    let config = QuasarConfig::load()?;
    let path = keypair_path(&config);

    if path.exists() && !force {
        return Err(CliError::message(format!(
            "keypair already exists: {}\n\n  Use quasar keys new --force to overwrite it.\n  \
             Warning: this will change your program address.",
            path.display()
        )));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let signing_key = ed25519_dalek::SigningKey::generate(&mut rand::thread_rng());
    let mut keypair_bytes = Vec::with_capacity(64);
    keypair_bytes.extend_from_slice(signing_key.as_bytes());
    keypair_bytes.extend_from_slice(signing_key.verifying_key().as_bytes());
    let keypair_json = serde_json::to_string(&keypair_bytes)?;

    fs::write(&path, &keypair_json)?;

    let id = bs58::encode(signing_key.verifying_key().as_bytes()).into_string();
    println!(
        "  {} {}",
        style::success("Generated keypair:"),
        style::bold(&id)
    );

    // Auto-sync declare_id!() if src/lib.rs exists
    if std::path::Path::new("src/lib.rs").exists() {
        if let Some(current_id) = current_program_id() {
            if current_id != id {
                replace_program_id(&current_id, &id)?;
                println!("  {} declare_id!() updated", style::success("Synced:"),);
            }
        }
    }

    Ok(())
}
