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
    std::{
        collections::{BTreeMap, BTreeSet, HashSet},
        path::Path,
    },
};

/// Raw struct definition (potential instruction argument type).
pub struct RawDataStruct {
    pub name: String,
    pub fields: Vec<(String, syn::Type)>,
}

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
    /// All struct definitions from source files (potential instruction arg
    /// types).
    pub data_structs: Vec<RawDataStruct>,
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

    // 9. Collect all struct definitions (potential instruction arg types)
    let mut data_structs = Vec::new();
    for file in &files {
        data_structs.extend(extract_plain_structs(&file.file));
    }

    // 10. Read version and crate name from Cargo.toml
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
        data_structs,
    }
}

/// Build the final `Idl` from parsed program data.
pub fn build_idl(parsed: ParsedProgram) -> Idl {
    // Check for discriminator collisions across instructions, accounts, and events
    check_discriminator_collisions(&parsed);
    check_instruction_input_name_collision(&parsed);

    let ParsedProgram {
        program_id,
        program_name,
        crate_name,
        version,
        instructions: raw_instructions,
        accounts_structs,
        state_accounts,
        events: raw_events,
        errors,
        data_structs,
    } = parsed;

    let instructions: Vec<IdlInstruction> = raw_instructions
        .into_iter()
        .map(|ix| {
            // Look up the accounts struct by name (borrows, not consumed)
            let accounts_items = accounts_structs
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
                discriminator: ix.discriminator,
                accounts: accounts_items,
                args,
                has_remaining: ix.has_remaining,
            }
        })
        .collect();

    let (account_defs, mut type_defs): (Vec<IdlAccountDef>, Vec<IdlTypeDef>) = state_accounts
        .into_iter()
        .map(|sa| {
            let fields = sa
                .fields
                .iter()
                .map(|(name, ty)| IdlField {
                    name: helpers::to_camel_case(name),
                    ty: helpers::map_type_from_syn(ty),
                })
                .collect();
            let account_def = IdlAccountDef {
                name: sa.name.clone(),
                discriminator: sa.discriminator,
            };
            let type_def = IdlTypeDef {
                name: sa.name,
                ty: IdlTypeDefType {
                    kind: "struct".to_string(),
                    fields,
                },
            };
            (account_def, type_def)
        })
        .unzip();

    let (event_defs, event_type_defs): (Vec<IdlEventDef>, Vec<IdlTypeDef>) = raw_events
        .into_iter()
        .map(|ev| {
            let fields = ev
                .fields
                .iter()
                .map(|(name, ty)| IdlField {
                    name: helpers::to_camel_case(name),
                    ty: helpers::map_type_from_syn(ty),
                })
                .collect();
            let event_def = IdlEventDef {
                name: ev.name.clone(),
                discriminator: ev.discriminator,
            };
            let type_def = IdlTypeDef {
                name: ev.name,
                ty: IdlTypeDefType {
                    kind: "struct".to_string(),
                    fields,
                },
            };
            (event_def, type_def)
        })
        .unzip();

    type_defs.extend(event_type_defs);

    // Resolve custom struct types referenced in instruction args
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for ix in &instructions {
        for arg in &ix.args {
            collect_defined_refs(&arg.ty, &mut referenced);
        }
    }

    let existing_names: HashSet<String> = type_defs.iter().map(|t| t.name.clone()).collect();
    let data_struct_map: BTreeMap<&str, &[(String, syn::Type)]> = data_structs
        .iter()
        .map(|ds| (ds.name.as_str(), ds.fields.as_slice()))
        .collect();

    let mut to_resolve: Vec<String> = referenced
        .into_iter()
        .filter(|n| !existing_names.contains(n))
        .collect();
    let mut resolved_names = existing_names;

    while let Some(type_name) = to_resolve.pop() {
        if resolved_names.contains(&type_name) {
            continue;
        }
        if let Some(fields) = data_struct_map.get(type_name.as_str()) {
            let idl_fields: Vec<IdlField> = fields
                .iter()
                .map(|(name, ty)| IdlField {
                    name: helpers::to_camel_case(name),
                    ty: helpers::map_type_from_syn(ty),
                })
                .collect();

            // Check for nested Defined types
            for field in &idl_fields {
                if let IdlType::Defined { defined } = &field.ty {
                    if !resolved_names.contains(defined) {
                        to_resolve.push(defined.clone());
                    }
                }
            }

            resolved_names.insert(type_name.clone());
            type_defs.push(IdlTypeDef {
                name: type_name,
                ty: IdlTypeDefType {
                    kind: "struct".to_string(),
                    fields: idl_fields,
                },
            });
        }
    }

    Idl {
        address: program_id,
        metadata: IdlMetadata {
            name: program_name,
            crate_name,
            version,
            spec: "0.1.0".to_string(),
        },
        instructions,
        accounts: account_defs,
        events: event_defs,
        types: type_defs,
        errors,
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

fn collect_defined_refs(ty: &IdlType, out: &mut BTreeSet<String>) {
    match ty {
        IdlType::Defined { defined } => {
            out.insert(defined.clone());
        }
        IdlType::DynVec { vec } => collect_defined_refs(&vec.items, out),
        _ => {}
    }
}

fn extract_plain_structs(file: &syn::File) -> Vec<RawDataStruct> {
    let mut result = Vec::new();
    for item in &file.items {
        if let syn::Item::Struct(item_struct) = item {
            let fields = match &item_struct.fields {
                syn::Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| {
                        let name = f.ident.as_ref().unwrap().to_string();
                        (name, f.ty.clone())
                    })
                    .collect(),
                _ => continue,
            };
            result.push(RawDataStruct {
                name: item_struct.ident.to_string(),
                fields,
            });
        }
    }
    result
}

/// Parse a program from inline source code. Used by integration tests.
pub fn parse_program_from_source(src: &str) -> ParsedProgram {
    let file = syn::parse_file(src).expect("failed to parse source");

    let program_id = program::extract_program_id(&file).unwrap_or_default();
    let (program_name, instructions) =
        program::extract_program_module(&file).unwrap_or_else(|| ("test".to_string(), vec![]));
    let accounts_structs = accounts::extract_accounts_structs(&file);
    let state_accounts = state::extract_state_accounts(&file);
    let all_events = events::extract_events(&file);
    let all_errors = errors::extract_errors(&file);
    let data_structs = extract_plain_structs(&file);

    ParsedProgram {
        program_id,
        program_name,
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions,
        accounts_structs,
        state_accounts,
        events: all_events,
        errors: all_errors,
        data_structs,
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
