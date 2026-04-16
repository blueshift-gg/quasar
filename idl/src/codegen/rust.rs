use {
    super::model::ProgramModel,
    crate::types::{Idl, IdlAccountItem, IdlField, IdlSeed, IdlType},
    quasar_schema::{
        camel_to_pascal, camel_to_snake, pascal_to_snake, snake_to_pascal,
        to_screaming_snake as pascal_to_screaming_snake,
    },
    std::{
        collections::{HashMap, HashSet},
        fmt::Write,
    },
};

/// Generate Cargo.toml content for the standalone client crate.
///
/// `quasar-lang` is sourced from the GitHub master branch rather than
/// crates.io so that source-build users always get a `quasar-lang` that
/// matches the CLI they built. The wincode and solana-address versions are
/// exact-pinned to avoid the wincode 0.4/0.5 split (`solana-address >= 2.3`
/// depends on wincode 0.5, but the generated code and quasar-lang both
/// target wincode 0.4).
pub fn generate_cargo_toml(name: &str, version: &str, has_pdas: bool) -> String {
    let solana_address = if has_pdas {
        r#"solana-address = { version = "=2.2.0", features = ["curve25519", "wincode"] }"#
    } else {
        r#"solana-address = { version = "=2.2.0", features = ["wincode"] }"#
    };
    format!(
        r#"[package]
name = "{name}-client"
version = "{version}"
edition = "2021"

[dependencies]
quasar-lang = {{ git = "https://github.com/blueshift-gg/quasar", branch = "master" }}
wincode = {{ version = "=0.4.9", features = ["derive"] }}
{solana_address}
solana-instruction = "3"
"#,
    )
}

pub fn generate_cargo_toml_for_program(model: &ProgramModel<'_>) -> String {
    generate_cargo_toml(
        &model.identity.client_name,
        &model.idl.metadata.version,
        model.features.has_pdas,
    )
}

/// Check whether the IDL has any resolvable PDA annotations.
/// Used by the CLI to decide whether `generate_cargo_toml` needs PDA deps.
pub fn has_pdas(idl: &Idl) -> bool {
    ProgramModel::new(idl).features.has_pdas
}

