//! Parses `#[account]` state structs for IDL generation (field types,
//! discriminators, dynamic layout classification).

use {
    super::helpers,
    crate::types::{IdlAccountDef, IdlField, IdlTypeDef, IdlTypeDefKind, IdlTypeDefType},
    syn::{Fields, Item},
};

/// Raw parsed data for a `#[account(discriminator = N)]` struct.
pub struct RawStateAccount {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub fields: Vec<(String, syn::Type)>,
    pub seeds: Option<RawTypedSeeds>,
}

/// Parsed `#[seeds(b"prefix", name: Type, ...)]` on a state type.
pub struct RawTypedSeeds {
    pub prefix: Vec<u8>,
    pub dynamic_seeds: Vec<(String, String)>, // (name, type_name)
}

/// Extract all `#[account(discriminator = N)]` structs from a parsed file.
pub fn extract_state_accounts(file: &syn::File) -> Vec<RawStateAccount> {
    let mut result = Vec::new();
    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if let Some(disc) = get_account_discriminator(&item_struct.attrs) {
                let name = item_struct.ident.to_string();
                let fields = match &item_struct.fields {
                    Fields::Named(named) => named
                        .named
                        .iter()
                        .map(|f| {
                            let field_name = f.ident.as_ref().unwrap().to_string();
                            (field_name, f.ty.clone())
                        })
                        .collect(),
                    _ => vec![],
                };

                let seeds = parse_seeds_attr(&item_struct.attrs);

                result.push(RawStateAccount {
                    name,
                    discriminator: disc,
                    fields,
                    seeds,
                });
            }
        }
    }
    result
}

/// Check if a struct has `#[account(discriminator = N)]` and extract the
/// discriminator. Distinguishes from `#[account(...)]` field attributes on
/// derive(Accounts) fields by checking if it's on a struct item (not a field).
fn get_account_discriminator(attrs: &[syn::Attribute]) -> Option<Vec<u8>> {
    for attr in attrs {
        if !attr.path().is_ident("account") {
            continue;
        }

        let tokens = match attr.meta.require_list() {
            Ok(list) => list.tokens.to_string(),
            Err(_) => continue,
        };

        if !tokens.contains("discriminator") {
            continue;
        }

        return helpers::parse_discriminator_value(&tokens);
    }
    None
}

/// Parse `#[seeds(b"prefix", name: Type, ...)]` from struct attributes using
/// syn for correct handling of complex types and spacing.
fn parse_seeds_attr(attrs: &[syn::Attribute]) -> Option<RawTypedSeeds> {
    for attr in attrs {
        if !attr.path().is_ident("seeds") {
            continue;
        }

        let tokens = match attr.meta.require_list() {
            Ok(list) => list.tokens.clone(),
            Err(_) => continue,
        };

        let parsed: SeedsTokens = match syn::parse2(tokens) {
            Ok(p) => p,
            Err(_) => continue,
        };

        return Some(RawTypedSeeds {
            prefix: parsed.prefix,
            dynamic_seeds: parsed.dynamic_seeds,
        });
    }
    None
}

/// Internal representation of parsed `#[seeds(...)]` tokens.
struct SeedsTokens {
    prefix: Vec<u8>,
    dynamic_seeds: Vec<(String, String)>,
}

impl syn::parse::Parse for SeedsTokens {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let prefix_expr: syn::Expr = input.parse()?;
        let prefix = match &prefix_expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::ByteStr(b),
                ..
            }) => b.value(),
            _ => {
                return Err(syn::Error::new_spanned(
                    prefix_expr,
                    "expected byte string literal",
                ))
            }
        };
        let mut dynamic_seeds = Vec::new();
        while !input.is_empty() {
            let _: syn::Token![,] = input.parse()?;
            if input.is_empty() {
                break;
            }
            let name: syn::Ident = input.parse()?;
            let _: syn::Token![:] = input.parse()?;
            let ty: syn::Type = input.parse()?;
            // Normalize type to string by printing it via syn's Display impl,
            // which handles spacing consistently.
            let ty_str = type_to_string(&ty);
            dynamic_seeds.push((name.to_string(), ty_str));
        }
        Ok(SeedsTokens {
            prefix,
            dynamic_seeds,
        })
    }
}

/// Convert a syn::Type to a normalized string representation.
fn type_to_string(ty: &syn::Type) -> String {
    let tokens = format!("{}", syn::__private::ToTokens::to_token_stream(ty));
    // Collapse multiple spaces and spaces around :: for consistency
    let mut result = String::with_capacity(tokens.len());
    let mut prev_space = false;
    for c in tokens.chars() {
        if c.is_whitespace() {
            if !prev_space && !result.is_empty() {
                result.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            result.push(c);
        }
    }
    result.trim().to_string()
}

/// Convert a `RawStateAccount` to an `IdlAccountDef` (for the "accounts"
/// array).
pub fn to_idl_account_def(raw: &RawStateAccount) -> IdlAccountDef {
    IdlAccountDef {
        name: raw.name.clone(),
        discriminator: raw.discriminator.clone(),
    }
}

/// Convert a `RawStateAccount` to an `IdlTypeDef` (for the "types" array).
pub fn to_idl_type_def(raw: &RawStateAccount) -> IdlTypeDef {
    let fields = raw
        .fields
        .iter()
        .map(|(name, ty)| IdlField {
            name: helpers::to_camel_case(name),
            ty: helpers::map_type_from_syn(ty),
        })
        .collect();

    IdlTypeDef {
        name: raw.name.clone(),
        ty: IdlTypeDefType {
            kind: IdlTypeDefKind::Struct,
            fields,
        },
    }
}
