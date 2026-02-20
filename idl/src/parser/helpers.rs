use crate::types::IdlType;

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

/// Extract the last segment identifier from a syn::Type.
/// e.g. `Account<TokenAccount>` → "Account", `Signer` → "Signer"
pub fn type_base_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(type_path) => type_path.path.segments.last().map(|s| s.ident.to_string()),
        syn::Type::Reference(type_ref) => type_base_name(&type_ref.elem),
        _ => None,
    }
}

/// Extract the first generic argument's base name from a type path.
/// e.g. `Account<TokenAccount>` → Some("TokenAccount"), `Signer` → None
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
