use crate::parser::{accounts::RawAccountField, helpers, ParsedProgram};

/// Generate Rust client source code from parsed program data.
pub fn generate_client(parsed: &ParsedProgram) -> String {
    let mut out = String::new();

    out.push_str("use alloc::vec;\n");
    out.push_str("use solana_address::Address;\n");
    out.push_str("use solana_instruction::{AccountMeta, Instruction};\n\n");

    for ix in &parsed.instructions {
        let accounts_struct = parsed
            .accounts_structs
            .iter()
            .find(|s| s.name == ix.accounts_type_name);

        let struct_name = snake_to_pascal(&ix.name);

        // --- Struct definition ---
        out.push_str(&format!("pub struct {}Instruction {{\n", struct_name));

        // Account fields (all Address)
        if let Some(accs) = accounts_struct {
            for field in &accs.fields {
                out.push_str(&format!("    pub {}: Address,\n", field.name));
            }
        }

        // Instruction arg fields
        for (name, ty) in &ix.args {
            let type_name = helpers::simple_type_name(ty);
            out.push_str(&format!("    pub {}: {},\n", name, type_name));
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
        out.push_str("        let accounts = vec![\n");
        if let Some(accs) = accounts_struct {
            for field in &accs.fields {
                out.push_str(&format!("            {},\n", account_meta_expr(field)));
            }
        }
        out.push_str("        ];\n");

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
            for (name, ty) in &ix.args {
                let type_name = helpers::simple_type_name(ty);
                out.push_str(&format!("        {};\n", extend_expr(name, &type_name)));
            }
        }

        out.push_str("        Instruction {\n");
        out.push_str("            program_id: crate::ID,\n");
        out.push_str("            accounts,\n");
        out.push_str("            data,\n");
        out.push_str("        }\n");
        out.push_str("    }\n");
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

fn extend_expr(name: &str, ty: &str) -> String {
    match ty {
        "bool" => format!("data.push(ix.{} as u8)", name),
        "u8" | "i8" => format!("data.push(ix.{} as u8)", name),
        "u16" | "u32" | "u64" | "u128" | "i16" | "i32" | "i64" | "i128" => {
            format!("data.extend_from_slice(&ix.{}.to_le_bytes())", name)
        }
        // Address / Pubkey — 32 bytes
        "Address" | "Pubkey" => format!("data.extend_from_slice(ix.{}.as_ref())", name),
        // Unknown type — fall back to to_le_bytes (will fail to compile if wrong)
        _ => format!("data.extend_from_slice(&ix.{}.to_le_bytes())", name),
    }
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
