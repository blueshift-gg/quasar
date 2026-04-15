use quasar_idl::{
    parser::{errors, events, helpers, program, state},
    types::IdlType,
};

fn parse_file(src: &str) -> syn::File {
    syn::parse_file(src).expect("failed to parse test source")
}

// ---------------------------------------------------------------------------
// Program ID extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_program_id_present() {
    let file = parse_file(
        r#"
        declare_id!("ABcDeFgHiJkLmNoPqRsTuVwXyZ123456789012345");
        "#,
    );
    let id = program::extract_program_id(&file);
    assert_eq!(
        id.as_deref(),
        Some("ABcDeFgHiJkLmNoPqRsTuVwXyZ123456789012345")
    );
}

#[test]
fn extract_program_id_absent() {
    let file = parse_file("pub fn foo() {}");
    assert!(program::extract_program_id(&file).is_none());
}

// ---------------------------------------------------------------------------
// Instruction extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_program_module_single_instruction() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            use super::*;

            #[instruction(discriminator = [1])]
            pub fn make(ctx: Ctx<Make>, amount: u64, price: u64) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (name, instructions) = program::extract_program_module(&file).unwrap();
    assert_eq!(name, "my_program");
    assert_eq!(instructions.len(), 1);
    assert_eq!(instructions[0].name, "make");
    assert_eq!(instructions[0].discriminator, vec![1]);
    assert_eq!(instructions[0].accounts_type_name, "Make");
    assert_eq!(instructions[0].args.len(), 2);
    assert_eq!(instructions[0].args[0].0, "amount");
    assert_eq!(instructions[0].args[1].0, "price");
}

#[test]
fn extract_program_module_no_extra_args() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [2])]
            pub fn init(ctx: Ctx<Init>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    assert_eq!(instructions[0].args.len(), 0);
}

#[test]
fn extract_program_module_multi_byte_discriminator() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1, 2, 3])]
            pub fn do_thing(ctx: Ctx<DoThing>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    assert_eq!(instructions[0].discriminator, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------------
// State account extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_state_account() {
    let file = parse_file(
        r#"
        #[account(discriminator = [1, 2])]
        pub struct Escrow {
            pub maker: Address,
            pub amount: u64,
        }
        "#,
    );
    let accounts = state::extract_state_accounts(&file);
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].name, "Escrow");
    assert_eq!(accounts[0].discriminator, vec![1, 2]);
    assert_eq!(accounts[0].fields.len(), 2);
    assert_eq!(accounts[0].fields[0].0, "maker");
    assert_eq!(accounts[0].fields[1].0, "amount");
}

// ---------------------------------------------------------------------------
// Accounts extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_state_account_no_fields() {
    let file = parse_file(
        r#"
        #[account(discriminator = [5])]
        pub struct Empty {}
        "#,
    );
    let accounts = state::extract_state_accounts(&file);
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].fields.len(), 0);
}

// ---------------------------------------------------------------------------
// Event extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_event() {
    let file = parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct TradeExecuted {
            pub maker: Address,
            pub amount: u64,
        }
        "#,
    );
    let evts = events::extract_events(&file);
    assert_eq!(evts.len(), 1);
    assert_eq!(evts[0].name, "TradeExecuted");
    assert_eq!(evts[0].discriminator, vec![10]);
    assert_eq!(evts[0].fields.len(), 2);
}

// ---------------------------------------------------------------------------
// Error extraction
// ---------------------------------------------------------------------------

#[test]
fn extract_errors_default_numbering() {
    let file = parse_file(
        r#"
        #[error_code]
        pub enum MyError {
            First,
            Second,
            Third,
        }
        "#,
    );
    let errs = errors::extract_errors(&file);
    assert_eq!(errs.len(), 3);
    assert_eq!(errs[0].code, 0);
    assert_eq!(errs[0].name, "First");
    assert_eq!(errs[1].code, 1);
    assert_eq!(errs[2].code, 2);
}

#[test]
fn extract_errors_explicit_discriminant() {
    let file = parse_file(
        r#"
        #[error_code]
        pub enum MyError {
            First = 100,
            Second,
        }
        "#,
    );
    let errs = errors::extract_errors(&file);
    assert_eq!(errs[0].code, 100);
    assert_eq!(errs[1].code, 101);
}

#[test]
fn extract_errors_gap() {
    let file = parse_file(
        r#"
        #[error_code]
        pub enum MyError {
            First = 0,
            Second = 10,
            Third,
        }
        "#,
    );
    let errs = errors::extract_errors(&file);
    assert_eq!(errs[0].code, 0);
    assert_eq!(errs[1].code, 10);
    assert_eq!(errs[2].code, 11);
}

// ---------------------------------------------------------------------------
// helpers — to_camel_case
// ---------------------------------------------------------------------------

#[test]
fn to_camel_case_basic() {
    assert_eq!(helpers::to_camel_case("make_offer"), "makeOffer");
    assert_eq!(helpers::to_camel_case("take"), "take");
    assert_eq!(helpers::to_camel_case("some_long_name"), "someLongName");
}

// ---------------------------------------------------------------------------
// helpers — parse_discriminator_value
// ---------------------------------------------------------------------------

#[test]
fn parse_discriminator_single_byte() {
    assert_eq!(
        helpers::parse_discriminator_value("discriminator = 42"),
        Some(vec![42])
    );
}

#[test]
fn parse_discriminator_array() {
    assert_eq!(
        helpers::parse_discriminator_value("discriminator = [1, 2, 3]"),
        Some(vec![1, 2, 3])
    );
}

#[test]
fn parse_discriminator_empty_array() {
    assert_eq!(
        helpers::parse_discriminator_value("discriminator = []"),
        None
    );
}

// ---------------------------------------------------------------------------
// helpers — map_type
// ---------------------------------------------------------------------------

#[test]
fn map_type_primitives() {
    assert!(matches!(
        helpers::map_type("Address"),
        IdlType::Primitive(s) if s == "publicKey"
    ));
    assert!(matches!(
        helpers::map_type("u64"),
        IdlType::Primitive(s) if s == "u64"
    ));
    assert!(matches!(
        helpers::map_type("bool"),
        IdlType::Primitive(s) if s == "bool"
    ));
}

#[test]
fn map_type_defined() {
    assert!(matches!(
        helpers::map_type("MyCustomType"),
        IdlType::Defined { defined } if defined == "MyCustomType"
    ));
}

