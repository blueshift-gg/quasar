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
    };

    let collisions = find_discriminator_collisions(&parsed);
    // 3 instructions with same disc → 3 pairwise collisions: (a,b), (a,c), (b,c)
    assert_eq!(collisions.len(), 3, "should detect all pairwise collisions");
}

// ---------------------------------------------------------------------------
// Rust codegen: events
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_events() {
    use quasar_idl::{codegen::rust::generate_client, parser::ParsedProgram};

    let evts = events::extract_events(&parse_file(
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

    let parsed = ParsedProgram {
        program_id: "ABcDeFgH111111111111111111111111111111111111".to_string(),
        program_name: "test_program".to_string(),
        crate_name: "test-program".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: evts,
        errors: vec![],
    };

    let code = generate_client(&parsed);

    // Event discriminator constants
    assert!(
        code.contains("TRADE_EXECUTED_EVENT_DISCRIMINATOR"),
        "should generate TradeExecuted discriminator constant"
    );
    assert!(
        code.contains("ORDER_CANCELLED_EVENT_DISCRIMINATOR"),
        "should generate OrderCancelled discriminator constant"
    );

    // Event struct with fields
    assert!(
        code.contains("pub struct TradeExecuted"),
        "should generate TradeExecuted struct"
    );
    assert!(
        code.contains("pub maker: Address"),
        "should generate maker field"
    );
    assert!(
        code.contains("pub amount: u64"),
        "should generate amount field"
    );

    // Event struct without fields
    assert!(
        code.contains("pub struct OrderCancelled"),
        "should generate empty OrderCancelled struct"
    );

    // Event enum
    assert!(
        code.contains("pub enum ProgramEvent"),
        "should generate ProgramEvent enum"
    );
    assert!(
        code.contains("TradeExecuted(TradeExecuted)"),
        "should have TradeExecuted variant with data"
    );
    assert!(
        code.contains("OrderCancelled,"),
        "should have OrderCancelled variant without data"
    );

    // Decoder function
    assert!(
        code.contains("pub fn decode_event"),
        "should generate decode_event function"
    );
    assert!(
        code.contains("data.starts_with(TRADE_EXECUTED_EVENT_DISCRIMINATOR)"),
        "should match TradeExecuted discriminator"
    );
    assert!(
        code.contains("data.starts_with(ORDER_CANCELLED_EVENT_DISCRIMINATOR)"),
        "should match OrderCancelled discriminator"
    );
}

// ---------------------------------------------------------------------------
// Rust codegen: remaining accounts
// ---------------------------------------------------------------------------

#[test]
fn rust_codegen_remaining_accounts() {
    use quasar_idl::{codegen::rust::generate_client, parser::ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "ABcDeFgH111111111111111111111111111111111111".to_string(),
        program_name: "test_program".to_string(),
        crate_name: "test-program".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![
            program::RawInstruction {
                name: "create".to_string(),
                discriminator: vec![0],
                accounts_type_name: "Create".to_string(),
                args: vec![],
                has_remaining: true,
            },
            program::RawInstruction {
                name: "deposit".to_string(),
                discriminator: vec![1],
                accounts_type_name: "Deposit".to_string(),
                args: vec![],
                has_remaining: false,
            },
        ],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![],
        errors: vec![],
    };

    let code = generate_client(&parsed);

    // Instruction with remaining should have the field and extend
    assert!(
        code.contains("pub remaining_accounts: Vec<AccountMeta>,"),
        "CreateInstruction should have remaining_accounts field"
    );
    assert!(
        code.contains("accounts.extend(ix.remaining_accounts)"),
        "CreateInstruction should extend accounts"
    );

    // Instruction without remaining should NOT have the field
    let deposit_section = code.split("pub struct DepositInstruction").nth(1).unwrap();
    let deposit_impl = deposit_section.split("impl From").next().unwrap();
    assert!(
        !deposit_impl.contains("remaining_accounts"),
        "DepositInstruction should NOT have remaining_accounts field"
    );
}

// ---------------------------------------------------------------------------
// TypeScript codegen: remaining accounts
// ---------------------------------------------------------------------------

#[test]
fn ts_codegen_remaining_accounts() {
    use quasar_idl::{codegen::typescript::generate_ts_client_kit, parser::build_idl};
    use quasar_idl::parser::ParsedProgram;

    let parsed = ParsedProgram {
        program_id: "ABcDeFgH111111111111111111111111111111111111".to_string(),
        program_name: "test_program".to_string(),
        crate_name: "test-program".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![program::RawInstruction {
            name: "create".to_string(),
            discriminator: vec![0],
            accounts_type_name: "Create".to_string(),
            args: vec![],
            has_remaining: true,
        }],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![],
        errors: vec![],
    };

    let idl = build_idl(parsed);
    let code = generate_ts_client_kit(&idl);

    assert!(
        code.contains("remainingAccounts?"),
        "Kit InstructionInput should have remainingAccounts field"
    );
    assert!(
        code.contains("...(input.remainingAccounts ?? [])"),
        "Kit instruction builder should spread remaining accounts"
    );
}

// ---------------------------------------------------------------------------
// IDL JSON: has_remaining serialization
// ---------------------------------------------------------------------------

#[test]
fn idl_json_has_remaining_serialization() {
    use quasar_idl::parser::{build_idl, ParsedProgram};

    let parsed = ParsedProgram {
        program_id: "ABcDeFgH111111111111111111111111111111111111".to_string(),
        program_name: "test_program".to_string(),
        crate_name: "test-program".to_string(),
        version: "0.1.0".to_string(),
        instructions: vec![
            program::RawInstruction {
                name: "create".to_string(),
                discriminator: vec![0],
                accounts_type_name: "Create".to_string(),
                args: vec![],
                has_remaining: true,
            },
            program::RawInstruction {
                name: "deposit".to_string(),
                discriminator: vec![1],
                accounts_type_name: "Deposit".to_string(),
                args: vec![],
                has_remaining: false,
            },
        ],
        accounts_structs: vec![],
        state_accounts: vec![],
        events: vec![],
        errors: vec![],
    };

    let idl = build_idl(parsed);
    let json = serde_json::to_string_pretty(&idl).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Instruction with remaining should have hasRemaining: true
    assert_eq!(
        value["instructions"][0]["hasRemaining"], true,
        "create should have hasRemaining: true"
    );

    // Instruction without remaining should omit the field (skip_serializing_if)
    assert!(
        value["instructions"][1].get("hasRemaining").is_none(),
        "deposit should not have hasRemaining in JSON"
    );
}

// ---------------------------------------------------------------------------
// Parser: CtxWithRemaining detection
// ---------------------------------------------------------------------------

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
    assert!(
        instructions[0].has_remaining,
        "create should have has_remaining = true"
    );
    assert!(
        !instructions[1].has_remaining,
        "deposit should have has_remaining = false"
    );
}
