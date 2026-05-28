//! Syntactic type-inspection helpers used by both `quasar-derive` and the
//! resolver in `quasar-hir`.

use syn::{GenericArgument, PathArguments, Type};

/// Returns `Some(inner)` when `ty` is `Wrapper<Inner, ...>` for the given
/// `wrapper` name on the path's last segment.
pub fn extract_generic_inner_type<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            if last.ident == wrapper {
                if let PathArguments::AngleBracketed(args) = &last.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// True when the field type should be treated as a composite (nested
/// `Accounts` struct) rather than a single account binding.
pub fn is_composite_type(ty: &Type) -> bool {
    if matches!(ty, Type::Reference(_)) {
        return false;
    }
    if extract_generic_inner_type(ty, "Option").is_some() {
        return false;
    }
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            if last.ident == "AccountsArray" {
                return true;
            }
        }
    }
    classify_lifetime_arg(ty)
}

/// True for the unit type `()`.
pub fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
}

/// True when the type's last path segment carries at least one lifetime
/// argument (e.g. `Account<'info, T>`).
pub fn classify_lifetime_arg(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(last) = tp.path.segments.last() {
            if let PathArguments::AngleBracketed(args) = &last.arguments {
                return args
                    .args
                    .iter()
                    .any(|a| matches!(a, GenericArgument::Lifetime(_)));
            }
        }
    }
    false
}