// ---------------------------------------------------------------------------
// helpers — map_type_from_syn (lifetime-parameterized forms)
// ---------------------------------------------------------------------------

#[test]
fn map_type_from_syn_string_with_lifetime() {
    // String<'a, 32> must parse the same as String<32>: DynString with 32 max, u8
    // prefix.
    let ty: syn::Type = syn::parse_str("String<'a, 32>").unwrap();
    assert!(
        matches!(
            helpers::map_type_from_syn(&ty),
            IdlType::DynString { string } if string.max_length == 32 && string.prefix_bytes == 1
        ),
        "String<'a, N> must be DynString with u8 prefix"
    );
}

#[test]
fn map_type_from_syn_vec_with_lifetime() {
    // Vec<'a, u64, 10> must parse the same as Vec<u64, 10>: DynVec<u64> with 10
    // max, u16 prefix.
    let ty: syn::Type = syn::parse_str("Vec<'a, u64, 10>").unwrap();
    assert!(
        matches!(
            helpers::map_type_from_syn(&ty),
            IdlType::DynVec { vec } if vec.max_length == 10 && vec.prefix_bytes == 2
        ),
        "Vec<'a, T, N> must be DynVec with u16 prefix"
    );
}

// ---------------------------------------------------------------------------
// Discriminator collision detection (tested via build_idl)
// ---------------------------------------------------------------------------

#[test]
fn no_collision_different_kinds() {
    // Instruction and account with same discriminator should NOT collide
    use quasar_idl::parser::ParsedProgram;
    let parsed = ParsedProgram {
        program_id: "11111111111111111111111111111111".to_string(),
        program_name: "test".to_string(),
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![program::RawInstruction {
            name: "make".to_string(),
            discriminator: vec![1],
            accounts_type_name: "Make".to_string(),
            args: vec![],
            has_remaining: false,
        }],
        accounts_structs: vec![],
        state_accounts: vec![state::RawStateAccount {
            name: "Escrow".to_string(),
            discriminator: vec![1], // same disc as instruction — should be fine
            fields: vec![],
            seeds: None,
        }],
        events: vec![],
        errors: vec![],
        data_structs: vec![],
    };
    // Should not panic
    let idl = quasar_idl::parser::build_idl(&parsed).unwrap();
    assert_eq!(idl.instructions.len(), 1);
    assert_eq!(idl.accounts.len(), 1);
}

// ---------------------------------------------------------------------------
// build_idl full pipeline
// ---------------------------------------------------------------------------

#[test]
fn build_idl_full_pipeline() {
    use quasar_idl::parser::ParsedProgram;

    let state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Escrow {
            pub maker: Address,
            pub amount: u64,
        }
        "#,
    ));
    let evts = events::extract_events(&parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct TradeExecuted {
            pub maker: Address,
            pub amount: u64,
        }
        "#,
    ));
    let errs = errors::extract_errors(&parse_file(
        r#"
        #[error_code]
        pub enum MyError {
            InsufficientFunds,
            InvalidState,
        }
        "#,
    ));

    let parsed = ParsedProgram {
        program_id: "ABcDeFgH111111111111111111111111111111111111".to_string(),
        program_name: "test_program".to_string(),
        crate_name: "test-program".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![program::RawInstruction {
            name: "make_offer".to_string(),
            discriminator: vec![1],
            accounts_type_name: "MakeOffer".to_string(),
            args: vec![],
            has_remaining: false,
        }],
        accounts_structs: vec![],
        state_accounts,
        events: evts,
        errors: errs,
        data_structs: vec![],
    };

    let idl = quasar_idl::parser::build_idl(&parsed).unwrap();

    // Verify structure
    assert_eq!(idl.address, "ABcDeFgH111111111111111111111111111111111111");
    assert_eq!(idl.metadata.name, "test_program");
    assert_eq!(idl.instructions[0].name, "makeOffer"); // camelCase
    assert_eq!(idl.instructions[0].discriminator, vec![1]);
    assert_eq!(idl.accounts.len(), 1);
    assert_eq!(idl.accounts[0].name, "Escrow");
    assert_eq!(idl.events.len(), 1);
    assert_eq!(idl.events[0].name, "TradeExecuted");
    assert_eq!(idl.errors.len(), 2);
    assert_eq!(idl.errors[0].code, 0);
    assert_eq!(idl.errors[0].name, "InsufficientFunds");

    // Verify JSON serialization works
    let json = serde_json::to_string_pretty(&idl).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value["address"],
        "ABcDeFgH111111111111111111111111111111111111"
    );
    assert_eq!(value["instructions"][0]["name"], "makeOffer");
}

// ---------------------------------------------------------------------------
// Discriminator collision detection
// ---------------------------------------------------------------------------

#[test]
fn collision_two_instructions_same_discriminator() {
    use quasar_idl::parser::{find_discriminator_collisions, ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "11111111111111111111111111111111".to_string(),
        program_name: "test".to_string(),
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![
            program::RawInstruction {
                name: "make".to_string(),
                discriminator: vec![1],
                accounts_type_name: "Make".to_string(),
                args: vec![],
                has_remaining: false,
            },
            program::RawInstruction {
                name: "take".to_string(),
                discriminator: vec![1], // collision with make
                accounts_type_name: "Take".to_string(),
                args: vec![],
                has_remaining: false,
            },
        ],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![],
        errors: vec![],
        data_structs: vec![],
    };

    let collisions = find_discriminator_collisions(&parsed);
    assert_eq!(collisions.len(), 1, "should detect one collision");
    assert!(collisions[0].contains("make"));
    assert!(collisions[0].contains("take"));
}

#[test]
fn collision_two_accounts_same_discriminator() {
    use quasar_idl::parser::{find_discriminator_collisions, ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "11111111111111111111111111111111".to_string(),
        program_name: "test".to_string(),
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![],
        accounts_structs: vec![],
        state_accounts: vec![
            state::RawStateAccount {
                name: "Escrow".to_string(),
                discriminator: vec![1, 2],
                fields: vec![],
                seeds: None,
            },
            state::RawStateAccount {
                name: "Vault".to_string(),
                discriminator: vec![1, 2], // collision with Escrow
                fields: vec![],
                seeds: None,
            },
        ],
        events: vec![],
        errors: vec![],
        data_structs: vec![],
    };

    let collisions = find_discriminator_collisions(&parsed);
    assert_eq!(collisions.len(), 1, "should detect one collision");
    assert!(collisions[0].contains("Escrow"));
    assert!(collisions[0].contains("Vault"));
}

