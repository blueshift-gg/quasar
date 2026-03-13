use crate::{
    parser::{accounts::RawAccountField, helpers, ParsedProgram},
    types::IdlType,
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
    out.push_str(&format!(
        "pub const ID: Address = solana_address::address!(\"{}\");\n\n",
        parsed.program_id
    ));

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
        out.push_str(&format!("pub struct {}Instruction {{\n", struct_name));

        // Account fields (all Address)
        if let Some(accs) = accounts_struct {
            for field in &accs.fields {
                out.push_str(&format!("    pub {}: Address,\n", field.name));
            }
        }

        // Instruction arg fields
        for (i, (name, _)) in ix.args.iter().enumerate() {
            out.push_str(&format!(
                "    pub {}: {},\n",
                name,
                rust_field_type(&arg_types[i])
            ));
        }

        // Remaining accounts field
        if ix.has_remaining {
            out.push_str("    pub remaining_accounts: Vec<AccountMeta>,\n");
        }

        out.push_str("}\n\n");

        // --- From impl ---
        out.push_str(&format!(
            "impl From<{}Instruction> for Instruction {{\n",
            struct_name
        ));
        out.push_str(&format!(
            "    fn from(ix: {}Instruction) -> Instruction {{\n",
            struct_name
        ));

        // Account metas
        if ix.has_remaining {
            out.push_str("        let mut accounts = vec![\n");
        } else {
            out.push_str("        let accounts = vec![\n");
        }
        if let Some(accs) = accounts_struct {
            for field in &accs.fields {
                out.push_str(&format!("            {},\n", account_meta_expr(field)));
            }
        }
        out.push_str("        ];\n");
        if ix.has_remaining {
            out.push_str("        accounts.extend(ix.remaining_accounts);\n");
        }

        // Instruction data
        let disc_bytes: Vec<String> = ix.discriminator.iter().map(|b| b.to_string()).collect();

        if ix.args.is_empty() {
            out.push_str(&format!(
                "        let data = vec![{}];\n",
                disc_bytes.join(", ")
            ));
        } else {
            out.push_str(&format!(
                "        let mut data = vec![{}];\n",
                disc_bytes.join(", ")
            ));
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
            let disc_bytes: Vec<String> = ev.discriminator.iter().map(|b| b.to_string()).collect();
            out.push_str(&format!(
                "pub const {}_EVENT_DISCRIMINATOR: &[u8] = &[{}];\n",
                const_name,
                disc_bytes.join(", ")
            ));
        }
        out.push('\n');

        // Event struct definitions
        for type_def in &event_types {
            out.push_str(&format!("pub struct {} {{\n", type_def.name));
            for field in &type_def.ty.fields {
                out.push_str(&format!(
                    "    pub {}: {},\n",
                    field.name,
                    rust_field_type(&field.ty)
                ));
            }
            out.push_str("}\n\n");
        }

        // Event enum
        out.push_str("pub enum ProgramEvent {\n");
        for type_def in &event_types {
            if type_def.ty.fields.is_empty() {
                out.push_str(&format!("    {},\n", type_def.name));
            } else {
                out.push_str(&format!("    {}({}),\n", type_def.name, type_def.name));
            }
        }
        out.push_str("}\n\n");

        // decode_event function
        out.push_str("pub fn decode_event(data: &[u8]) -> Option<ProgramEvent> {\n");
        for (i, ev) in parsed.events.iter().enumerate() {
            let const_name = pascal_to_screaming_snake(&ev.name);
            let type_def = &event_types[i];
            out.push_str(&format!(
                "    if data.starts_with({}_EVENT_DISCRIMINATOR) {{\n",
                const_name
            ));
            if type_def.ty.fields.is_empty() {
                out.push_str(&format!(
                    "        return Some(ProgramEvent::{});\n",
                    type_def.name
                ));
            } else {
                out.push_str(&format!(
                    "        let data = &data[{}_EVENT_DISCRIMINATOR.len()..];\n",
                    const_name
                ));
                out.push_str("        let mut offset = 0usize;\n");
                for field in &type_def.ty.fields {
                    out.push_str(&deserialize_field_expr(&field.name, &field.ty));
                }
                let field_names: Vec<&str> =
                    type_def.ty.fields.iter().map(|f| f.name.as_str()).collect();
                out.push_str(&format!(
                    "        return Some(ProgramEvent::{}({} {{ {} }}));\n",
                    type_def.name,
                    type_def.name,
                    field_names.join(", ")
                ));
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

fn pascal_to_screaming_snake(s: &str) -> String {
    let mut result = String::new();
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
