//! Parser for `#[account(...)]` attribute arguments.
//!
//! [`parse_strict`] returns the first error as `syn::Error`.
//! [`parse_recoverable`] returns a (possibly partial) AST and pushes every
//! error to a [`Diagnostics`] sink so a single broken directive doesn't
//! discard the rest of the input.
//!
//! Recovery speculatively parses each directive on a fork; on failure the
//! real stream advances to the next sync point (comma at attr-body depth).

use crate::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity};
use proc_macro2::{Span, TokenTree};
use syn::{
    parse::{discouraged::Speculative, Parse, ParseStream},
    spanned::Spanned,
    Expr, Result as SynResult, Token,
};

#[derive(Debug, Default)]
pub struct AccountAttrAst {
    pub discriminator: Option<Discriminator>,
    pub one_of: Option<Span>,
    pub unsafe_no_disc: Option<Span>,
    pub fixed_capacity: Option<Span>,
}

#[derive(Debug, Clone)]
pub enum Discriminator {
    Int { value: u64, span: Span },
    Bytes { values: Vec<u8>, span: Span },
}

/// Wrapper to expose `parse_strict` via `syn::parse_macro_input!`.
pub struct AccountAttr(pub AccountAttrAst);

impl Parse for AccountAttr {
    fn parse(input: ParseStream) -> SynResult<Self> {
        parse_strict(input).map(AccountAttr)
    }
}

pub fn parse_strict(input: ParseStream) -> SynResult<AccountAttrAst> {
    let mut sink = Diagnostics::new();
    let ast = parse_recoverable(input, &mut sink);
    if let Some(first) = sink.into_items().into_iter().next() {
        return Err(syn::Error::new(first.primary, first.message));
    }
    Ok(ast)
}

pub fn parse_recoverable(input: ParseStream, sink: &mut Diagnostics) -> AccountAttrAst {
    let mut ast = AccountAttrAst::default();

    while !input.is_empty() {
        let fork = input.fork();
        match parse_one_directive(&fork) {
            Ok(directive) => {
                input.advance_to(&fork);
                apply_directive(&mut ast, directive, sink);
            }
            Err(err) => {
                let span = err.span();
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::AccountAttrMalformedDirective,
                    message: err.to_string(),
                    primary: span,
                    labels: vec![],
                    fixes: vec![],
                });
                sink.mark_parse_failed(span);
                skip_to_next_directive(input);
            }
        }

        if input.is_empty() {
            break;
        }
        if input.peek(Token![,]) {
            let _: Token![,] = input
                .parse()
                .expect("peek(Token![,]) succeeded, parse must too");
        } else {
            // Stray content between directives — emit and recover by skipping
            // to the next sync point. Ensures the parser always consumes its
            // entire input even on pathological cases like `one_of bogus`.
            let stray = input.span();
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountAttrMalformedDirective,
                message: "expected `,` between account directives".into(),
                primary: stray,
                labels: vec![],
                fixes: vec![],
            });
            sink.mark_parse_failed(stray);
            skip_to_next_directive(input);
        }
    }

    ast
}

#[derive(Debug)]
enum ParsedDirective {
    Discriminator(Discriminator),
    OneOf(Span),
    UnsafeNoDisc(Span),
    FixedCapacity(Span),
    Unknown { name: String, span: Span },
}

fn parse_one_directive(input: ParseStream) -> SynResult<ParsedDirective> {
    let ident: syn::Ident = input.parse()?;
    let name = ident.to_string();
    let span = ident.span();

    match name.as_str() {
        "discriminator" => {
            let _: Token![=] = input.parse()?;
            let expr: Expr = input.parse()?;
            let disc = parse_discriminator(&expr)?;
            Ok(ParsedDirective::Discriminator(disc))
        }
        "one_of" => Ok(ParsedDirective::OneOf(span)),
        "unsafe_no_disc" => Ok(ParsedDirective::UnsafeNoDisc(span)),
        "fixed_capacity" => Ok(ParsedDirective::FixedCapacity(span)),
        _ => Ok(ParsedDirective::Unknown { name, span }),
    }
}

fn apply_directive(ast: &mut AccountAttrAst, d: ParsedDirective, sink: &mut Diagnostics) {
    match d {
        ParsedDirective::Discriminator(disc) => {
            if ast.discriminator.is_some() {
                let span = match &disc {
                    Discriminator::Int { span, .. } => *span,
                    Discriminator::Bytes { span, .. } => *span,
                };
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::AccountAttrDuplicateDirective,
                    message: "duplicate `discriminator` directive".into(),
                    primary: span,
                    labels: vec![],
                    fixes: vec![],
                });
            } else {
                ast.discriminator = Some(disc);
            }
        }
        ParsedDirective::OneOf(s) => {
            if ast.one_of.is_none() {
                ast.one_of = Some(s);
            }
        }
        ParsedDirective::UnsafeNoDisc(s) => {
            if ast.unsafe_no_disc.is_none() {
                ast.unsafe_no_disc = Some(s);
            }
        }
        ParsedDirective::FixedCapacity(s) => {
            if ast.fixed_capacity.is_none() {
                ast.fixed_capacity = Some(s);
            }
        }
        ParsedDirective::Unknown { name, span } => {
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountAttrUnknownDirective,
                message: format!("unknown account directive `{}`", name),
                primary: span,
                labels: vec![],
                fixes: vec![],
            });
        }
    }
}

fn parse_discriminator(expr: &Expr) -> SynResult<Discriminator> {
    match expr {
        Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Int(int) => {
                let value: u64 = int.base10_parse()?;
                Ok(Discriminator::Int {
                    value,
                    span: int.span(),
                })
            }
            _ => Err(syn::Error::new_spanned(
                expr,
                "discriminator must be an integer literal or byte array",
            )),
        },
        Expr::Array(arr) => {
            let mut values = Vec::with_capacity(arr.elems.len());
            for e in &arr.elems {
                let Expr::Lit(lit) = e else {
                    return Err(syn::Error::new_spanned(
                        e,
                        "discriminator bytes must be integer literals",
                    ));
                };
                let syn::Lit::Int(int) = &lit.lit else {
                    return Err(syn::Error::new_spanned(
                        e,
                        "discriminator bytes must be integer literals",
                    ));
                };
                let v: u64 = int.base10_parse()?;
                if v > u8::MAX as u64 {
                    return Err(syn::Error::new_spanned(
                        e,
                        format!("discriminator byte {} out of range (max 255)", v),
                    ));
                }
                values.push(v as u8);
            }
            Ok(Discriminator::Bytes {
                values,
                span: arr.bracket_token.span.span(),
            })
        }
        _ => Err(syn::Error::new_spanned(
            expr,
            "discriminator must be an integer literal or byte array",
        )),
    }
}

/// Advance past the current malformed directive to the next comma at attr-body
/// depth. The outer `#[account(...)]` parens are already balanced by syn at the
/// attribute level, so depth-tracking inside is not needed — we only stop at
/// top-level commas.
fn skip_to_next_directive(input: ParseStream) {
    while !input.is_empty() && !input.peek(Token![,]) {
        let _ = input.parse::<TokenTree>();
    }
}
