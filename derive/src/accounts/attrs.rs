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
        let mut is_mut = false;
        let mut is_init = false;
        let mut init_if_needed = false;
        let mut dup = false;
        let mut close = None;
        let mut payer = None;
        let mut space = None;
        let mut has_ones = Vec::new();
        let mut constraints = Vec::new();
        let mut seeds = None;
        let mut bump = None;
        let mut address = None;
        let mut token_mint = None;
        let mut token_authority = None;
        let mut associated_token_mint = None;
        let mut associated_token_authority = None;
        let mut associated_token_token_program = None;
        let mut realloc = None;
        let mut realloc_payer = None;
        let mut metadata_name = None;
        let mut metadata_symbol = None;
        let mut metadata_uri = None;
        let mut metadata_seller_fee_basis_points = None;
        let mut metadata_is_mutable = None;
        let mut master_edition_max_supply = None;
        let mut mint_decimals = None;
        let mut mint_init_authority = None;
        let mut mint_freeze_authority = None;
        for d in directives {
            match d {
                AccountDirective::Mut => is_mut = true,
                AccountDirective::Init => is_init = true,
                AccountDirective::InitIfNeeded => init_if_needed = true,
                AccountDirective::Dup => dup = true,
                AccountDirective::Close(ident) => close = Some(ident),
                AccountDirective::Payer(ident) => payer = Some(ident),
                AccountDirective::Space(expr) => space = Some(expr),
                AccountDirective::HasOne(ident, err) => has_ones.push((ident, err)),
                AccountDirective::Constraint(expr, err) => constraints.push((expr, err)),
                AccountDirective::Seeds(s) => seeds = Some(s),
                AccountDirective::Bump(b) => bump = Some(b),
                AccountDirective::Address(expr, err) => address = Some((expr, err)),
                AccountDirective::TokenMint(ident) => token_mint = Some(ident),
                AccountDirective::TokenAuthority(ident) => token_authority = Some(ident),
                AccountDirective::AssociatedTokenMint(ident) => associated_token_mint = Some(ident),
                AccountDirective::AssociatedTokenAuthority(ident) => {
                    associated_token_authority = Some(ident)
                }
                AccountDirective::AssociatedTokenTokenProgram(ident) => {
                    associated_token_token_program = Some(ident)
                }
                AccountDirective::Realloc(expr) => realloc = Some(expr),
                AccountDirective::ReallocPayer(ident) => realloc_payer = Some(ident),
                AccountDirective::MetadataName(expr) => metadata_name = Some(expr),
                AccountDirective::MetadataSymbol(expr) => metadata_symbol = Some(expr),
                AccountDirective::MetadataUri(expr) => metadata_uri = Some(expr),
                AccountDirective::MetadataSellerFeeBasisPoints(expr) => {
                    metadata_seller_fee_basis_points = Some(expr)
                }
                AccountDirective::MetadataIsMutable(expr) => metadata_is_mutable = Some(expr),
                AccountDirective::MasterEditionMaxSupply(expr) => {
                    master_edition_max_supply = Some(expr)
                }
                AccountDirective::MintDecimals(expr) => mint_decimals = Some(expr),
                AccountDirective::MintInitAuthority(ident) => mint_init_authority = Some(ident),
                AccountDirective::MintFreezeAuthority(ident) => mint_freeze_authority = Some(ident),
            }
        }
        Ok(Self {
            is_mut,
            is_init,
            init_if_needed,
            dup,
            close,
            payer,
            space,
            has_ones,
            constraints,
            seeds,
            bump,
            address,
            token_mint,
            token_authority,
            associated_token_mint,
            associated_token_authority,
            associated_token_token_program,
            realloc,
            realloc_payer,
            metadata_name,
            metadata_symbol,
            metadata_uri,
            metadata_seller_fee_basis_points,
            metadata_is_mutable,
            master_edition_max_supply,
            mint_decimals,
            mint_init_authority,
            mint_freeze_authority,
        })
    }
}

pub(super) fn parse_field_attrs(field: &syn::Field) -> syn::Result<AccountFieldAttrs> {
    for attr in &field.attrs {
        if attr.path().is_ident("account") {
            return attr.parse_args::<AccountFieldAttrs>();
        }
    }
    Ok(AccountFieldAttrs {
        is_mut: false,
        is_init: false,
        init_if_needed: false,
        dup: false,
        close: None,
        payer: None,
        space: None,
        has_ones: vec![],
        constraints: vec![],
        seeds: None,
        bump: None,
        address: None,
        token_mint: None,
        token_authority: None,
        associated_token_mint: None,
        associated_token_authority: None,
        associated_token_token_program: None,
        realloc: None,
        realloc_payer: None,
        metadata_name: None,
        metadata_symbol: None,
        metadata_uri: None,
        metadata_seller_fee_basis_points: None,
        metadata_is_mutable: None,
        master_edition_max_supply: None,
        mint_decimals: None,
        mint_init_authority: None,
        mint_freeze_authority: None,
    })
}
