//! Constraint attribute types and parsing for `#[account(...)]` field
//! attributes.
//!
//! Handles: `init`, `mut`, `signer`, `address`, `seeds`, `bump`, `space`,
//! `payer`, `token_*`, `mint_*`, `associated_token_*`, `constraint`, and more.

use syn::{
    parse::{Parse, ParseStream},
    Expr, Ident, Path, Token,
};

/// Typed seeds: `seeds = Vault::seeds(authority, index)`
#[derive(Clone)]
pub(crate) struct TypedSeeds {
    pub type_path: syn::Path,
    pub args: Vec<Expr>,
}

pub(crate) enum AccountDirective {
    Mut,
    Init,
    InitIfNeeded,
    Dup,
    Close(Ident),
    Payer(Ident),
    Space(Expr),
    HasOne(Ident, Option<Expr>),
    Constraint(Expr, Option<Expr>),
    Seeds(Vec<Expr>),
    TypedSeeds(TypedSeeds),
    Bump(Option<Expr>),
    Address(Expr, Option<Expr>),
    TokenMint(Ident),
    TokenAuthority(Ident),
    TokenTokenProgram(Ident),
    AssociatedTokenMint(Ident),
    AssociatedTokenAuthority(Ident),
    AssociatedTokenTokenProgram(Ident),
    Sweep(Ident),
    Realloc(Expr),
    ReallocPayer(Ident),
    MintDecimals(Expr),
    MintInitAuthority(Ident),
    MintFreezeAuthority(Ident),
    MintTokenProgram(Ident),
}

struct ParsedDirective {
    key: DirectiveKey,
    value: Option<Expr>,
    error: Option<Expr>,
}

enum DirectiveKey {
    Mut,
    Path(Path),
}

impl Parse for ParsedDirective {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![mut]) {
            let _: Token![mut] = input.parse()?;
            return Ok(Self {
                key: DirectiveKey::Mut,
                value: None,
                error: None,
            });
        }

        let path: Path = input.parse()?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse::<Expr>()?)
        } else {
            None
        };
        let error = if input.peek(Token![@]) {
            input.parse::<Token![@]>()?;
            Some(input.parse::<Expr>()?)
        } else {
            None
        };

        Ok(Self {
            key: DirectiveKey::Path(path),
            value,
            error,
        })
    }
}

pub(crate) fn parse_field_attrs(field: &syn::Field) -> syn::Result<Vec<AccountDirective>> {
    let attr = field.attrs.iter().find(|a| a.path().is_ident("account"));
    match attr {
        Some(a) => {
            let directives: syn::punctuated::Punctuated<ParsedDirective, syn::Token![,]> =
                a.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;
            directives.into_iter().map(lower_directive).collect()
        }
        None => Ok(Vec::new()),
    }
}

