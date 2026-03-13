//! Constraint attribute types and parsing for `#[account(...)]` field
//! attributes.
//!
//! Handles: `init`, `mut`, `signer`, `address`, `seeds`, `bump`, `space`,
//! `payer`, `token_*`, `mint_*`, `associated_token_*`, `constraint`, and more.

use syn::{
    parse::{Parse, ParseStream},
    Expr, ExprArray, Ident, Token,
};

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
    Bump(Option<Expr>),
    Address(Expr, Option<Expr>),
    TokenMint(Ident),
    TokenAuthority(Ident),
    AssociatedTokenMint(Ident),
    AssociatedTokenAuthority(Ident),
    AssociatedTokenTokenProgram(Ident),
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
                let arr: ExprArray = input.parse()?;
                Ok(Self::Seeds(arr.elems.into_iter().collect()))
            }
            "bump" => {
                if input.peek(Token![=]) {
                    let _: Token![=] = input.parse()?;
                    Ok(Self::Bump(Some(input.parse()?)))
                } else {
                    Ok(Self::Bump(None))
                }
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
    pub payer: Option<Ident>,
    pub space: Option<Expr>,
    pub has_ones: Vec<(Ident, Option<Expr>)>,
    pub constraints: Vec<(Expr, Option<Expr>)>,
    pub seeds: Option<Vec<Expr>>,
    pub bump: Option<Option<Expr>>,
    pub address: Option<(Expr, Option<Expr>)>,
    pub token_mint: Option<Ident>,
    pub token_authority: Option<Ident>,
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
}

impl Parse for AccountFieldAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let directives = input.parse_terminated(AccountDirective::parse, Token![,])?;
        let mut r = Self::default();
        for d in directives {
            match d {
                AccountDirective::Mut => r.is_mut = true,
                AccountDirective::Init => r.is_init = true,
                AccountDirective::InitIfNeeded => r.init_if_needed = true,
                AccountDirective::Dup => r.dup = true,
                AccountDirective::Close(v) => r.close = Some(v),
                AccountDirective::Payer(v) => r.payer = Some(v),
                AccountDirective::Space(v) => r.space = Some(v),
                AccountDirective::HasOne(id, err) => r.has_ones.push((id, err)),
                AccountDirective::Constraint(expr, err) => r.constraints.push((expr, err)),
                AccountDirective::Seeds(v) => r.seeds = Some(v),
                AccountDirective::Bump(v) => r.bump = Some(v),
                AccountDirective::Address(expr, err) => r.address = Some((expr, err)),
                AccountDirective::TokenMint(v) => r.token_mint = Some(v),
                AccountDirective::TokenAuthority(v) => r.token_authority = Some(v),
                AccountDirective::AssociatedTokenMint(v) => r.associated_token_mint = Some(v),
                AccountDirective::AssociatedTokenAuthority(v) => {
                    r.associated_token_authority = Some(v)
                }
                AccountDirective::AssociatedTokenTokenProgram(v) => {
                    r.associated_token_token_program = Some(v)
                }
                AccountDirective::Realloc(v) => r.realloc = Some(v),
                AccountDirective::ReallocPayer(v) => r.realloc_payer = Some(v),
                AccountDirective::MetadataName(v) => r.metadata_name = Some(v),
                AccountDirective::MetadataSymbol(v) => r.metadata_symbol = Some(v),
                AccountDirective::MetadataUri(v) => r.metadata_uri = Some(v),
                AccountDirective::MetadataSellerFeeBasisPoints(v) => {
                    r.metadata_seller_fee_basis_points = Some(v)
                }
                AccountDirective::MetadataIsMutable(v) => r.metadata_is_mutable = Some(v),
                AccountDirective::MasterEditionMaxSupply(v) => {
                    r.master_edition_max_supply = Some(v)
                }
                AccountDirective::MintDecimals(v) => r.mint_decimals = Some(v),
                AccountDirective::MintInitAuthority(v) => r.mint_init_authority = Some(v),
                AccountDirective::MintFreezeAuthority(v) => r.mint_freeze_authority = Some(v),
            }
        }
        Ok(r)
    }
}

pub(super) fn parse_field_attrs(field: &syn::Field) -> syn::Result<AccountFieldAttrs> {
    field
        .attrs
        .iter()
        .find(|a| a.path().is_ident("account"))
        .map(|a| a.parse_args::<AccountFieldAttrs>())
        .unwrap_or_else(|| Ok(AccountFieldAttrs::default()))
}
