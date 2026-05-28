//! Directive parser for `#[account(...)]` on `#[derive(Accounts)]` fields.
//!
//! Grammar:
//! - core: `mut`, `dup`, `init`, `init(idempotent)`, `payer = ident`,
//!   `address = expr`, `realloc = expr`, `close(dest = ident)`
//! - behavior: `path(arg = value, ...)`
//! - check: `has_one(...)`, `constraints(...)`
//!
//! [`parse_field_attrs`] returns the first error as `syn::Error`.
//! [`parse_field_attrs_recoverable`] returns a (possibly partial) directive
//! list and pushes every error to a [`Diagnostics`] sink so a single broken
//! directive doesn't discard the rest of the attribute body.

use super::ast::{BehaviorArg, BehaviorGroup, CoreDirective, Directive, UserCheck};
use crate::diagnostics::{DiagCode, Diagnostic, Diagnostics, Severity};
use proc_macro2::TokenTree;
use syn::{
    parse::{discouraged::Speculative, ParseStream, Parser as _},
    Expr, Ident, Token,
};

pub fn parse_field_attrs(field: &syn::Field) -> syn::Result<Vec<Directive>> {
    let mut sink = Diagnostics::new();
    let directives = parse_field_attrs_recoverable(field, &mut sink);
    if let Some(first) = sink.into_items().into_iter().next() {
        return Err(syn::Error::new(first.primary, first.message));
    }
    Ok(directives)
}

pub fn parse_field_attrs_recoverable(
    field: &syn::Field,
    sink: &mut Diagnostics,
) -> Vec<Directive> {
    let mut chosen: Option<&syn::Attribute> = None;
    for candidate in field.attrs.iter().filter(|a| a.path().is_ident("account")) {
        if chosen.is_some() {
            sink.emit(Diagnostic {
                severity: Severity::Error,
                code: DiagCode::AccountsDirectiveDuplicate,
                message: "duplicate #[account(...)] attribute on field".into(),
                primary: candidate.path().get_ident().map_or_else(
                    || candidate.path().segments[0].ident.span(),
                    syn::spanned::Spanned::span,
                ),
                labels: vec![],
                fixes: vec![],
            });
            continue;
        }
        chosen = Some(candidate);
    }

    let Some(attr) = chosen else { return Vec::new() };

    let tokens = match &attr.meta {
        syn::Meta::List(list) => list.tokens.clone(),
        _ => return Vec::new(),
    };

    let mut out = Vec::new();
    // Closure can't capture &mut out and &mut sink directly through Parser, so
    // we work via interior shape: a stateful parser that owns the references.
    let parsed: syn::Result<()> = (|input: ParseStream| -> syn::Result<()> {
        parse_directives_recoverable(input, &mut out, sink);
        Ok(())
    })
    .parse2(tokens);
    let _ = parsed;
    out
}

fn parse_directives_recoverable(
    input: ParseStream,
    out: &mut Vec<Directive>,
    sink: &mut Diagnostics,
) {
    while !input.is_empty() {
        let fork = input.fork();
        match parse_one_directive(&fork) {
            Ok(directive) => {
                input.advance_to(&fork);
                out.push(directive);
            }
            Err(err) => {
                let span = err.span();
                sink.emit(Diagnostic {
                    severity: Severity::Error,
                    code: DiagCode::AccountsDirectiveMalformed,
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
                code: DiagCode::AccountsDirectiveMalformed,
                message: "expected `,` between account directives".into(),
                primary: stray,
                labels: vec![],
                fixes: vec![],
            });
            sink.mark_parse_failed(stray);
            skip_to_next_directive(input);
        }
    }
}

fn parse_one_directive(input: ParseStream) -> syn::Result<Directive> {
    if input.peek(Token![mut]) {
        let _kw: Token![mut] = input.parse()?;
        return Ok(Directive::Core(CoreDirective::Mut));
    }

    let path: syn::Path = input.parse()?;
    let name = path_to_string(&path);

    if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;
        match name.as_str() {
            "payer" => {
                let ident: Ident = input.parse()?;
                return Ok(Directive::Core(CoreDirective::Payer(ident)));
            }
            "address" => {
                let expr: Expr = input.parse()?;
                let error = parse_trailing_error(input)?;
                return Ok(Directive::Core(CoreDirective::Address(expr, error)));
            }
            "realloc" => {
                let expr: Expr = input.parse()?;
                return Ok(Directive::Core(CoreDirective::Realloc(expr)));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    &path,
                    format!("unknown key-value directive `{name} = ...`"),
                ));
            }
        }
    }

    if input.peek(syn::token::Paren) {
        let content;
        syn::parenthesized!(content in input);

        match name.as_str() {
            "init" => {
                let idempotent = if content.is_empty() {
                    false
                } else {
                    let flag: Ident = content.parse()?;
                    if flag != "idempotent" {
                        return Err(syn::Error::new_spanned(
                            &flag,
                            format!(
                                "unknown init flag `{flag}`. Only `init` or \
                                 `init(idempotent)` are valid."
                            ),
                        ));
                    }
                    if !content.is_empty() {
                        let _: Token![,] = content.parse()?;
                        return Err(syn::Error::new(
                            content.span(),
                            "`init(idempotent)` does not accept additional arguments",
                        ));
                    }
                    true
                };
                return Ok(Directive::Core(CoreDirective::Init { idempotent }));
            }
            "has_one" => {
                let targets = parse_ident_list(&content)?;
                let error = parse_trailing_error(input)?;
                return Ok(Directive::Check(UserCheck::HasOne { targets, error }));
            }
            "constraints" => {
                let exprs = parse_expr_list(&content)?;
                let error = parse_trailing_error(input)?;
                return Ok(Directive::Check(UserCheck::Constraints { exprs, error }));
            }
            "close" => {
                let args = parse_group_args(&content)?;
                if args.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        &path,
                        "`close(...)` accepts only `dest = field`",
                    ));
                }
                let dest = &args[0];
                if dest.key != "dest" {
                    return Err(syn::Error::new_spanned(
                        &path,
                        "`close(...)` requires `dest = field`",
                    ));
                }
                if let Expr::Path(ep) = &dest.value {
                    if ep.qself.is_none() && ep.path.segments.len() == 1 {
                        return Ok(Directive::Core(CoreDirective::Close(
                            ep.path.segments[0].ident.clone(),
                        )));
                    }
                }
                return Err(syn::Error::new_spanned(
                    &dest.value,
                    "`close(dest = ...)` must be a field name",
                ));
            }
            _ => {
                let args = parse_group_args(&content)?;
                return Ok(Directive::Behavior(BehaviorGroup { path, args }));
            }
        }
    }

    match name.as_str() {
        "init" => Ok(Directive::Core(CoreDirective::Init { idempotent: false })),
        "dup" => Ok(Directive::Core(CoreDirective::Dup)),
        "group" => Ok(Directive::Core(CoreDirective::Group)),
        _ => Err(syn::Error::new_spanned(
            &path,
            format!("unknown bare directive `{name}`; did you mean `{name}(...)`?"),
        )),
    }
}

