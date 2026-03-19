use {
    quasar_idl::parser::helpers,
    syn::{spanned::Spanned, Fields, Item},
};

pub(crate) struct AuditAccountsStruct {
    pub name: String,
    pub file_path: String,
    pub fields: Vec<AuditField>,
}

pub(crate) struct AuditField {
    pub name: String,
    pub line: usize,
    pub type_name: String,
    pub type_inner: String,
    pub writable: bool,
    pub signer: bool,
    #[allow(dead_code)]
    pub has_init: bool,
    pub has_init_if_needed: bool,
    pub has_close: bool,
    pub has_one: Vec<String>,
    pub has_constraint: bool,
    pub has_address: bool,
    pub has_owner: bool,
    pub has_token_constraint: bool,
    pub token_authority_ref: String,
    pub pda_seed_count: usize,
    pub pda_has_account_ref: bool,
    pub pda_seed_refs: Vec<String>,
}

pub fn extract_audit_accounts(file: &syn::File, file_path: &str) -> Vec<AuditAccountsStruct> {
    let mut result = Vec::new();
    for item in &file.items {
        if let Item::Struct(item_struct) = item {
            if !has_derive_accounts(&item_struct.attrs) {
                continue;
            }

            let name = item_struct.ident.to_string();
            let sibling_names: Vec<String> = match &item_struct.fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
                    .collect(),
                _ => vec![],
            };

            let fields = match &item_struct.fields {
                Fields::Named(named) => named
                    .named
                    .iter()
                    .map(|f| parse_audit_field(f, &sibling_names))
                    .collect(),
                _ => continue,
            };

            result.push(AuditAccountsStruct {
                name,
                file_path: file_path.to_string(),
                fields,
            });
        }
    }
    result
}

fn has_derive_accounts(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .meta
                .require_list()
                .ok()
                .is_some_and(|l| l.tokens.to_string().contains("Accounts"))
    })
}

fn parse_audit_field(field: &syn::Field, sibling_names: &[String]) -> AuditField {
    let line = field.span().start().line;
    let name = field
        .ident
        .as_ref()
        .expect("named field must have ident")
        .to_string();
    let type_name = helpers::type_base_name(&field.ty).unwrap_or_default();
    let type_inner = helpers::type_inner_name(&field.ty).unwrap_or_default();
    let signer = helpers::is_signer_type(&field.ty);

    let mut writable = helpers::is_mut_ref(&field.ty);
    let mut has_init = false;
    let mut has_init_if_needed = false;
    let mut has_close = false;
    let mut has_one = Vec::new();
    let mut has_constraint = false;
    let mut has_address = false;
    let mut has_owner = false;
    let mut has_token_constraint = false;
    let mut token_authority_ref = String::new();
    let mut pda_seed_count = 0;
    let mut pda_has_account_ref = false;
    let mut pda_seed_refs = Vec::new();

    for attr in &field.attrs {
        if !attr.path().is_ident("account") {
            continue;
        }
        let tokens_str = match attr.meta.require_list() {
            Ok(list) => list.tokens.to_string(),
            Err(_) => continue,
        };

        for d in split_directives(&tokens_str) {
            if d == "mut" {
                writable = true;
            } else if d == "init" {
                has_init = true;
                writable = true;
            } else if d == "init_if_needed" {
                has_init_if_needed = true;
                writable = true;
            } else if d.starts_with("close") {
                has_close = true;
                writable = true;
            } else if d.starts_with("has_one") {
                if let Some(val) = d
                    .strip_prefix("has_one")
                    .and_then(|s| s.trim().strip_prefix('='))
                {
                    has_one.push(val.trim().to_string());
                }
            } else if d.starts_with("constraint") {
                has_constraint = true;
            } else if d.starts_with("address") {
                has_address = true;
            } else if d.starts_with("owner") {
                has_owner = true;
            } else if d.starts_with("seeds") {
                let (count, has_ref, refs) = count_seeds(d, sibling_names);
                pda_seed_count = count;
                pda_has_account_ref = has_ref;
                pda_seed_refs = refs;
            } else if d.starts_with("token :: mint") || d.starts_with("token::mint") {
                has_token_constraint = true;
            } else if d.starts_with("token :: authority") || d.starts_with("token::authority") {
                has_token_constraint = true;
                if let Some(val) = d.split('=').nth(1) {
                    let val = val.trim();
                    if sibling_names.iter().any(|n| n == val) {
                        token_authority_ref = val.to_string();
                    }
                }
            }
        }
    }

    if type_name == "SystemProgram" || type_name == "Sysvar" {
        has_address = true;
    }

    AuditField {
        name,
        line,
        type_name,
        type_inner,
        writable,
        signer,
        has_init,
        has_init_if_needed,
        has_close,
        has_one,
        has_constraint,
        has_address,
        has_owner,
        has_token_constraint,
        token_authority_ref,
        pda_seed_count,
        pda_has_account_ref,
        pda_seed_refs,
    }
}

fn split_directives(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0u32;
    let mut in_string = false;

    for (i, c) in s.char_indices() {
        match c {
            '"' => in_string = !in_string,
            '[' | '(' if !in_string => depth += 1,
            ']' | ')' if !in_string => depth = depth.saturating_sub(1),
            ',' if depth == 0 && !in_string => {
                let trimmed = s[start..i].trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed);
                }
                start = i + 1;
            }
            _ => {}
        }
    }

    let trimmed = s[start..].trim();
    if !trimmed.is_empty() {
        parts.push(trimmed);
    }

    parts
}

fn count_seeds(seeds_directive: &str, sibling_names: &[String]) -> (usize, bool, Vec<String>) {
    let eq_pos = match seeds_directive.find('=') {
        Some(idx) => idx,
        None => return (0, false, vec![]),
    };
    let after_eq = seeds_directive[eq_pos + 1..].trim();

    let start = match after_eq.find('[') {
        Some(idx) => idx,
        None => return (0, false, vec![]),
    };
    let mut depth = 0;
    let mut end = None;
    for (i, c) in after_eq[start..].chars().enumerate() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = match end {
        Some(idx) => idx,
        None => return (0, false, vec![]),
    };

    let inner = &after_eq[start + 1..end];
    let seeds: Vec<&str> = inner
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let count = seeds.len();
    let refs: Vec<String> = seeds
        .iter()
        .filter_map(|s| {
            let s = s.trim();
            if !s.starts_with("b\"")
                && s.chars().all(|c| c.is_alphanumeric() || c == '_')
                && sibling_names.iter().any(|n| n == s)
            {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect();
    let has_ref = !refs.is_empty();

    (count, has_ref, refs)
}
