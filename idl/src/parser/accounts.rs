use syn::{Fields, Item};

use crate::parser::helpers;
use crate::types::{IdlAccountItem, IdlPda, IdlSeed};

/// Raw parsed data for a `#[derive(Accounts)]` struct.
pub struct RawAccountsStruct {
    pub name: String,
    pub fields: Vec<RawAccountField>,
}

pub struct RawAccountField {
    pub name: String,
    pub writable: bool,
    pub signer: bool,
    pub pda: Option<RawPda>,
    pub address: Option<String>,
}

pub struct RawPda {
    pub seeds: Vec<RawSeed>,
}

pub enum RawSeed {
    ByteString(Vec<u8>),
    AccountRef(String),
}

/// Extract all `#[derive(Accounts)]` structs from a parsed file.
pub fn extract_accounts_structs(file: &syn::File) -> Vec<RawAccountsStruct> {
    let mut result = Vec::new();
    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if !has_derive_accounts(&item_struct.attrs) {
                continue;
            }

            let name = item_struct.ident.to_string();
            let fields = match &item_struct.fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| parse_account_field(f, item_struct))
                    .collect(),
                _ => continue,
            };

            result.push(RawAccountsStruct { name, fields });
        }
    }
    result
}

fn has_derive_accounts(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("derive") {
            let tokens = attr.meta.require_list().ok().map(|l| l.tokens.to_string());
            if let Some(t) = tokens {
                if t.contains("Accounts") {
                    return true;
                }
            }
        }
    }
    false
}

fn parse_account_field(field: &syn::Field, parent: &syn::ItemStruct) -> RawAccountField {
    let name = field.ident.as_ref().unwrap().to_string();
    let writable = helpers::is_mut_ref(&field.ty);
    let signer = helpers::is_signer_type(&field.ty);

    // Collect sibling field names for seed reference detection
    let sibling_names: Vec<String> = match &parent.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
            .collect(),
        _ => vec![],
    };

    let pda = parse_pda_from_attrs(&field.attrs, &sibling_names);
    let address = detect_known_address(&field.ty);

    RawAccountField {
        name,
        writable,
        signer,
        pda,
        address,
    }
}

/// Detect known addresses for sysvars and programs.
/// Returns a base58 address string for known types.
fn detect_known_address(ty: &syn::Type) -> Option<String> {
    let base = helpers::type_base_name(ty)?;

    match base.as_str() {
        "SystemProgram" => Some("11111111111111111111111111111111".to_string()),
        "Sysvar" => {
            let inner = helpers::type_inner_name(ty)?;
            match inner.as_str() {
                "Rent" => Some("SysvarRent111111111111111111111111111111111".to_string()),
                "Clock" => Some("SysvarC1ock11111111111111111111111111111111".to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Parse `#[account(seeds = [...], bump)]` from field attributes.
fn parse_pda_from_attrs(attrs: &[syn::Attribute], sibling_names: &[String]) -> Option<RawPda> {
    for attr in attrs {
        if !attr.path().is_ident("account") {
            continue;
        }

        let tokens = match attr.meta.require_list() {
            Ok(list) => list.tokens.clone(),
            Err(_) => continue,
        };

        let tokens_str = tokens.to_string();

        // Check if this attribute contains seeds
        if !tokens_str.contains("seeds") {
            continue;
        }

        // Parse the seeds expression
        let seeds = parse_seeds_from_tokens(&tokens, sibling_names);
        if !seeds.is_empty() {
            return Some(RawPda { seeds });
        }
    }
    None
}

/// Parse seeds from the attribute token stream.
/// Handles: `seeds = [b"escrow", maker], bump`
fn parse_seeds_from_tokens(
    tokens: &proc_macro2::TokenStream,
    sibling_names: &[String],
) -> Vec<RawSeed> {
    // Parse as a sequence of directives separated by commas
    // We need to find `seeds = [...]` and extract the array contents
    let tokens_str = tokens.to_string();

    // Find the seeds array
    let seeds_idx = match tokens_str.find("seeds") {
        Some(idx) => idx,
        None => return vec![],
    };

    let after_seeds = &tokens_str[seeds_idx..];
    let eq_idx = match after_seeds.find('=') {
        Some(idx) => idx,
        None => return vec![],
    };

    let after_eq = after_seeds[eq_idx + 1..].trim();

    // Find the matching brackets
    let bracket_start = match after_eq.find('[') {
        Some(idx) => idx,
        None => return vec![],
    };

    let mut depth = 0;
    let mut bracket_end = None;
    for (i, c) in after_eq[bracket_start..].chars().enumerate() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    bracket_end = Some(bracket_start + i);
                    break;
                }
            }
            _ => {}
        }
    }

    let bracket_end = match bracket_end {
        Some(idx) => idx,
        None => return vec![],
    };

    let inner = &after_eq[bracket_start + 1..bracket_end];

    // Parse each seed expression
    // Split by comma, but respect nested brackets/strings
    let seed_strs = split_seeds(inner);

    seed_strs
        .iter()
        .filter_map(|s| parse_single_seed(s.trim(), sibling_names))
        .collect()
}

/// Split seed expressions by comma, respecting nested brackets and string literals.
fn split_seeds(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_string = false;

    for c in s.chars() {
        match c {
            '"' => {
                in_string = !in_string;
                current.push(c);
            }
            '[' | '(' if !in_string => {
                depth += 1;
                current.push(c);
            }
            ']' | ')' if !in_string => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 && !in_string => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }

    parts
}

/// Parse a single seed expression string.
fn parse_single_seed(s: &str, sibling_names: &[String]) -> Option<RawSeed> {
    let s = s.trim();

    // Byte string literal: b"escrow"
    if s.starts_with("b\"") && s.ends_with('"') {
        let inner = &s[2..s.len() - 1];
        return Some(RawSeed::ByteString(inner.as_bytes().to_vec()));
    }

    // Simple identifier that matches a sibling field name
    if s.chars().all(|c| c.is_alphanumeric() || c == '_') && sibling_names.contains(&s.to_string())
    {
        return Some(RawSeed::AccountRef(s.to_string()));
    }

    // Byte array: &[u8] or similar
    // For now, try to interpret as a const byte literal
    None
}

/// Convert a `RawAccountsStruct` into IDL account items.
pub fn to_idl_accounts(raw: &RawAccountsStruct) -> Vec<IdlAccountItem> {
    raw.fields.iter().map(to_idl_account_item).collect()
}

fn to_idl_account_item(field: &RawAccountField) -> IdlAccountItem {
    let pda = field.pda.as_ref().map(|pda| IdlPda {
        seeds: pda
            .seeds
            .iter()
            .map(|seed| match seed {
                RawSeed::ByteString(bytes) => IdlSeed::Const {
                    value: bytes.clone(),
                },
                RawSeed::AccountRef(name) => IdlSeed::Account {
                    path: helpers::to_camel_case(name),
                },
            })
            .collect(),
    });

    IdlAccountItem {
        name: helpers::to_camel_case(&field.name),
        writable: field.writable,
        signer: field.signer,
        pda,
        address: field.address.clone(),
    }
}