/// Generate a standalone Rust client crate from the IDL.
///
/// Returns a `Vec<(relative_path, file_content)>` where paths are relative to
/// the client crate `src/` directory (e.g. `"lib.rs"`,
/// `"instructions/mod.rs"`).
pub fn generate_client(idl: &Idl) -> Vec<(String, String)> {
    let model = ProgramModel::new(idl);
    let mut files: Vec<(String, String)> = Vec::new();

    // Build type map for custom data types. The IDL already resolved these
    // transitively in build_idl — this is the single source of truth.
    let type_map: HashMap<String, Vec<IdlField>> = idl
        .types
        .iter()
        .map(|td| (td.name.clone(), td.ty.fields.clone()))
        .collect();

    let has_instructions = model.features.has_instructions;
    let has_state = model.features.has_accounts;
    let has_events = model.features.has_events;
    let has_types = model.features.has_types;
    let has_errors = model.features.has_errors;

    // Collect PDA info for pda.rs generation
    let pdas = collect_pdas(idl);
    let has_pdas = model.features.has_pdas;

    // --- lib.rs ---
    files.push((
        "lib.rs".to_string(),
        emit_lib_rs(
            idl,
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
        let (mod_rs, ix_files) = emit_instructions(idl, &type_map);
        files.push(("instructions/mod.rs".to_string(), mod_rs));
        for (name, content) in ix_files {
            files.push((format!("instructions/{}.rs", name), content));
        }
    }

    // --- state/ ---
    if has_state {
        let (mod_rs, state_files) = emit_discriminated_module(
            &idl.accounts,
            "account",
            "ProgramAccount",
            "decode_account",
            &type_map,
        );
        files.push(("state/mod.rs".to_string(), mod_rs));
        for (name, content) in state_files {
            files.push((format!("state/{}.rs", name), content));
        }
    }

    // --- events/ ---
    if has_events {
        let (mod_rs, event_files) = emit_discriminated_module(
            &idl.events,
            "event",
            "ProgramEvent",
            "decode_event",
            &type_map,
        );
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
        files.push(("errors.rs".to_string(), emit_errors(idl)));
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
    idl: &Idl,
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
        idl.address
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

    let active: Vec<&str> = modules
        .iter()
        .filter(|(_, active)| *active)
        .map(|(name, _)| *name)
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
    idl: &Idl,
    type_map: &HashMap<String, Vec<IdlField>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut ix_files: Vec<(String, String)> = Vec::new();

    // Scan all instruction arg types for imports needed by mod.rs
    let mut needs_dyn_string = false;
    let mut needs_dyn_vec = false;
    let mut needs_address = false;
    for ix in &idl.instructions {
        for arg in &ix.args {
            collect_wrapper_needs(&arg.ty, &mut needs_dyn_string, &mut needs_dyn_vec);
            if field_needs_address(&arg.ty) {
                needs_address = true;
            }
        }
    }
    emit_wrapper_imports(&mut mod_rs, needs_dyn_string, needs_dyn_vec);
    if needs_address {
        mod_rs.push_str("use solana_address::Address;\n");
    }
    // Import defined types used in instruction args
    for ix in &idl.instructions {
        for arg in &ix.args {
            emit_type_use_imports(&mut mod_rs, &arg.ty, type_map);
        }
    }

    // mod declarations and re-exports
    for ix in &idl.instructions {
        let snake = camel_to_snake(&ix.name);
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    mod_rs.push('\n');
    for ix in &idl.instructions {
        let snake = camel_to_snake(&ix.name);
        writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
    }
    mod_rs.push('\n');

    // ProgramInstruction enum
    mod_rs.push_str("pub enum ProgramInstruction {\n");
    for ix in &idl.instructions {
        let pascal = camel_to_pascal(&ix.name);
        if ix.args.is_empty() {
            writeln!(mod_rs, "    {},", pascal).expect("write to String");
        } else {
            write!(mod_rs, "    {} {{ ", pascal).expect("write to String");
            for (i, arg) in ix.args.iter().enumerate() {
                if i > 0 {
                    write!(mod_rs, ", ").expect("write to String");
                }
                write!(
                    mod_rs,
                    "{}: {}",
                    camel_to_snake(&arg.name),
                    rust_field_type(&arg.ty)
                )
                .expect("write to String");
            }
            writeln!(mod_rs, " }},").expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // decode_instruction function
    mod_rs.push_str("pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction> {\n");

    // All instructions share the same discriminator width (enforced by the parser).
    let disc_len = idl
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

    for ix in &idl.instructions {
        let pascal = camel_to_pascal(&ix.name);
        let disc_str = super::format_disc_decimal(&ix.discriminator);

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
            let arg_count = ix.args.len();
            if arg_count > 1 {
                mod_rs.push_str("            let mut offset = 0usize;\n");
            }
            for (i, arg) in ix.args.iter().enumerate() {
                let name = camel_to_snake(&arg.name);
                let rty = rust_field_type(&arg.ty);
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
            for (i, arg) in ix.args.iter().enumerate() {
                if i > 0 {
                    write!(mod_rs, ", ").expect("write to String");
                }
                write!(mod_rs, "{}", camel_to_snake(&arg.name)).expect("write to String");
            }
            mod_rs.push_str(" })\n");
            mod_rs.push_str("        }\n");
        }
    }

    mod_rs.push_str("        _ => None,\n");
    mod_rs.push_str("    }\n");
    mod_rs.push_str("}\n");

    // Individual instruction files
    for ix in &idl.instructions {
        let snake = camel_to_snake(&ix.name);
        let content = emit_single_instruction(ix, type_map);
        ix_files.push((snake, content));
    }

    (mod_rs, ix_files)
}

fn emit_single_instruction(
    ix: &crate::types::IdlInstruction,
    type_map: &HashMap<String, Vec<IdlField>>,
) -> String {
    let mut out = String::new();

    let struct_name = camel_to_pascal(&ix.name);

    // --- Per-file imports ---
    if ix.has_remaining {
        out.push_str("use std::vec::Vec;\n");
    }

    out.push_str("use solana_address::Address;\n");
    out.push_str("use solana_instruction::{AccountMeta, Instruction};\n");
    out.push_str("use crate::ID;\n");

    emit_field_imports(&mut out, ix.args.iter().map(|a| &a.ty), type_map);

    out.push('\n');

    // --- Struct definition ---
    writeln!(out, "pub struct {}Instruction {{", struct_name).expect("write to String");

    for account in &ix.accounts {
        writeln!(out, "    pub {}: Address,", camel_to_snake(&account.name))
            .expect("write to String");
    }

    for arg in &ix.args {
        writeln!(
            out,
            "    pub {}: {},",
            camel_to_snake(&arg.name),
            rust_field_type(&arg.ty)
        )
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
    for account in &ix.accounts {
        writeln!(out, "            {},", account_meta_expr(account)).expect("write to String");
    }
    out.push_str("        ];\n");
    if ix.has_remaining {
        out.push_str("        accounts.extend(ix.remaining_accounts);\n");
    }

    // Instruction data
    let disc_str = super::format_disc_decimal(&ix.discriminator);

    if ix.args.is_empty() {
        writeln!(out, "        let data = vec![{}];", disc_str).expect("write to String");
    } else {
        writeln!(out, "        let mut data = vec![{}];", disc_str).expect("write to String");
        for arg in &ix.args {
            writeln!(
                out,
                "        wincode::serialize_into(&mut data, &ix.{}).expect(\"serialization into \
                 Vec<u8> is infallible\");",
                camel_to_snake(&arg.name)
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
// state/ and events/ — unified via DiscriminatedItem trait
// ===========================================================================

/// Trait abstracting over IdlAccountDef and IdlEventDef for shared codegen.
trait DiscriminatedItem {
    fn name(&self) -> &str;
    fn discriminator(&self) -> &[u8];
}

impl DiscriminatedItem for crate::types::IdlAccountDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn discriminator(&self) -> &[u8] {
        &self.discriminator
    }
}

impl DiscriminatedItem for crate::types::IdlEventDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn discriminator(&self) -> &[u8] {
        &self.discriminator
    }
}

/// Generate mod.rs + individual files for a discriminated module (state or
/// events).
///
/// `kind` is `"account"` or `"event"` — controls discriminator constant suffix,
/// enum name stripping, and SchemaRead error messages.
fn emit_discriminated_module<T: DiscriminatedItem>(
    items: &[T],
    kind: &str,
    enum_name: &str,
    decode_fn: &str,
    type_map: &HashMap<String, Vec<IdlField>>,
) -> (String, Vec<(String, String)>) {
    let mut mod_rs = String::new();
    let mut item_files: Vec<(String, String)> = Vec::new();

    let has_fields = |item: &T| type_map.get(item.name()).is_some_and(|f| !f.is_empty());

    let with_fields: Vec<_> = items.iter().filter(|item| has_fields(item)).collect();
    let without_fields: Vec<_> = items.iter().filter(|item| !has_fields(item)).collect();

    // mod declarations for items with fields
    for item in &with_fields {
        let snake = pascal_to_snake(item.name());
        writeln!(mod_rs, "pub mod {};", snake).expect("write to String");
    }
    if !with_fields.is_empty() {
        mod_rs.push('\n');
        for item in &with_fields {
            let snake = pascal_to_snake(item.name());
            writeln!(mod_rs, "pub use {}::*;", snake).expect("write to String");
        }
        mod_rs.push('\n');
    }

    // Discriminator constants for fieldless items (in mod.rs)
    let kind_upper = kind.to_ascii_uppercase();
    for item in &without_fields {
        let base = disc_base_name(item.name(), kind);
        let const_name = pascal_to_screaming_snake(base);
        let disc_str = super::format_disc_decimal(item.discriminator());
        writeln!(
            mod_rs,
            "pub const {}_{}_DISCRIMINATOR: &[u8] = &[{}];",
            const_name, kind_upper, disc_str
        )
        .expect("write to String");
    }
    if !without_fields.is_empty() {
        mod_rs.push('\n');
    }

    // Enum
    writeln!(mod_rs, "pub enum {} {{", enum_name).expect("write to String");
    for item in items {
        if has_fields(item) {
            writeln!(mod_rs, "    {}({}),", item.name(), item.name()).expect("write to String");
        } else {
            writeln!(mod_rs, "    {},", item.name()).expect("write to String");
        }
    }
    mod_rs.push_str("}\n\n");

    // decode function
    writeln!(
        mod_rs,
        "pub fn {}(data: &[u8]) -> Option<{}> {{",
        decode_fn, enum_name
    )
    .expect("write to String");
    for item in items {
        let base = disc_base_name(item.name(), kind);
        let const_name = pascal_to_screaming_snake(base);
        writeln!(
            mod_rs,
            "    if data.starts_with({}_{}_DISCRIMINATOR) {{",
            const_name, kind_upper
        )
        .expect("write to String");
        if has_fields(item) {
            writeln!(
                mod_rs,
                "        return wincode::deserialize::<{}>(data).ok().map({}::{});",
                item.name(),
                enum_name,
                item.name()
            )
            .expect("write to String");
        } else {
            writeln!(
                mod_rs,
                "        return Some({}::{});",
                enum_name,
                item.name()
            )
            .expect("write to String");
        }
        mod_rs.push_str("    }\n");
    }
    mod_rs.push_str("    None\n");
    mod_rs.push_str("}\n");

    // Individual files
    for item in &with_fields {
        let snake = pascal_to_snake(item.name());
        let fields = type_map
            .get(item.name())
            .expect("invariant: with_fields only contains items present in type_map");
        let content =
            emit_single_state_or_event(item.name(), item.discriminator(), fields, kind, type_map);
        item_files.push((snake, content));
    }

    (mod_rs, item_files)
}

/// Strip "Event" suffix for event discriminator constant names to avoid stutter
/// (e.g. MakeEvent → MAKE, not MAKE_EVENT). Accounts keep their full name.
fn disc_base_name<'a>(name: &'a str, kind: &str) -> &'a str {
    if kind == "event" {
        name.strip_suffix("Event").unwrap_or(name)
    } else {
        name
    }
}

fn emit_single_state_or_event(
    name: &str,
    discriminator: &[u8],
    fields: &[IdlField],
    kind: &str,
    type_map: &HashMap<String, Vec<IdlField>>,
) -> String {
    let mut out = String::new();

    // Imports for manual impls
    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");
    out.push_str("use wincode::config::ConfigCore;\n");
    out.push_str("use wincode::error::{ReadError, ReadResult, WriteResult};\n");
    out.push_str("use wincode::io::{Reader, Writer};\n");
    out.push_str("use std::mem::MaybeUninit;\n");

    emit_field_imports(&mut out, fields.iter().map(|f| &f.ty), type_map);

    out.push('\n');

    // Discriminator constant
    let base = disc_base_name(name, kind);
    let const_name = pascal_to_screaming_snake(base);
    let kind_upper = kind.to_ascii_uppercase();
    let disc_str = super::format_disc_decimal(discriminator);
    writeln!(
        out,
        "pub const {}_{}_DISCRIMINATOR: &[u8] = &[{}];",
        const_name, kind_upper, disc_str
    )
    .expect("write to String");
    out.push('\n');

    // Struct + manual impls
    emit_manual_impls(&mut out, name, discriminator, fields, kind);

    out
}

// ===========================================================================
// types/
// ===========================================================================

fn emit_types(type_map: &HashMap<String, Vec<IdlField>>) -> (String, Vec<(String, String)>) {
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
    fields: &[IdlField],
    type_map: &HashMap<String, Vec<IdlField>>,
) -> String {
    let mut out = String::new();

    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");

    emit_field_imports(&mut out, fields.iter().map(|f| &f.ty), type_map);

    out.push('\n');

    out.push_str("#[derive(SchemaWrite, SchemaRead)]\n");
    writeln!(out, "pub struct {} {{", type_name).expect("write to String");
    for field in fields {
        writeln!(
            out,
            "    pub {}: {},",
            camel_to_snake(&field.name),
            rust_field_type(&field.ty)
        )
        .expect("write to String");
    }
    out.push_str("}\n");

    out
}

// ===========================================================================
// errors.rs
// ===========================================================================

fn emit_errors(idl: &Idl) -> String {
    let model = ProgramModel::new(idl);
    let mut out = String::new();

    let enum_name = format!("{}Error", snake_to_pascal(&model.identity.program_name));

    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    out.push_str("#[repr(u32)]\n");
    writeln!(out, "pub enum {} {{", enum_name).expect("write to String");
    for err in &idl.errors {
        writeln!(out, "    {} = {},", err.name, err.code).expect("write to String");
    }
    out.push_str("}\n\n");

    writeln!(out, "impl {} {{", enum_name).expect("write to String");

    // from_code
    out.push_str("    pub fn from_code(code: u32) -> Option<Self> {\n");
    out.push_str("        match code {\n");
    for err in &idl.errors {
        writeln!(out, "            {} => Some(Self::{}),", err.code, err.name)
            .expect("write to String");
    }
    out.push_str("            _ => None,\n");
    out.push_str("        }\n");
    out.push_str("    }\n\n");

    // message
    out.push_str("    pub fn message(&self) -> &'static str {\n");
    out.push_str("        match self {\n");
    for err in &idl.errors {
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

/// A collected PDA with its field name and seeds.
struct PdaInfo {
    field_name: String,
    seeds: Vec<IdlSeed>,
}

fn collect_pdas(idl: &Idl) -> Vec<PdaInfo> {
    let mut pdas: Vec<PdaInfo> = Vec::new();
    let mut seen_seeds: HashSet<Vec<IdlSeed>> = HashSet::new();

    for ix in &idl.instructions {
        for account in &ix.accounts {
            if let Some(pda) = &account.pda {
                if pda.seeds.is_empty() {
                    continue;
                }

                // Dedup by seed identity. When two instructions name the same
                // PDA differently, the first occurrence's field name wins.
                if !seen_seeds.insert(pda.seeds.clone()) {
                    continue;
                }

                pdas.push(PdaInfo {
                    field_name: camel_to_snake(&account.name),
                    seeds: pda.seeds.clone(),
                });
            }
        }
    }

    pdas
}

/// Format a const seed value for display (doc comments or code expressions).
fn format_const_seed_display(value: &[u8]) -> String {
    if value.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        format!("b\"{}\"", String::from_utf8_lossy(value))
    } else {
        let byte_list: Vec<String> = value.iter().map(|b| b.to_string()).collect();
        format!("&[{}]", byte_list.join(", "))
    }
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
                IdlSeed::Const { value } => format_const_seed_display(value),
                IdlSeed::Account { path } => camel_to_snake(path),
                IdlSeed::Arg { path } => format!("arg:{}", camel_to_snake(path)),
            })
            .collect();
        writeln!(out, "/// Seeds: [{}]", seed_desc.join(", ")).expect("write to String");

        // Function parameters
        let mut params: Vec<String> = Vec::new();
        for seed in &pda.seeds {
            match seed {
                IdlSeed::Account { path } => {
                    params.push(format!("{}: &Address", camel_to_snake(path)));
                }
                IdlSeed::Arg { path } => {
                    params.push(format!("{}: &[u8]", camel_to_snake(path)));
                }
                _ => {}
            }
        }
        params.push("program_id: &Address".to_string());

        let fn_name = format!("find_{}_address", pda.field_name);
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
                IdlSeed::Const { value } => format_const_seed_display(value),
                IdlSeed::Account { path } => format!("{}.as_ref()", camel_to_snake(path)),
                IdlSeed::Arg { path } => camel_to_snake(path),
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

/// Scan field types and emit wrapper imports (DynString, DynVec), Address
/// import, and defined type imports. Used by instruction, state/event, and type
/// emitters.
fn emit_field_imports<'a>(
    out: &mut String,
    types: impl Iterator<Item = &'a IdlType>,
    type_map: &HashMap<String, Vec<IdlField>>,
) {
    let mut needs_address = false;
    let mut needs_dyn_string = false;
    let mut needs_dyn_vec = false;
    for ty in types {
        collect_wrapper_needs(ty, &mut needs_dyn_string, &mut needs_dyn_vec);
        if field_needs_address(ty) {
            needs_address = true;
        }
        emit_type_use_imports(out, ty, type_map);
    }
    if needs_address {
        out.push_str("use solana_address::Address;\n");
    }
    emit_wrapper_imports(out, needs_dyn_string, needs_dyn_vec);
}

/// Emit struct definition + manual SchemaWrite/SchemaRead impls with
/// discriminator handling. Used for both accounts and events.
fn emit_manual_impls(
    out: &mut String,
    name: &str,
    discriminator: &[u8],
    idl_fields: &[IdlField],
    kind: &str,
) {
    let has_dynamic = idl_fields
        .iter()
        .any(|f| matches!(f.ty, IdlType::DynString { .. } | IdlType::DynVec { .. }));

    if has_dynamic {
        out.push_str("#[derive(Clone)]\n");
    } else {
        out.push_str("#[derive(Clone, Copy)]\n");
    }
    writeln!(out, "pub struct {} {{", name).expect("write to String");
    let fields: Vec<(String, String)> = idl_fields
        .iter()
        .map(|f| (camel_to_snake(&f.name), rust_field_type(&f.ty)))
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

    let base = disc_base_name(name, kind);
    let const_name = pascal_to_screaming_snake(base);
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
        let disc_str = super::format_disc_decimal(discriminator);
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

fn account_meta_expr(account: &IdlAccountItem) -> String {
    let field_name = camel_to_snake(&account.name);
    let signer = account.signer;
    if account.writable {
        format!("AccountMeta::new(ix.{}, {})", field_name, signer)
    } else {
        format!("AccountMeta::new_readonly(ix.{}, {})", field_name, signer)
    }
}

/// Map an `IdlType` to its Rust field type for the client struct.
fn rust_field_type(ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "pubkey" => "Address".to_string(),
            other => other.to_string(),
        },
        IdlType::Option { option } => format!("Option<{}>", rust_field_type(option)),
        IdlType::DynString { string } => prefix_generic("DynString", string.prefix_bytes),
        IdlType::DynVec { vec } => {
            let inner = rust_field_type(&vec.items);
            format!("DynVec<{}, {}>", inner, prefix_rust_type(vec.prefix_bytes))
        }
        IdlType::Defined { defined } => defined.clone(),
    }
}

fn prefix_generic(wrapper: &str, prefix_bytes: usize) -> String {
    format!("{}<{}>", wrapper, prefix_rust_type(prefix_bytes))
}

fn prefix_rust_type(prefix_bytes: usize) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        _ => "u64",
    }
}

fn collect_wrapper_needs(ty: &IdlType, needs_dyn_string: &mut bool, needs_dyn_vec: &mut bool) {
    match ty {
        IdlType::Option { option } => {
            collect_wrapper_needs(option, needs_dyn_string, needs_dyn_vec)
        }
        IdlType::DynString { .. } => *needs_dyn_string = true,
        IdlType::DynVec { vec } => {
            *needs_dyn_vec = true;
            collect_wrapper_needs(&vec.items, needs_dyn_string, needs_dyn_vec);
        }
        _ => {}
    }
}

fn field_needs_address(ty: &IdlType) -> bool {
    match ty {
        IdlType::Primitive(p) => p == "pubkey",
        IdlType::Option { option } => field_needs_address(option),
        IdlType::DynVec { vec } => field_needs_address(&vec.items),
        _ => false,
    }
}

fn emit_wrapper_imports(out: &mut String, needs_dyn_string: bool, needs_dyn_vec: bool) {
    let mut wrappers = Vec::new();
    if needs_dyn_string {
        wrappers.push("DynString");
    }
    if needs_dyn_vec {
        wrappers.push("DynVec");
    }
    if !wrappers.is_empty() {
        writeln!(out, "use quasar_lang::client::{{{}}};", wrappers.join(", "))
            .expect("write to String");
    }
}

fn emit_type_use_imports(
    out: &mut String,
    ty: &IdlType,
    type_map: &HashMap<String, Vec<IdlField>>,
) {
    match ty {
        IdlType::Defined { defined } if type_map.contains_key(defined) => {
            let import = format!("use crate::types::{};\n", defined);
            if !out.contains(&import) {
                out.push_str(&import);
            }
        }
        IdlType::Option { option } => emit_type_use_imports(out, option, type_map),
        IdlType::DynVec { vec } => emit_type_use_imports(out, &vec.items, type_map),
        _ => {}
    }
}
