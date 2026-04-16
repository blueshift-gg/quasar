//! Shared helpers for IDL parsing: type mapping, name conversion, and
//! dynamic field classification.

use crate::types::{IdlDynString, IdlDynVec, IdlType};
pub use quasar_schema::to_camel_case;

/// Map a Rust type name string to an IDL type.
pub fn map_type(rust_type: &str) -> IdlType {
    match rust_type {
        "Address" | "Pubkey" => IdlType::Primitive("pubkey".to_string()),
        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" => {
            IdlType::Primitive(rust_type.to_string())
        }
        "f32" | "f64" => IdlType::Primitive(rust_type.to_string()),
        "bool" => IdlType::Primitive("bool".to_string()),
        "String" => IdlType::Primitive("string".to_string()),
        other => IdlType::Defined {
            defined: other.to_string(),
        },
    }
}

/// Map a `syn::Type` to an `IdlType`, detecting dynamic fields:
///
/// - `String<N>` / `String<N, u16>` / `PodString<N, u32>` →
///   `IdlType::DynString`
/// - `Vec<T, N>` / `Vec<T, N, u8>` / `PodVec<T, N, u64>` → `IdlType::DynVec`
///
/// The optional third argument for String (or fourth for Vec) sets the prefix
/// width: `u8`=1, `u16`=2, `u32`=4, `u64`=8 bytes. Defaults: String→1, Vec→2.
/// Leading lifetime parameters (e.g. `'a`) are skipped transparently.
/// Falls back to `simple_type_name + map_type` for everything else.
pub fn map_type_from_syn(ty: &syn::Type) -> IdlType {
    let inner = match ty {
        syn::Type::Reference(r) => &*r.elem,
        other => other,
    };

    // Handle fixed-size arrays: [T; N]
    if let syn::Type::Array(arr) = inner {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit_int),
            ..
        }) = &arr.len
        {
            if let Ok(n) = lit_int.base10_parse::<usize>() {
                let elem_name = simple_type_name(&arr.elem);
                return IdlType::Primitive(format!("[{}; {}]", elem_name, n));
            }
        }
    }

    if let syn::Type::Path(type_path) = inner {
        if let Some(seg) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                let ident = seg.ident.to_string();
                let mut iter = args.args.iter();

                if ident == "Option" {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = iter.next() {
                        return IdlType::Option {
                            option: Box::new(map_type_from_syn(inner_ty)),
                        };
                    }
                } else if ident == "String" || ident == "PodString" {
                    // String<N[, PFX]> / String<'a, N[, PFX]>
                    // Skip an optional leading lifetime parameter.
                    let first = iter.next();
                    let first = if matches!(first, Some(syn::GenericArgument::Lifetime(_))) {
                        iter.next()
                    } else {
                        first
                    };
                    if let Some(max_length) = first.and_then(extract_const_usize) {
                        let prefix_bytes = iter.next().and_then(extract_prefix_bytes).unwrap_or(1);
                        return IdlType::DynString {
                            string: IdlDynString {
                                max_length,
                                prefix_bytes,
                            },
                        };
                    }
                } else if ident == "Vec" || ident == "PodVec" {
                    // Vec<T, N[, PFX]> / Vec<'a, T, N[, PFX]>
                    let first = iter.next();
                    if let Some(result) = parse_pod_vec_args(first, &mut iter) {
                        return result;
                    }
                }
            }
        }
    }

    let type_name = simple_type_name(ty);
    map_type(&type_name)
}

/// Extract the last segment identifier from a syn::Type.
/// e.g. `Account<Token>` → "Account", `Signer` → "Signer"
pub fn type_base_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(type_path) => type_path.path.segments.last().map(|s| s.ident.to_string()),
        syn::Type::Reference(type_ref) => type_base_name(&type_ref.elem),
        _ => None,
    }
}