fn lower_directive(directive: ParsedDirective) -> syn::Result<AccountDirective> {
    if matches!(directive.key, DirectiveKey::Mut) {
        return expect_bare(directive, AccountDirective::Mut);
    }

    let path = directive_path(&directive)?;
    let names = path_idents(path);
    match names.as_slice() {
        [name] if *name == "init" => expect_bare(directive, AccountDirective::Init),
        [name] if *name == "init_if_needed" => {
            expect_bare(directive, AccountDirective::InitIfNeeded)
        }
        [name] if *name == "dup" => expect_bare(directive, AccountDirective::Dup),
        [name] if *name == "close" => Ok(AccountDirective::Close(expect_ident_value(directive)?)),
        [name] if *name == "payer" => Ok(AccountDirective::Payer(expect_ident_value(directive)?)),
        [name] if *name == "space" => Ok(AccountDirective::Space(expect_expr_value(directive)?)),
        [name] if *name == "has_one" => {
            let error = directive.error.clone();
            Ok(AccountDirective::HasOne(
                expect_ident_value_without_error(directive)?,
                error,
            ))
        }
        [name] if *name == "constraint" => {
            let error = directive.error.clone();
            Ok(AccountDirective::Constraint(
                expect_expr_value_without_error(directive)?,
                error,
            ))
        }
        [name] if *name == "address" => {
            let error = directive.error.clone();
            Ok(AccountDirective::Address(
                expect_expr_value_without_error(directive)?,
                error,
            ))
        }
        [name] if *name == "seeds" => lower_seeds_directive(directive),
        [name] if *name == "bump" => Ok(AccountDirective::Bump(expect_optional_expr(directive)?)),
        [name] if *name == "sweep" => Ok(AccountDirective::Sweep(expect_ident_value(directive)?)),
        [ns] if *ns == "realloc" => Ok(AccountDirective::Realloc(expect_expr_value(directive)?)),
        [ns, sub] if *ns == "realloc" && *sub == "payer" => Ok(AccountDirective::ReallocPayer(
            expect_ident_value(directive)?,
        )),
        [ns, sub] if *ns == "token" && *sub == "mint" => {
            Ok(AccountDirective::TokenMint(expect_ident_value(directive)?))
        }
        [ns, sub] if *ns == "token" && *sub == "authority" => Ok(AccountDirective::TokenAuthority(
            expect_ident_value(directive)?,
        )),
        [ns, sub] if *ns == "token" && *sub == "token_program" => Ok(
            AccountDirective::TokenTokenProgram(expect_ident_value(directive)?),
        ),
        [ns, sub] if *ns == "mint" && *sub == "decimals" => Ok(AccountDirective::MintDecimals(
            expect_expr_value(directive)?,
        )),
        [ns, sub] if *ns == "mint" && *sub == "authority" => Ok(
            AccountDirective::MintInitAuthority(expect_ident_value(directive)?),
        ),
        [ns, sub] if *ns == "mint" && *sub == "freeze_authority" => Ok(
            AccountDirective::MintFreezeAuthority(expect_ident_value(directive)?),
        ),
        [ns, sub] if *ns == "mint" && *sub == "token_program" => Ok(
            AccountDirective::MintTokenProgram(expect_ident_value(directive)?),
        ),
        [ns, sub] if *ns == "associated_token" && *sub == "mint" => Ok(
            AccountDirective::AssociatedTokenMint(expect_ident_value(directive)?),
        ),
        [ns, sub] if *ns == "associated_token" && *sub == "authority" => Ok(
            AccountDirective::AssociatedTokenAuthority(expect_ident_value(directive)?),
        ),
        [ns, sub] if *ns == "associated_token" && *sub == "token_program" => Ok(
            AccountDirective::AssociatedTokenTokenProgram(expect_ident_value(directive)?),
        ),
        [ns, sub_key] if *ns == "realloc" => Err(syn::Error::new(
            path.segments.last().expect("non-empty path").ident.span(),
            format!("unknown realloc attribute: `realloc::{sub_key}`"),
        )),
        [ns, sub_key] if *ns == "token" => Err(syn::Error::new(
            path.segments.last().expect("non-empty path").ident.span(),
            format!("unknown token attribute: `token::{sub_key}`"),
        )),
        [ns, sub_key] if *ns == "mint" => Err(syn::Error::new(
            path.segments.last().expect("non-empty path").ident.span(),
            format!("unknown mint attribute: `mint::{sub_key}`"),
        )),
        [ns, sub_key] if *ns == "associated_token" => Err(syn::Error::new(
            path.segments.last().expect("non-empty path").ident.span(),
            format!("unknown associated_token attribute: `associated_token::{sub_key}`"),
        )),
        _ => Err(syn::Error::new(
            path.segments.first().expect("non-empty path").ident.span(),
            format!("unknown account attribute: `{}`", join_path(path)),
        )),
    }
}

