use {
    crate::{
        config::QuasarConfig,
        error::{CliError, CliResult},
        program_keypair::{self, CompanionUpdate, ProgramKeypair},
        style,
    },
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

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

/// Find the current `declare_id!("...")` value in src/lib.rs.
fn program_id_in_source(source: &str) -> Option<String> {
    // Simple string extraction: find declare_id!("...") pattern
    let marker = "declare_id!(\"";
    let start = source.find(marker)? + marker.len();
    let end = source[start..].find('\"')? + start;
    Some(source[start..end].to_owned())
}

/// Replace the address inside `declare_id!("...")` in src/lib.rs.
fn updated_program_source(source: &str, old_id: &str, new_id: &str) -> String {
    source.replace(
        &format!("declare_id!(\"{old_id}\")"),
        &format!("declare_id!(\"{new_id}\")"),
    )
}

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

    let id = ProgramKeypair::read(&path)?.program_id();
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

    let keypair_id = ProgramKeypair::read(&path)?.program_id();
    let source_path = Path::new("src/lib.rs");
    let source = fs::read_to_string(source_path)
        .map_err(|error| CliError::io_path("read", source_path, error))?;
    let current_id = match program_id_in_source(&source) {
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

    let updated = updated_program_source(&source, &current_id, &keypair_id);
    program_keypair::replace_regular_file(source_path, source.as_bytes(), updated.as_bytes())?;

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
    program_keypair::validate_write_target(&path, force)?;

    let keypair = ProgramKeypair::generate();
    let id = keypair.program_id();

    let source_path = Path::new("src/lib.rs");
    let source_update = if source_path.exists() {
        let source = fs::read_to_string(source_path)
            .map_err(|error| CliError::io_path("read", source_path, error))?;
        program_id_in_source(&source).and_then(|current_id| {
            (current_id != id).then(|| {
                let updated = updated_program_source(&source, &current_id, &id);
                (source, updated)
            })
        })
    } else {
        None
    };
    let companion = source_update
        .as_ref()
        .map(|(source, updated)| CompanionUpdate {
            path: source_path,
            expected: source.as_bytes(),
            replacement: updated.as_bytes(),
        });

    program_keypair::write(&path, &keypair, force, companion)?;

    println!(
        "  {} {}",
        style::success("Generated keypair:"),
        style::bold(&id)
    );

    if source_update.is_some() {
        println!("  {} declare_id!() updated", style::success("Synced:"),);
    }

    Ok(())
}
