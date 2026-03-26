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
        }],
        events: vec![],
        errors: vec![],
        data_structs: vec![],
    };
    // Should not panic
    let idl = quasar_idl::parser::build_idl(parsed);
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

    let idl = quasar_idl::parser::build_idl(parsed);

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
            },
            state::RawStateAccount {
                name: "Vault".to_string(),
                discriminator: vec![1, 2], // collision with Escrow
                fields: vec![],
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
    codegen::rust::generate_client,
    parser::{accounts::RawAccountField, ParsedProgram},
};

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
    let code = generate_client(&parsed);

    assert!(code.contains("pub struct InitializeInstruction {"), "{code}");
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
    let code = generate_client(&parsed);

    // Struct fields use native types
    assert!(code.contains("pub amount: u64,"), "{code}");
    assert!(code.contains("pub flag: bool,"), "{code}");
    // Each arg serialized via wincode
    assert!(code.contains("wincode::serialize(&ix.amount)"), "{code}");
    assert!(code.contains("wincode::serialize(&ix.flag)"), "{code}");
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
    parsed.accounts_structs.push(
        quasar_idl::parser::accounts::RawAccountsStruct {
            name: "Transfer".to_string(),
            fields: vec![
                RawAccountField {
                    name: "from".to_string(),
                    writable: true,
                    signer: true,
                    pda: None,
                    address: None,
                },
                RawAccountField {
                    name: "to".to_string(),
                    writable: true,
                    signer: false,
                    pda: None,
                    address: None,
                },
                RawAccountField {
                    name: "authority".to_string(),
                    writable: false,
                    signer: true,
                    pda: None,
                    address: None,
                },
            ],
        },
    );
    let code = generate_client(&parsed);

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
// This is the critical wire compatibility test. DynString and DynVec
// must map to DynBytes/DynVec<T> (u32 LE prefix), NOT to plain
// Vec<u8>/Vec<T> whose wincode encoding may differ.
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
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub name: DynBytes,"),
        "DynString must map to DynBytes for u32 LE wire compat, got:\n{code}"
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
    let code = generate_client(&parsed);

    assert!(
        code.contains("use quasar_lang::client::{DynBytes, DynVec, TailBytes};"),
        "dynamic types must be imported: {code}"
    );
}

// ---------------------------------------------------------------------------
// Instruction codegen: prefix-width variants for dynamic types
//
// Verifies that String<P, N> / Vec<T, P, N> with non-default prefix
// types produce DynBytes<P> / DynVec<T, P> in generated code.
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_dyn_bytes_u8_prefix() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<u8, 100>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub name: DynBytes<u8>,"),
        "String<u8, N> must map to DynBytes<u8>, got:\n{code}"
    );
}

#[test]
fn rust_codegen_dyn_bytes_u16_prefix() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_name(ctx: Ctx<SetName>, name: String<u16, 1000>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub name: DynBytes<u16>,"),
        "String<u16, N> must map to DynBytes<u16>, got:\n{code}"
    );
}

#[test]
fn rust_codegen_dyn_bytes_u32_default() {
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
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub name: DynBytes,"),
        "String<N> (default u32) must map to DynBytes (no generic param), got:\n{code}"
    );
    assert!(
        !code.contains("DynBytes<u32>"),
        "default u32 should omit the generic, got:\n{code}"
    );
}

#[test]
fn rust_codegen_dyn_vec_u8_prefix() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_tags(ctx: Ctx<SetTags>, tags: Vec<u64, u8, 10>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub tags: DynVec<u64, u8>,"),
        "Vec<u64, u8, N> must map to DynVec<u64, u8>, got:\n{code}"
    );
}

#[test]
fn rust_codegen_dyn_vec_u16_prefix() {
    let file = parse_file(
        r#"
        #[program]
        mod my_program {
            #[instruction(discriminator = [1])]
            pub fn set_tags(ctx: Ctx<SetTags>, tags: Vec<u64, u16, 500>) -> Result<(), ProgramError> {
                Ok(())
            }
        }
        "#,
    );
    let (_, instructions) = program::extract_program_module(&file).unwrap();
    let mut parsed = test_program();
    parsed.instructions = instructions;
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub tags: DynVec<u64, u16>,"),
        "Vec<u64, u16, N> must map to DynVec<u64, u16>, got:\n{code}"
    );
}

