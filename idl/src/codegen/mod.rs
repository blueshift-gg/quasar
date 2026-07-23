pub mod c;
pub mod golang;
pub mod model;
mod naming;
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
            AccountFlag, Idl, IdlAccountNode, IdlArg, IdlErrorDef, IdlInstruction, IdlMetadata,
            IdlPdaProgram, IdlPdaSeed, IdlResolver, IdlType,
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

    fn idl_with_snake_case_instruction() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        idl.instructions[0].name = "execute_transfer".to_owned();
        idl
    }

    fn idl_with_arg_and_account_field_recipes_for_one_pda() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        idl.types.push(
            serde_json::from_value(serde_json::json!({
                "name": "Escrow",
                "kind": "struct",
                "fields": [{ "name": "amount", "type": "u64" }]
            }))
            .unwrap(),
        );
        idl.instructions[0].accounts[0].name = "escrow".to_owned();
        let mut account_field_instruction = idl.instructions[0].clone();
        account_field_instruction.name = "take".to_owned();
        account_field_instruction.args.clear();
        account_field_instruction.accounts[0].resolver = IdlResolver::Pda {
            program: IdlPdaProgram::ProgramId {},
            seeds: vec![IdlPdaSeed::AccountField {
                path: "escrow".to_owned(),
                account: "Escrow".to_owned(),
                field: "amount".to_owned(),
            }],
        };
        idl.instructions.push(account_field_instruction);
        idl
    }

    fn idl_with_bare_account_field_seed() -> Idl {
        // A single instruction whose only PDA is seeded by the stored `seed`
        // field of `escrow`. Nothing else shares that name, so the synthesized
        // input is the bare `seed` (the escrow Take/Refund shape).
        let mut idl = idl_with_u64_arg_seed();
        idl.instructions[0].name = "take".to_owned();
        idl.instructions[0].args.clear();
        idl.instructions[0].accounts[0].name = "escrow".to_owned();
        idl.instructions[0].accounts[0].resolver = IdlResolver::Pda {
            program: IdlPdaProgram::ProgramId {},
            seeds: vec![IdlPdaSeed::AccountField {
                path: "escrow".to_owned(),
                account: "Escrow".to_owned(),
                field: "seed".to_owned(),
            }],
        };
        idl.types.push(
            serde_json::from_value(serde_json::json!({
                "name": "Escrow",
                "kind": "struct",
                "fields": [{ "name": "seed", "type": "u64" }]
            }))
            .unwrap(),
        );
        idl
    }

    fn idl_with_colliding_account_field_seed() -> Idl {
        // The same instruction also takes a `seed` arg, so the stored-field
        // input must escalate to `escrow_seed`.
        let mut idl = idl_with_bare_account_field_seed();
        idl.instructions[0].args.push(IdlArg {
            name: "seed".to_owned(),
            ty: IdlType::Primitive("u64".to_owned()),
            codec: None,
            docs: vec![],
        });
        idl
    }

    fn idl_with_fixed_and_dynamic_accounts() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        // A fixed-layout account: pubkey + u64 → 40-byte body.
        idl.accounts.push(
            serde_json::from_value(serde_json::json!({
                "name": "Config", "discriminator": [9]
            }))
            .unwrap(),
        );
        idl.types.push(
            serde_json::from_value(serde_json::json!({
                "name": "Config",
                "kind": "struct",
                "fields": [
                    { "name": "authority", "type": "pubkey" },
                    { "name": "count", "type": "u64" }
                ]
            }))
            .unwrap(),
        );
        // A variable-layout account: a size-prefixed string body.
        idl.accounts.push(
            serde_json::from_value(serde_json::json!({
                "name": "Registry", "discriminator": [10]
            }))
            .unwrap(),
        );
        idl.types.push(
            serde_json::from_value(serde_json::json!({
                "name": "Registry",
                "kind": "struct",
                "fields": [{
                    "name": "label",
                    "type": "string",
                    "codec": {
                        "kind": "sizePrefixed",
                        "prefix": { "type": "u8", "endian": "le" },
                        "storage": "tail",
                        "maxBytes": 32,
                        "encoding": "utf8"
                    }
                }]
            }))
            .unwrap(),
        );
        idl
    }

    fn idl_with_associated_token() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        idl.instructions[0].accounts = vec![
            IdlAccountNode {
                name: "owner".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(false),
                signer: AccountFlag::Fixed(true),
                resolver: IdlResolver::Input {},
                docs: vec![],
            },
            IdlAccountNode {
                name: "mint".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(false),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Input {},
                docs: vec![],
            },
            IdlAccountNode {
                name: "tokenProgram".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(false),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Const {
                    address: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_owned(),
                },
                docs: vec![],
            },
            IdlAccountNode {
                name: "ownerTokens".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(true),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::AssociatedToken {
                    mint: "mint".to_owned(),
                    owner: "owner".to_owned(),
                    token_program: Some("tokenProgram".to_owned()),
                },
                docs: vec![],
            },
        ];
        idl
    }

    fn idl_with_out_of_order_external_program_pdas() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        let instruction = &mut idl.instructions[0];
        instruction.accounts = vec![
            IdlAccountNode {
                name: "child".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(true),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Pda {
                    program: IdlPdaProgram::Account {
                        path: "tokenProgram".to_owned(),
                    },
                    seeds: vec![IdlPdaSeed::Account {
                        path: "parent".to_owned(),
                    }],
                },
                docs: vec![],
            },
            IdlAccountNode {
                name: "tokenProgram".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(false),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Const {
                    address: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA".to_owned(),
                },
                docs: vec![],
            },
            IdlAccountNode {
                name: "parent".to_owned(),
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
            },
        ];
        idl
    }

    fn idl_with_optional_pda_dependencies() -> Idl {
        let mut idl = idl_with_u64_arg_seed();
        idl.instructions[0].accounts = vec![
            IdlAccountNode {
                name: "child".to_owned(),
                optional: false,
                writable: AccountFlag::Fixed(true),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Pda {
                    program: IdlPdaProgram::Account {
                        path: "maybeProgram".to_owned(),
                    },
                    seeds: vec![IdlPdaSeed::Account {
                        path: "maybeSeed".to_owned(),
                    }],
                },
                docs: vec![],
            },
            IdlAccountNode {
                name: "maybeSeed".to_owned(),
                optional: true,
                writable: AccountFlag::Fixed(false),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Input {},
                docs: vec![],
            },
            IdlAccountNode {
                name: "maybeProgram".to_owned(),
                optional: true,
                writable: AccountFlag::Fixed(false),
                signer: AccountFlag::Fixed(false),
                resolver: IdlResolver::Input {},
                docs: vec![],
            },
        ];
        idl
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
        assert!(output.contains("SEED_TEST_IX_PDA_KEY_BUFFER_TOO_SMALL"));
        assert!(output.contains("uint64_t pda_keys_len;"));
        assert!(output.contains("Pubkey *pda_key_buf"));
        assert!(output.contains("uint64_t pda_key_buf_capacity"));
        assert!(output.contains("uint64_t pda_status = find_program_address"));
        assert!(output.contains("&derived_pda_keys[0]"));
        assert!(output.contains("meta_buf[0] = meta_writable(&pda_key_buf[0]);"));
        assert!(output.contains(".pda_status = pda_status"));
        assert!(!output.contains("sizeof(args->amount)"));
    }

    #[test]
    fn c_non_pda_builder_does_not_require_pda_storage() {
        let output = generate_c_client(&idl_with_pubkey_arg()).unwrap();

        assert!(!output.contains("Pubkey *pda_key_buf"));
        assert!(!output.contains("uint64_t pda_key_buf_capacity"));
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
    fn rust_pda_helpers_dedupe_by_name_and_prefer_typed_arguments() {
        let files =
            generate_rust_client(&idl_with_arg_and_account_field_recipes_for_one_pda()).unwrap();
        let pda_rs = files
            .iter()
            .find_map(|(path, contents)| (path == "pda.rs").then_some(contents))
            .expect("pda.rs generated");

        assert_eq!(pda_rs.matches("pub fn find_escrow_address").count(), 1);
        assert!(pda_rs.contains("find_escrow_address(amount: u64, program_id: &Address)"));
        assert!(!pda_rs.contains("escrow_amount_seed: &[u8]"));
    }

    #[test]
    fn account_field_seed_input_uses_bare_field_name_when_unambiguous() {
        let idl = idl_with_bare_account_field_seed();

        let take = generate_rust_client(&idl)
            .unwrap()
            .into_iter()
            .find_map(|(path, contents)| (path == "instructions/take.rs").then_some(contents))
            .expect("take instruction generated");
        assert!(take.contains("pub seed: u64,"));
        assert!(!take.contains("escrow_seed_seed"));
        assert!(take.contains("ix.seed"));

        let go = generate_go_client(&idl).unwrap();
        assert!(go.contains("Seed uint64"));
        assert!(go.contains("input.Seed"));
        assert!(!go.contains("EscrowSeedSeed"));

        let python = generate_python_client(&idl).unwrap();
        assert!(python.contains("input.seed"));
        assert!(!python.contains("escrow_seed_seed"));

        let c = generate_c_client(&idl).unwrap();
        assert!(c.contains("const uint8_t *seed;"));
        assert!(c.contains("accounts->seed;"));
        assert!(!c.contains("escrow_seed_seed"));
    }

    #[test]
    fn ts_account_codec_bundle_carries_framing_and_omits_size_when_variable() {
        let idl = idl_with_fixed_and_dynamic_accounts();

        for ts in [
            generate_ts_client(&idl).unwrap(),
            generate_ts_client_kit(&idl).unwrap(),
        ] {
            // Fixed account: full bundle incl. size (disc 1 + pubkey 32 + u64 8).
            assert!(ts.contains("export const ConfigAccount = {"));
            assert!(ts.contains("decode: ConfigCodec.decode,"));
            assert!(ts.contains("encode: ConfigCodec.encode,"));
            assert!(ts.contains("discriminator: CONFIG_DISCRIMINATOR,"));
            assert!(ts.contains("size: 41,"));

            // Variable account: bundle present, but no size.
            assert!(ts.contains("export const RegistryAccount = {"));
            assert!(ts.contains("decode: RegistryCodec.decode,"));
            let registry = ts
                .split("export const RegistryAccount = {")
                .nth(1)
                .expect("registry bundle")
                .split("};")
                .next()
                .expect("registry bundle body");
            assert!(!registry.contains("size:"));
        }

        // Owner is spelled per target.
        assert!(generate_ts_client_kit(&idl)
            .unwrap()
            .contains("owner: PROGRAM_ADDRESS,"));
        assert!(generate_ts_client(&idl)
            .unwrap()
            .contains("owner: new Address(\"11111111111111111111111111111111\"),"));
    }

    #[test]
    fn account_field_seed_input_escalates_to_base_field_on_collision() {
        let idl = idl_with_colliding_account_field_seed();

        let take = generate_rust_client(&idl)
            .unwrap()
            .into_iter()
            .find_map(|(path, contents)| (path == "instructions/take.rs").then_some(contents))
            .expect("take instruction generated");
        // The `seed` arg keeps its name; the stored-field input escalates.
        assert!(take.contains("pub escrow_seed: u64,"));
        assert!(take.contains("pub seed: u64,"));
        assert!(take.contains("ix.escrow_seed"));

        let go = generate_go_client(&idl).unwrap();
        assert!(go.contains("EscrowSeed uint64"));
        assert!(go.contains("input.EscrowSeed"));

        let python = generate_python_client(&idl).unwrap();
        assert!(python.contains("input.escrow_seed"));

        let c = generate_c_client(&idl).unwrap();
        assert!(c.contains("const uint8_t *escrow_seed;"));
    }

    #[test]
    fn rust_instruction_resolves_pdas_and_keeps_a_raw_escape_hatch() {
        let files = generate_rust_client(&idl_with_u64_arg_seed()).unwrap();
        let instruction_rs = files
            .iter()
            .find_map(|(path, contents)| (path == "instructions/create.rs").then_some(contents))
            .expect("create instruction generated");

        assert!(instruction_rs.contains("pub struct CreateInstructionRaw {"));
        assert!(instruction_rs.contains("pub vault: Address,"));
        assert!(instruction_rs.contains("pub struct CreateInstruction {"));
        assert!(!instruction_rs
            .split("pub struct CreateInstruction {")
            .nth(1)
            .expect("resolved instruction body")
            .split('}')
            .next()
            .expect("resolved instruction fields")
            .contains("pub vault:"));
        assert!(instruction_rs.contains("ix.amount.to_le_bytes().as_ref()"));
        assert!(instruction_rs.contains("impl From<CreateInstruction> for Instruction"));
    }

    #[test]
    fn generated_clients_order_pda_dependencies_and_resolve_external_programs() {
        let idl = idl_with_out_of_order_external_program_pdas();
        let files = generate_rust_client(&idl).unwrap();
        let instruction_rs = files
            .iter()
            .find_map(|(path, contents)| (path == "instructions/create.rs").then_some(contents))
            .expect("create instruction generated");

        let const_program = instruction_rs
            .find("let token_program = solana_address::address!")
            .expect("constant program binding");
        let parent = instruction_rs
            .find("let parent = Address::find_program_address")
            .expect("parent PDA derivation");
        let child = instruction_rs
            .find("let child = Address::find_program_address")
            .expect("child PDA derivation");
        assert!(const_program < parent && parent < child);
        assert!(instruction_rs.contains("parent.as_ref()"));
        assert!(instruction_rs.contains("&token_program"));

        for typescript in [
            generate_ts_client(&idl).unwrap(),
            generate_ts_client_kit(&idl).unwrap(),
        ] {
            let const_program = typescript
                .find("accountsMap[\"tokenProgram\"] =")
                .expect("constant program binding");
            let parent = typescript
                .find("accountsMap[\"parent\"] =")
                .expect("parent PDA derivation");
            let child = typescript
                .find("accountsMap[\"child\"] =")
                .expect("child PDA derivation");
            assert!(const_program < parent && parent < child);
            assert!(typescript.contains("accountsMap[\"parent\"]"));
            assert!(typescript.contains("accountsMap[\"tokenProgram\"]"));
            assert!(typescript.contains("export async function findParentAddress"));
            assert!(!typescript.contains("export async function findChildAddress"));
        }

        let python = generate_python_client(&idl).unwrap();
        let python_parent = python
            .find("accounts_map[\"parent\"] =")
            .expect("Python parent PDA derivation");
        let python_child = python
            .find("accounts_map[\"child\"] =")
            .expect("Python child PDA derivation");
        assert!(python_parent < python_child);
        assert!(python.contains(
            "Pubkey.find_program_address([bytes(accounts_map[\"parent\"])], \
             accounts_map[\"tokenProgram\"])[0]"
        ));

        let go = generate_go_client(&idl).unwrap();
        let go_parent = go
            .find("accountsMap[\"parent\"] =")
            .expect("Go parent PDA derivation");
        let go_child = go
            .find("accountsMap[\"child\"] =")
            .expect("Go child PDA derivation");
        assert!(go_parent < go_child);
        assert!(go.contains("accountsMap[\"tokenProgram\"]"));

        let c = generate_c_client(&idl).unwrap();
        let c_parent = c
            .find("&derived_pda_keys[1]")
            .expect("C parent PDA derivation");
        let c_child = c
            .rfind("&derived_pda_keys[0]")
            .expect("C child PDA derivation");
        assert!(c_parent < c_child);
        assert!(c.contains("derived_pda_keys[1].bytes"));
        assert!(c.contains("SEED_TEST_CREATE_TOKEN_PROGRAM_ID"));
    }

    #[test]
    fn generated_clients_use_program_sentinels_for_optional_pda_dependencies() {
        let idl = idl_with_optional_pda_dependencies();
        let rust = generate_rust_client(&idl)
            .unwrap()
            .into_iter()
            .find_map(|(path, contents)| (path == "instructions/create.rs").then_some(contents))
            .expect("create instruction generated");
        assert!(rust.contains("let maybe_seed = ix.maybe_seed.unwrap_or(ID);"));
        assert!(rust.contains("let maybe_program = ix.maybe_program.unwrap_or(ID);"));
        assert!(rust.contains("maybe_seed.as_ref()], &maybe_program"));
        assert!(rust.contains("maybe_seed: ix.maybe_seed"));

        for typescript in [
            generate_ts_client(&idl).unwrap(),
            generate_ts_client_kit(&idl).unwrap(),
        ] {
            assert!(typescript.contains("accountOverrides.maybeSeed ?? input.maybeSeed ??"));
            assert!(typescript.contains("accountOverrides.maybeProgram ?? input.maybeProgram ??"));
        }

        let python = generate_python_client(&idl).unwrap();
        assert!(python.contains(
            "accounts_map[\"maybeSeed\"] = input.maybe_seed if input.maybe_seed is not None else \
             PROGRAM_ID"
        ));
        assert!(python.contains(
            "accounts_map[\"maybeProgram\"] = input.maybe_program if input.maybe_program is not \
             None else PROGRAM_ID"
        ));

        let go = generate_go_client(&idl).unwrap();
        assert!(
            go.contains("if input.MaybeSeed != nil { return *input.MaybeSeed }; return ProgramID")
        );
        assert!(go.contains(
            "if input.MaybeProgram != nil { return *input.MaybeProgram }; return ProgramID"
        ));

        let c = generate_c_client(&idl).unwrap();
        assert!(c.contains(
            "(accounts->maybeSeed ? accounts->maybeSeed : (Pubkey *)&SEED_TEST_PROGRAM_ID)->bytes"
        ));
        assert!(c.contains(
            "accounts->maybeProgram ? accounts->maybeProgram : (Pubkey *)&SEED_TEST_PROGRAM_ID"
        ));
    }

    #[test]
    fn generated_clients_resolve_associated_token_accounts() {
        let idl = idl_with_associated_token();
        let rust_files = generate_rust_client(&idl).unwrap();
        let rust = rust_files
            .iter()
            .find_map(|(path, contents)| (path == "instructions/create.rs").then_some(contents))
            .expect("create instruction generated");
        let typescript = generate_ts_client(&idl).unwrap();
        let kit = generate_ts_client_kit(&idl).unwrap();
        let python = generate_python_client(&idl).unwrap();
        let go = generate_go_client(&idl).unwrap();
        let c = generate_c_client(&idl).unwrap();

        let rust_input = rust
            .split("pub struct CreateInstruction {")
            .nth(1)
            .expect("resolved Rust input")
            .split('}')
            .next()
            .expect("resolved Rust input body");
        assert!(!rust_input.contains("owner_tokens"));
        assert!(rust.contains("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        assert!(typescript.contains("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        assert!(kit.contains("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        for output in [&typescript, &kit] {
            assert!(output.contains("export interface CreateInstructionAccountOverrides"));
            assert!(output.contains("return this.createCreateInstructionRaw(input, {});"));
            assert!(output.contains("createCreateInstructionRaw("));
            assert!(output.contains("accountOverrides.ownerTokens ?? accountsMap[\"ownerTokens\"]"));
        }
        assert!(python.contains("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        assert!(go.contains("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));
        assert!(c.contains("SEED_TEST_ASSOCIATED_TOKEN_PROGRAM_ID"));
        assert!(!c.contains("Pubkey *ownerTokens;"));
    }

    #[test]
    fn rust_errors_convert_to_custom_error_codes() {
        let mut idl = idl_with_u64_arg_seed();
        idl.errors.push(IdlErrorDef {
            code: 6_000,
            name: "Unauthorized".to_owned(),
            msg: Some("unauthorized".to_owned()),
        });
        let files = generate_rust_client(&idl).unwrap();
        let errors_rs = files
            .iter()
            .find_map(|(path, contents)| (path == "errors.rs").then_some(contents))
            .expect("errors generated");
        let web3 = generate_ts_client(&idl).unwrap();
        let kit = generate_ts_client_kit(&idl).unwrap();

        assert!(errors_rs.contains("impl From<SeedTestError> for u32"));
        assert!(errors_rs.contains("error as u32"));
        for typescript in [web3, kit] {
            assert!(typescript.contains("export const PROGRAM_ERROR_CODES = {"));
            assert!(typescript.contains("Unauthorized: 6000,"));
        }
    }

    #[test]
    fn rust_client_sources_end_in_exactly_one_newline() {
        let files = generate_rust_client(&idl_with_u64_arg_seed()).unwrap();

        assert!(files
            .iter()
            .all(|(_, contents)| { contents.ends_with('\n') && !contents.ends_with("\n\n") }));
    }

    #[test]
    fn rust_instruction_symbols_convert_snake_case_to_pascal_case() {
        let files = generate_rust_client(&idl_with_snake_case_instruction()).unwrap();
        let mod_rs = files
            .iter()
            .find_map(|(path, contents)| (path == "instructions/mod.rs").then_some(contents))
            .expect("instructions/mod.rs generated");
        let instruction_rs = files
            .iter()
            .find_map(|(path, contents)| {
                (path == "instructions/execute_transfer.rs").then_some(contents)
            })
            .expect("execute_transfer.rs generated");

        assert!(mod_rs.contains("ExecuteTransfer { amount: u64 }"));
        assert!(mod_rs.contains("ProgramInstruction::ExecuteTransfer { amount }"));
        assert!(instruction_rs.contains("pub struct ExecuteTransferInstruction"));
        assert!(!mod_rs.contains("Execute_transfer"));
        assert!(!instruction_rs.contains("Execute_transfer"));
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
