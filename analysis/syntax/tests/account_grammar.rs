//! Covers every directive and every cross-directive rule of `#[account(...)]`
//! parsing in both strict and recoverable modes.

use quasar_syntax::account::{
    parse_discriminator_bytes, parse_recoverable, parse_strict, validate_recoverable,
    AccountAttrAst,
};
use quasar_syntax::diagnostics::{DiagCode, Diagnostics};
use syn::parse::{Parser, ParseStream};

fn strict(source: &str) -> Result<AccountAttrAst, String> {
    let parser = |input: ParseStream| parse_strict(input);
    parser.parse_str(source).map_err(|e| e.to_string())
}

fn recoverable(source: &str) -> (AccountAttrAst, Diagnostics) {
    let mut sink = Diagnostics::new();
    let parser = |input: ParseStream| -> syn::Result<AccountAttrAst> {
        Ok(parse_recoverable(input, &mut sink))
    };
    let ast = parser
        .parse_str(source)
        .expect("recoverable parse never returns Err");
    (ast, sink)
}

fn recoverable_with_validation(source: &str) -> (AccountAttrAst, Diagnostics) {
    let mut sink = Diagnostics::new();
    let parser = |input: ParseStream| -> syn::Result<AccountAttrAst> {
        let fallback = input.span();
        let ast = parse_recoverable(input, &mut sink);
        validate_recoverable(&ast, &mut sink, fallback);
        Ok(ast)
    };
    let ast = parser
        .parse_str(source)
        .expect("recoverable parse never returns Err");
    (ast, sink)
}

// ---- Each directive in isolation -------------------------------------------

#[test]
fn discriminator_single_int() {
    let ast = strict("discriminator = 5").unwrap();
    let clause = ast.discriminator.unwrap();
    assert_eq!(clause.lits.len(), 1);
    let v: u8 = clause.lits[0].base10_parse().unwrap();
    assert_eq!(v, 5);
}

#[test]
fn discriminator_byte_array() {
    let ast = strict("discriminator = [1, 2, 3, 4]").unwrap();
    let clause = ast.discriminator.unwrap();
    assert_eq!(clause.lits.len(), 4);
    let bytes: Vec<u8> = clause
        .lits
        .iter()
        .map(|l| l.base10_parse().unwrap())
        .collect();
    assert_eq!(bytes, vec![1, 2, 3, 4]);
}

#[test]
fn unsafe_no_disc_flag() {
    let ast = strict("unsafe_no_disc").unwrap();
    assert!(ast.is_unsafe_no_disc());
}

#[test]
fn set_inner_flag_requires_disc_or_unsafe() {
    // `set_inner` alone violates rule 1.
    assert!(strict("set_inner").is_err());
    let ast = strict("set_inner, unsafe_no_disc").unwrap();
    assert!(ast.is_set_inner());
}

#[test]
fn fixed_capacity_flag() {
    let ast = strict("fixed_capacity, unsafe_no_disc").unwrap();
    assert!(ast.is_fixed_capacity());
}

#[test]
fn one_of_flag() {
    let ast = strict("one_of").unwrap();
    assert!(ast.is_one_of());
}

#[test]
fn implements_path() {
    let ast = strict("one_of, implements(my::Trait)").unwrap();
    let path = ast.implements_path().unwrap();
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    assert_eq!(segments, vec!["my", "Trait"]);
}

// ---- Cross-directive rules -------------------------------------------------

#[test]
fn rule_1_missing_discriminator_or_unsafe() {
    let err = strict("set_inner").unwrap_err();
    assert!(
        err.contains("expected `discriminator` or `unsafe_no_disc`"),
        "got: {}",
        err
    );
}

#[test]
fn rule_2_implements_requires_one_of() {
    let err = strict("discriminator = 1, implements(my::T)").unwrap_err();
    assert!(
        err.contains("`implements` can only be used with `one_of`"),
        "got: {}",
        err
    );
}

