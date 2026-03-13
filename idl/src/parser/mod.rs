//! Source-level parser for Quasar programs.
//!
//! Reads Rust source files via `syn` and extracts program structure:
//! instructions, account types, events, errors, and state. Also performs
//! discriminator collision detection across all parsed types.

pub mod accounts;
pub mod errors;
pub mod events;
pub mod helpers;
pub mod module_resolver;
pub mod program;
pub mod state;

use {
    crate::types::*,
    std::{collections::BTreeMap, path::Path},
};

/// All data extracted from parsing a quasar program crate.
pub struct ParsedProgram {
    pub program_id: String,
    /// The Rust module name from `#[program] mod <name>` (uses underscores).
    pub program_name: String,
    /// The package name from `Cargo.toml` (uses dashes).
    pub crate_name: String,
    pub version: String,
    pub instructions: Vec<program::RawInstruction>,
    pub accounts_structs: Vec<accounts::RawAccountsStruct>,
    pub state_accounts: Vec<state::RawStateAccount>,
    pub events: Vec<events::RawEvent>,
    pub errors: Vec<IdlError>,
}

/// Parse an entire quasar program crate and produce a `ParsedProgram`.
pub fn parse_program(crate_root: &Path) -> ParsedProgram {
    // 1. Resolve all source files
    let files = module_resolver::resolve_crate(crate_root);

    // 2. Find lib.rs (first resolved file that has declare_id! or #[program])
    let lib_file = files
        .iter()
        .find(|f| f.path.ends_with("lib.rs"))
        .expect("could not find lib.rs");

    // 3. Extract program ID
    let program_id =
        program::extract_program_id(&lib_file.file).expect("could not find declare_id! in lib.rs");

    // 4. Extract program module and instructions
    let (program_name, instructions) = program::extract_program_module(&lib_file.file)
        .expect("could not find #[program] module in lib.rs");

    // 5. Collect all #[derive(Accounts)] structs across all files
    let mut accounts_structs = Vec::new();
    for file in &files {
        accounts_structs.extend(accounts::extract_accounts_structs(&file.file));
    }

    // 6. Collect all #[account(discriminator = N)] state structs
    let mut state_accounts = Vec::new();
    for file in &files {
        state_accounts.extend(state::extract_state_accounts(&file.file));
    }

    // 7. Collect all #[event(discriminator = N)] structs
    let mut all_events = Vec::new();
    for file in &files {
        all_events.extend(events::extract_events(&file.file));
    }

    // 8. Collect all #[error_code] enums
    let mut all_errors = Vec::new();
    for file in &files {
        all_errors.extend(errors::extract_errors(&file.file));
    }

    // 9. Read version and crate name from Cargo.toml
    let version = read_cargo_version(crate_root).unwrap_or_else(|| "0.1.0".to_string());
    let crate_name = read_cargo_name(crate_root).unwrap_or_else(|| program_name.replace('_', "-"));

    ParsedProgram {
        program_id,
        program_name,
        crate_name,
        version,
        instructions,
        accounts_structs,
        state_accounts,
        events: all_events,
        errors: all_errors,
    }
}

/// Build the final `Idl` from parsed program data.
pub fn build_idl(parsed: ParsedProgram) -> Idl {
    // Check for discriminator collisions across instructions, accounts, and events
    check_discriminator_collisions(&parsed);
    check_instruction_input_name_collision(&parsed);

    let instructions: Vec<IdlInstruction> = parsed
        .instructions
        .iter()
        .map(|ix| {
            // Look up the accounts struct by name
            let accounts_items = parsed
                .accounts_structs
                .iter()
                .find(|s| s.name == ix.accounts_type_name)
                .map(accounts::to_idl_accounts)
                .unwrap_or_default();

            let args: Vec<IdlField> = ix
                .args
                .iter()
                .map(|(name, ty)| IdlField {
                    name: helpers::to_camel_case(name),
                    ty: helpers::map_type_from_syn(ty),
                })
                .collect();

            IdlInstruction {
                name: helpers::to_camel_case(&ix.name),
                discriminator: ix.discriminator.clone(),
                accounts: accounts_items,
                args,
                has_remaining: ix.has_remaining,
            }
        })
        .collect();

    let account_defs: Vec<IdlAccountDef> = parsed
        .state_accounts
        .iter()
        .map(state::to_idl_account_def)
        .collect();

    let event_defs: Vec<IdlEventDef> = parsed.events.iter().map(events::to_idl_event_def).collect();

    let mut type_defs: Vec<IdlTypeDef> = parsed
        .state_accounts
        .iter()
        .map(state::to_idl_type_def)
        .collect();

    type_defs.extend(parsed.events.iter().map(events::to_idl_type_def));

    Idl {
        address: parsed.program_id,
        metadata: IdlMetadata {
            name: parsed.program_name,
            crate_name: parsed.crate_name,
            version: parsed.version,
            spec: "0.1.0".to_string(),
        },
        instructions,
        accounts: account_defs,
        events: event_defs,
        types: type_defs,
        errors: parsed.errors,
    }
}