#[test]
fn collision_two_events_same_discriminator() {
    use quasar_idl::parser::{find_discriminator_collisions, ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "11111111111111111111111111111111".to_string(),
        program_name: "test".to_string(),
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![
            events::RawEvent {
                name: "TradeExecuted".to_string(),
                discriminator: vec![10],
                fields: vec![],
            },
            events::RawEvent {
                name: "OrderFilled".to_string(),
                discriminator: vec![10], // collision
                fields: vec![],
            },
        ],
        errors: vec![],
        data_structs: vec![],
    };

    let collisions = find_discriminator_collisions(&parsed);
    assert_eq!(collisions.len(), 1);
    assert!(collisions[0].contains("TradeExecuted"));
    assert!(collisions[0].contains("OrderFilled"));
}

#[test]
fn no_collision_same_disc_different_kinds() {
    use quasar_idl::parser::{find_discriminator_collisions, ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "11111111111111111111111111111111".to_string(),
        program_name: "test".to_string(),
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![program::RawInstruction {
            name: "make".to_string(),
            discriminator: vec![1],
            accounts_type_name: "Make".to_string(),
            args: vec![],
            has_remaining: false,
        }],
        accounts_structs: vec![],
        state_accounts: vec![state::RawStateAccount {
            name: "Escrow".to_string(),
            discriminator: vec![1], // same disc, different kind — OK
            fields: vec![],
            seeds: None,
        }],
        events: vec![events::RawEvent {
            name: "Trade".to_string(),
            discriminator: vec![1], // same disc, different kind — OK
            fields: vec![],
        }],
        errors: vec![],
        data_structs: vec![],
    };

    let collisions = find_discriminator_collisions(&parsed);
    assert!(
        collisions.is_empty(),
        "cross-kind same-disc should not collide"
    );
}

#[test]
fn collision_three_instructions_pairwise() {
    use quasar_idl::parser::{find_discriminator_collisions, ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "11111111111111111111111111111111".to_string(),
        program_name: "test".to_string(),
        crate_name: "test".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![
            program::RawInstruction {
                name: "a".to_string(),
                discriminator: vec![1],
                accounts_type_name: "A".to_string(),
                args: vec![],
                has_remaining: false,
            },
            program::RawInstruction {
                name: "b".to_string(),
                discriminator: vec![1],
                accounts_type_name: "B".to_string(),
                args: vec![],
                has_remaining: false,
            },
            program::RawInstruction {
                name: "c".to_string(),
                discriminator: vec![1],
                accounts_type_name: "C".to_string(),
                args: vec![],
                has_remaining: false,
            },
        ],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![],
        errors: vec![],
        data_structs: vec![],
    };

    let collisions = find_discriminator_collisions(&parsed);
    // 3 instructions with same disc → 3 pairwise collisions: (a,b), (a,c), (b,c)
    assert_eq!(collisions.len(), 3, "should detect all pairwise collisions");
}

// ===========================================================================
// Rust codegen
//
// Tests that the generated Rust client code is structurally correct,
// uses the right types for wire compatibility, and handles all feature
// combinations (events, accounts, dynamic types, remaining accounts).
// ===========================================================================

use quasar_idl::{
    codegen::rust::{generate_cargo_toml, generate_client},
    lint::constraints::{FieldClass, FieldConstraints},
    parser::{
        self,
        accounts::{RawAccountField, RawAccountsStruct, RawPda, RawSeed},
        ParsedProgram,
    },
    types::IdlError,
};

/// Build IDL from parsed program and generate the Rust client.
/// Convenience helper that routes through the IDL (single source of truth).
fn build_and_generate(parsed: &ParsedProgram) -> Vec<(String, String)> {
    let idl = parser::build_idl(parsed).unwrap();
    generate_client(&idl)
}

fn test_program() -> ParsedProgram {
    ParsedProgram {
        program_id: "ABcDeFgH111111111111111111111111111111111111".to_string(),
        program_name: "test_program".to_string(),
        crate_name: "test-program".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![],
        errors: vec![],
        data_structs: vec![],
    }
}

/// Concatenate all generated file contents for assertion checking.
fn all_content(files: &[(String, String)]) -> String {
    files
        .iter()
        .map(|(_, content)| content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Instruction codegen: no args
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_no_arg_instruction() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "initialize".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Initialize".to_string(),
        args: vec![],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("pub struct InitializeInstruction {"),
        "{code}"
    );
    // No args → immutable `data` (no `mut`)
    assert!(code.contains("let data = vec![0];"), "{code}");
    // No manual serialization
    assert!(!code.contains("to_le_bytes"), "{code}");
}

// ---------------------------------------------------------------------------
// Instruction codegen: primitive args
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_primitive_args() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![1],
        accounts_type_name: "Deposit".to_string(),
        args: vec![
            ("amount".to_string(), syn::parse_str("u64").unwrap()),
            ("flag".to_string(), syn::parse_str("bool").unwrap()),
        ],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Struct fields use native types
    assert!(code.contains("pub amount: u64,"), "{code}");
    assert!(code.contains("pub flag: bool,"), "{code}");
    // Each arg serialized directly into the data buffer
    assert!(
        code.contains("wincode::serialize_into(&mut data, &ix.amount)"),
        "{code}"
    );
    assert!(
        code.contains("wincode::serialize_into(&mut data, &ix.flag)"),
        "{code}"
    );
}

// ---------------------------------------------------------------------------
// Instruction codegen: account metas
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_account_metas() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "transfer".to_string(),
        discriminator: vec![2],
        accounts_type_name: "Transfer".to_string(),
        args: vec![("amount".to_string(), syn::parse_str("u64").unwrap())],
        has_remaining: false,
    });
    parsed
        .accounts_structs
        .push(quasar_idl::parser::accounts::RawAccountsStruct {
            name: "Transfer".to_string(),
            fields: vec![
                RawAccountField {
                    name: "from".to_string(),
                    writable: true,
                    signer: true,
                    pda: None,
                    address: None,
                    field_class: FieldClass::Unchecked,
                    inner_type_name: None,
                    constraints: FieldConstraints::default(),
                    seed_type: None,
                },
                RawAccountField {
                    name: "to".to_string(),
                    writable: true,
                    signer: false,
                    pda: None,
                    address: None,
                    field_class: FieldClass::Unchecked,
                    inner_type_name: None,
                    constraints: FieldConstraints::default(),
                    seed_type: None,
                },
                RawAccountField {
                    name: "authority".to_string(),
                    writable: false,
                    signer: true,
                    pda: None,
                    address: None,
                    field_class: FieldClass::Unchecked,
                    inner_type_name: None,
                    constraints: FieldConstraints::default(),
                    seed_type: None,
                },
            ],
        });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("AccountMeta::new(ix.from, true)"),
        "writable+signer → new(..., true): {code}"
    );
    assert!(
        code.contains("AccountMeta::new(ix.to, false)"),
        "writable+!signer → new(..., false): {code}"
    );
    assert!(
        code.contains("AccountMeta::new_readonly(ix.authority, true)"),
        "!writable+signer → new_readonly(..., true): {code}"
    );
}

