//! Constraint attribute types and parsing for `#[account(...)]` field
//! attributes.
//!
//! Handles: `init`, `mut`, `signer`, `address`, `seeds`, `bump`, `space`,
//! `payer`, `token_*`, `mint_*`, `associated_token_*`, `constraint`, and more.

use syn::{
    parse::{Parse, ParseStream},
    Expr, ExprArray, Ident, Token,
};

/// Typed seeds: `seeds = Vault::seeds(authority, index)`
pub(super) struct TypedSeeds {
    /// The type path (e.g., `Vault`)
    pub type_path: syn::Path,
    /// The arguments passed (e.g., [authority, index])
    pub args: Vec<Expr>,
}

pub(super) enum AccountDirective {
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
    MetadataName(Expr),
    MetadataSymbol(Expr),
    MetadataUri(Expr),
    MetadataSellerFeeBasisPoints(Expr),
    MetadataIsMutable(Expr),
    MasterEditionMaxSupply(Expr),
    MintDecimals(Expr),
    MintInitAuthority(Ident),
    MintFreezeAuthority(Ident),
    MintTokenProgram(Ident),
}

impl Parse for AccountDirective {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![mut]) {
            let _: Token![mut] = input.parse()?;
            return Ok(Self::Mut);
        }
        let key: Ident = input.parse()?;
        match key.to_string().as_str() {
            "init" => Ok(Self::Init),
            "init_if_needed" => Ok(Self::InitIfNeeded),
            "dup" => Ok(Self::Dup),
            "close" => {
                let _: Token![=] = input.parse()?;
                let ident: Ident = input.parse()?;
                Ok(Self::Close(ident))
            }
            "payer" => {
                let _: Token![=] = input.parse()?;
                let ident: Ident = input.parse()?;
                Ok(Self::Payer(ident))
            }
            "space" => {
                let _: Token![=] = input.parse()?;
                let expr: Expr = input.parse()?;
                Ok(Self::Space(expr))
            }
            "has_one" => {
                let _: Token![=] = input.parse()?;
                let ident: Ident = input.parse()?;
                let error = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                Ok(Self::HasOne(ident, error))
            }
            "constraint" => {
                let _: Token![=] = input.parse()?;
                let expr: Expr = input.parse()?;
                let error = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                Ok(Self::Constraint(expr, error))
            }
            "address" => {
                let _: Token![=] = input.parse()?;
                let expr: Expr = input.parse()?;
                let error = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                Ok(Self::Address(expr, error))
            }
            "seeds" => {
                let _: Token![=] = input.parse()?;
                if input.peek(syn::token::Bracket) {
                    // Old syntax: seeds = [expr1, expr2, ...]
                    let arr: ExprArray = input.parse()?;
                    Ok(Self::Seeds(arr.elems.into_iter().collect()))
                } else {
                    // New syntax: seeds = Type::seeds(arg1, arg2)
                    let expr: Expr = input.parse()?;
                    match expr {
                        Expr::Call(call) => {
                            if let Expr::Path(ref func_path) = *call.func {
                                let segments = &func_path.path.segments;
                                if segments.last().map(|s| s.ident == "seeds") != Some(true) {
                                    return Err(syn::Error::new_spanned(
                                        &func_path.path,
                                        "expected Type::seeds(...)",
                                    ));
                                }
                                // Build type path: all segments except the last "seeds"
                                let all: Vec<syn::PathSegment> = segments.iter().cloned().collect();
                                if all.len() < 2 {
                                    return Err(syn::Error::new_spanned(
                                        &func_path.path,
                                        "expected Type::seeds(...), not just seeds(...)",
                                    ));
                                }
                                let type_segs = &all[..all.len() - 1];
                                let mut type_segments = syn::punctuated::Punctuated::new();
                                for (i, seg) in type_segs.iter().enumerate() {
                                    type_segments.push_value(seg.clone());
                                    if i < type_segs.len() - 1 {
                                        type_segments.push_punct(<Token![::]>::default());
                                    }
                                }
                                let type_path = syn::Path {
                                    leading_colon: func_path.path.leading_colon,
                                    segments: type_segments,
                                };
                                Ok(Self::TypedSeeds(TypedSeeds {
                                    type_path,
                                    args: call.args.into_iter().collect(),
                                }))
                            } else {
                                Err(syn::Error::new_spanned(
                                    call.func,
                                    "expected Type::seeds(...)",
                                ))
                            }
                        }
                        _ => Err(syn::Error::new_spanned(
                            expr,
                            "expected seeds = [...] or seeds = Type::seeds(...)",
                        )),
                    }
                }
            }
            "bump" => {
                if input.peek(Token![=]) {
                    let _: Token![=] = input.parse()?;
                    Ok(Self::Bump(Some(input.parse()?)))
                } else {
                    Ok(Self::Bump(None))
                }
            }
            "sweep" => {
                let _: Token![=] = input.parse()?;
                let ident: Ident = input.parse()?;
                Ok(Self::Sweep(ident))
            }
            "realloc" => {
                if input.peek(Token![::]) {
                    input.parse::<Token![::]>()?;
                    let sub_key: Ident = input.parse()?;
                    match sub_key.to_string().as_str() {
                        "payer" => {
                            let _: Token![=] = input.parse()?;
                            let ident: Ident = input.parse()?;
                            Ok(Self::ReallocPayer(ident))
                        }
                        _ => Err(syn::Error::new(
                            sub_key.span(),
                            format!("unknown realloc attribute: `realloc::{}`", sub_key),
                        )),
                    }
                } else {
                    let _: Token![=] = input.parse()?;
                    let expr: Expr = input.parse()?;
                    Ok(Self::Realloc(expr))
                }
            }
            "token" => {
                input.parse::<Token![::]>()?;
                let sub_key: Ident = input.parse()?;
                match sub_key.to_string().as_str() {
                    "mint" => {
                        let _: Token![=] = input.parse()?;
                        let ident: Ident = input.parse()?;
                        Ok(Self::TokenMint(ident))
                    }
                    "authority" => {
                        let _: Token![=] = input.parse()?;
                        let ident: Ident = input.parse()?;
                        Ok(Self::TokenAuthority(ident))
                    }
                    "token_program" => {
                        let _: Token![=] = input.parse()?;
                        let ident: Ident = input.parse()?;
                        Ok(Self::TokenTokenProgram(ident))
                    }
                    _ => Err(syn::Error::new(
                        sub_key.span(),
                        format!("unknown token attribute: `token::{}`", sub_key),
                    )),
                }
            }
            "mint" => {
                input.parse::<Token![::]>()?;
                let sub_key: Ident = input.parse()?;
                let _: Token![=] = input.parse()?;
                match sub_key.to_string().as_str() {
                    "decimals" => Ok(Self::MintDecimals(input.parse()?)),
                    "authority" => {
                        let ident: Ident = input.parse()?;
                        Ok(Self::MintInitAuthority(ident))
                    }
                    "freeze_authority" => {
                        let ident: Ident = input.parse()?;
                        Ok(Self::MintFreezeAuthority(ident))
                    }
                    "token_program" => {
                        let ident: Ident = input.parse()?;
                        Ok(Self::MintTokenProgram(ident))
                    }
                    _ => Err(syn::Error::new(
                        sub_key.span(),
                        format!("unknown mint attribute: `mint::{}`", sub_key),
                    )),
                }
            }
            "associated_token" => {
                input.parse::<Token![::]>()?;
                let sub_key: Ident = input.parse()?;
                match sub_key.to_string().as_str() {
                    "mint" => {
                        let _: Token![=] = input.parse()?;
                        let ident: Ident = input.parse()?;
                        Ok(Self::AssociatedTokenMint(ident))
                    }
                    "authority" => {
                        let _: Token![=] = input.parse()?;
                        let ident: Ident = input.parse()?;
                        Ok(Self::AssociatedTokenAuthority(ident))
                    }
                    "token_program" => {
                        let _: Token![=] = input.parse()?;
                        let ident: Ident = input.parse()?;
                        Ok(Self::AssociatedTokenTokenProgram(ident))
                    }
                    _ => Err(syn::Error::new(
                        sub_key.span(),
                        format!(
                            "unknown associated_token attribute: `associated_token::{}`",
                            sub_key
                        ),
                    )),
                }
            }
            "metadata" => {
                input.parse::<Token![::]>()?;
                let sub_key: Ident = input.parse()?;
                let _: Token![=] = input.parse()?;
                match sub_key.to_string().as_str() {
                    "name" => Ok(Self::MetadataName(input.parse()?)),
                    "symbol" => Ok(Self::MetadataSymbol(input.parse()?)),
                    "uri" => Ok(Self::MetadataUri(input.parse()?)),
                    "seller_fee_basis_points" => {
                        Ok(Self::MetadataSellerFeeBasisPoints(input.parse()?))
                    }
                    "is_mutable" => Ok(Self::MetadataIsMutable(input.parse()?)),
                    _ => Err(syn::Error::new(
                        sub_key.span(),
                        format!("unknown metadata attribute: `metadata::{}`", sub_key),
                    )),
                }
            }
            "master_edition" => {
                input.parse::<Token![::]>()?;
                let sub_key: Ident = input.parse()?;
                let _: Token![=] = input.parse()?;
                match sub_key.to_string().as_str() {
                    "max_supply" => Ok(Self::MasterEditionMaxSupply(input.parse()?)),
                    _ => Err(syn::Error::new(
                        sub_key.span(),
                        format!(
                            "unknown master_edition attribute: `master_edition::{}`",
                            sub_key
                        ),
                    )),
                }
            }
            _ => Err(syn::Error::new(
                key.span(),
                format!("unknown account attribute: `{}`", key),
            )),
        }
    }
}