/// Extract the first generic argument's base name from a type path.
/// e.g. `Account<Token>` → Some("Token"), `Signer` → None
pub fn type_inner_name(ty: &syn::Type) -> Option<String> {
    let inner = match ty {
        syn::Type::Reference(type_ref) => &*type_ref.elem,
        other => other,
    };
    match inner {
        syn::Type::Path(type_path) => {
            let last = type_path.path.segments.last()?;
            match &last.arguments {
                syn::PathArguments::AngleBracketed(args) => match args.args.first()? {
                    syn::GenericArgument::Type(inner_ty) => type_base_name(inner_ty),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

/// Check if a field type's reference is mutable (`&'a mut T`).
pub fn is_mut_ref(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Reference(r) if r.mutability.is_some())
}

/// Check if the base type name is "Signer".
pub fn is_signer_type(ty: &syn::Type) -> bool {
    type_base_name(ty).as_deref() == Some("Signer")
}

/// Parse a discriminator value from a token string containing `discriminator =
/// N` or `discriminator = [N, M, ...]`.
///
/// Shared by event, account, and instruction parsers.
pub fn parse_discriminator_value(tokens_str: &str) -> Option<Vec<u8>> {
    let eq_pos = tokens_str.find('=')?;
    let value_str = tokens_str[eq_pos + 1..].trim();

    if value_str.starts_with('[') {
        let inner = value_str.trim_start_matches('[').trim_end_matches(']');
        let bytes: Result<Vec<u8>, _> = inner
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().parse::<u8>())
            .collect();
        let bytes = bytes.ok()?;
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

/// Extract the simple type name string from a syn::Type for IDL field types.
/// Strips references and returns just the final identifier.
pub fn simple_type_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        syn::Type::Reference(type_ref) => simple_type_name(&type_ref.elem),
        syn::Type::Array(arr) => {
            let inner = simple_type_name(&arr.elem);
            format!("[{}]", inner)
        }
        _ => "unknown".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers for parsing dynamic type generic arguments
// ---------------------------------------------------------------------------

fn extract_const_usize(arg: &syn::GenericArgument) -> Option<usize> {
    if let syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(lit_int),
        ..
    })) = arg
    {
        lit_int.base10_parse().ok()
    } else {
        None
    }
}

/// Parse Vec/PodVec generic args: `<T, N[, PFX]>` (where `first` is already
/// consumed). Default prefix is 2 bytes (u16). Optional third arg sets the
/// prefix width.
fn parse_pod_vec_args<'a>(
    first: Option<&'a syn::GenericArgument>,
    rest: &mut impl Iterator<Item = &'a syn::GenericArgument>,
) -> Option<IdlType> {
    // Skip an optional leading lifetime parameter (e.g. Vec<'a, T, N>).
    let first = if matches!(first, Some(syn::GenericArgument::Lifetime(_))) {
        rest.next()
    } else {
        first
    };
    let syn::GenericArgument::Type(elem_ty) = first? else {
        return None;
    };
    let second = rest.next()?;
    let max_length = extract_const_usize(second)?;
    let prefix_bytes = rest.next().and_then(extract_prefix_bytes).unwrap_or(2);
    Some(IdlType::DynVec {
        vec: IdlDynVec {
            items: Box::new(map_type_from_syn(elem_ty)),
            max_length,
            prefix_bytes,
        },
    })
}

/// Extract prefix byte count from a generic argument.
/// Accepts type-path form (`u8`→1, `u16`→2, `u32`→4, `u64`→8) and
/// const-integer form (`1`, `2`, `4`, `8`).
fn extract_prefix_bytes(arg: &syn::GenericArgument) -> Option<usize> {
    match arg {
        syn::GenericArgument::Type(syn::Type::Path(tp)) => {
            match tp.path.segments.last()?.ident.to_string().as_str() {
                "u8" => Some(1),
                "u16" => Some(2),
                "u32" => Some(4),
                "u64" => Some(8),
                _ => None,
            }
        }
        syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(n),
            ..
        })) => n.base10_parse().ok(),
        _ => None,
    }
}