// ---------------------------------------------------------------------------
// Instruction codegen: dynamic types use wrapper types
//
// This is the critical wire compatibility test. String<N> maps to
// DynBytes<u8> (u8 prefix) and Vec<T, N> maps to DynVec<T, u16> (u16 prefix).
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_dynamic_string_uses_dyn_bytes() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<8>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("pub name: DynBytes<u8>,"),
        "String<N> must map to DynBytes<u8>, got:\n{code}"
    );
    assert!(
        !code.contains("pub name: Vec<u8>"),
        "must NOT use Vec<u8> — different length prefix: {code}"
    );
}

#[test]
fn rust_codegen_dynamic_types_import() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<8>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Only the actually-used wrapper type is imported
    assert!(
        code.contains("DynBytes"),
        "DynBytes must be imported: {code}"
    );
    assert!(
        !code.contains("DynVec"),
        "DynVec must not be imported when unused: {code}"
    );
    assert!(
        !code.contains("TailBytes"),
        "TailBytes must not be imported when unused: {code}"
    );
}

// ---------------------------------------------------------------------------
// Instruction codegen: Pod dynamic types (String<N> / Vec<T, N>)
//
// String<N> always uses u8 prefix (prefix_bytes: 1).
// Vec<T, N> always uses u16 prefix (prefix_bytes: 2).
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_dyn_bytes_u8_prefix() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<100>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("pub name: DynBytes<u8>,"),
        "String<N> must map to DynBytes<u8>, got:\n{code}"
    );
}

#[test]
fn rust_codegen_dyn_vec_u16_prefix() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_tags(ctx: Ctx<SetTags>, tags: Vec<u64, 10>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("pub tags: DynVec<u64, u16>,"),
        "Vec<u64, N> must map to DynVec<u64, u16>, got:\n{code}"
    );
}

// ---------------------------------------------------------------------------
// Instruction codegen: remaining accounts
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_remaining_accounts_present() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "create".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Create".to_string(),
        args: vec![],
        has_remaining: true,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("pub remaining_accounts: Vec<AccountMeta>,"),
        "{code}"
    );
    assert!(
        code.contains("accounts.extend(ix.remaining_accounts)"),
        "{code}"
    );
}

#[test]
fn rust_codegen_remaining_accounts_absent() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![1],
        accounts_type_name: "Deposit".to_string(),
        args: vec![],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(!code.contains("remaining_accounts"), "{code}");
}

// ---------------------------------------------------------------------------
// Account codegen
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_accounts() {
    let mut parsed = test_program();
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Escrow {
            pub maker: Address,
            pub amount: u64,
        }

        #[account(discriminator = [2])]
        pub struct Empty {}
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Discriminator constants
    assert!(
        code.contains("ESCROW_ACCOUNT_DISCRIMINATOR: &[u8] = &[1]"),
        "{code}"
    );
    assert!(
        code.contains("EMPTY_ACCOUNT_DISCRIMINATOR: &[u8] = &[2]"),
        "{code}"
    );

    // Struct with manual impls (no SchemaWrite/SchemaRead derive, no repr(C))
    assert!(
        code.contains("#[derive(Clone, Copy)]\npub struct Escrow {"),
        "{code}"
    );
    assert!(code.contains("pub maker: Address,"), "{code}");
    assert!(code.contains("pub amount: u64,"), "{code}");

    // Manual SchemaWrite/SchemaRead impls with discriminator handling
    assert!(
        code.contains("unsafe impl<C: ConfigCore> SchemaWrite<C> for Escrow"),
        "{code}"
    );
    assert!(
        code.contains("unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Escrow"),
        "{code}"
    );
    assert!(
        code.contains("writer.write(ESCROW_ACCOUNT_DISCRIMINATOR)"),
        "{code}"
    );

    // Enum
    assert!(code.contains("Escrow(Escrow),"), "{code}");
    assert!(code.contains("Empty,"), "{code}");

    // Decoder passes full data (SchemaRead handles discriminator)
    assert!(code.contains("pub fn decode_account"), "{code}");
    assert!(
        code.contains("wincode::deserialize::<Escrow>(data)"),
        "{code}"
    );
    assert!(!code.contains("let mut offset"), "{code}");
}

