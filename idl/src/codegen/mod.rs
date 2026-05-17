pub mod c;
pub mod golang;
pub mod model;
pub mod python;
pub mod rust;
pub mod typescript;

/// Parse the size from a fixed-size array primitive like `"[u8; 8]"`.
pub fn parse_fixed_array_size(p: &str) -> Option<usize> {
    let inner = p.strip_prefix('[')?.strip_suffix(']')?;
    let (_, size_str) = inner.split_once(';')?;
    size_str.trim().parse().ok()
}

/// Format discriminator bytes as a decimal comma-separated list (no brackets).
pub fn format_disc_decimal(disc: &[u8]) -> String {
    disc.iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format discriminator bytes as a hex comma-separated list (no brackets).
pub fn format_disc_hex(disc: &[u8]) -> String {
    disc.iter()
        .map(|b| format!("0x{:02x}", b))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Format discriminator bytes as a decimal array with brackets: `[1, 2, 3]`.
pub fn format_disc_array(disc: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(disc.len() * 4 + 2);
    s.push('[');
    for (i, b) in disc.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write!(s, "{}", b).expect("write to String");
    }
    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use {
        super::{
            c::generate_c_client, golang::generate_go_client, python::generate_python_client,
            rust::generate_client as generate_rust_client,
        },
        crate::types::{
            AccountFlag, Idl, IdlAccountNode, IdlArg, IdlInstruction, IdlMetadata, IdlPdaProgram,
            IdlPdaSeed, IdlResolver, IdlType,
        },
    };

    fn idl_with_u64_arg_seed() -> Idl {
        Idl {
            spec: "quasar-idl/1.0.0".to_owned(),
            name: "seed_test".to_owned(),
            version: "0.1.0".to_owned(),
            address: "11111111111111111111111111111111".to_owned(),
            metadata: IdlMetadata::default(),
            docs: vec![],
            instructions: vec![IdlInstruction {
                name: "create".to_owned(),
                discriminator: vec![7],
                docs: vec![],
                accounts: vec![IdlAccountNode {
                    name: "vault".to_owned(),
                    client_type: None,
                    writable: AccountFlag::Fixed(true),
                    signer: AccountFlag::Fixed(false),
                    resolver: IdlResolver::Pda {
                        program: IdlPdaProgram::ProgramId {},
                        seeds: vec![IdlPdaSeed::Arg {
                            path: "amount".to_owned(),
                            ty: IdlType::Primitive("u64".to_owned()),
                            encoding: None,
                        }],
                        bump: None,
                    },
                    docs: vec![],
                }],
                args: vec![IdlArg {
                    name: "amount".to_owned(),
                    ty: IdlType::Primitive("u64".to_owned()),
                    codec: None,
                    docs: vec![],
                }],
                layout: None,
                returns: None,
                effects: vec![],
                remaining_accounts: None,
            }],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            constants: vec![],
            wrappers: None,
            extensions: None,
            hashes: None,
        }
    }

    #[test]
    fn go_pda_arg_seed_uses_typed_encoding() {
        let output = generate_go_client(&idl_with_u64_arg_seed());

        assert!(output.contains("binary.LittleEndian.PutUint64(b, input.Amount)"));
    }

    #[test]
    fn c_pda_arg_seed_uses_typed_encoding() {
        let output = generate_c_client(&idl_with_u64_arg_seed());

        assert!(output.contains("uint8_t arg_seed_0[8];"));
        assert!(output.contains("uint64_t arg_seed_0_value = (uint64_t)args->amount;"));
        assert!(!output.contains("sizeof(args->amount)"));
    }

    #[test]
    fn rust_pda_arg_seed_uses_typed_encoding() {
        let files = generate_rust_client(&idl_with_u64_arg_seed());
        let pda_rs = files
            .iter()
            .find_map(|(path, contents)| (path == "pda.rs").then_some(contents))
            .expect("pda.rs generated");

        assert!(pda_rs.contains("pub fn find_vault_address(amount: u64, program_id: &Address)"));
        assert!(pda_rs.contains("let amount_seed = amount.to_le_bytes();"));
        assert!(pda_rs.contains("Address::find_program_address(&[amount_seed.as_ref()]"));
    }

    #[test]
    fn python_pda_arg_seed_uses_typed_encoding() {
        let output = generate_python_client(&idl_with_u64_arg_seed());

        assert!(output.contains("struct.pack(\"<Q\", input.amount)"));
    }
}