#[derive(Default)]
pub(super) struct AccountFieldAttrs {
    pub is_mut: bool,
    pub is_init: bool,
    pub init_if_needed: bool,
    pub dup: bool,
    pub close: Option<Ident>,
    pub sweep: Option<Ident>,
    pub payer: Option<Ident>,
    pub space: Option<Expr>,
    pub has_ones: Vec<(Ident, Option<Expr>)>,
    pub constraints: Vec<(Expr, Option<Expr>)>,
    pub seeds: Option<Vec<Expr>>,
    pub typed_seeds: Option<TypedSeeds>,
    pub bump: Option<Option<Expr>>,
    pub address: Option<(Expr, Option<Expr>)>,
    pub token_mint: Option<Ident>,
    pub token_authority: Option<Ident>,
    pub token_token_program: Option<Ident>,
    pub associated_token_mint: Option<Ident>,
    pub associated_token_authority: Option<Ident>,
    pub associated_token_token_program: Option<Ident>,
    pub realloc: Option<Expr>,
    pub realloc_payer: Option<Ident>,
    pub metadata_name: Option<Expr>,
    pub metadata_symbol: Option<Expr>,
    pub metadata_uri: Option<Expr>,
    pub metadata_seller_fee_basis_points: Option<Expr>,
    pub metadata_is_mutable: Option<Expr>,
    pub master_edition_max_supply: Option<Expr>,
    pub mint_decimals: Option<Expr>,
    pub mint_init_authority: Option<Ident>,
    pub mint_freeze_authority: Option<Ident>,
    pub mint_token_program: Option<Ident>,
}