// ---------------------------------------------------------------------------
// Event codegen
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_events() {
    let mut parsed = test_program();
    parsed.events = events::extract_events(&parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct TradeExecuted {
            pub maker: Address,
            pub amount: u64,
        }

        #[event(discriminator = 5)]
        pub struct OrderCancelled {}
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Discriminator constants
    assert!(
        code.contains("TRADE_EXECUTED_EVENT_DISCRIMINATOR: &[u8] = &[10]"),
        "{code}"
    );
    assert!(
        code.contains("ORDER_CANCELLED_EVENT_DISCRIMINATOR: &[u8] = &[5]"),
        "{code}"
    );

    // Struct with manual impls (no derives, same pattern as accounts)
    assert!(
        code.contains("#[derive(Clone, Copy)]\npub struct TradeExecuted {"),
        "{code}"
    );
    assert!(code.contains("pub maker: Address,"), "{code}");
    assert!(code.contains("pub amount: u64,"), "{code}");

    // Manual SchemaWrite/SchemaRead impls
    assert!(
        code.contains("unsafe impl<C: ConfigCore> SchemaWrite<C> for TradeExecuted"),
        "{code}"
    );
    assert!(
        code.contains("unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for TradeExecuted"),
        "{code}"
    );
    assert!(
        code.contains("writer.write(TRADE_EXECUTED_EVENT_DISCRIMINATOR)"),
        "{code}"
    );

    // Enum
    assert!(code.contains("TradeExecuted(TradeExecuted),"), "{code}");
    assert!(code.contains("OrderCancelled,"), "{code}");

    // Decoder passes full data (SchemaRead handles discriminator)
    assert!(code.contains("pub fn decode_event"), "{code}");
    assert!(
        code.contains("wincode::deserialize::<TradeExecuted>(data)"),
        "{code}"
    );
    assert!(!code.contains("let mut offset"), "{code}");
    assert!(
        !code.contains("payload"),
        "no manual payload slicing: {code}"
    );
}

// ---------------------------------------------------------------------------
// Custom data struct codegen
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_custom_data_structs() {
    let mut parsed = test_program();
    parsed.data_structs.push(quasar_idl::parser::RawDataStruct {
        name: "OrderConfig".to_string(),
        fields: vec![
            ("limit".to_string(), syn::parse_str("u64").unwrap()),
            ("owner".to_string(), syn::parse_str("Address").unwrap()),
        ],
    });
    parsed.instructions.push(program::RawInstruction {
        name: "place_order".to_string(),
        discriminator: vec![3],
        accounts_type_name: "PlaceOrder".to_string(),
        args: vec![("config".to_string(), syn::parse_str("OrderConfig").unwrap())],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Custom struct generated with wincode derives
    assert!(
        code.contains("#[derive(SchemaWrite, SchemaRead)]\npub struct OrderConfig {"),
        "{code}"
    );
    assert!(code.contains("pub limit: u64,"), "{code}");
    assert!(code.contains("pub owner: Address,"), "{code}");

    // Instruction uses the custom type
    assert!(code.contains("pub config: OrderConfig,"), "{code}");
    assert!(
        code.contains("wincode::serialize_into(&mut data, &ix.config)"),
        "{code}"
    );
}

// ---------------------------------------------------------------------------
// Cargo.toml generation
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_cargo_toml() {
    use quasar_idl::codegen::rust::generate_cargo_toml;
    let toml = generate_cargo_toml("my-program", "0.1.0", false);
    assert!(toml.contains("name = \"my-program-client\""), "{toml}");
    assert!(toml.contains("version = \"0.1.0\""), "{toml}");
    // Git dep so source-build users get a quasar-lang that matches their CLI.
    assert!(
        toml.contains("blueshift-gg/quasar"),
        "quasar-lang must be a git dep: {toml}"
    );
    // Exact-pinned to avoid the wincode 0.4/0.5 split (solana-address >= 2.3
    // depends on wincode 0.5, which conflicts with the quasar-lang 0.4 binding).
    assert!(
        toml.contains("=0.4.9"),
        "wincode must be exact-pinned: {toml}"
    );
    assert!(
        toml.contains("=2.2.0"),
        "solana-address must be exact-pinned: {toml}"
    );
}

// ---------------------------------------------------------------------------
// Program ID in output
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_program_id() {
    let parsed = test_program();
    let files = build_and_generate(&parsed);
    let code = all_content(&files);
    assert!(
        code.contains(
            r#"pub const ID: Address = solana_address::address!("ABcDeFgH111111111111111111111111111111111111");"#
        ),
        "{code}"
    );
}

// ---------------------------------------------------------------------------
// Imports: conditional Vec and wrapper types
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_imports_no_dynamic() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "init".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Init".to_string(),
        args: vec![],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // No dynamic types → no Vec import, no wrapper import
    assert!(!code.contains("use std::vec::Vec;"), "{code}");
    assert!(!code.contains("DynBytes"), "{code}");
}

#[test]
fn rust_codegen_imports_with_remaining() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "multi".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Multi".to_string(),
        args: vec![],
        has_remaining: true,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // remaining_accounts needs Vec
    assert!(code.contains("use std::vec::Vec;"), "{code}");
}

#[test]
fn rust_codegen_no_wincode_derive_import_when_unused() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "init".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Init".to_string(),
        args: vec![("amount".to_string(), syn::parse_str("u64").unwrap())],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // No custom types, events, or accounts → no SchemaWrite/SchemaRead import
    assert!(
        !code.contains("use wincode::{SchemaWrite, SchemaRead}"),
        "should not import derive macros when unused: {code}"
    );
    // But wincode::serialize is still used inline
    assert!(code.contains("wincode::serialize"), "{code}");
}

