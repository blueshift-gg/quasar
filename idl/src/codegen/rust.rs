use {
    crate::{
        parser::{
            accounts::{RawAccountField, RawSeed},
            helpers, ParsedProgram,
        },
        types::IdlType,
    },
    std::{collections::HashMap, fmt::Write},
};

/// Generate Cargo.toml content for the standalone client crate.
pub fn generate_cargo_toml(name: &str, version: &str, has_pdas: bool) -> String {
    let solana_address = if has_pdas {
        r#"solana-address = { version = "2.2", features = ["curve25519"] }"#
    } else {
        r#"solana-address = "2.2""#
    };
    format!(
        r#"[package]
name = "{name}-client"
version = "{version}"
edition = "2021"

[dependencies]
quasar-lang = "0.0"
wincode = {{ version = "0.4", features = ["derive"] }}
{solana_address}
solana-instruction = "3"
"#,
    )
}

/// Check whether the parsed program has any resolvable PDA annotations.
/// Used by the CLI to decide whether `generate_cargo_toml` needs PDA deps.
pub fn has_pdas(parsed: &ParsedProgram) -> bool {
    !collect_pdas(parsed).is_empty()
}

/// Generate a standalone Rust client crate from parsed program data.
///
/// Returns a `Vec<(relative_path, file_content)>` where paths are relative to
/// the client crate `src/` directory (e.g. `"lib.rs"`,
/// `"instructions/mod.rs"`).
pub fn generate_client(parsed: &ParsedProgram) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();

    // Build type map for custom data types referenced anywhere: instruction args,
    // state account fields, and event fields. Transitively resolves nested types.
    let type_map: HashMap<String, Vec<(String, IdlType)>> = build_type_map(parsed);

    let has_instructions = !parsed.instructions.is_empty();
    let has_state = !parsed.state_accounts.is_empty();
    let has_events = !parsed.events.is_empty();
    let has_types = !type_map.is_empty();
    let has_errors = !parsed.errors.is_empty();

    // Collect PDA info for pda.rs generation
    let pdas = collect_pdas(parsed);
    let has_pdas = !pdas.is_empty();

    // --- lib.rs ---
    files.push((
        "lib.rs".to_string(),
        emit_lib_rs(
            parsed,
            has_instructions,
            has_state,
            has_events,
            has_types,
            has_errors,
            has_pdas,
        ),
    ));

    // --- instructions/ ---
    if has_instructions {
        let (mod_rs, ix_files) = emit_instructions(parsed, &type_map);
        files.push(("instructions/mod.rs".to_string(), mod_rs));
        for (name, content) in ix_files {
            files.push((format!("instructions/{}.rs", name), content));
        }
    }

    // --- state/ ---
    if has_state {
        let (mod_rs, state_files) = emit_state(parsed, &type_map);
        files.push(("state/mod.rs".to_string(), mod_rs));
        for (name, content) in state_files {
            files.push((format!("state/{}.rs", name), content));
        }
    }

    // --- events/ ---
    if has_events {
        let (mod_rs, event_files) = emit_events(parsed, &type_map);
        files.push(("events/mod.rs".to_string(), mod_rs));
        for (name, content) in event_files {
            files.push((format!("events/{}.rs", name), content));
        }
    }

    // --- types/ ---
    if has_types {
        let (mod_rs, type_files) = emit_types(&type_map);
        files.push(("types/mod.rs".to_string(), mod_rs));
        for (name, content) in type_files {
            files.push((format!("types/{}.rs", name), content));
        }
    }

    // --- errors.rs ---
    if has_errors {
        files.push(("errors.rs".to_string(), emit_errors(parsed)));
    }

    // --- pda.rs ---
    if has_pdas {
        files.push(("pda.rs".to_string(), emit_pda(&pdas)));
    }

    files
}

// ===========================================================================
// lib.rs
// ===========================================================================

fn emit_lib_rs(
    parsed: &ParsedProgram,
    has_instructions: bool,
    has_state: bool,
    has_events: bool,
    has_types: bool,
    has_errors: bool,
    has_pdas: bool,
) -> String {
    let mut out = String::new();
    out.push_str("use solana_address::Address;\n\n");

    writeln!(
        out,
        "pub const ID: Address = solana_address::address!(\"{}\");",
        parsed.program_id
    )
    .expect("write to String");

    let modules: &[(&str, bool)] = &[
        ("instructions", has_instructions),
        ("state", has_state),
        ("events", has_events),
        ("types", has_types),
        ("errors", has_errors),
        ("pda", has_pdas),
    ];

    let active: Vec<&&str> = modules
        .iter()
        .filter(|(_, active)| *active)
        .map(|(name, _)| name)
        .collect();

    if !active.is_empty() {
        out.push('\n');
        for name in &active {
            writeln!(out, "pub mod {};", name).expect("write to String");
        }
        out.push('\n');
        for name in &active {
            writeln!(out, "pub use {}::*;", name).expect("write to String");
        }
    }

    out
}

// ===========================================================================
// instructions/
// ===========================================================================

