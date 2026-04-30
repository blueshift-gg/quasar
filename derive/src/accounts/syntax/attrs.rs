//! V3 directive parser for `#[account(...)]` attributes.
//!
//! Grammar:
//!   directive ::= bare_flag | init | key_value | group | check
//!   bare_flag ::= 'mut' | 'dup'
//!   init      ::= 'init' | 'init' '(' 'idempotent' ')'
//!   key_value ::= 'payer' '=' ident | 'address' '=' expr | 'realloc' '=' expr
//!   group     ::= path '(' args ')'
//!   check     ::= 'has_one' '(' ident_list ')' | 'constraints' '(' expr_list
//! ')'   args      ::= (ident '=' expr),*
//!
//! Phase placement is NOT part of the user syntax. No `pre(...)` or
//! `exit(...)`. The lowering layer decides which phases each op participates
//! in.

use {
    super::super::resolve::{GroupArg, GroupDirective, UserCheck},
    syn::{
        parse::{Parse, ParseStream},
        Expr, Ident, Token,
    },
};

pub(crate) enum Directive {
    Bare(Ident),
    Init { idempotent: bool },
    Payer(Ident),
    Address(syn::Expr, Option<syn::Expr>),
    Realloc(syn::Expr),
    Group(GroupDirective),
    Check(UserCheck),
}

struct ParsedDirective {
    inner: Directive,
}

impl Parse for ParsedDirective {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // `mut` is a keyword, needs special handling
        if input.peek(Token![mut]) {
            let kw: Token![mut] = input.parse()?;
            return Ok(ParsedDirective {
                inner: Directive::Bare(Ident::new("mut", kw.span)),
            });
        }

        let path: syn::Path = input.parse()?;
        let name = path_to_string(&path);

        // Key-value: `name = value`
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            match name.as_str() {
                "payer" => {
                    let ident: Ident = input.parse()?;
                    return Ok(ParsedDirective {
                        inner: Directive::Payer(ident),
                    });
                }
                "address" => {
                    let expr: Expr = input.parse()?;
                    let error = parse_trailing_error(input)?;
                    return Ok(ParsedDirective {
                        inner: Directive::Address(expr, error),
                    });
                }
                "realloc" => {
                    let expr: Expr = input.parse()?;
                    return Ok(ParsedDirective {
                        inner: Directive::Realloc(expr),
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

        // Group / check / init: `name(...)`
        if input.peek(syn::token::Paren) {
            let content;
            syn::parenthesized!(content in input);

            match name.as_str() {
                // init / init(idempotent)
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
                        inner: Directive::Init { idempotent },
                    });
                }

                // Structural checks
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

                // All other groups: token, mint, close, sweep, etc.
                _ => {
                    let args = parse_group_args(&content)?;
                    return Ok(ParsedDirective {
                        inner: Directive::Group(GroupDirective { path, args }),
                    });
                }
            }
        }

        // Bare flags (no parens, no `=`)
        match name.as_str() {
            "init" => Ok(ParsedDirective {
                inner: Directive::Init { idempotent: false },
            }),
            "dup" => Ok(ParsedDirective {
                inner: Directive::Bare(last_ident(&path)),
            }),
            _ => Err(syn::Error::new_spanned(
                &path,
                format!("unknown bare directive `{name}`; did you mean `{name}(...)`?"),
            )),
        }
    }
}

/// Parse `key = value` pairs separated by commas.
fn parse_group_args(input: ParseStream) -> syn::Result<Vec<GroupArg>> {
    let mut args = Vec::new();
    while !input.is_empty() {
        let key: Ident = input.parse()?;

        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            input.parse::<Expr>()?
        } else {
            // Bare key with no value — treat as `key = true`
            syn::parse_quote!(true)
        };

        args.push(GroupArg { key, value });

        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
    }
    Ok(args)
}

/// Validate that an op-arg value conforms to the phase-polymorphic grammar.
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
pub(crate) fn validate_op_arg(key: &Ident, expr: &Expr) -> syn::Result<()> {
    if is_valid_op_arg(expr) {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            expr,
            format!(
                "op arg `{}` has a value that is not valid in all lifecycle phases. Op args must \
                 be bare field idents, literals, const paths, `Some(field)`, or `None`. Move \
                 complex expressions to `constraints(...)` or handler code.",
                key,
            ),
        ))
    }
}

/// Check if an expression conforms to the op-arg grammar.
fn is_valid_op_arg(expr: &Expr) -> bool {
    match expr {
        // Path: bare ident (field ref) or multi-segment (const/type path) — valid
        Expr::Path(ep) => ep.qself.is_none(),
        // Literal — valid
        Expr::Lit(_) => true,
        // Some(inner) — valid if inner is valid
        Expr::Call(call) => {
            if let Expr::Path(func) = &*call.func {
                if func.qself.is_none()
                    && func.path.segments.len() == 1
                    && func.path.segments[0].ident == "Some"
                {
                    return call.args.len() == 1 && call.args.iter().all(is_valid_op_arg);
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
    let attr = field.attrs.iter().find(|a| a.path().is_ident("account"));
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

fn last_ident(path: &syn::Path) -> Ident {
    path.segments.last().unwrap().ident.clone()
}