#[test]
fn rust_codegen_wincode_derive_import_with_events() {
    let mut parsed = test_program();
    parsed.events = events::extract_events(&parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct Transfer {
            pub amount: u64,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    assert!(
        code.contains("use wincode::{SchemaWrite, SchemaRead}"),
        "events with fields need derive import: {code}"
    );
}

// ---------------------------------------------------------------------------
// Account codegen: dynamic fields omit Copy
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_account_with_dynamic_field_no_copy() {
    let mut parsed = test_program();
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Profile {
            pub owner: Address,
            pub name: String<100>,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Dynamic field (String) → no Copy, manual impls (no derives)
    assert!(
        !code.contains("#[derive(Clone, Copy)]\npub struct Profile"),
        "account with dynamic field must not derive Copy: {code}"
    );
    assert!(
        code.contains("#[derive(Clone)]\npub struct Profile {"),
        "{code}"
    );
    assert!(
        code.contains("unsafe impl<C: ConfigCore> SchemaWrite<C> for Profile"),
        "{code}"
    );
    assert!(
        code.contains("unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Profile"),
        "{code}"
    );
    assert!(
        !code.contains("#[repr(C)]\npub struct Profile"),
        "account with dynamic field must not use repr(C): {code}"
    );
}

#[test]
fn rust_codegen_account_fixed_fields_has_copy() {
    let mut parsed = test_program();
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Escrow {
            pub maker: Address,
            pub amount: u64,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // All fixed-size fields → derive Copy, manual impls (no repr(C))
    assert!(
        code.contains("#[derive(Clone, Copy)]\npub struct Escrow {"),
        "{code}"
    );
    assert!(
        code.contains("unsafe impl<C: ConfigCore> SchemaWrite<C> for Escrow"),
        "{code}"
    );
    assert!(
        !code.contains("#[repr(C)]"),
        "accounts no longer use repr(C): {code}"
    );
}

// ---------------------------------------------------------------------------
// Nested struct resolution: accounts and events
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_account_with_inner_struct() {
    let mut parsed = test_program();
    parsed.data_structs.push(quasar_idl::parser::RawDataStruct {
        name: "InnerConfig".to_string(),
        fields: vec![
            ("limit".to_string(), syn::parse_str("u64").unwrap()),
            ("enabled".to_string(), syn::parse_str("bool").unwrap()),
        ],
    });
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Vault {
            pub owner: Address,
            pub config: InnerConfig,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // InnerConfig must be defined in generated code
    assert!(
        code.contains("pub struct InnerConfig {"),
        "inner struct must be generated for account field: {code}"
    );
    assert!(code.contains("pub limit: u64,"), "{code}");
    assert!(code.contains("pub enabled: bool,"), "{code}");

    // Account uses the inner type
    assert!(code.contains("pub config: InnerConfig,"), "{code}");

    // decode_account passes full data (SchemaRead handles discriminator)
    assert!(
        code.contains("wincode::deserialize::<Vault>(data)"),
        "{code}"
    );
}

#[test]
fn rust_codegen_event_with_inner_struct() {
    let mut parsed = test_program();
    parsed.data_structs.push(quasar_idl::parser::RawDataStruct {
        name: "TradeInfo".to_string(),
        fields: vec![
            ("price".to_string(), syn::parse_str("u64").unwrap()),
            ("quantity".to_string(), syn::parse_str("u32").unwrap()),
        ],
    });
    parsed.events = events::extract_events(&parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct TradeExecuted {
            pub info: TradeInfo,
            pub maker: Address,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // TradeInfo must be defined
    assert!(
        code.contains("pub struct TradeInfo {"),
        "inner struct must be generated for event field: {code}"
    );

    // Event uses the inner type
    assert!(code.contains("pub info: TradeInfo,"), "{code}");
}

#[test]
fn rust_codegen_deeply_nested_structs() {
    let mut parsed = test_program();
    parsed.data_structs.push(quasar_idl::parser::RawDataStruct {
        name: "Inner".to_string(),
        fields: vec![("value".to_string(), syn::parse_str("u64").unwrap())],
    });
    parsed.data_structs.push(quasar_idl::parser::RawDataStruct {
        name: "Outer".to_string(),
        fields: vec![
            ("inner".to_string(), syn::parse_str("Inner").unwrap()),
            ("count".to_string(), syn::parse_str("u32").unwrap()),
        ],
    });
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct MyAccount {
            pub data: Outer,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Both Inner and Outer must be defined
    assert!(
        code.contains("pub struct Inner {"),
        "transitively referenced struct must be generated: {code}"
    );
    assert!(
        code.contains("pub struct Outer {"),
        "directly referenced struct must be generated: {code}"
    );
    assert!(code.contains("pub inner: Inner,"), "{code}");
    assert!(code.contains("pub data: Outer,"), "{code}");
}

// ===========================================================================
// TypeScript codegen
// ===========================================================================

#[test]
fn ts_codegen_remaining_accounts() {
    use quasar_idl::{codegen::typescript::generate_ts_client_kit, parser::build_idl};

    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "create".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Create".to_string(),
        args: vec![],
        has_remaining: true,
    });

    let idl = build_idl(&parsed).unwrap();
    let code = generate_ts_client_kit(&idl);

    assert!(code.contains("remainingAccounts?"), "{code}");
    assert!(
        code.contains("...(input.remainingAccounts ?? [])"),
        "{code}"
    );
}

#[test]
fn ts_codegen_prefix_aware_codecs() {
    use quasar_idl::{codegen::typescript::generate_ts_client_kit, parser::build_idl};

    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<100>) -> Result<(), ProgramError> {
                Ok(())
            }

            #[instruction(discriminator = [2])]
            pub fn set_tags(ctx: Ctx<SetTags>, tags: Vec<u64, 500>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let idl = build_idl(&parsed).unwrap();
    let code = generate_ts_client_kit(&idl);

    // String<100> → u8 prefix codec
    assert!(
        code.contains("addCodecSizePrefix(getUtf8Codec(), getU8Codec())"),
        "String<N> must use getU8Codec(): {code}"
    );

    // Vec<u64, 500> → u16 prefix codec
    assert!(
        code.contains("getArrayCodec(getU64Codec(), { size: getU16Codec() })"),
        "Vec<T, N> must use getU16Codec(): {code}"
    );

    // No helper functions emitted
    assert!(
        !code.contains("function getDynStringCodec"),
        "should not emit helper functions: {code}"
    );
    assert!(
        !code.contains("function getDynVecCodec"),
        "should not emit helper functions: {code}"
    );
}

// ===========================================================================
// IDL JSON serialization
// ===========================================================================

#[test]
fn idl_json_has_remaining() {
    use quasar_idl::parser::build_idl;

    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "create".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Create".to_string(),
        args: vec![],
        has_remaining: true,
    });
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![1],
        accounts_type_name: "Deposit".to_string(),
        args: vec![],
        has_remaining: false,
    });

    let idl = build_idl(&parsed).unwrap();
    let json = serde_json::to_string_pretty(&idl).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(value["instructions"][0]["hasRemaining"], true);
    assert!(value["instructions"][1].get("hasRemaining").is_none());
}

// ===========================================================================
// IDL JSON: prefixBytes serialization
// ===========================================================================

#[test]
fn idl_json_prefix_bytes_serialization() {
    use quasar_idl::parser::build_idl;

    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<100>) -> Result<(), ProgramError> {
                Ok(())
            }

            #[instruction(discriminator = [2])]
            pub fn set_tags(ctx: Ctx<SetTags>, tags: Vec<u64, 500>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let idl = build_idl(&parsed).unwrap();
    let json = serde_json::to_string_pretty(&idl).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    // String<100> → prefixBytes: 1
    let name_ty = &value["instructions"][0]["args"][0]["type"]["string"];
    assert_eq!(name_ty["maxLength"], 100);
    assert_eq!(
        name_ty["prefixBytes"], 1,
        "u8 prefix must serialize: {json}"
    );

    // Vec<u64, 500> → prefixBytes: 2
    let tags_ty = &value["instructions"][1]["args"][0]["type"]["vec"];
    assert_eq!(tags_ty["maxLength"], 500);
    assert_eq!(
        tags_ty["prefixBytes"], 2,
        "u16 prefix must serialize: {json}"
    );
}

// ===========================================================================
// Parser: CtxWithRemaining detection
// ===========================================================================

#[test]
fn extract_instruction_ctx_with_remaining() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = 0)]
            pub fn create(ctx: CtxWithRemaining<Create>, threshold: u8) -> Result<(), ProgramError> {
                Ok(())
            }

            #[instruction(discriminator = 1)]
            pub fn deposit(ctx: Ctx<Deposit>, amount: u64) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );

    let (_, instructions) = program::extract_program_module(&file).unwrap();
    assert_eq!(instructions.len(), 2);
    assert!(instructions[0].has_remaining);
    assert!(!instructions[1].has_remaining);
}

