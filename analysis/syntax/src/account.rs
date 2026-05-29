//! Parser for `#[account(...)]` attribute arguments.
//!
//! [`parse_strict`] returns the first error as `syn::Error`.
//! [`parse_recoverable`] returns a (possibly partial) AST and pushes every
//! error to a [`Diagnostics`] sink so a single broken directive doesn't
//! discard the rest of the input.
//!
//! Recovery speculatively parses each directive on a fork; on failure the
//! real stream advances to the next sync point (comma at attr-body depth).

use {
    crate::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity},
    proc_macro2::{Span, TokenTree},
    syn::{
        parse::{discouraged::Speculative, Parse, ParseStream},
        LitInt, Path, Result as SynResult, Token,
    },
};

#[derive(Debug, Default)]
pub struct AccountAttrAst {
    pub discriminator: Option<DiscriminatorClause>,
    pub implements: Option<ImplementsClause>,
    pub one_of: Option<Span>,
    pub unsafe_no_disc: Option<Span>,
    pub set_inner: Option<Span>,
    pub fixed_capacity: Option<Span>,
}

impl AccountAttrAst {
    pub fn is_one_of(&self) -> bool {
        self.one_of.is_some()
    }
    pub fn is_unsafe_no_disc(&self) -> bool {
        self.unsafe_no_disc.is_some()
    }
    pub fn is_set_inner(&self) -> bool {
        self.set_inner.is_some()
    }
    pub fn is_fixed_capacity(&self) -> bool {
        self.fixed_capacity.is_some()
    }
    pub fn disc_bytes(&self) -> &[LitInt] {
        self.discriminator
            .as_ref()
            .map(|d| d.lits.as_slice())
            .unwrap_or(&[])
    }
    pub fn implements_path(&self) -> Option<&Path> {
        self.implements.as_ref().map(|c| &c.path)
    }
}

#[derive(Debug, Clone)]
pub struct DiscriminatorClause {
    pub lits: Vec<LitInt>,
    pub keyword_span: Span,
}

#[derive(Debug, Clone)]
pub struct ImplementsClause {
    pub path: Path,
    pub keyword_span: Span,
}

impl Parse for AccountAttrAst {
    fn parse(input: ParseStream) -> SynResult<Self> {
        parse_strict(input)
    }
}

pub fn parse_strict(input: ParseStream) -> SynResult<AccountAttrAst> {
    let mut sink = Diagnostics::new();
    let fallback_span = input.span();
    let ast = parse_recoverable(input, &mut sink);
    validate_recoverable(&ast, &mut sink, fallback_span);
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

/// Applies the cross-directive rules on top of a (possibly partial) AST and
/// pushes any violations to the sink. `fallback_span` is used when the
/// violation refers to absence rather than a specific token.
pub fn validate_recoverable(ast: &AccountAttrAst, sink: &mut Diagnostics, fallback_span: Span) {
    let has_disc = ast.discriminator.is_some();
    let has_unsafe = ast.unsafe_no_disc.is_some();
    let is_one_of = ast.one_of.is_some();

    if !is_one_of && !has_disc && !has_unsafe {
        sink.emit(Diagnostic {
            severity: Severity::Error,
            code: DiagCode::AccountAttrMissingDiscriminatorOrUnsafe,
            message: "expected `discriminator` or `unsafe_no_disc`".into(),
            primary: fallback_span,
            labels: vec![],
            fixes: vec![],
        });
    }

    if let Some(implements) = &ast.implements {
        if !is_one_of {
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountAttrImplementsRequiresOneOf,
                message: "`implements` can only be used with `one_of`".into(),
                primary: implements.keyword_span,
                labels: vec![],
                fixes: vec![],
            });
        }
    }

    if !is_one_of && has_disc && has_unsafe {
        let primary = ast
            .discriminator
            .as_ref()
            .map(|d| d.keyword_span)
            .unwrap_or(fallback_span);
        sink.emit(Diagnostic {
            severity: Severity::Error,
            code: DiagCode::AccountAttrDiscriminatorWithUnsafe,
            message: "`discriminator` cannot be combined with `unsafe_no_disc`".into(),
            primary,
            labels: vec![],
            fixes: vec![],
        });
    }

    if is_one_of && (has_disc || has_unsafe) {
        let primary = ast.one_of.unwrap_or(fallback_span);
        sink.emit(Diagnostic {
            severity: Severity::Error,
            code: DiagCode::AccountAttrOneOfWithDiscriminator,
            message: "`one_of` cannot be combined with `discriminator` or `unsafe_no_disc`".into(),
            primary,
            labels: vec![],
            fixes: vec![],
        });
    }
}