#[test]
fn rust_codegen_dyn_vec_u32_default() {
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
    let code = generate_client(&parsed);

    assert!(
        code.contains("pub tags: DynVec<u64>,"),
        "Vec<u64, N> (default u32) must map to DynVec<u64> (no prefix param), got:\n{code}"
    );
    assert!(
        !code.contains("DynVec<u64, u32>"),
        "default u32 should omit the prefix generic, got:\n{code}"
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
    let code = generate_client(&parsed);

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
    let code = generate_client(&parsed);

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
    let code = generate_client(&parsed);

    // Discriminator constants
    assert!(
        code.contains("ESCROW_ACCOUNT_DISCRIMINATOR: &[u8] = &[1]"),
        "{code}"
    );
    assert!(
        code.contains("EMPTY_ACCOUNT_DISCRIMINATOR: &[u8] = &[2]"),
        "{code}"
    );

    // Struct with wincode derives and repr(C)
    assert!(
        code.contains("#[derive(Clone, Copy, SchemaWrite, SchemaRead)]\n#[repr(C)]\npub struct Escrow {"),
        "{code}"
    );
    assert!(code.contains("pub maker: Address,"), "{code}");
    assert!(code.contains("pub amount: u64,"), "{code}");

    // Enum
    assert!(code.contains("Escrow(Escrow),"), "{code}");
    assert!(code.contains("Empty,"), "{code}");

    // Decoder uses wincode::deserialize, no manual offset tracking
    assert!(code.contains("pub fn decode_account"), "{code}");
    assert!(
        code.contains("wincode::deserialize::<Escrow>(payload)"),
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
    let code = generate_client(&parsed);

    // Discriminator constants
    assert!(
        code.contains("TRADE_EXECUTED_EVENT_DISCRIMINATOR: &[u8] = &[10]"),
        "{code}"
    );
    assert!(
        code.contains("ORDER_CANCELLED_EVENT_DISCRIMINATOR: &[u8] = &[5]"),
        "{code}"
    );

    // Struct with wincode derives
    assert!(
        code.contains("#[derive(SchemaWrite, SchemaRead)]\npub struct TradeExecuted {"),
        "{code}"
    );
    assert!(code.contains("pub maker: Address,"), "{code}");
    assert!(code.contains("pub amount: u64,"), "{code}");

    // Enum
    assert!(code.contains("TradeExecuted(TradeExecuted),"), "{code}");
    assert!(code.contains("OrderCancelled,"), "{code}");

    // Decoder uses wincode, no manual offset tracking
    assert!(code.contains("pub fn decode_event"), "{code}");
    assert!(
        code.contains("wincode::deserialize::<TradeExecuted>(payload)"),
        "{code}"
    );
    assert!(!code.contains("let mut offset"), "{code}");
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
        args: vec![(
            "config".to_string(),
            syn::parse_str("OrderConfig").unwrap(),
        )],
        has_remaining: false,
    });
    let code = generate_client(&parsed);

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
        code.contains("wincode::serialize(&ix.config)"),
        "{code}"
    );
}

// ---------------------------------------------------------------------------
// Cargo.toml generation
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_cargo_toml() {
    use quasar_idl::codegen::rust::generate_cargo_toml;
    let toml = generate_cargo_toml("my-program", "0.1.0");
    assert!(toml.contains("name = \"my-program-client\""), "{toml}");
    assert!(toml.contains("version = \"0.1.0\""), "{toml}");
    assert!(toml.contains("quasar-lang"), "{toml}");
    assert!(toml.contains("wincode"), "{toml}");
}

// ---------------------------------------------------------------------------
// Program ID in output
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_program_id() {
    let parsed = test_program();
    let code = generate_client(&parsed);
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
    let code = generate_client(&parsed);

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
    let code = generate_client(&parsed);

    // remaining_accounts needs Vec
    assert!(code.contains("use std::vec::Vec;"), "{code}");
}

// ===========================================================================
// TypeScript codegen
// ===========================================================================

#[test]
fn ts_codegen_remaining_accounts() {
    use quasar_idl::{
        codegen::typescript::generate_ts_client_kit,
        parser::build_idl,
    };

    let mut parsed = test_program();
    parsed.instructions.push(program::RawInstruction {
        name: "create".to_string(),
        discriminator: vec![0],
        accounts_type_name: "Create".to_string(),
        args: vec![],
        has_remaining: true,
    });

    let idl = build_idl(parsed);
    let code = generate_ts_client_kit(&idl);

    assert!(code.contains("remainingAccounts?"), "{code}");
    assert!(
        code.contains("...(input.remainingAccounts ?? [])"),
        "{code}"
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

    let idl = build_idl(parsed);
    let json = serde_json::to_string_pretty(&idl).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(value["instructions"][0]["hasRemaining"], true);
    assert!(value["instructions"][1].get("hasRemaining").is_none());
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