// ===========================================================================
// V2 codegen: error enum
// ===========================================================================

#[test]
fn rust_codegen_error_enum() {
    let mut parsed = test_program();
    parsed.errors = vec![
        IdlError {
            code: 100,
            name: "Unauthorized".to_string(),
            msg: Some("Not authorized".to_string()),
        },
        IdlError {
            code: 101,
            name: "Overflow".to_string(),
            msg: None,
        },
    ];
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Error enum with repr(u32)
    assert!(code.contains("#[repr(u32)]"), "{code}");
    assert!(code.contains("pub enum TestProgramError {"), "{code}");
    assert!(code.contains("Unauthorized = 100,"), "{code}");
    assert!(code.contains("Overflow = 101,"), "{code}");

    // from_code
    assert!(
        code.contains("pub fn from_code(code: u32) -> Option<Self>"),
        "{code}"
    );
    assert!(code.contains("100 => Some(Self::Unauthorized),"), "{code}");

    // message — custom message used when provided, variant name as fallback
    assert!(
        code.contains("Self::Unauthorized => \"Not authorized\","),
        "{code}"
    );
    assert!(code.contains("Self::Overflow => \"Overflow\","), "{code}");
}

#[test]
fn rust_codegen_error_message_escaping() {
    let mut parsed = test_program();
    parsed.errors = vec![IdlError {
        code: 200,
        name: "BadQuote".to_string(),
        msg: Some("can't use \"this\"".to_string()),
    }];
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Quotes and backslashes must be escaped in the generated string literal
    assert!(
        code.contains(r#"Self::BadQuote => "can't use \"this\""#),
        "error message must escape double quotes: {code}"
    );
}

// ===========================================================================
// V2 codegen: PDA helpers
// ===========================================================================

#[test]
fn rust_codegen_pda_helpers() {
    let mut parsed = test_program();
    // PDA data flows through IDL instructions, so we need a matching instruction.
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![1],
        accounts_type_name: "Deposit".to_string(),
        args: vec![],
        has_remaining: false,
    });
    parsed.accounts_structs = vec![RawAccountsStruct {
        name: "Deposit".to_string(),
        fields: vec![
            RawAccountField {
                name: "vault".to_string(),
                writable: true,
                signer: false,
                pda: Some(RawPda {
                    seeds: vec![
                        RawSeed::ByteString(b"vault".to_vec()),
                        RawSeed::AccountRef("user".to_string()),
                    ],
                }),
                address: None,
                field_class: FieldClass::Unchecked,
                inner_type_name: None,
                constraints: FieldConstraints::default(),
                seed_type: None,
            },
            RawAccountField {
                name: "user".to_string(),
                writable: false,
                signer: true,
                pda: None,
                address: None,
                field_class: FieldClass::Unchecked,
                inner_type_name: None,
                constraints: FieldConstraints::default(),
                seed_type: None,
            },
        ],
    }];
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // pda.rs must be generated with find_ helper
    assert!(code.contains("pub fn find_vault_address("), "{code}");
    assert!(code.contains("Address::find_program_address"), "{code}");
    assert!(code.contains(r#"b"vault""#), "{code}");
    assert!(code.contains("user.as_ref()"), "{code}");

    // lib.rs must declare the pda module
    let lib_rs = files.iter().find(|(p, _)| p == "lib.rs").unwrap();
    assert!(lib_rs.1.contains("pub mod pda;"), "{}", lib_rs.1);
}

#[test]
fn rust_codegen_pda_dedup() {
    // Two accounts structs with the same PDA seeds should produce only one helper
    let mut parsed = test_program();
    // PDA data flows through IDL instructions, so we need matching instructions.
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![1],
        accounts_type_name: "Deposit".to_string(),
        args: vec![],
        has_remaining: false,
    });
    parsed.instructions.push(program::RawInstruction {
        name: "withdraw".to_string(),
        discriminator: vec![2],
        accounts_type_name: "Withdraw".to_string(),
        args: vec![],
        has_remaining: false,
    });
    let pda = Some(RawPda {
        seeds: vec![
            RawSeed::ByteString(b"vault".to_vec()),
            RawSeed::AccountRef("user".to_string()),
        ],
    });
    parsed.accounts_structs = vec![
        RawAccountsStruct {
            name: "Deposit".to_string(),
            fields: vec![RawAccountField {
                name: "vault".to_string(),
                writable: true,
                signer: false,
                pda: pda.clone(),
                address: None,
                field_class: FieldClass::Unchecked,
                inner_type_name: None,
                constraints: FieldConstraints::default(),
                seed_type: None,
            }],
        },
        RawAccountsStruct {
            name: "Withdraw".to_string(),
            fields: vec![RawAccountField {
                name: "vault".to_string(),
                writable: true,
                signer: false,
                pda,
                address: None,
                field_class: FieldClass::Unchecked,
                inner_type_name: None,
                constraints: FieldConstraints::default(),
                seed_type: None,
            }],
        },
    ];
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // Only one find_ function despite two accounts with the same seeds
    assert_eq!(
        code.matches("pub fn find_vault_address(").count(),
        1,
        "duplicate PDA seeds must be deduplicated: {code}"
    );
}

// ===========================================================================
// V2 codegen: Cargo.toml with PDAs
// ===========================================================================