/// Converts the parsed discriminator literals into raw bytes. Each literal
/// must fit in a `u8`; otherwise an error diagnostic is pushed and the
/// offending byte is replaced with `0` in the returned vector.
pub fn parse_discriminator_bytes(disc: &DiscriminatorClause, sink: &mut Diagnostics) -> Vec<u8> {
    disc.lits
        .iter()
        .map(|lit| match lit.base10_parse::<u8>() {
            Ok(v) => v,
            Err(_) => {
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::AccountAttrDiscriminatorByteOutOfRange,
                    message: "discriminator byte must be 0-255".into(),
                    primary: lit.span(),
                    labels: vec![],
                    fixes: vec![],
                });
                0
            }
        })
        .collect()
}

#[derive(Debug)]
enum ParsedDirective {
    Discriminator(DiscriminatorClause),
    Implements(ImplementsClause),
    OneOf(Span),
    UnsafeNoDisc(Span),
    SetInner(Span),
    FixedCapacity(Span),
    Unknown { name: String, span: Span },
}

fn parse_one_directive(input: ParseStream) -> SynResult<ParsedDirective> {
    let ident: syn::Ident = input.parse()?;
    let name = ident.to_string();
    let keyword_span = ident.span();

    match name.as_str() {
        "discriminator" => {
            let _: Token![=] = input.parse()?;
            let lits = parse_discriminator_value(input)?;
            Ok(ParsedDirective::Discriminator(DiscriminatorClause {
                lits,
                keyword_span,
            }))
        }
        "implements" => {
            let content;
            syn::parenthesized!(content in input);
            let path: Path = content.parse()?;
            Ok(ParsedDirective::Implements(ImplementsClause {
                path,
                keyword_span,
            }))
        }
        "one_of" => Ok(ParsedDirective::OneOf(keyword_span)),
        "unsafe_no_disc" => Ok(ParsedDirective::UnsafeNoDisc(keyword_span)),
        "set_inner" => Ok(ParsedDirective::SetInner(keyword_span)),
        "fixed_capacity" => Ok(ParsedDirective::FixedCapacity(keyword_span)),
        _ => Ok(ParsedDirective::Unknown {
            name,
            span: keyword_span,
        }),
    }
}

fn parse_discriminator_value(input: ParseStream) -> SynResult<Vec<LitInt>> {
    if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let lits = content.parse_terminated(LitInt::parse, Token![,])?;
        let disc: Vec<LitInt> = lits.into_iter().collect();
        if disc.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "discriminator must have at least one byte",
            ));
        }
        Ok(disc)
    } else {
        let lit: LitInt = input.parse()?;
        Ok(vec![lit])
    }
}

fn apply_directive(ast: &mut AccountAttrAst, d: ParsedDirective, sink: &mut Diagnostics) {
    match d {
        ParsedDirective::Discriminator(clause) => {
            if let Some(existing) = &ast.discriminator {
                let _ = existing;
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::AccountAttrDuplicateDirective,
                    message: "duplicate `discriminator`".into(),
                    primary: clause.keyword_span,
                    labels: vec![],
                    fixes: vec![],
                });
            } else {
                ast.discriminator = Some(clause);
            }
        }
        ParsedDirective::Implements(clause) => {
            if ast.implements.is_some() {
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::AccountAttrDuplicateDirective,
                    message: "duplicate `implements`".into(),
                    primary: clause.keyword_span,
                    labels: vec![],
                    fixes: vec![],
                });
            } else {
                ast.implements = Some(clause);
            }
        }
        ParsedDirective::OneOf(span) => set_flag(&mut ast.one_of, span, "one_of", sink),
        ParsedDirective::UnsafeNoDisc(span) => {
            set_flag(&mut ast.unsafe_no_disc, span, "unsafe_no_disc", sink)
        }
        ParsedDirective::SetInner(span) => set_flag(&mut ast.set_inner, span, "set_inner", sink),
        ParsedDirective::FixedCapacity(span) => {
            set_flag(&mut ast.fixed_capacity, span, "fixed_capacity", sink)
        }
        ParsedDirective::Unknown { name, span } => {
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountAttrUnknownDirective,
                message: format!(
                    "expected `discriminator`, `unsafe_no_disc`, `set_inner`, `fixed_capacity`, \
                     `one_of`, or `implements`; found `{}`",
                    name
                ),
                primary: span,
                labels: vec![],
                fixes: vec![],
            });
        }
    }
}

fn set_flag(slot: &mut Option<Span>, span: Span, name: &str, sink: &mut Diagnostics) {
    if slot.is_some() {
        sink.emit(Diagnostic {
            severity: Severity::Error,
            code: DiagCode::AccountAttrDuplicateDirective,
            message: format!("duplicate `{}`", name),
            primary: span,
            labels: vec![],
            fixes: vec![],
        });
    } else {
        *slot = Some(span);
    }
}

/// Advance past the current malformed directive to the next comma at attr-body
/// depth. The outer `#[account(...)]` parens are already balanced by syn at the
/// attribute level, so we only stop at top-level commas.
fn skip_to_next_directive(input: ParseStream) {
    while !input.is_empty() && !input.peek(Token![,]) {
        let _ = input.parse::<TokenTree>();
    }
}