fn emit_instructions(
    parsed: &ParsedProgram,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut ix_files: Vec<(String, String)> = Vec::new();

    // Scan all instruction arg types for imports needed by mod.rs
    // (ProgramInstruction enum + decode_instruction use these types)
    let mut needs_dyn_bytes = false;
    let mut needs_dyn_vec = false;
    let mut needs_address = false;
    for ix in &parsed.instructions {
        for (_, ty) in &ix.args {
            let idl_ty = helpers::map_type_from_syn(ty);
            collect_wrapper_needs(
                &idl_ty,
                &mut needs_dyn_bytes,
                &mut needs_dyn_vec,
            );
            if field_needs_address(&idl_ty) {
                needs_address = true;
            }
        }
    }
    emit_wrapper_imports(&mut mod_rs, needs_dyn_bytes, needs_dyn_vec);
    if needs_address {
        mod_rs.push_str("use solana_address::Address;\n");
    }
    // Import defined types used in instruction args
    for ix in &parsed.instructions {
        for (_, ty) in &ix.args {
            let idl_ty = helpers::map_type_from_syn(ty);
            emit_type_use_imports(&mut mod_rs, &idl_ty, type_map);
        }
    }

    // mod declarations and re-exports
    for ix in &parsed.instructions {
        let snake = pascal_to_snake(&snake_to_pascal(&ix.name));
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    mod_rs.push('\n');
    for ix in &parsed.instructions {
        let snake = pascal_to_snake(&snake_to_pascal(&ix.name));
        writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
    }
    mod_rs.push('\n');

    // ProgramInstruction enum
    mod_rs.push_str("pub enum ProgramInstruction {\n");
    for ix in &parsed.instructions {
        let pascal = snake_to_pascal(&ix.name);
        let arg_types: Vec<IdlType> = ix
            .args
            .iter()
            .map(|(_, ty)| helpers::map_type_from_syn(ty))
            .collect();
        if ix.args.is_empty() {
            writeln!(mod_rs, "    {},", pascal).expect("write to String");
        } else {
            write!(mod_rs, "    {} {{ ", pascal).expect("write to String");
            for (i, (name, _)) in ix.args.iter().enumerate() {
                if i > 0 {
                    write!(mod_rs, ", ").expect("write to String");
                }
                write!(mod_rs, "{}: {}", name, rust_field_type(&arg_types[i]))
                    .expect("write to String");
            }
            writeln!(mod_rs, " }},").expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // decode_instruction function
    mod_rs.push_str("pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {\n");

    // Determine discriminator width (use first instruction's disc length)
    let disc_len = parsed
        .instructions
        .first()
        .map(|ix| ix.discriminator.len())
        .unwrap_or(1);

    if disc_len == 1 {
        mod_rs.push_str("    let disc = *data.first()?;\n");
        mod_rs.push_str("    match disc {\n");
    } else {
        writeln!(mod_rs, "    let disc = data.get(..{})?;", disc_len).expect("write to String");
        mod_rs.push_str("    match disc {\n");
    }

    for ix in &parsed.instructions {
        let pascal = snake_to_pascal(&ix.name);
        let disc_str = format_disc_list(&ix.discriminator);

        if disc_len == 1 {
            write!(mod_rs, "        {} => ", disc_str).expect("write to String");
        } else {
            write!(mod_rs, "        [{}] => ", disc_str).expect("write to String");
        }

        if ix.args.is_empty() {
            writeln!(mod_rs, "Some(ProgramInstruction::{}),", pascal).expect("write to String");
        } else {
            mod_rs.push_str("{\n");
            writeln!(mod_rs, "            let payload = &data[{}..];", disc_len)
                .expect("write to String");
            let arg_types: Vec<IdlType> = ix
                .args
                .iter()
                .map(|(_, ty)| helpers::map_type_from_syn(ty))
                .collect();
            let arg_count = ix.args.len();
            if arg_count > 1 {
                mod_rs.push_str("            let mut offset = 0usize;\n");
            }
            for (i, (name, _)) in ix.args.iter().enumerate() {
                let rty = rust_field_type(&arg_types[i]);
                if arg_count == 1 {
                    writeln!(
                        mod_rs,
                        "            let {}: {} = wincode::deserialize(payload).ok()?;",
                        name, rty
                    )
                    .expect("write to String");
                } else {
                    writeln!(
                        mod_rs,
                        "            let {}: {} = wincode::deserialize(&payload[offset..]).ok()?;",
                        name, rty
                    )
                    .expect("write to String");
                    if i + 1 < arg_count {
                        writeln!(
                            mod_rs,
                            "            offset += wincode::serialized_size(&{}).ok()? as usize;",
                            name
                        )
                        .expect("write to String");
                    }
                }
            }
            write!(
                mod_rs,
                "            Some(ProgramInstruction::{} {{ ",
                pascal
            )
            .expect("write to String");
            for (i, (name, _)) in ix.args.iter().enumerate() {
                if i > 0 {
                    write!(mod_rs, ", ").expect("write to String");
                }
                write!(mod_rs, "{}", name).expect("write to String");
            }
            mod_rs.push_str(" })\n");
            mod_rs.push_str("        }\n");
        }
    }

    mod_rs.push_str("        _ => None,\n");
    mod_rs.push_str("    }\n");
    mod_rs.push_str("}\n");

    // Individual instruction files
    for ix in &parsed.instructions {
        let snake = pascal_to_snake(&snake_to_pascal(&ix.name));
        let content = emit_single_instruction(parsed, ix, type_map);
        ix_files.push((snake, content));
    }

    (mod_rs, ix_files)
}

fn emit_single_instruction(
    parsed: &ParsedProgram,
    ix: &crate::parser::program::RawInstruction,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> String {
    let mut out = String::new();

    let accounts_struct = parsed
        .accounts_structs
        .iter()
        .find(|s| s.name == ix.accounts_type_name);

    let struct_name = snake_to_pascal(&ix.name);

    let arg_types: Vec<IdlType> = ix
        .args
        .iter()
        .map(|(_, ty)| helpers::map_type_from_syn(ty))
        .collect();

    // --- Per-file imports ---
    if ix.has_remaining {
        out.push_str("use std::vec::Vec;\n");
    }

    out.push_str("use solana_address::Address;\n");
    out.push_str("use solana_instruction::{AccountMeta, Instruction};\n");
    out.push_str("use crate::ID;\n");

    // Check if this instruction needs wrapper imports
    let mut needs_dyn_bytes = false;
    let mut needs_dyn_vec = false;
    for arg_ty in &arg_types {
        collect_wrapper_needs(arg_ty, &mut needs_dyn_bytes, &mut needs_dyn_vec);
    }
    emit_wrapper_imports(&mut out, needs_dyn_bytes, needs_dyn_vec);

    // Check if this instruction references any defined types
    for arg_ty in &arg_types {
        emit_type_use_imports(&mut out, arg_ty, type_map);
    }

    out.push('\n');

    // --- Struct definition ---
    writeln!(out, "pub struct {}Instruction {{", struct_name).expect("write to String");

    if let Some(accs) = accounts_struct {
        for field in &accs.fields {
            writeln!(out, "    pub {}: Address,", field.name).expect("write to String");
        }
    }

    for (i, (name, _)) in ix.args.iter().enumerate() {
        writeln!(out, "    pub {}: {},", name, rust_field_type(&arg_types[i]))
            .expect("write to String");
    }

    if ix.has_remaining {
        out.push_str("    pub remaining_accounts: Vec<AccountMeta>,\n");
    }

    out.push_str("}\n\n");

    // --- From impl ---
    writeln!(
        out,
        "impl From<{}Instruction> for Instruction {{",
        struct_name
    )
    .expect("write to String");
    writeln!(
        out,
        "    fn from(ix: {}Instruction) -> Instruction {{",
        struct_name
    )
    .expect("write to String");

    if ix.has_remaining {
        out.push_str("        let mut accounts = vec![\n");
    } else {
        out.push_str("        let accounts = vec![\n");
    }
    if let Some(accs) = accounts_struct {
        for field in &accs.fields {
            writeln!(out, "            {},", account_meta_expr(field)).expect("write to String");
        }
    }
    out.push_str("        ];\n");
    if ix.has_remaining {
        out.push_str("        accounts.extend(ix.remaining_accounts);\n");
    }

    // Instruction data
    let disc_str = format_disc_list(&ix.discriminator);

    if ix.args.is_empty() {
        writeln!(out, "        let data = vec![{}];", disc_str).expect("write to String");
    } else {
        writeln!(out, "        let mut data = vec![{}];", disc_str).expect("write to String");
        for (name, _) in &ix.args {
            writeln!(
                out,
                "        wincode::serialize_into(&mut data, &ix.{}).unwrap();",
                name
            )
            .expect("write to String");
        }
    }

    out.push_str("        Instruction {\n");
    out.push_str("            program_id: ID,\n");
    out.push_str("            accounts,\n");
    out.push_str("            data,\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    out
}

// ===========================================================================
// state/
// ===========================================================================

fn emit_state(
    parsed: &ParsedProgram,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut state_files: Vec<(String, String)> = Vec::new();

    // Accounts with fields get their own file
    let accounts_with_fields: Vec<_> = parsed
        .state_accounts
        .iter()
        .filter(|acc| !acc.fields.is_empty())
        .collect();
    let accounts_without_fields: Vec<_> = parsed
        .state_accounts
        .iter()
        .filter(|acc| acc.fields.is_empty())
        .collect();

    // mod declarations for accounts with fields
    for acc in &accounts_with_fields {
        let snake = pascal_to_snake(&acc.name);
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    if !accounts_with_fields.is_empty() {
        mod_rs.push('\n');
        for acc in &accounts_with_fields {
            let snake = pascal_to_snake(&acc.name);
            writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
        }
        mod_rs.push('\n');
    }

    // Discriminator constants for fieldless accounts (in mod.rs)
    for acc in &accounts_without_fields {
        let const_name = pascal_to_screaming_snake(&acc.name);
        let disc_str = format_disc_list(&acc.discriminator);
        writeln!(
            mod_rs,
            "pub const {}_ACCOUNT_DISCRIMINATOR: &[u8] = &[{}];",
            const_name, disc_str
        )
        .expect("write to String");
    }
    if !accounts_without_fields.is_empty() {
        mod_rs.push('\n');
    }

    // ProgramAccount enum
    mod_rs.push_str("pub enum ProgramAccount {\n");
    for acc in &parsed.state_accounts {
        if acc.fields.is_empty() {
            writeln!(mod_rs, "    {},", acc.name).expect("write to String");
        } else {
            writeln!(mod_rs, "    {}({}),", acc.name, acc.name).expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // decode_account function
    mod_rs.push_str("pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {\n");
    for acc in &parsed.state_accounts {
        let const_name = pascal_to_screaming_snake(&acc.name);
        writeln!(
            mod_rs,
            "    if data.starts_with({}_ACCOUNT_DISCRIMINATOR) {{",
            const_name
        )
        .expect("write to String");
        if acc.fields.is_empty() {
            writeln!(mod_rs, "        return Some(ProgramAccount::{});", acc.name)
                .expect("write to String");
        } else {
            writeln!(
                mod_rs,
                "        return wincode::deserialize::<{}>(data).ok().map(ProgramAccount::{});",
                acc.name, acc.name
            )
            .expect("write to String");
        }
        mod_rs.push_str("    }\n");
    }
    mod_rs.push_str("    None\n");
    mod_rs.push_str("}\n");

    // Individual state files
    for acc in &accounts_with_fields {
        let snake = pascal_to_snake(&acc.name);
        let content = emit_single_state_account(acc, type_map);
        state_files.push((snake, content));
    }

    (mod_rs, state_files)
}

fn emit_single_state_account(
    acc: &crate::parser::state::RawStateAccount,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> String {
    let mut out = String::new();

    // Imports for manual impls
    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");
    out.push_str("use wincode::config::ConfigCore;\n");
    out.push_str("use wincode::error::{ReadError, ReadResult, WriteResult};\n");
    out.push_str("use wincode::io::{Reader, Writer};\n");
    out.push_str("use std::mem::MaybeUninit;\n");

    // Check for wrapper type needs and Address
    let mut needs_address = false;
    let mut needs_dyn_bytes = false;
    let mut needs_dyn_vec = false;
    for (_, ty) in &acc.fields {
        let idl_ty = helpers::map_type_from_syn(ty);
        collect_wrapper_needs(&idl_ty, &mut needs_dyn_bytes, &mut needs_dyn_vec);
        if field_needs_address(&idl_ty) {
            needs_address = true;
        }
        emit_type_use_imports(&mut out, &idl_ty, type_map);
    }
    if needs_address {
        out.push_str("use solana_address::Address;\n");
    }
    emit_wrapper_imports(&mut out, needs_dyn_bytes, needs_dyn_vec);

    out.push('\n');

    // Discriminator constant
    let const_name = pascal_to_screaming_snake(&acc.name);
    let disc_str = format_disc_list(&acc.discriminator);
    writeln!(
        out,
        "pub const {}_ACCOUNT_DISCRIMINATOR: &[u8] = &[{}];",
        const_name, disc_str
    )
    .expect("write to String");
    out.push('\n');

    // Struct + manual impls
    emit_manual_impls(
        &mut out,
        &acc.name,
        &acc.discriminator,
        &acc.fields,
        "account",
    );

    out
}

// ===========================================================================
// events/
// ===========================================================================

fn emit_events(
    parsed: &ParsedProgram,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut event_files: Vec<(String, String)> = Vec::new();

    let events_with_fields: Vec<_> = parsed
        .events
        .iter()
        .filter(|ev| !ev.fields.is_empty())
        .collect();
    let events_without_fields: Vec<_> = parsed
        .events
        .iter()
        .filter(|ev| ev.fields.is_empty())
        .collect();

    // mod declarations for events with fields
    for ev in &events_with_fields {
        let snake = pascal_to_snake(&ev.name);
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    if !events_with_fields.is_empty() {
        mod_rs.push('\n');
        for ev in &events_with_fields {
            let snake = pascal_to_snake(&ev.name);
            writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
        }
        mod_rs.push('\n');
    }

    // Discriminator constants for fieldless events (in mod.rs)
    for ev in &events_without_fields {
        let base = ev.name.strip_suffix("Event").unwrap_or(&ev.name);
        let const_name = pascal_to_screaming_snake(base);
        let disc_str = format_disc_list(&ev.discriminator);
        writeln!(
            mod_rs,
            "pub const {}_EVENT_DISCRIMINATOR: &[u8] = &[{}];",
            const_name, disc_str
        )
        .expect("write to String");
    }
    if !events_without_fields.is_empty() {
        mod_rs.push('\n');
    }

    // ProgramEvent enum
    mod_rs.push_str("pub enum ProgramEvent {\n");
    for ev in &parsed.events {
        if ev.fields.is_empty() {
            writeln!(mod_rs, "    {},", ev.name).expect("write to String");
        } else {
            writeln!(mod_rs, "    {}({}),", ev.name, ev.name).expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // decode_event function
    mod_rs.push_str("pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {\n");
    for ev in &parsed.events {
        let base = ev.name.strip_suffix("Event").unwrap_or(&ev.name);
        let const_name = pascal_to_screaming_snake(base);
        writeln!(
            mod_rs,
            "    if data.starts_with({}_EVENT_DISCRIMINATOR) {{",
            const_name
        )
        .expect("write to String");
        if ev.fields.is_empty() {
            writeln!(mod_rs, "        return Some(ProgramEvent::{});", ev.name)
                .expect("write to String");
        } else {
            writeln!(
                mod_rs,
                "        return wincode::deserialize::<{}>(data).ok().map(ProgramEvent::{});",
                ev.name, ev.name
            )
            .expect("write to String");
        }
        mod_rs.push_str("    }\n");
    }
    mod_rs.push_str("    None\n");
    mod_rs.push_str("}\n");

    // Individual event files
    for ev in &events_with_fields {
        let snake = pascal_to_snake(&ev.name);
        let content = emit_single_event(ev, type_map);
        event_files.push((snake, content));
    }

    (mod_rs, event_files)
}

fn emit_single_event(
    ev: &crate::parser::events::RawEvent,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> String {
    let mut out = String::new();

    // Imports for manual impls
    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");
    out.push_str("use wincode::config::ConfigCore;\n");
    out.push_str("use wincode::error::{ReadError, ReadResult, WriteResult};\n");
    out.push_str("use wincode::io::{Reader, Writer};\n");
    out.push_str("use std::mem::MaybeUninit;\n");

    // Check for wrapper type needs and Address
    let mut needs_address = false;
    let mut needs_dyn_bytes = false;
    let mut needs_dyn_vec = false;
    for (_, ty) in &ev.fields {
        let idl_ty = helpers::map_type_from_syn(ty);
        collect_wrapper_needs(&idl_ty, &mut needs_dyn_bytes, &mut needs_dyn_vec);
        if field_needs_address(&idl_ty) {
            needs_address = true;
        }
        emit_type_use_imports(&mut out, &idl_ty, type_map);
    }
    if needs_address {
        out.push_str("use solana_address::Address;\n");
    }
    emit_wrapper_imports(&mut out, needs_dyn_bytes, needs_dyn_vec);

    out.push('\n');

    // Discriminator constant — strip trailing "Event" to avoid stutter
    // (e.g. MakeEvent → MAKE_EVENT_DISCRIMINATOR, not
    // MAKE_EVENT_EVENT_DISCRIMINATOR)
    let base_name = ev.name.strip_suffix("Event").unwrap_or(&ev.name);
    let const_name = pascal_to_screaming_snake(base_name);
    let disc_str = format_disc_list(&ev.discriminator);
    writeln!(
        out,
        "pub const {}_EVENT_DISCRIMINATOR: &[u8] = &[{}];",
        const_name, disc_str
    )
    .expect("write to String");
    out.push('\n');

    // Struct + manual impls
    emit_manual_impls(&mut out, &ev.name, &ev.discriminator, &ev.fields, "event");

    out
}

// ===========================================================================
// types/
// ===========================================================================

fn emit_types(
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut type_files: Vec<(String, String)> = Vec::new();

    // Sort for deterministic output
    let mut type_names: Vec<&String> = type_map.keys().collect();
    type_names.sort();

    for type_name in &type_names {
        let snake = pascal_to_snake(type_name);
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    mod_rs.push('\n');
    for type_name in &type_names {
        let snake = pascal_to_snake(type_name);
        writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
    }

    for type_name in &type_names {
        let fields = &type_map[*type_name];
        let snake = pascal_to_snake(type_name);
        let content = emit_single_type(type_name, fields, type_map);
        type_files.push((snake, content));
    }

    (mod_rs, type_files)
}

fn emit_single_type(
    type_name: &str,
    fields: &[(String, IdlType)],
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) -> String {
    let mut out = String::new();

    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");

    // Check for wrapper type needs and Address
    let mut needs_address = false;
    let mut needs_dyn_bytes = false;
    let mut needs_dyn_vec = false;
    for (_, fty) in fields {
        collect_wrapper_needs(fty, &mut needs_dyn_bytes, &mut needs_dyn_vec);
        if field_needs_address(fty) {
            needs_address = true;
        }
        emit_type_use_imports(&mut out, fty, type_map);
    }
    if needs_address {
        out.push_str("use solana_address::Address;\n");
    }
    emit_wrapper_imports(&mut out, needs_dyn_bytes, needs_dyn_vec);

    out.push('\n');

    out.push_str("#[derive(SchemaWrite, SchemaRead)]\n");
    writeln!(out, "pub struct {} {{", type_name).expect("write to String");
    for (field_name, field_ty) in fields {
        writeln!(
            out,
            "    pub {}: {},",
            field_name,
            rust_field_type(field_ty)
        )
        .expect("write to String");
    }
    out.push_str("}\n");

    out
}

// ===========================================================================
// errors.rs
// ===========================================================================

fn emit_errors(parsed: &ParsedProgram) -> String {
    let mut out = String::new();

    let enum_name = format!("{}Error", snake_to_pascal(&parsed.program_name));

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    out.push_str("#[repr(u32)]\n");
    writeln!(out, "pub enum {} {{", enum_name).expect("write to String");
    for err in &parsed.errors {
        writeln!(out, "    {} = {},", err.name, err.code).expect("write to String");
    }
    out.push_str("}\n\n");

    writeln!(out, "impl {} {{", enum_name).expect("write to String");

    // from_code
    out.push_str("    pub fn from_code(code: u32) -> Option<Self> {\n");
    out.push_str("        match code {\n");
    for err in &parsed.errors {
        writeln!(out, "            {} => Some(Self::{}),", err.code, err.name)
            .expect("write to String");
    }
    out.push_str("            _ => None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    // message
    out.push_str("    pub fn message(&self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for err in &parsed.errors {
        let msg = err.msg.as_deref().unwrap_or(&err.name);
        let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
        writeln!(out, "            Self::{} => \"{}\",", err.name, escaped)
            .expect("write to String");
    }
    out.push_str("        }\n");
    out.push_str("    }\n");

    out.push_str("}\n");

    out
}

// ===========================================================================
// pda.rs
// ===========================================================================

/// A seed element for PDA generation (owned copy of parser's RawSeed).
enum OwnedSeed {
    ByteString(Vec<u8>),
    AccountRef(String),
    ArgRef(String),
}

/// A collected PDA with its field name and seeds.
struct PdaInfo {
    field_name: String,
    seeds: Vec<OwnedSeed>,
}

fn raw_seed_to_owned(seed: &RawSeed) -> OwnedSeed {
    match seed {
        RawSeed::ByteString(bytes) => OwnedSeed::ByteString(bytes.clone()),
        RawSeed::AccountRef(name) => OwnedSeed::AccountRef(name.clone()),
        RawSeed::ArgRef(name) => OwnedSeed::ArgRef(name.clone()),
    }
}

fn collect_pdas(parsed: &ParsedProgram) -> Vec<PdaInfo> {
    let mut pdas: Vec<PdaInfo> = Vec::new();
    let mut seen_seeds: Vec<Vec<u8>> = Vec::new(); // serialized seeds for dedup

    for acc_struct in &parsed.accounts_structs {
        for field in &acc_struct.fields {
            if let Some(pda) = &field.pda {
                // Only include PDAs with resolvable seeds
                if pda.seeds.is_empty() {
                    continue;
                }

                // Serialize seeds for dedup
                let seed_key = serialize_seeds_for_dedup(&pda.seeds);
                if seen_seeds.contains(&seed_key) {
                    continue;
                }
                seen_seeds.push(seed_key);

                pdas.push(PdaInfo {
                    field_name: field.name.clone(),
                    seeds: pda.seeds.iter().map(raw_seed_to_owned).collect(),
                });
            }
        }
    }

    pdas
}

fn serialize_seeds_for_dedup(seeds: &[RawSeed]) -> Vec<u8> {
    let mut buf = Vec::new();
    for seed in seeds {
        match seed {
            RawSeed::ByteString(bytes) => {
                buf.push(0);
                buf.extend(bytes);
            }
            RawSeed::AccountRef(name) => {
                buf.push(1);
                buf.extend(name.as_bytes());
            }
            RawSeed::ArgRef(name) => {
                buf.push(2);
                buf.extend(name.as_bytes());
            }
        }
        buf.push(0xFF); // separator
    }
    buf
}

fn emit_pda(pdas: &[PdaInfo]) -> String {
    let mut out = String::new();

    out.push_str("use solana_address::Address;\n\n");

    for pda in pdas {
        // Build doc comment showing seeds
        let seed_desc: Vec<String> = pda
            .seeds
            .iter()
            .map(|s| match s {
                OwnedSeed::ByteString(bytes) => {
                    if bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
                        format!("b\"{}\"", String::from_utf8_lossy(bytes))
                    } else {
                        format!("&{:?}", bytes)
                    }
                }
                OwnedSeed::AccountRef(name) => name.clone(),
                OwnedSeed::ArgRef(name) => format!("arg:{}", name),
            })
            .collect();
        writeln!(out, "/// Seeds: [{}]", seed_desc.join(", ")).expect("write to String");

        // Function parameters: collect AccountRef seeds as parameters
        let mut params: Vec<String> = Vec::new();
        for seed in &pda.seeds {
            match seed {
                OwnedSeed::AccountRef(name) => {
                    params.push(format!("{}: &Address", name));
                }
                OwnedSeed::ArgRef(name) => {
                    params.push(format!("{}: &[u8]", name));
                }
                _ => {}
            }
        }
        params.push("program_id: &Address".to_string());

        let fn_name = format!("find_{}_address", pascal_to_snake(&pda.field_name));
        writeln!(
            out,
            "pub fn {}({}) -> (Address, u8) {{",
            fn_name,
            params.join(", ")
        )
        .expect("write to String");

        // Build seeds array
        let seed_exprs: Vec<String> = pda
            .seeds
            .iter()
            .map(|s| match s {
                OwnedSeed::ByteString(bytes) => {
                    if bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
                        format!("b\"{}\"", String::from_utf8_lossy(bytes))
                    } else {
                        let byte_list: Vec<String> =
                            bytes.iter().map(|b| format!("{}", b)).collect();
                        format!("&[{}]", byte_list.join(", "))
                    }
                }
                OwnedSeed::AccountRef(name) => format!("{}.as_ref()", name),
                OwnedSeed::ArgRef(name) => name.clone(),
            })
            .collect();

        writeln!(
            out,
            "    Address::find_program_address(&[{}], program_id)",
            seed_exprs.join(", ")
        )
        .expect("write to String");
        out.push_str("}\n\n");
    }

    out
}

// ===========================================================================
// Shared helpers
// ===========================================================================

/// Build type map for custom data types referenced anywhere.
fn build_type_map(parsed: &ParsedProgram) -> HashMap<String, Vec<(String, IdlType)>> {
    let mut map = HashMap::new();

    let mut referenced = std::collections::BTreeSet::new();
    for ix in &parsed.instructions {
        for (_, ty) in &ix.args {
            let idl_ty = helpers::map_type_from_syn(ty);
            collect_defined_refs(&idl_ty, &mut referenced);
        }
    }
    for acc in &parsed.state_accounts {
        for (_, ty) in &acc.fields {
            let idl_ty = helpers::map_type_from_syn(ty);
            collect_defined_refs(&idl_ty, &mut referenced);
        }
    }
    for ev in &parsed.events {
        for (_, ty) in &ev.fields {
            let idl_ty = helpers::map_type_from_syn(ty);
            collect_defined_refs(&idl_ty, &mut referenced);
        }
    }

    let struct_map: HashMap<&str, &[(String, syn::Type)]> = parsed
        .data_structs
        .iter()
        .map(|ds| (ds.name.as_str(), ds.fields.as_slice()))
        .collect();

    let mut to_resolve: Vec<String> = referenced.into_iter().collect();
    let mut resolved = std::collections::HashSet::new();

    while let Some(name) = to_resolve.pop() {
        if resolved.contains(&name) {
            continue;
        }
        if let Some(fields) = struct_map.get(name.as_str()) {
            let idl_fields: Vec<(String, IdlType)> = fields
                .iter()
                .map(|(fname, fty)| (fname.clone(), helpers::map_type_from_syn(fty)))
                .collect();
            for (_, fty) in &idl_fields {
                if let IdlType::Defined { defined } = fty {
                    if !resolved.contains(defined) {
                        to_resolve.push(defined.clone());
                    }
                }
            }
            resolved.insert(name.clone());
            map.insert(name, idl_fields);
        }
    }
    map
}

/// Emit struct definition + manual SchemaWrite/SchemaRead impls with
/// discriminator handling. Used for both accounts and events.
fn emit_manual_impls(
    out: &mut String,
    name: &str,
    discriminator: &[u8],
    raw_fields: &[(String, syn::Type)],
    kind: &str,
) {
    let has_dynamic = raw_fields.iter().any(|(_, ty)| {
        matches!(
            helpers::map_type_from_syn(ty),
            IdlType::DynString { .. } | IdlType::DynVec { .. }
        )
    });

    if has_dynamic {
        out.push_str("#[derive(Clone)]\n");
    } else {
        out.push_str("#[derive(Clone, Copy)]\n");
    }
    writeln!(out, "pub struct {} {{", name).expect("write to String");
    let fields: Vec<(String, String)> = raw_fields
        .iter()
        .map(|(fname, fty)| {
            (
                fname.clone(),
                rust_field_type(&helpers::map_type_from_syn(fty)),
            )
        })
        .collect();
    for (field_name, field_type) in &fields {
        writeln!(out, "    pub {}: {},", field_name, field_type).expect("write to String");
    }
    out.push_str("}\n\n");

    let unique_types: Vec<String> = {
        let mut types: Vec<String> = fields.iter().map(|(_, ty)| ty.clone()).collect();
        types.sort();
        types.dedup();
        types
    };

    // Strip trailing kind suffix to avoid stutter (e.g. MakeEvent + EVENT →
    // MAKE_EVENT, not MAKE_EVENT_EVENT)
    let base_name = if kind == "event" {
        name.strip_suffix("Event").unwrap_or(name)
    } else {
        name
    };
    let const_name = pascal_to_screaming_snake(base_name);
    let disc_const = format!("{}_{}_DISCRIMINATOR", const_name, kind.to_ascii_uppercase());

    // --- SchemaWrite impl ---
    writeln!(
        out,
        "unsafe impl<C: ConfigCore> SchemaWrite<C> for {}",
        name
    )
    .expect("write to String");
    out.push_str("where\n");
    for ty in &unique_types {
        writeln!(out, "    {ty}: SchemaWrite<C, Src = {ty}>,").expect("write to String");
    }
    out.push_str("{\n");
    out.push_str("    type Src = Self;\n\n");

    out.push_str("    fn size_of(src: &Self) -> WriteResult<usize> {\n");
    write!(out, "        Ok({}", discriminator.len()).expect("write to String");
    for (field_name, field_type) in &fields {
        write!(
            out,
            "\n            + <{field_type} as SchemaWrite<C>>::size_of(&src.{field_name})?"
        )
        .expect("write to String");
    }
    out.push_str(")\n");
    out.push_str("    }\n\n");

    out.push_str("    fn write(mut writer: impl Writer, src: &Self) -> WriteResult<()> {\n");
    writeln!(out, "        writer.write({disc_const})?;").expect("write to String");
    for (field_name, field_type) in &fields {
        writeln!(
            out,
            "        <{field_type} as SchemaWrite<C>>::write(writer.by_ref(), &src.{field_name})?;"
        )
        .expect("write to String");
    }
    out.push_str("        Ok(())\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    // --- SchemaRead impl ---
    writeln!(
        out,
        "unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for {}",
        name
    )
    .expect("write to String");
    out.push_str("where\n");
    for ty in &unique_types {
        writeln!(out, "    {ty}: SchemaRead<'de, C, Dst = {ty}>,").expect("write to String");
    }
    out.push_str("{\n");
    out.push_str("    type Dst = Self;\n\n");
    out.push_str(
        "    fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self>) -> ReadResult<()> \
         {\n",
    );

    if discriminator.len() == 1 {
        out.push_str("        let disc = reader.take_byte()?;\n");
        writeln!(out, "        if disc != {} {{", discriminator[0]).expect("write to String");
    } else {
        writeln!(
            out,
            "        let disc = reader.take_array::<{}>()?;",
            discriminator.len()
        )
        .expect("write to String");
        let disc_str = format_disc_list(discriminator);
        writeln!(out, "        if disc != [{disc_str}] {{").expect("write to String");
    }
    let disc_kind = if kind == "account" {
        "account discriminator"
    } else {
        "event discriminator"
    };
    writeln!(
        out,
        "            return Err(ReadError::InvalidValue(\"invalid {disc_kind}\"));"
    )
    .expect("write to String");
    out.push_str("        }\n");

    out.push_str("        dst.write(Self {\n");
    for (field_name, field_type) in &fields {
        writeln!(
            out,
            "            {field_name}: <{field_type} as SchemaRead<'de, \
             C>>::get(reader.by_ref())?,"
        )
        .expect("write to String");
    }
    out.push_str("        });\n");
    out.push_str("        Ok(())\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");
}

fn account_meta_expr(field: &RawAccountField) -> String {
    let signer = field.signer;
    if field.writable {
        format!("AccountMeta::new(ix.{}, {})", field.name, signer)
    } else {
        format!("AccountMeta::new_readonly(ix.{}, {})", field.name, signer)
    }
}

/// Map an `IdlType` to its Rust field type for the client struct.
fn rust_field_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "publicKey" => "Address".to_string(),
            other => other.to_string(),
        },
        IdlType::DynString { string } => prefix_generic("DynBytes", string.prefix_bytes),
        IdlType::DynVec { vec } => {
            let inner = rust_field_type(&vec.items);
            match vec.prefix_bytes {
                4 => format!("DynVec<{}>", inner),
                _ => format!("DynVec<{}, {}>", inner, prefix_rust_type(vec.prefix_bytes)),
            }
        }
        IdlType::Defined { defined } => defined.clone(),
    }
}

/// Map prefix byte width to a Rust type name or generic type string.
fn prefix_generic(wrapper: &str, prefix_bytes: usize) -> String {
    match prefix_bytes {
        4 => wrapper.to_string(),
        _ => format!("{}<{}>", wrapper, prefix_rust_type(prefix_bytes)),
    }
}

fn prefix_rust_type(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        _ => "u32",
    }
}

fn collect_defined_refs(ty: &IdlType, out: &mut std::collections::BTreeSet<String>) {
    match ty {
        IdlType::Defined { defined } => {
            out.insert(defined.clone());
        }
        IdlType::DynVec { vec } => collect_defined_refs(&vec.items, out),
        _ => {}
    }
}

/// Scan an IdlType for wrapper type usage (DynBytes, DynVec).
fn collect_wrapper_needs(
    ty: &IdlType,
    needs_dyn_bytes: &mut bool,
    needs_dyn_vec: &mut bool,
) {
    match ty {
        IdlType::DynString { .. } => *needs_dyn_bytes = true,
        IdlType::DynVec { vec } => {
            *needs_dyn_vec = true;
            collect_wrapper_needs(&vec.items, needs_dyn_bytes, needs_dyn_vec);
        }
        _ => {}
    }
}

/// Format discriminator bytes as a comma-separated list (no brackets).
fn format_disc_list(disc: &[u8]) -> String {
    let mut s = String::with_capacity(disc.len() * 4);
    for (i, b) in disc.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "{}", b).expect("write to String");
    }
    s
}

fn pascal_to_screaming_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.collect::<String>(),
            }
        })
        .collect()
}

/// Convert PascalCase to snake_case.
fn pascal_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev_lower = chars[i - 1].is_lowercase();
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if prev_lower || next_lower {
                result.push('_');
            }
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

/// Check if a field type references Address (publicKey).
fn field_needs_address(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "publicKey",
        IdlType::DynVec { vec } => field_needs_address(&vec.items),
        _ => false,
    }
}

/// Emit `use quasar_lang::client::{...};` for wrapper types.
fn emit_wrapper_imports(out: &mut String, needs_dyn_bytes: bool, needs_dyn_vec: bool) {
    let mut wrappers = Vec::new();
    if needs_dyn_bytes {
        wrappers.push("DynBytes");
    }
    if needs_dyn_vec {
        wrappers.push("DynVec");
    }
    if !wrappers.is_empty() {
        writeln!(out, "use quasar_lang::client::{{{}}};", wrappers.join(", "))
            .expect("write to String");
    }
}

/// Emit `use crate::types::TypeName;` for defined types referenced by a field.
fn emit_type_use_imports(
    out: &mut String,
    ty: &IdlType,
    type_map: &HashMap<String, Vec<(String, IdlType)>>,
) {
    match ty {
        IdlType::Defined { defined } if type_map.contains_key(defined) => {
            let import = format!("use crate::types::{};\n", defined);
            if !out.contains(&import) {
                out.push_str(&import);
            }
        }
        IdlType::DynVec { vec } => emit_type_use_imports(out, &vec.items, type_map),
        _ => {}
    }
}