fn parse_group_args(input: ParseStream) -> syn::Result<Vec<BehaviorArg>> {
    let mut args = Vec::new();
    while !input.is_empty() {
        let key: Ident = input.parse()?;

        if !input.peek(Token![=]) {
            return Err(syn::Error::new_spanned(
                &key,
                format!(
                    "behavior arg `{key}` requires a value: `{key} = ...`. Bare flags are not \
                     supported in behavior groups",
                ),
            ));
        }
        input.parse::<Token![=]>()?;
        let value: Expr = input.parse()?;

        if args.iter().any(|a: &BehaviorArg| a.key == key) {
            return Err(syn::Error::new_spanned(
                &key,
                format!("duplicate arg `{key}`: each arg may only appear once"),
            ));
        }
        args.push(BehaviorArg { key, value });

        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(args)
}

/// Validate that a behavior arg value conforms to the phase-polymorphic
/// grammar.
///
/// Allowed forms (valid in raw-slot, typed, and epilogue contexts):
/// - Bare field ident: `authority`
/// - Literal: `true`, `42`, `"str"`
/// - Const/type path: `MY_CONST`, `module::Type`
/// - `Some(valid_arg)`: Option wrapper with a valid inner
/// - `None`: empty option
///
/// Banned: method calls, field paths, casts, arithmetic, instruction args.
/// These belong in `constraints(...)` or handler code.
pub fn validate_behavior_arg(key: &Ident, expr: &Expr) -> syn::Result<()> {
    if is_valid_behavior_arg(expr) {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            expr,
            invalid_behavior_arg_message(key),
        ))
    }
}

pub fn validate_behavior_arg_recoverable(key: &Ident, expr: &Expr, sink: &mut Diagnostics) {
    if !is_valid_behavior_arg(expr) {
        sink.emit(Diagnostic {
            severity: Severity::Error,
            code: DiagCode::AccountsBehaviorArgInvalid,
            message: invalid_behavior_arg_message(key),
            primary: syn::spanned::Spanned::span(expr),
            labels: vec![],
            fixes: vec![],
        });
    }
}

fn invalid_behavior_arg_message(key: &Ident) -> String {
    format!(
        "behavior arg `{}` has a value that is not valid in all lifecycle phases. \
         Behavior args must be bare field idents, literals, const paths, `Some(field)`, \
         or `None`. Move complex expressions to `constraints(...)` or handler code.",
        key,
    )
}

fn is_valid_behavior_arg(expr: &Expr) -> bool {
    match expr {
        Expr::Path(ep) => ep.qself.is_none(),
        Expr::Lit(_) => true,
        Expr::Call(call) => {
            if let Expr::Path(func) = &*call.func {
                if func.qself.is_none()
                    && func.path.segments.len() == 1
                    && func.path.segments[0].ident == "Some"
                {
                    return call.args.len() == 1 && call.args.iter().all(is_valid_behavior_arg);
                }
            }
            false
        }
        _ => false,
    }
}

fn parse_ident_list(input: ParseStream) -> syn::Result<Vec<Ident>> {
    let mut idents = Vec::new();
    while !input.is_empty() {
        idents.push(input.parse::<Ident>()?);
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(idents)
}

fn parse_expr_list(input: ParseStream) -> syn::Result<Vec<Expr>> {
    let mut exprs = Vec::new();
    while !input.is_empty() {
        exprs.push(input.parse::<Expr>()?);
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(exprs)
}

fn parse_trailing_error(input: ParseStream) -> syn::Result<Option<Expr>> {
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        Ok(Some(input.parse::<Expr>()?))
    } else {
        Ok(None)
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn skip_to_next_directive(input: ParseStream) {
    while !input.is_empty() && !input.peek(Token![,]) {
        let _ = input.parse::<TokenTree>();
    }
}
