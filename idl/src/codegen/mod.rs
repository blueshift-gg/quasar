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
            c::generate_c_client,
            golang::generate_go_client,
            python::generate_python_client,
            rust::{
                generate_cargo_toml as generate_rust_cargo_toml,
                generate_client as generate_rust_client,
            },
            typescript::{generate_ts_client, generate_ts_client_kit},
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
                    optional: false,
                    writable: AccountFlag::Fixed(true),
                    signer: AccountFlag::Fixed(false),
                    resolver: IdlResolver::Pda {
                        program: IdlPdaProgram::ProgramId {},
                        seeds: vec![IdlPdaSeed::Arg {
                            path: "amount".to_owned(),
                            ty: IdlType::Primitive("u64".to_owned()),
                        }],
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
                remaining_accounts: None,
            }],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        }
    }

    fn idl_with_pubkey_arg() -> Idl {
        Idl {
            spec: "quasar-idl/1.0.0".to_owned(),
            name: "address_test".to_owned(),
            version: "0.1.0".to_owned(),
            address: "11111111111111111111111111111111".to_owned(),
            metadata: IdlMetadata::default(),
            docs: vec![],
            instructions: vec![IdlInstruction {
                name: "set_authority".to_owned(),
                discriminator: vec![9],
                docs: vec![],
                accounts: vec![],
                args: vec![IdlArg {
                    name: "authority".to_owned(),
                    ty: IdlType::Primitive("pubkey".to_owned()),
                    codec: None,
                    docs: vec![],
                }],
                layout: None,
                remaining_accounts: None,
            }],
            accounts: vec![],
            types: vec![],
            events: vec![],
            errors: vec![],
            extensions: None,
            hashes: None,
        }
    }

    fn idl_with_generic_arg() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        idl.instructions[0].args[0].ty = IdlType::Generic {
            generic: "T".to_owned(),
        };
        idl
    }

    #[test]
    fn go_pda_arg_seed_uses_typed_encoding() {
        let output = generate_go_client(&idl_with_u64_arg_seed()).unwrap();

        assert!(output.contains("binary.LittleEndian.PutUint64(b, input.Amount)"));
    }

    #[test]
    fn c_pda_arg_seed_uses_typed_encoding() {
        let output = generate_c_client(&idl_with_u64_arg_seed()).unwrap();

        assert!(output.contains("uint8_t arg_seed_0[8];"));
        assert!(output.contains("uint64_t arg_seed_0_value = (uint64_t)args->amount;"));
        assert!(output.contains("uint64_t meta_buf_capacity"));
        assert!(output.contains("SEED_TEST_IX_ACCOUNT_BUFFER_TOO_SMALL"));
        assert!(output.contains("SEED_TEST_IX_DATA_BUFFER_TOO_SMALL"));
        assert!(output.contains("uint64_t pda_status = find_program_address"));
        assert!(output.contains(".pda_status = pda_status"));
        assert!(!output.contains("sizeof(args->amount)"));
    }

    #[test]
    fn rust_client_manifest_pins_the_generator_version() {
        let manifest = generate_rust_cargo_toml("example", "1.2.3", false);

        assert!(manifest.contains(&format!("quasar-lang = \"={}\"", env!("CARGO_PKG_VERSION"))));
        assert!(!manifest.contains("git ="));
        assert!(!manifest.contains("branch ="));
    }

    #[test]
    fn rust_pda_arg_seed_uses_typed_encoding() {
        let files = generate_rust_client(&idl_with_u64_arg_seed()).unwrap();
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
        let output = generate_python_client(&idl_with_u64_arg_seed()).unwrap();

        assert!(output.contains("struct.pack(\"<Q\", input.amount)"));
    }

    #[test]
    fn typescript_address_codec_is_target_specific() {
        let web3js = generate_ts_client(&idl_with_pubkey_arg()).unwrap();
        let kit = generate_ts_client_kit(&idl_with_pubkey_arg()).unwrap();

        assert!(web3js.contains("function getWeb3jsAddressCodec()"));
        assert!(web3js.contains("[\"authority\", getWeb3jsAddressCodec()]"));
        assert!(!web3js.contains("function getAddressCodec()"));

        assert!(kit.contains("getAddressCodec"));
        assert!(kit.contains("[\"authority\", getAddressCodec()]"));
        assert!(!kit.contains("getWeb3jsAddressCodec()"));
    }

    #[test]
    fn unsupported_generics_are_errors_not_backend_panics() {
        let idl = idl_with_generic_arg();

        let c = std::panic::catch_unwind(|| generate_c_client(&idl));
        let go = std::panic::catch_unwind(|| generate_go_client(&idl));
        let python = std::panic::catch_unwind(|| generate_python_client(&idl));

        assert!(c.expect("C backend must not panic").is_err());
        assert!(go.expect("Go backend must not panic").is_err());
        assert!(python.expect("Python backend must not panic").is_err());
    }
}
