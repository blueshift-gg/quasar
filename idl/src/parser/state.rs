use {
    crate::{
        parser::helpers,
        types::{IdlAccountDef, IdlField, IdlTypeDef, IdlTypeDefType},
    },
    syn::{Fields, Item},
};

/// Raw parsed data for a `#[account(discriminator = N)]` struct.
pub struct RawStateAccount {
    pub name: String,
    pub discriminator: Vec<u8>,
    pub fields: Vec<(String, syn::Type)>,
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

                result.push(RawStateAccount {
                    name,
                    discriminator: disc,
                    fields,
                });
            }
        }
    }
    result
}

/// Check if a struct has `#[account(discriminator = N)]` and extract the discriminator.
/// Distinguishes from `#[account(...)]` field attributes on derive(Accounts) fields
/// by checking if it's on a struct item (not a field).
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

        return parse_discriminator_value(&tokens);
    }
    None
}

fn parse_discriminator_value(tokens_str: &str) -> Option<Vec<u8>> {
    let eq_pos = tokens_str.find('=')?;
    let value_str = tokens_str[eq_pos + 1..].trim();

    if value_str.starts_with('[') {
        let inner = value_str.trim_start_matches('[').trim_end_matches(']');
        let bytes: Vec<u8> = inner
            .split(',')
            .filter_map(|s| s.trim().parse::<u8>().ok())
            .collect();
        if bytes.is_empty() {
            None
        } else {
            Some(bytes)
        }
    } else {
        let byte: u8 = value_str
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        Some(vec![byte])
    }
}

/// Convert a `RawStateAccount` to an `IdlAccountDef` (for the "accounts" array).
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
            kind: "struct".to_string(),
            fields,
        },
    }
}