fn lower_seeds_directive(directive: ParsedDirective) -> syn::Result<AccountDirective> {
    ensure_no_error(&directive)?;
    let Some(expr) = directive.value else {
        return Err(syn::Error::new_spanned(
            directive_path(&directive)?,
            "`seeds` requires a value",
        ));
    };

    match expr {
        Expr::Array(arr) => Ok(AccountDirective::Seeds(arr.elems.into_iter().collect())),
        Expr::Call(call) => {
            let Expr::Path(func_path) = *call.func else {
                return Err(syn::Error::new_spanned(
                    call.func,
                    "expected Type::seeds(...)",
                ));
            };
            let segments = &func_path.path.segments;
            if segments.last().map(|s| s.ident == "seeds") != Some(true) {
                return Err(syn::Error::new_spanned(
                    &func_path.path,
                    "expected Type::seeds(...)",
                ));
            }
            if segments.len() < 2 {
                return Err(syn::Error::new_spanned(
                    &func_path.path,
                    "expected Type::seeds(...), not just seeds(...)",
                ));
            }

            let mut type_segments = syn::punctuated::Punctuated::new();
            for (i, seg) in segments.iter().take(segments.len() - 1).enumerate() {
                type_segments.push_value(seg.clone());
                if i + 1 != segments.len() - 1 {
                    type_segments.push_punct(<Token![::]>::default());
                }
            }

            Ok(AccountDirective::TypedSeeds(TypedSeeds {
                type_path: Path {
                    leading_colon: func_path.path.leading_colon,
                    segments: type_segments,
                },
                args: call.args.into_iter().collect(),
            }))
        }
        _ => Err(syn::Error::new_spanned(
            expr,
            "expected seeds = [...] or seeds = Type::seeds(...)",
        )),
    }
}

fn expect_bare(
    directive: ParsedDirective,
    bare: AccountDirective,
) -> syn::Result<AccountDirective> {
    ensure_no_value(&directive)?;
    ensure_no_error(&directive)?;
    Ok(bare)
}

fn expect_ident_value(directive: ParsedDirective) -> syn::Result<Ident> {
    ensure_no_error(&directive)?;
    expect_ident_value_without_error(directive)
}

fn expect_ident_value_without_error(directive: ParsedDirective) -> syn::Result<Ident> {
    let expr = expect_value(directive)?;
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Ok(path.path.segments[0].ident.clone())
        }
        _ => Err(syn::Error::new_spanned(expr, "expected an identifier")),
    }
}

fn expect_expr_value(directive: ParsedDirective) -> syn::Result<Expr> {
    ensure_no_error(&directive)?;
    expect_expr_value_without_error(directive)
}

fn expect_expr_value_without_error(directive: ParsedDirective) -> syn::Result<Expr> {
    expect_value(directive)
}

fn expect_optional_expr(directive: ParsedDirective) -> syn::Result<Option<Expr>> {
    ensure_no_error(&directive)?;
    Ok(directive.value)
}

fn expect_value(directive: ParsedDirective) -> syn::Result<Expr> {
    let label = join_directive(&directive);
    let span = directive_path(&directive)?.clone();
    directive
        .value
        .ok_or_else(|| syn::Error::new_spanned(span, format!("`{label}` requires a value")))
}

fn ensure_no_value(directive: &ParsedDirective) -> syn::Result<()> {
    if let Some(value) = &directive.value {
        Err(syn::Error::new_spanned(
            value,
            format!("`{}` does not take a value", join_directive(directive)),
        ))
    } else {
        Ok(())
    }
}

fn ensure_no_error(directive: &ParsedDirective) -> syn::Result<()> {
    if let Some(error) = &directive.error {
        Err(syn::Error::new_spanned(
            error,
            format!(
                "`{}` does not support custom error syntax",
                join_directive(directive)
            ),
        ))
    } else {
        Ok(())
    }
}

fn directive_path(directive: &ParsedDirective) -> syn::Result<&Path> {
    match &directive.key {
        DirectiveKey::Mut => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`mut` does not have a directive path",
        )),
        DirectiveKey::Path(path) => Ok(path),
    }
}

fn join_directive(directive: &ParsedDirective) -> String {
    match &directive.key {
        DirectiveKey::Mut => "mut".to_owned(),
        DirectiveKey::Path(path) => join_path(path),
    }
}

fn path_idents(path: &Path) -> Vec<&syn::Ident> {
    path.segments.iter().map(|segment| &segment.ident).collect()
}

fn join_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
