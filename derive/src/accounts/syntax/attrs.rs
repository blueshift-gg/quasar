//! Directive parser: grammar only, no semantic decisions.
//!
//! Grammar summary:
//! - core: `mut`, `dup`, `init`, `init(idempotent)`, `payer = ident`, `address
//!   = expr`, `realloc = expr`, `close(dest = ident)`
//! - behavior: `path(arg = value, ...)`
//! - check: `has_one(...)`, `constraints(...)`
//!
//! Phase placement is NOT part of the user syntax. No `pre(...)` or
//! `exit(...)`. The lowering layer decides which phases each behavior
//! participates in.
//!
//! All groups are open behavior groups. The derive is protocol-neutral: it
//! does not know what `token`, `mint`, or `metadata` mean.

use {
    super::super::resolve::{BehaviorArg, BehaviorGroup, UserCheck},
    syn::{
        parse::{Parse, ParseStream},
        Expr, Ident, Token,
    },
};

/// Parsed directive from `#[account(...)]`. Core directives are structural
/// (owned by the derive); behavior directives are protocol-owned (lowered to
/// trait calls).
pub(crate) enum Directive {
    Core(CoreDirective),
    Behavior(BehaviorGroup),
    Check(UserCheck),
}

/// Core structural directives: owned by the derive, not by protocol crates.
pub(crate) enum CoreDirective {
    Mut,
    Dup,
    Group,
    Init { idempotent: bool },
    Payer(Ident),
    Address(Expr, Option<Expr>),
    Realloc(Expr),
    Close(Ident),
}

struct ParsedDirective {
    inner: Directive,
}

impl Parse for ParsedDirective {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // `mut` is a Rust keyword, so it cannot be parsed through `syn::Path`.
        if input.peek(Token![mut]) {
            let _kw: Token![mut] = input.parse()?;
            return Ok(ParsedDirective {
                inner: Directive::Core(CoreDirective::Mut),
            });
        }

        let path: syn::Path = input.parse()?;
        let name = path_to_string(&path);

        // Key-value directives: `name = value`.
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            match name.as_str() {
                "payer" => {
                    let ident: Ident = input.parse()?;
                    return Ok(ParsedDirective {
                        inner: Directive::Core(CoreDirective::Payer(ident)),
                    });
                }
                "address" => {
                    let expr: Expr = input.parse()?;
                    let error = parse_trailing_error(input)?;
                    return Ok(ParsedDirective {
                        inner: Directive::Core(CoreDirective::Address(expr, error)),
                    });
                }
                "realloc" => {
                    let expr: Expr = input.parse()?;
                    return Ok(ParsedDirective {
                        inner: Directive::Core(CoreDirective::Realloc(expr)),
                    });
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &path,
                        format!("unknown key-value directive `{name} = ...`"),
                    ));
                }
            }
        }

        // Group directives and structural checks: `name(...)`.
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
                    return Ok(ParsedDirective {
                        inner: Directive::Core(CoreDirective::Init { idempotent }),
                    });
                }

                "has_one" => {
                    let targets = parse_ident_list(&content)?;
                    let error = parse_trailing_error(input)?;
                    return Ok(ParsedDirective {
                        inner: Directive::Check(UserCheck::HasOne { targets, error }),
                    });
                }
                "constraints" => {
                    let exprs = parse_expr_list(&content)?;
                    let error = parse_trailing_error(input)?;
                    return Ok(ParsedDirective {
                        inner: Directive::Check(UserCheck::Constraints { exprs, error }),
                    });
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
                            return Ok(ParsedDirective {
                                inner: Directive::Core(CoreDirective::Close(
                                    ep.path.segments[0].ident.clone(),
                                )),
                            });
                        }
                    }
                    return Err(syn::Error::new_spanned(
                        &dest.value,
                        "`close(dest = ...)` must be a field name",
                    ));
                }

                _ => {
                    let args = parse_group_args(&content)?;
                    return Ok(ParsedDirective {
                        inner: Directive::Behavior(BehaviorGroup { path, args }),
                    });
                }
            }
        }

        // Bare flags without parentheses.
        match name.as_str() {
            "init" => Ok(ParsedDirective {
                inner: Directive::Core(CoreDirective::Init { idempotent: false }),
            }),
            "dup" => Ok(ParsedDirective {
                inner: Directive::Core(CoreDirective::Dup),
            }),
            "group" => Ok(ParsedDirective {
                inner: Directive::Core(CoreDirective::Group),
            }),
            _ => Err(syn::Error::new_spanned(
                &path,
                format!("unknown bare directive `{name}`; did you mean `{name}(...)`?"),
            )),
        }
    }
}

/// Parse `key = value` pairs separated by commas.
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
pub(crate) fn validate_behavior_arg(key: &Ident, expr: &Expr) -> syn::Result<()> {
    if is_valid_behavior_arg(expr) {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            expr,
            format!(
                "behavior arg `{}` has a value that is not valid in all lifecycle phases. \
                 Behavior args must be bare field idents, literals, const paths, `Some(field)`, \
                 or `None`. Move complex expressions to `constraints(...)` or handler code.",
                key,
            ),
        ))
    }
}

/// Check if an expression conforms to the behavior arg grammar.
fn is_valid_behavior_arg(expr: &Expr) -> bool {
    match expr {
        // Path: bare ident (field ref) or multi-segment (const/type path): valid
        Expr::Path(ep) => ep.qself.is_none(),
        // Literal: valid
        Expr::Lit(_) => true,
        // Some(inner): valid if inner is valid
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

/// Parse a comma-separated list of identifiers.
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

/// Parse a comma-separated list of expressions.
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

/// Parse optional `@ error_expr` after a check directive.
fn parse_trailing_error(input: ParseStream) -> syn::Result<Option<Expr>> {
    if input.peek(Token![@]) {
        input.parse::<Token![@]>()?;
        Ok(Some(input.parse::<Expr>()?))
    } else {
        Ok(None)
    }
}

pub(crate) fn parse_field_attrs(field: &syn::Field) -> syn::Result<Vec<Directive>> {
    let mut attr = None;
    for candidate in field.attrs.iter().filter(|a| a.path().is_ident("account")) {
        if attr.replace(candidate).is_some() {
            return Err(syn::Error::new_spanned(
                candidate,
                "duplicate #[account(...)] attribute on field",
            ));
        }
    }
    match attr {
        Some(a) => {
            let directives: syn::punctuated::Punctuated<ParsedDirective, Token![,]> =
                a.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;
            Ok(directives.into_iter().map(|pd| pd.inner).collect())
        }
        None => Ok(Vec::new()),
    }
}

fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