#[test]
fn rule_3_discriminator_with_unsafe() {
    let err = strict("discriminator = 1, unsafe_no_disc").unwrap_err();
    assert!(
        err.contains("`discriminator` cannot be combined with `unsafe_no_disc`"),
        "got: {}",
        err
    );
}

#[test]
fn rule_4_one_of_with_discriminator() {
    let err = strict("one_of, discriminator = 1").unwrap_err();
    assert!(
        err.contains("`one_of` cannot be combined with `discriminator` or `unsafe_no_disc`"),
        "got: {}",
        err
    );
}

#[test]
fn rule_4_one_of_with_unsafe() {
    let err = strict("one_of, unsafe_no_disc").unwrap_err();
    assert!(
        err.contains("`one_of` cannot be combined with `discriminator` or `unsafe_no_disc`"),
        "got: {}",
        err
    );
}

// ---- Duplicate-directive detection ----------------------------------------

#[test]
fn duplicate_one_of() {
    let (_, sink) = recoverable("one_of, one_of");
    assert!(sink
        .items()
        .iter()
        .any(|d| d.code == DiagCode::AccountAttrDuplicateDirective));
}

#[test]
fn duplicate_unsafe_no_disc() {
    let (_, sink) = recoverable("unsafe_no_disc, unsafe_no_disc");
    assert!(sink
        .items()
        .iter()
        .any(|d| d.code == DiagCode::AccountAttrDuplicateDirective));
}

#[test]
fn duplicate_implements() {
    let (_, sink) = recoverable("one_of, implements(A), implements(B)");
    assert!(sink
        .items()
        .iter()
        .any(|d| d.code == DiagCode::AccountAttrDuplicateDirective));
}

// ---- Validation in recoverable mode produces diagnostics, not Err ---------

#[test]
fn validation_in_recoverable_mode_emits_diagnostics() {
    let (_, sink) = recoverable_with_validation("set_inner");
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountAttrMissingDiscriminatorOrUnsafe),
        "expected missing-discriminator diagnostic, got {:?}",
        codes
    );
}

#[test]
fn validation_flags_implements_without_one_of() {
    let (_, sink) = recoverable_with_validation("discriminator = 1, implements(T)");
    let codes: Vec<_> = sink.items().iter().map(|d| d.code).collect();
    assert!(codes.contains(&DiagCode::AccountAttrImplementsRequiresOneOf));
}

// ---- Discriminator byte conversion ----------------------------------------

#[test]
fn discriminator_bytes_in_range() {
    let ast = strict("discriminator = [0, 128, 255]").unwrap();
    let mut sink = Diagnostics::new();
    let bytes = parse_discriminator_bytes(ast.discriminator.as_ref().unwrap(), &mut sink);
    assert_eq!(bytes, vec![0, 128, 255]);
    assert!(sink.is_empty());
}

#[test]
fn discriminator_byte_out_of_range_emits_diagnostic() {
    // `[255, 256]` triggers the LitInt parse error during attribute parsing
    // already, but we also want `parse_discriminator_bytes` to catch overflow
    // independently when the input *did* parse as a LitInt but doesn't fit u8.
    // Construct a discriminator clause manually via the recoverable parser:
    let (ast, _) = recoverable("discriminator = 999");
    if let Some(clause) = ast.discriminator.as_ref() {
        let mut sink = Diagnostics::new();
        let _ = parse_discriminator_bytes(clause, &mut sink);
        assert!(sink
            .items()
            .iter()
            .any(|d| d.code == DiagCode::AccountAttrDiscriminatorByteOutOfRange));
    }
}

// ---- Whitespace / formatting tolerance ------------------------------------

#[test]
fn arbitrary_whitespace_between_directives() {
    let ast = strict("discriminator   =   42  ,   set_inner").unwrap();
    assert!(ast.discriminator.is_some());
    assert!(ast.is_set_inner());
}

#[test]
fn trailing_comma_is_tolerated() {
    let ast = strict("discriminator = 1,").unwrap();
    assert!(ast.discriminator.is_some());
}
