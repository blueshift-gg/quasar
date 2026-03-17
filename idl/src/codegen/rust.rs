use {
    crate::{
        parser::{accounts::RawAccountField, helpers, ParsedProgram},
        types::IdlType,
    },
    std::fmt::Write,
};

/// Generate Cargo.toml content for the standalone client crate.
pub fn generate_cargo_toml(name: &str, version: &str) -> String {
    format!(
        r#"[package]
name = "{name}-client"
version = "{version}"
edition = "2021"

[dependencies]
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
    out.push_str("use solana_address::Address;\n");
    out.push_str("use solana_instruction::{AccountMeta, Instruction};\n\n");

    // Program ID constant
    write!(
        out,
        "pub const ID: Address = solana_address::address!(\"{}\");\n\n",
        parsed.program_id
    )
    .expect("write to String");

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
            for (i, (name, _)) in ix.args.iter().enumerate() {
                out.push_str(&serialize_expr(name, &arg_types[i]));
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

    // --- Events ---
    if !parsed.events.is_empty() {
        // Build IDL type defs for events (to get field info)
        let event_types: Vec<_> = parsed
            .events
            .iter()
            .map(crate::parser::events::to_idl_type_def)
            .collect();

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

        // Event struct definitions
        for type_def in &event_types {
            writeln!(out, "pub struct {} {{", type_def.name).expect("write to String");
            for field in &type_def.ty.fields {
                writeln!(
                    out,
                    "    pub {}: {},",
                    field.name,
                    rust_field_type(&field.ty)
                )
                .expect("write to String");
            }
            out.push_str("}\n\n");
        }

        // Event enum
        out.push_str("pub enum ProgramEvent {\n");
        for type_def in &event_types {
            if type_def.ty.fields.is_empty() {
                writeln!(out, "    {},", type_def.name).expect("write to String");
            } else {
                writeln!(out, "    {}({}),", type_def.name, type_def.name)
                    .expect("write to String");
            }
        }
        out.push_str("}\n\n");

        // decode_event function
        out.push_str("pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {\n");
        for (i, ev) in parsed.events.iter().enumerate() {
            let const_name = pascal_to_screaming_snake(&ev.name);
            let type_def = &event_types[i];
            writeln!(
                out,
                "    if data.starts_with({}_EVENT_DISCRIMINATOR) {{",
                const_name
            )
            .expect("write to String");
            if type_def.ty.fields.is_empty() {
                writeln!(out, "        return Some(ProgramEvent::{});", type_def.name)
                    .expect("write to String");
            } else {
                writeln!(
                    out,
                    "        let data = &data[{}_EVENT_DISCRIMINATOR.len()..];",
                    const_name
                )
                .expect("write to String");
                out.push_str("        let mut offset = 0usize;\n");
                for field in &type_def.ty.fields {
                    out.push_str(&deserialize_field_expr(&field.name, &field.ty));
                }
                let field_names: Vec<&str> =
                    type_def.ty.fields.iter().map(|f| f.name.as_str()).collect();
                writeln!(
                    out,
                    "        return Some(ProgramEvent::{}({} {{ {} }}));",
                    type_def.name,
                    type_def.name,
                    field_names.join(", ")
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
        IdlType::DynString { .. } => "Vec<u8>".to_string(),
        IdlType::DynVec { vec } => {
            format!("Vec<{}>", rust_field_type(&vec.items))
        }
        IdlType::Defined { defined } => defined.clone(),
        IdlType::Tail { .. } => "Vec<u8>".to_string(),
    }
}

/// Generate serialization code for an instruction argument.
fn serialize_expr(name: &str, ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "bool" => format!("        data.push(ix.{} as u8);\n", name),
            "u8" => format!("        data.push(ix.{});\n", name),
            "i8" => format!("        data.push(ix.{} as u8);\n", name),
            "publicKey" => {
                format!("        data.extend_from_slice(ix.{}.as_ref());\n", name)
            }
            _ => format!(
                "        data.extend_from_slice(&ix.{}.to_le_bytes());\n",
                name
            ),
        },
        IdlType::DynString { .. } => {
            format!(
                "        data.extend_from_slice(&(ix.{n}.len() as u32).to_le_bytes());\n\
                 \x20       data.extend_from_slice(&ix.{n});\n",
                n = name,
            )
        }
        IdlType::DynVec { vec } => {
            let item_ser = match &*vec.items {
                IdlType::Primitive(p) if p == "publicKey" => "__item.as_ref()".to_string(),
                IdlType::Primitive(p) if p == "u8" || p == "i8" || p == "bool" => {
                    "&[*__item as u8]".to_string()
                }
                IdlType::Primitive(_) => "&__item.to_le_bytes()".to_string(),
                _ => "__item.as_ref()".to_string(),
            };
            format!(
                "        data.extend_from_slice(&(ix.{n}.len() as u32).to_le_bytes());\n\
                 \x20       for __item in &ix.{n} {{ data.extend_from_slice({ser}); }}\n",
                n = name,
                ser = item_ser,
            )
        }
        IdlType::Defined { .. } => format!(
            "        data.extend_from_slice(&ix.{}.to_le_bytes());\n",
            name
        ),
        IdlType::Tail { .. } => {
            format!("        data.extend_from_slice(&ix.{});\n", name)
        }
    }
}

/// Generate deserialization code for a single event field (reads from `data` at
/// `offset`).
fn deserialize_field_expr(name: &str, ty: &IdlType) -> String {
    match ty {
        IdlType::Primitive(p) => match p.as_str() {
            "bool" => format!(
                "        let {} = data[offset] != 0;\n        offset += 1;\n",
                name
            ),
            "u8" => format!(
                "        let {} = data[offset];\n        offset += 1;\n",
                name
            ),
            "i8" => format!(
                "        let {} = data[offset] as i8;\n        offset += 1;\n",
                name
            ),
            "publicKey" => format!(
                "        let {n} = Address::from(<[u8; 32]>::try_from(&data[offset..offset + \
                 32]).ok()?);\n\x20       offset += 32;\n",
                n = name,
            ),
            other => {
                let size = primitive_size(other);
                format!(
                    "        let {n} = {ty}::from_le_bytes(data[offset..offset + \
                     {sz}].try_into().ok()?);\n\x20       offset += {sz};\n",
                    n = name,
                    ty = other,
                    sz = size,
                )
            }
        },
        _ => format!(
            "        let {} = Default::default(); // unsupported type\n",
            name
        ),
    }
}

fn primitive_size(p: &str) -> usize {
    match p {
        "u8" | "i8" | "bool" => 1,
        "u16" | "i16" => 2,
        "u32" | "i32" => 4,
        "u64" | "i64" => 8,
        "u128" | "i128" => 16,
        "publicKey" => 32,
        _ => 0,
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