#[test]
fn rust_codegen_cargo_toml_with_pdas() {
    let toml = generate_cargo_toml("my-program", "0.1.0", true);
    assert!(
        toml.contains(r#"features = ["curve25519"]"#),
        "PDA programs need curve25519 feature: {toml}"
    );
}

#[test]
fn rust_codegen_cargo_toml_without_pdas() {
    let toml = generate_cargo_toml("my-program", "0.1.0", false);
    assert!(
        !toml.contains("curve25519"),
        "non-PDA programs must not pull in curve25519: {toml}"
    );
}

// ===========================================================================
// V2 codegen: decode_instruction
// ===========================================================================

#[test]
fn rust_codegen_decode_instruction_no_args() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "initialize".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Initialize".to_string(),
        args: vec![],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let ix_mod = files
        .iter()
        .find(|(p, _)| p == "instructions/mod.rs")
        .unwrap();

    assert!(
        ix_mod.1.contains("pub enum ProgramInstruction {"),
        "{}",
        ix_mod.1
    );
    assert!(ix_mod.1.contains("Initialize,"), "{}", ix_mod.1);
    assert!(
        ix_mod
            .1
            .contains("pub fn decode_instruction(data: &[u8]) -> Option<ProgramInstruction>"),
        "{}",
        ix_mod.1
    );
    assert!(
        ix_mod.1.contains("Some(ProgramInstruction::Initialize),"),
        "{}",
        ix_mod.1
    );
}

#[test]
fn rust_codegen_decode_instruction_single_arg() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Deposit".to_string(),
        args: vec![("amount".to_string(), syn::parse_str("u64").unwrap())],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let ix_mod = files
        .iter()
        .find(|(p, _)| p == "instructions/mod.rs")
        .unwrap();

    // Single arg: no offset tracking, deserialize directly from payload
    assert!(
        ix_mod.1.contains("wincode::deserialize(payload).ok()?"),
        "{}",
        ix_mod.1
    );
    assert!(
        !ix_mod.1.contains("let mut offset"),
        "single-arg must not use offset: {}",
        ix_mod.1
    );
}

#[test]
fn rust_codegen_decode_instruction_multi_arg() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "make".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Make".to_string(),
        args: vec![
            ("deposit".to_string(), syn::parse_str("u64").unwrap()),
            ("receive".to_string(), syn::parse_str("u64").unwrap()),
        ],
        has_remaining: false,
    });
    let files = build_and_generate(&parsed);
    let ix_mod = files
        .iter()
        .find(|(p, _)| p == "instructions/mod.rs")
        .unwrap();

    // Multi-arg: uses offset tracking
    assert!(
        ix_mod.1.contains("let mut offset = 0usize;"),
        "{}",
        ix_mod.1
    );
    assert!(
        ix_mod
            .1
            .contains("wincode::serialized_size(&deposit).ok()? as usize"),
        "{}",
        ix_mod.1
    );
    // Last arg does NOT increment offset
    assert!(
        !ix_mod.1.contains("wincode::serialized_size(&receive)"),
        "last arg must not increment offset: {}",
        ix_mod.1
    );
}

// ===========================================================================
// V2 codegen: lib.rs module declarations
// ===========================================================================

#[test]
fn rust_codegen_lib_rs_modules() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Deposit".to_string(),
        args: vec![],
        has_remaining: false,
    });
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Vault {
            pub amount: u64,
        }
        "#,
    ));
    parsed.events = events::extract_events(&parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct Transfer {
            pub amount: u64,
        }
        "#,
    ));
    parsed.errors = vec![IdlError {
        code: 100,
        name: "Unauthorized".to_string(),
        msg: None,
    }];
    let files = build_and_generate(&parsed);
    let lib_rs = files.iter().find(|(p, _)| p == "lib.rs").unwrap();

    assert!(lib_rs.1.contains("pub mod instructions;"), "{}", lib_rs.1);
    assert!(lib_rs.1.contains("pub mod state;"), "{}", lib_rs.1);
    assert!(lib_rs.1.contains("pub mod events;"), "{}", lib_rs.1);
    assert!(lib_rs.1.contains("pub mod errors;"), "{}", lib_rs.1);
}

#[test]
fn rust_codegen_lib_rs_omits_empty_modules() {
    // Empty program: no instructions, state, events, or errors
    let parsed = test_program();
    let files = build_and_generate(&parsed);
    let lib_rs = files.iter().find(|(p, _)| p == "lib.rs").unwrap();

    assert!(!lib_rs.1.contains("pub mod instructions;"), "{}", lib_rs.1);
    assert!(!lib_rs.1.contains("pub mod state;"), "{}", lib_rs.1);
    assert!(!lib_rs.1.contains("pub mod events;"), "{}", lib_rs.1);
    assert!(!lib_rs.1.contains("pub mod errors;"), "{}", lib_rs.1);
    assert!(!lib_rs.1.contains("pub mod pda;"), "{}", lib_rs.1);
}

// ===========================================================================
// V2 codegen: file path correctness
// ===========================================================================

#[test]
fn rust_codegen_file_paths() {
    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "deposit".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Deposit".to_string(),
        args: vec![("amount".to_string(), syn::parse_str("u64").unwrap())],
        has_remaining: false,
    });
    parsed.state_accounts = state::extract_state_accounts(&parse_file(
        r#"
        #[account(discriminator = [1])]
        pub struct Vault {
            pub amount: u64,
        }
        "#,
    ));
    let files = build_and_generate(&parsed);
    let paths: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();

    assert!(paths.contains(&"lib.rs"), "{paths:?}");
    assert!(paths.contains(&"instructions/mod.rs"), "{paths:?}");
    assert!(paths.contains(&"instructions/deposit.rs"), "{paths:?}");
    assert!(paths.contains(&"state/mod.rs"), "{paths:?}");
    assert!(paths.contains(&"state/vault.rs"), "{paths:?}");
}

// ===========================================================================
// V2 codegen: event discriminator constant naming (no stutter)
// ===========================================================================

#[test]
fn rust_codegen_event_discriminator_no_stutter() {
    let mut parsed = test_program();
    parsed.events = events::extract_events(&parse_file(
        r#"
        #[event(discriminator = [10])]
        pub struct MakeEvent {
            pub amount: u64,
        }

        #[event(discriminator = 5)]
        pub struct OrderCancelled {}
        "#,
    ));
    let files = build_and_generate(&parsed);
    let code = all_content(&files);

    // MakeEvent → MAKE_EVENT_DISCRIMINATOR (not MAKE_EVENT_EVENT_DISCRIMINATOR)
    assert!(
        code.contains("MAKE_EVENT_DISCRIMINATOR"),
        "event const should not stutter: {code}"
    );
    assert!(
        !code.contains("MAKE_EVENT_EVENT_DISCRIMINATOR"),
        "event const must not stutter EVENT_EVENT: {code}"
    );

    // OrderCancelled (no Event suffix) → ORDER_CANCELLED_EVENT_DISCRIMINATOR
    assert!(
        code.contains("ORDER_CANCELLED_EVENT_DISCRIMINATOR"),
        "{code}"
    );
}
