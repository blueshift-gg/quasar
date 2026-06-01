//! Recoverable-mode behavior for the `#[derive(Accounts)]` parsers:
//! per-directive recovery, validate_behavior_arg as diagnostic, instruction
//! args with continued parsing past errors.

use quasar_syntax::{
    accounts::{
        parse_field_attrs, parse_field_attrs_recoverable,
        parse_struct_instruction_args_recoverable, validate_behavior_arg_recoverable,
        CoreDirective, Directive, UserCheck,
    },
    diagnostics::{DiagCode, Diagnostics},
};

fn parse_directives(field_attr_body: &str) -> (Vec<Directive>, Diagnostics) {
    // Wrap the body in a fake `#[account(...)]` attribute on a synthetic field
    // so we can use parse_field_attrs_recoverable.
    let source = format!(
        "struct __F {{ #[account({})] pub x: u8, }}",
        field_attr_body
    );
    let derive_input: syn::DeriveInput = syn::parse_str(&source).expect("synthetic struct parses");
    let field = match &derive_input.data {
        syn::Data::Struct(s) => s.fields.iter().next().unwrap(),
        _ => unreachable!(),
    };

    let mut sink = Diagnostics::new();
    let directives = parse_field_attrs_recoverable(field, &mut sink);
    (directives, sink)
}

#[test]
fn strict_round_trips_well_formed_attribute() {
    let (directives, sink) = parse_directives("mut, init, payer = signer");
    assert!(
        sink.is_empty(),
        "no diagnostics expected, got {:?}",
        sink.items()
    );
    assert_eq!(directives.len(), 3);
    assert!(matches!(directives[0], Directive::Core(CoreDirective::Mut)));
    assert!(matches!(
        directives[1],
        Directive::Core(CoreDirective::Init { idempotent: false })
    ));
    assert!(matches!(
        directives[2],
        Directive::Core(CoreDirective::Payer(_))
    ));
}

#[test]
fn recovery_keeps_directives_after_unknown_kv() {
    // `mystery = expr` is unknown; `mut` afterwards must still appear.
    let (directives, sink) = parse_directives("mystery = 42, mut");
    let kinds: Vec<_> = directives
        .iter()
        .map(|d| matches!(d, Directive::Core(CoreDirective::Mut)))
        .collect();
    assert!(
        kinds.contains(&true),
        "mut should be parsed after the unknown directive, got {} directives",
        directives.len()
    );
    assert!(sink
        .items()
        .iter()
        .any(|d| d.code == DiagCode::AccountsDirectiveMalformed));
}

#[test]
fn recovery_keeps_directives_after_unknown_bare() {
    let (directives, sink) = parse_directives("nonsense, has_one(authority)");
    assert!(
        directives
            .iter()
            .any(|d| matches!(d, Directive::Check(UserCheck::HasOne { .. }))),
        "has_one should still parse after unknown bare directive"
    );
    assert!(!sink.is_empty());
}

#[test]
fn recovery_emits_malformed_for_bad_init_flag() {
    let (_directives, sink) = parse_directives("init(bogus)");
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountsDirectiveMalformed),
        "expected malformed diagnostic for init(bogus), got {:?}",
        codes
    );
}

#[test]
fn recovery_handles_behavior_group_then_unknown() {
    let (directives, sink) = parse_directives("token(mint = some_mint), bogus_kv = 1");
    assert!(directives
        .iter()
        .any(|d| matches!(d, Directive::Behavior(_))));
    assert!(sink
        .items()
        .iter()
        .any(|d| d.code == DiagCode::AccountsDirectiveMalformed));
}

#[test]
fn strict_mode_returns_first_error_as_syn_error() {
    let source = "struct __F { #[account(mystery = 1)] pub x: u8, }";
    let derive_input: syn::DeriveInput = syn::parse_str(source).unwrap();
    let field = match &derive_input.data {
        syn::Data::Struct(s) => s.fields.iter().next().unwrap(),
        _ => unreachable!(),
    };
    let err = parse_field_attrs(field).expect_err("strict mode must err on unknown directive");
    assert!(
        err.to_string().contains("unknown key-value directive"),
        "got: {}",
        err
    );
}

#[test]
fn validate_behavior_arg_recoverable_emits_for_invalid_value() {
    let key: syn::Ident = syn::parse_str("amount").unwrap();
    // method call is rejected by the behavior arg grammar
    let expr: syn::Expr = syn::parse_str("self.amount.checked_add(1)").unwrap();
    let mut sink = Diagnostics::new();
    validate_behavior_arg_recoverable(&key, &expr, &mut sink);
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountsBehaviorArgInvalid),
        "expected behavior-arg-invalid diagnostic, got {:?}",
        codes
    );
}

#[test]
fn validate_behavior_arg_recoverable_silent_on_valid_value() {
    let key: syn::Ident = syn::parse_str("authority").unwrap();
    let expr: syn::Expr = syn::parse_str("authority").unwrap();
    let mut sink = Diagnostics::new();
    validate_behavior_arg_recoverable(&key, &expr, &mut sink);
    assert!(sink.is_empty());
}

#[test]
fn instruction_args_recovery_keeps_args_after_malformed() {
    let source = r#"
        #[instruction(amount: u64, : invalid, recipient: Address)]
        struct __X { }
    "#;
    let derive_input: syn::DeriveInput = syn::parse_str(source).unwrap();
    let mut sink = Diagnostics::new();
    let args = parse_struct_instruction_args_recoverable(&derive_input, &mut sink)
        .expect("attribute exists");
    let names: Vec<_> = args.iter().map(|a| a.name.to_string()).collect();
    assert!(
        names.contains(&"amount".to_string()),
        "amount arg should parse, got {:?}",
        names
    );
    assert!(
        names.contains(&"recipient".to_string()),
        "recipient should parse despite middle arg being malformed, got {:?}",
        names
    );
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(codes.contains(&DiagCode::InstructionArgMalformed));
}

#[test]
fn instruction_args_recovery_emits_duplicate() {
    let source = r#"
        #[instruction(amount: u64, amount: u32)]
        struct __X { }
    "#;
    let derive_input: syn::DeriveInput = syn::parse_str(source).unwrap();
    let mut sink = Diagnostics::new();
    let args = parse_struct_instruction_args_recoverable(&derive_input, &mut sink)
        .expect("attribute exists");
    assert_eq!(args.len(), 1, "duplicate should not be kept in args list");
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(codes.contains(&DiagCode::InstructionArgDuplicate));
}

#[test]
fn instruction_args_no_attribute_returns_none() {
    let derive_input: syn::DeriveInput = syn::parse_str("struct __X { }").unwrap();
    let mut sink = Diagnostics::new();
    let result = parse_struct_instruction_args_recoverable(&derive_input, &mut sink);
    assert!(result.is_none());
    assert!(sink.is_empty());
}

#[test]
fn duplicate_account_attribute_emits_diagnostic() {
    let source = r#"
        struct __F {
            #[account(mut)]
            #[account(init)]
            pub x: u8,
        }
    "#;
    let derive_input: syn::DeriveInput = syn::parse_str(source).unwrap();
    let field = match &derive_input.data {
        syn::Data::Struct(s) => s.fields.iter().next().unwrap(),
        _ => unreachable!(),
    };
    let mut sink = Diagnostics::new();
    let _ = parse_field_attrs_recoverable(field, &mut sink);
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountsDirectiveDuplicate),
        "expected duplicate diagnostic, got {:?}",
        codes
    );
}
