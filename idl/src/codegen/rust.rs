use {
    crate::{
        parser::{accounts::RawAccountField, helpers, ParsedProgram},
        types::IdlType,
    },
    std::{collections::HashMap, fmt::Write},
};

/// Generate Cargo.toml content for the standalone client crate.
pub fn generate_cargo_toml(name: &str, version: &str) -> String {
    format!(
        r#"[package]
name = "{name}-client"
version = "{version}"
edition = "2021"

[dependencies]
quasar-lang = "0.0"
wincode = {{ version = "0.4", features = ["derive"] }}
solana-address = "2"
solana-instruction = "3"
"#,
    )
}

/// Generate a standalone Rust client lib.rs from parsed program data.
pub fn generate_client(parsed: &ParsedProgram) -> String {
    let mut out = String::new();

    // Check if any instruction uses dynamic types or remaining accounts (need Vec
    // import)
    let has_dynamic = parsed.instructions.iter().any(|ix| {
        ix.args.iter().any(|(_, ty)| {
            matches!(
                helpers::map_type_from_syn(ty),
                IdlType::DynString { .. } | IdlType::DynVec { .. } | IdlType::Tail { .. }
            )
        })
    });
    let has_remaining = parsed.instructions.iter().any(|ix| ix.has_remaining);

    if has_dynamic || has_remaining {
        out.push_str("use std::vec;\nuse std::vec::Vec;\n");
    } else {
        out.push_str("use std::vec;\n");
    }
    if has_dynamic {
        out.push_str("use quasar_lang::client::{DynBytes, DynVec, TailBytes};\n");
    }
    out.push_str("use wincode::{SchemaWrite, SchemaRead};\n");
    out.push_str("use solana_address::Address;\n");
    out.push_str("use solana_instruction::{AccountMeta, Instruction};\n\n");

    // Program ID constant
    write!(
        out,
        "pub const ID: Address = solana_address::address!(\"{}\");\n\n",
        parsed.program_id
    )
    .expect("write to String");

    // Build type map for custom data types referenced in instruction args
    let type_map: HashMap<String, Vec<(String, IdlType)>> = {
        let mut map = HashMap::new();

        let mut referenced = std::collections::BTreeSet::new();
        for ix in &parsed.instructions {
            for (_, ty) in &ix.args {
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
    };

    // --- Custom data type definitions ---
    for (type_name, fields) in &type_map {
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
        out.push_str("}\n\n");
    }

    for ix in &parsed.instructions {
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

        // --- Struct definition ---
        writeln!(out, "pub struct {}Instruction {{", struct_name).expect("write to String");

        // Account fields (all Address)
        if let Some(accs) = accounts_struct {
            for field in &accs.fields {
                writeln!(out, "    pub {}: Address,", field.name).expect("write to String");
            }
        }

        // Instruction arg fields
        for (i, (name, _)) in ix.args.iter().enumerate() {
            writeln!(out, "    pub {}: {},", name, rust_field_type(&arg_types[i]))
                .expect("write to String");
        }

        // Remaining accounts field
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

        // Account metas
        if ix.has_remaining {
            out.push_str("        let mut accounts = vec![\n");
        } else {
            out.push_str("        let accounts = vec![\n");
        }
        if let Some(accs) = accounts_struct {
            for field in &accs.fields {
                writeln!(out, "            {},", account_meta_expr(field))
                    .expect("write to String");
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
                    "        data.extend_from_slice(&wincode::serialize(&ix.{}).unwrap());",
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
        out.push_str("}\n\n");
    }

    // --- Accounts ---
    if !parsed.state_accounts.is_empty() {
        // Account discriminator constants
        for acc in &parsed.state_accounts {
            let const_name = pascal_to_screaming_snake(&acc.name);
            let disc_str = format_disc_list(&acc.discriminator);
            writeln!(
                out,
                "pub const {}_ACCOUNT_DISCRIMINATOR: &[u8] = &[{}];",
                const_name, disc_str
            )
            .expect("write to String");
        }
        out.push('\n');

        // Account struct definitions (use original snake_case field names)
        for acc in &parsed.state_accounts {
            out.push_str("#[derive(Clone, Copy, SchemaWrite, SchemaRead)]\n#[repr(C)]\n");
            writeln!(out, "pub struct {} {{", acc.name).expect("write to String");
            for (field_name, field_ty) in &acc.fields {
                writeln!(
                    out,
                    "    pub {}: {},",
                    field_name,
                    rust_field_type(&helpers::map_type_from_syn(field_ty))
                )
                .expect("write to String");
            }
            out.push_str("}\n\n");
        }

        // ProgramAccount enum
        out.push_str("pub enum ProgramAccount {\n");
        for acc in &parsed.state_accounts {
            if acc.fields.is_empty() {
                writeln!(out, "    {},", acc.name).expect("write to String");
            } else {
                writeln!(out, "    {}({}),", acc.name, acc.name).expect("write to String");
            }
        }
        out.push_str("}\n\n");

        // decode_account function
        out.push_str("pub fn decode_account(data: &[u8]) -> Option<ProgramAccount> {\n");
        for acc in &parsed.state_accounts {
            let const_name = pascal_to_screaming_snake(&acc.name);
            writeln!(
                out,
                "    if data.starts_with({}_ACCOUNT_DISCRIMINATOR) {{",
                const_name
            )
            .expect("write to String");
            if acc.fields.is_empty() {
                writeln!(out, "        return Some(ProgramAccount::{});", acc.name)
                    .expect("write to String");
            } else {
                writeln!(
                    out,
                    "        let payload = &data[{}_ACCOUNT_DISCRIMINATOR.len()..];",
                    const_name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "        return \
                     wincode::deserialize::<{}>(payload).ok().map(ProgramAccount::{});",
                    acc.name, acc.name
                )
                .expect("write to String");
            }
            out.push_str("    }\n");
        }
        out.push_str("    None\n");
        out.push_str("}\n\n");
    }

    // --- Events ---
    if !parsed.events.is_empty() {
        // Event discriminator constants
        for ev in &parsed.events {
            let const_name = pascal_to_screaming_snake(&ev.name);
            let disc_str = format_disc_list(&ev.discriminator);
            writeln!(
                out,
                "pub const {}_EVENT_DISCRIMINATOR: &[u8] = &[{}];",
                const_name, disc_str
            )
            .expect("write to String");
        }
        out.push('\n');

        // Event struct definitions (use original snake_case field names)
        for ev in &parsed.events {
            out.push_str("#[derive(SchemaWrite, SchemaRead)]\n");
            writeln!(out, "pub struct {} {{", ev.name).expect("write to String");
            for (field_name, field_ty) in &ev.fields {
                writeln!(
                    out,
                    "    pub {}: {},",
                    field_name,
                    rust_field_type(&helpers::map_type_from_syn(field_ty))
                )
                .expect("write to String");
            }
            out.push_str("}\n\n");
        }

        // Event enum
        out.push_str("pub enum ProgramEvent {\n");
        for ev in &parsed.events {
            if ev.fields.is_empty() {
                writeln!(out, "    {},", ev.name).expect("write to String");
            } else {
                writeln!(out, "    {}({}),", ev.name, ev.name).expect("write to String");
            }
        }
        out.push_str("}\n\n");

        // decode_event function
        out.push_str("pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {\n");
        for ev in &parsed.events {
            let const_name = pascal_to_screaming_snake(&ev.name);
            writeln!(
                out,
                "    if data.starts_with({}_EVENT_DISCRIMINATOR) {{",
                const_name
            )
            .expect("write to String");
            if ev.fields.is_empty() {
                writeln!(out, "        return Some(ProgramEvent::{});", ev.name)
                    .expect("write to String");
            } else {
                writeln!(
                    out,
                    "        let payload = &data[{}_EVENT_DISCRIMINATOR.len()..];",
                    const_name
                )
                .expect("write to String");
                writeln!(
                    out,
                    "        return \
                     wincode::deserialize::<{}>(payload).ok().map(ProgramEvent::{});",
                    ev.name, ev.name
                )
                .expect("write to String");
            }
            out.push_str("    }\n");
        }
        out.push_str("    None\n");
        out.push_str("}\n\n");
    }

    out
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
        IdlType::Tail { .. } => "TailBytes".to_string(),
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