fn check_instruction_input_name_collision(parsed: &ParsedProgram) {
    let mut issues = Vec::new();

    for ix in &parsed.instructions {
        let accounts_struct = parsed
            .accounts_structs
            .iter()
            .find(|s| s.name == ix.accounts_type_name);

        let mut input_field_sources: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();

        if let Some(accounts_struct) = accounts_struct {
            for field in &accounts_struct.fields {
                let name = helpers::to_camel_case(&field.name);

                // Only user-provided accounts are part of InstructionInput.
                if field.pda.is_none() && field.address.is_none() {
                    // Duplicate checks are scoped to fields that end up in InstructionInput.
                    input_field_sources.entry(name).or_default().push("account");
                }
            }
        }

        for (name, _) in &ix.args {
            let name = helpers::to_camel_case(name);

            // Instruction args are always part of InstructionInput.
            input_field_sources.entry(name).or_default().push("arg");
        }

        for (name, sources) in input_field_sources {
            if sources.len() > 1 {
                issues.push(format!(
                    "  instruction '{}' has duplicate input field '{}' from {}",
                    ix.name,
                    name,
                    sources.join(" + "),
                ));
            }
        }
    }

    if !issues.is_empty() {
        eprintln!("Error: duplicate instruction input field names detected:");
        for issue in &issues {
            eprintln!("{}", issue);
        }
        std::process::exit(1);
    }
}

/// Check for discriminator collisions across all instruction, account, and
/// event discriminators. Returns a list of collision descriptions, empty if no
/// collisions found.
pub fn find_discriminator_collisions(parsed: &ParsedProgram) -> Vec<String> {
    struct DiscEntry {
        kind: &'static str,
        name: String,
        discriminator: Vec<u8>,
    }

    let mut entries: Vec<DiscEntry> = Vec::new();

    for ix in &parsed.instructions {
        entries.push(DiscEntry {
            kind: "instruction",
            name: ix.name.clone(),
            discriminator: ix.discriminator.clone(),
        });
    }

    for acc in &parsed.state_accounts {
        entries.push(DiscEntry {
            kind: "account",
            name: acc.name.clone(),
            discriminator: acc.discriminator.clone(),
        });
    }

    for ev in &parsed.events {
        entries.push(DiscEntry {
            kind: "event",
            name: ev.name.clone(),
            discriminator: ev.discriminator.clone(),
        });
    }

    let mut collisions = Vec::new();

    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            // Only check within same kind
            if entries[i].kind != entries[j].kind {
                continue;
            }
            if entries[i].discriminator == entries[j].discriminator {
                collisions.push(format!(
                    "  {} '{}' and {} '{}' share discriminator {:?}",
                    entries[i].kind,
                    entries[i].name,
                    entries[j].kind,
                    entries[j].name,
                    entries[i].discriminator,
                ));
            }
        }
    }

    collisions
}

fn check_discriminator_collisions(parsed: &ParsedProgram) {
    let collisions = find_discriminator_collisions(parsed);
    if !collisions.is_empty() {
        eprintln!("Error: discriminator collisions detected:");
        for c in &collisions {
            eprintln!("{}", c);
        }
        std::process::exit(1);
    }
}

fn read_cargo_name(crate_root: &Path) -> Option<String> {
    let cargo_path = crate_root.join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_path).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let package = table.get("package")?.as_table()?;
    package.get("name")?.as_str().map(|s| s.to_string())
}

fn read_cargo_version(crate_root: &Path) -> Option<String> {
    let cargo_path = crate_root.join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_path).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let package = table.get("package")?.as_table()?;
    package.get("version")?.as_str().map(|s| s.to_string())
}
