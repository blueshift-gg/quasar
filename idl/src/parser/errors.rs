//! Parses `#[error_code]` enums for IDL generation.

use {crate::types::IdlError, syn::Item};

/// Extract all `#[error_code]` enums from a parsed file.
pub fn extract_errors(file: &syn::File) -> Vec<IdlError> {
    let mut result = Vec::new();
    for item in &file.items {
        if let Item::Enum(item_enum) = item {
            if !has_error_code_attr(&item_enum.attrs) {
                continue;
            }

            let mut next_code: u32 = 0;
            for variant in &item_enum.variants {
                if let Some((
                    _,
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit_int),
                        ..
                    }),
                )) = &variant.discriminant
                {
                    if let Ok(v) = lit_int.base10_parse::<u32>() {
                        next_code = v;
                    }
                }

                result.push(IdlError {
                    code: next_code,
                    name: variant.ident.to_string(),
                    msg: None,
                });

                next_code += 1;
            }
        }
    }
    result
}

fn has_error_code_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("error_code"))
}
