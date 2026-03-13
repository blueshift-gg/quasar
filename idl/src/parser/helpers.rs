//! Shared helpers for IDL parsing: type mapping, name conversion, and
//! dynamic field classification.

use crate::types::{IdlDynString, IdlDynVec, IdlTail, IdlType};

/// Convert `snake_case` to `camelCase`.
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Map a Rust type name string to an IDL type.
pub fn map_type(rust_type: &str) -> IdlType {
    match rust_type {
        "Address" | "Pubkey" => IdlType::Primitive("publicKey".to_string()),
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
/// - `String<'a, N>` / `String<N>` → `IdlType::DynString { maxLength: N }`
/// - `Vec<'a, T, N>` / `Vec<T, N>` → `IdlType::DynVec { items: T, maxLength: N
///   }`
///
/// Falls back to `simple_type_name + map_type` for everything else.
pub fn map_type_from_syn(ty: &syn::Type) -> IdlType {
    if let syn::Type::Reference(ref_ty) = ty {
        match &*ref_ty.elem {
            syn::Type::Path(type_path) => {
                if let Some(seg) = type_path.path.segments.last() {
                    if seg.ident == "str" && type_path.path.segments.len() == 1 {
                        return IdlType::Tail {
                            tail: IdlTail {
                                element: "string".to_string(),
                            },
                        };
                    }
                }
            }
            syn::Type::Slice(slice_ty) => {
                if let syn::Type::Path(type_path) = &*slice_ty.elem {
                    if let Some(seg) = type_path.path.segments.last() {
                        if seg.ident == "u8" && type_path.path.segments.len() == 1 {
                            return IdlType::Tail {
                                tail: IdlTail {
                                    element: "bytes".to_string(),
                                },
                            };
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let inner = match ty {
        syn::Type::Reference(r) => &*r.elem,
        other => other,
    };

    if let syn::Type::Path(type_path) = inner {
        if let Some(seg) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                let ident = seg.ident.to_string();
                let mut iter = args.args.iter();

                // Skip leading lifetime if present
                let first = iter.next();
                let has_lifetime = matches!(first, Some(syn::GenericArgument::Lifetime(_)));

                if ident == "String" {
                    // String<'a, N> or String<N>
                    let const_arg = if has_lifetime { iter.next() } else { first };
                    if let Some(syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Int(lit_int),
                        ..
                    }))) = const_arg
                    {
                        if let Ok(max_length) = lit_int.base10_parse::<usize>() {
                            return IdlType::DynString {
                                string: IdlDynString { max_length },
                            };
                        }
                    }
                } else if ident == "Vec" {
                    // Vec<'a, T, N> or Vec<T, N>
                    let type_arg = if has_lifetime { iter.next() } else { first };
                    if let Some(syn::GenericArgument::Type(elem_ty)) = type_arg {
                        if let Some(syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Int(lit_int),
                            ..
                        }))) = iter.next()
                        {
                            if let Ok(max_length) = lit_int.base10_parse::<usize>() {
                                let items = map_type_from_syn(elem_ty);
                                return IdlType::DynVec {
                                    vec: IdlDynVec {
                                        items: Box::new(items),
                                        max_length,
                                    },
                                };
                            }
                        }
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
#[allow(dead_code)]
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
