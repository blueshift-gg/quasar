//! Parses `#[derive(Accounts)]` structs to extract account metadata,
//! constraints, PDA seeds, and field types for IDL generation.

use {
    crate::{
        lint::constraints::{self, FieldClass, FieldConstraints},
        parser::helpers,
        types::{IdlAccountItem, IdlPda, IdlSeed},
    },
    quasar_schema::known_address_for_type,
    syn::{Fields, Item},
};

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
    pub field_class: FieldClass,
    pub inner_type_name: Option<String>,
    pub constraints: FieldConstraints,
    pub seed_type: Option<String>,
}

#[derive(Clone)]
pub struct RawPda {
    pub seeds: Vec<RawSeed>,
}

#[derive(Clone)]
pub enum RawSeed {
    ByteString(Vec<u8>),
    AccountRef(String),
    /// Instruction argument or field access expression used as a seed.
    ArgRef(String),
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

fn has_writable_directive(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("account") {
            continue;
        }
        let tokens_str = match attr.meta.require_list() {
            Ok(list) => list.tokens.to_string(),
            Err(_) => continue,
        };
        for directive in tokens_str.split(',') {
            let d = directive.trim();
            if d == "mut"
                || d == "init"
                || d == "init_if_needed"
                || d.starts_with("close")
                || d.starts_with("realloc")
                || d.starts_with("sweep")
            {
                return true;
            }
        }
    }
    false
}

fn parse_account_field(field: &syn::Field, parent: &syn::ItemStruct) -> RawAccountField {
    let name = field.ident.as_ref().unwrap().to_string();
    let writable = helpers::is_mut_ref(&field.ty) || has_writable_directive(&field.attrs);

    // Collect sibling field names for seed reference detection
    let sibling_names: Vec<String> = match &parent.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
            .collect(),
        _ => vec![],
    };

    let (pda, seed_type) = parse_pda_from_attrs(&field.attrs, &sibling_names);
    let address = detect_known_address(&field.ty);

    let signer = helpers::is_signer_type(&field.ty);

    let (field_class, inner_type_name) = constraints::classify_field_type(&field.ty);
    let constraints = constraints::parse_field_constraints(&field.attrs);

    RawAccountField {
        name,
        writable,
        signer,
        pda,
        address,
        field_class,
        inner_type_name,
        constraints,
        seed_type,
    }
}

/// Detect known addresses for sysvars and programs.
/// Returns a base58 address string for known types.
fn detect_known_address(ty: &syn::Type) -> Option<String> {
    let base = helpers::type_base_name(ty)?;
    let inner = helpers::type_inner_name(ty);
    known_address_for_type(&base, inner.as_deref()).map(str::to_owned)
}

/// Parse `#[account(seeds = [...], bump)]` or `#[account(seeds =
/// Type::seeds(...), bump)]` from field attributes. Returns `(Option<RawPda>,
/// Option<seed_type_name>)`.
fn parse_pda_from_attrs(
    attrs: &[syn::Attribute],
    sibling_names: &[String],
) -> (Option<RawPda>, Option<String>) {
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

        // Check for Type::seeds(...) syntax first
        if let Some((type_name, args)) = parse_typed_seeds_call(&tokens_str) {
            let seeds: Vec<RawSeed> = args
                .iter()
                .filter_map(|s| parse_single_seed(s.trim(), sibling_names))
                .collect();
            return (Some(RawPda { seeds }), Some(type_name));
        }

        // Fall back to seeds = [...] syntax
        let seeds = parse_seeds_from_tokens(&tokens, sibling_names);
        if !seeds.is_empty() {
            return (Some(RawPda { seeds }), None);
        }
    }
    (None, None)
}

/// Parse `seeds = Type::seeds(arg1, arg2)` from a token string.
/// Returns `(type_name, vec_of_arg_strings)` if found.
fn parse_typed_seeds_call(tokens_str: &str) -> Option<(String, Vec<String>)> {
    // Look for pattern: `seeds = <Ident> :: seeds (`
    let seeds_idx = tokens_str.find("seeds")?;
    let after_seeds = &tokens_str[seeds_idx..];

    let eq_idx = after_seeds.find('=')?;
    let after_eq = after_seeds[eq_idx + 1..].trim();

    // Check that this is Type::seeds( and NOT seeds = [
    let colons_idx = after_eq.find("::")?;

    // Extract the type name (everything before ::)
    let type_name = after_eq[..colons_idx].trim().to_string();
    if type_name.is_empty() {
        return None;
    }

    // After :: should be "seeds (" or "seeds("
    let after_colons = after_eq[colons_idx + 2..].trim();
    if !after_colons.starts_with("seeds") {
        return None;
    }

    let after_seeds_kw = after_colons["seeds".len()..].trim();
    if !after_seeds_kw.starts_with('(') {
        return None;
    }

    // Find matching closing paren
    let mut depth = 0;
    let mut paren_end = None;
    for (i, c) in after_seeds_kw.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    paren_end = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }

    let paren_end = paren_end?;
    let inner = &after_seeds_kw[1..paren_end];

    // Split args by comma (simple split — no nested parens expected)
    let args: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Some((type_name, args))
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

/// Split seed expressions by comma, respecting nested brackets and string
/// literals.
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

    // Simple identifier that matches a sibling field name → account ref
    if s.chars().all(|c| c.is_alphanumeric() || c == '_') && sibling_names.contains(&s.to_string())
    {
        return Some(RawSeed::AccountRef(s.to_string()));
    }

    // Simple identifier (instruction arg) or dotted access (field.subfield) → arg
    // ref
    let clean = s.replace(' ', "");
    if !clean.is_empty()
        && clean
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return Some(RawSeed::ArgRef(clean));
    }

    None
}

/// Convert a `RawAccountsStruct` into IDL account items.
/// `state_accounts` is used to look up seed prefixes for `Type::seeds(...)`.
pub fn to_idl_accounts(
    raw: &RawAccountsStruct,
    state_accounts: &[super::state::RawStateAccount],
) -> Vec<IdlAccountItem> {
    raw.fields
        .iter()
        .map(|f| to_idl_account_item(f, state_accounts))
        .collect()
}

fn to_idl_account_item(
    field: &RawAccountField,
    state_accounts: &[super::state::RawStateAccount],
) -> IdlAccountItem {
    let pda = field.pda.as_ref().map(|pda| {
        let mut seeds: Vec<IdlSeed> = Vec::new();

        // If this field uses Type::seeds(...), look up the prefix from the state
        // account's #[seeds] definition and prepend it.
        if let Some(ref type_name) = field.seed_type {
            if let Some(sa) = state_accounts.iter().find(|sa| sa.name == *type_name) {
                if let Some(ref typed_seeds) = sa.seeds {
                    if !typed_seeds.prefix.is_empty() {
                        seeds.push(IdlSeed::Const {
                            value: typed_seeds.prefix.clone(),
                        });
                    }
                }
            }
        }

        seeds.extend(pda.seeds.iter().map(|seed| match seed {
            RawSeed::ByteString(bytes) => IdlSeed::Const {
                value: bytes.clone(),
            },
            RawSeed::AccountRef(name) => IdlSeed::Account {
                path: helpers::to_camel_case(name),
            },
            RawSeed::ArgRef(path) => IdlSeed::Arg {
                path: helpers::to_camel_case(path),
            },
        }));

        IdlPda { seeds }
    });

    IdlAccountItem {
        name: helpers::to_camel_case(&field.name),
        writable: field.writable,
        signer: field.signer,
        pda,
        address: field.address.clone(),
    }
}