impl AccountFieldAttrs {
    /// Apply a single directive to this attrs struct.
    ///
    /// **Exhaustive match** — adding a new `AccountDirective` variant without
    /// a match arm here is a compile error. This is the primary completeness
    /// guarantee for the parse → flat-struct conversion.
    pub(super) fn apply(&mut self, d: &AccountDirective) {
        match d {
            AccountDirective::Mut => self.is_mut = true,
            AccountDirective::Init => self.is_init = true,
            AccountDirective::InitIfNeeded => self.init_if_needed = true,
            AccountDirective::Dup => self.dup = true,
            AccountDirective::Close(v) => self.close = Some(v.clone()),
            AccountDirective::Sweep(v) => self.sweep = Some(v.clone()),
            AccountDirective::Payer(v) => self.payer = Some(v.clone()),
            AccountDirective::Space(v) => self.space = Some(v.clone()),
            AccountDirective::HasOne(id, err) => self.has_ones.push((id.clone(), err.clone())),
            AccountDirective::Constraint(expr, err) => {
                self.constraints.push((expr.clone(), err.clone()))
            }
            AccountDirective::Seeds(v) => self.seeds = Some(v.clone()),
            AccountDirective::TypedSeeds(ts) => {
                self.typed_seeds = Some(TypedSeeds {
                    type_path: ts.type_path.clone(),
                    args: ts.args.clone(),
                })
            }
            AccountDirective::Bump(v) => self.bump = Some(v.clone()),
            AccountDirective::Address(expr, err) => {
                self.address = Some((expr.clone(), err.clone()))
            }
            AccountDirective::TokenMint(v) => self.token_mint = Some(v.clone()),
            AccountDirective::TokenAuthority(v) => self.token_authority = Some(v.clone()),
            AccountDirective::TokenTokenProgram(v) => self.token_token_program = Some(v.clone()),
            AccountDirective::AssociatedTokenMint(v) => {
                self.associated_token_mint = Some(v.clone())
            }
            AccountDirective::AssociatedTokenAuthority(v) => {
                self.associated_token_authority = Some(v.clone())
            }
            AccountDirective::AssociatedTokenTokenProgram(v) => {
                self.associated_token_token_program = Some(v.clone())
            }
            AccountDirective::Realloc(v) => self.realloc = Some(v.clone()),
            AccountDirective::ReallocPayer(v) => self.realloc_payer = Some(v.clone()),
            AccountDirective::MetadataName(v) => self.metadata_name = Some(v.clone()),
            AccountDirective::MetadataSymbol(v) => self.metadata_symbol = Some(v.clone()),
            AccountDirective::MetadataUri(v) => self.metadata_uri = Some(v.clone()),
            AccountDirective::MetadataSellerFeeBasisPoints(v) => {
                self.metadata_seller_fee_basis_points = Some(v.clone())
            }
            AccountDirective::MetadataIsMutable(v) => self.metadata_is_mutable = Some(v.clone()),
            AccountDirective::MasterEditionMaxSupply(v) => {
                self.master_edition_max_supply = Some(v.clone())
            }
            AccountDirective::MintDecimals(v) => self.mint_decimals = Some(v.clone()),
            AccountDirective::MintInitAuthority(v) => self.mint_init_authority = Some(v.clone()),
            AccountDirective::MintFreezeAuthority(v) => {
                self.mint_freeze_authority = Some(v.clone())
            }
            AccountDirective::MintTokenProgram(v) => self.mint_token_program = Some(v.clone()),
        }
    }
}

impl Parse for AccountFieldAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let directives = input.parse_terminated(AccountDirective::parse, Token![,])?;
        let mut r = Self::default();
        for d in directives {
            r.apply(&d);
        }
        Ok(r)
    }
}

/// Parsed result: the flat struct for backward compat + the raw directive list
/// for exhaustive-match verification.
pub(super) struct ParsedAttrs {
    pub attrs: AccountFieldAttrs,
    pub directives: Vec<AccountDirective>,
}

pub(super) fn parse_field_attrs(field: &syn::Field) -> syn::Result<ParsedAttrs> {
    let attr = field.attrs.iter().find(|a| a.path().is_ident("account"));
    match attr {
        Some(a) => {
            let directives: syn::punctuated::Punctuated<AccountDirective, syn::Token![,]> =
                a.parse_args_with(syn::punctuated::Punctuated::parse_terminated)?;
            let directives: Vec<AccountDirective> = directives.into_iter().collect();
            let mut r = AccountFieldAttrs::default();
            for d in &directives {
                r.apply(d);
            }
            Ok(ParsedAttrs { attrs: r, directives })
        }
        None => Ok(ParsedAttrs {
            attrs: AccountFieldAttrs::default(),
            directives: Vec::new(),
        }),
    }
}
