use quasar_syntax::account::{parse_recoverable, parse_strict, AccountAttrAst, Discriminator};
use quasar_syntax::diagnostics::{DiagCode, Diagnostics};
use syn::parse::{Parser, ParseStream};

/// Helper: run `parse_recoverable` against a string and return (ast, sink).
fn parse(source: &str) -> (AccountAttrAst, Diagnostics) {
    let mut diagnostics = Diagnostics::new();
    let parser = |input: ParseStream| -> syn::Result<AccountAttrAst> {
        Ok(parse_recoverable(input, &mut diagnostics))
    };
    let ast = parser.parse_str(source).expect("recoverable parse never returns Err");
    (ast, diagnostics)
}

#[test]
fn strict_mode_accepts_well_formed_input() {
    let parser = |input: ParseStream| parse_strict(input);
    let ast = parser
        .parse_str("discriminator = 7, one_of")
        .expect("well-formed input parses cleanly");
    match ast.discriminator {
        Some(Discriminator::Int { value: 7, .. }) => {}
        other => panic!("expected discriminator = 7, got {:?}", other),
    }
    assert!(ast.one_of.is_some());
}

#[test]
fn strict_mode_returns_first_error() {
    let parser = |input: ParseStream| parse_strict(input);
    let err = parser
        .parse_str("discriminator = , one_of")
        .expect_err("strict mode bails on malformed input");
    let msg = err.to_string();
    assert!(
        msg.contains("expected an expression")
            || msg.contains("expected expression")
            || msg.contains("unexpected"),
        "got: {}",
        msg
    );
}

#[test]
fn recoverable_mode_continues_past_malformed_directive() {
    // First directive is broken (no expression after `=`); recovery must skip
    // to the next comma and successfully parse `one_of`.
    let (ast, diagnostics) = parse("discriminator = , one_of");

    assert!(
        ast.discriminator.is_none(),
        "broken discriminator must not appear in AST"
    );
    assert!(
        ast.one_of.is_some(),
        "recovery must reach and parse the second directive"
    );
    assert!(
        !diagnostics.is_empty(),
        "broken input must produce at least one diagnostic"
    );
    let codes: Vec<_> = diagnostics
        .items()
        .iter()
        .map(|d| d.code)
        .collect();
    assert!(
        codes.contains(&DiagCode::AccountAttrMalformedDirective),
        "expected malformed-directive diagnostic, got {:?}",
        codes
    );
}

#[test]
fn recoverable_mode_emits_unknown_directive_diagnostic() {
    let (ast, diagnostics) = parse("bogus, one_of");
    assert!(ast.one_of.is_some(), "good directive after unknown still parses");
    let codes: Vec<_> = diagnostics.items().iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountAttrUnknownDirective),
        "expected unknown-directive diagnostic, got {:?}",
        codes
    );
}

#[test]
fn recoverable_mode_does_not_panic_on_pathological_input() {
    // Spam every shape the parser might choke on. The contract is: returns
    // *something*, doesn't panic.
    let inputs = [
        "",
        ",",
        ",,,,",
        "=",
        "= = =",
        "discriminator =",
        "discriminator = [255, 256]",
        "[1, 2, 3]",
        "one_of one_of",
        "discriminator = \"not an int\"",
    ];
    for input in inputs {
        let (_ast, _diagnostics) = parse(input);
        // Reaching here without panic is the assertion.
    }
}

#[test]
fn dedup_collapses_repeated_identical_diagnostics() {
    // Two identical malformed directives should produce two distinct diagnostics
    // (different spans). But emitting the *same* diagnostic twice should dedup.
    let mut diagnostics = Diagnostics::new();
    let parser = |input: ParseStream| -> syn::Result<AccountAttrAst> {
        Ok(parse_recoverable(input, &mut diagnostics))
    };
    parser
        .parse_str("bogus, bogus")
        .expect("recoverable parse never returns Err");

    let unknowns: Vec<_> = diagnostics
        .items()
        .iter()
        .filter(|d| d.code == DiagCode::AccountAttrUnknownDirective)
        .collect();
    assert_eq!(
        unknowns.len(),
        2,
        "two `bogus`s at different spans must produce two diagnostics"
    );
}

#[test]
fn duplicate_discriminator_is_diagnosed() {
    let (_ast, diagnostics) = parse("discriminator = 1, discriminator = 2");
    let codes: Vec<_> = diagnostics.items().iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&DiagCode::AccountAttrDuplicateDirective),
        "expected duplicate-directive diagnostic, got {:?}",
        codes
    );
}
