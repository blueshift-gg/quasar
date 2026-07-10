//! IDL token emission: the derive-side helpers that build
//! `quasar_lang::idl_build` constructor tokens for a type's IDL `type`,
//! `codec`, and `layout`.
//!
//! [`emit_idl_layout`] is the single source for the `IdlLayout` fragment shared
//! by `#[account]` (`account/mod.rs`) and `#[program]` instructions
//! (`program/idl.rs`), which previously hand-wrote near-verbatim copies.

use {
    crate::helpers::{classify_pod_dynamic, extract_generic_inner_type, PodDynField},
    quote::quote,
    syn::{GenericArgument, PathArguments, Type},
};

/// Project an ordered field list into the `IdlLayout` tokens by partitioning it
/// into inline (fixed) and tail (dynamic) names, then deferring to
/// [`emit_idl_layout`].
///
/// The dynamic flag per field is the SAME classification that drives the
/// compact wire schema (`fi.pod_dyn.is_some()` for `#[account]` fields,
/// `instruction_arg_is_compact` for instruction args — both mirror
/// `schema_ir::LayoutClass::is_dynamic`). Partitioning here, from that single
/// classification, is what keeps the emitted IDL layout from drifting away from
/// the wire layout: a field cannot be inline in the IDL yet tail on the wire.
/// Field order is preserved within each region, matching the wire ordering.
pub(crate) fn project_idl_layout(fields: &[(String, bool)]) -> proc_macro2::TokenStream {
    let inline: Vec<String> = fields
        .iter()
        .filter(|(_, dynamic)| !dynamic)
        .map(|(name, _)| name.clone())
        .collect();
    let tail: Vec<String> = fields
        .iter()
        .filter(|(_, dynamic)| *dynamic)
        .map(|(name, _)| name.clone())
        .collect();
    emit_idl_layout(&inline, &tail)
}

/// Emit the `Some(IdlLayout::{Fixed|Compact})` tokens for a field/arg list
/// split into inline (fixed) and tail (dynamic) names. A `Fixed` layout is
/// emitted when there are no tail fields; otherwise a `Compact` layout with the
/// standard wire ordering. Callers that need `None` for an empty list handle
/// that before calling.
fn emit_idl_layout(inline: &[String], tail: &[String]) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    if tail.is_empty() {
        quote! {
            Some(#krate::idl_build::__reexport::IdlLayout::Fixed {
                fields: #krate::idl_build::vec![
                    #(#krate::idl_build::s(#inline)),*
                ],
            })
        }
    } else {
        quote! {
            Some(#krate::idl_build::__reexport::IdlLayout::Compact {
                inline_fields: #krate::idl_build::vec![
                    #(#krate::idl_build::s(#inline)),*
                ],
                tail_fields: #krate::idl_build::vec![
                    #(#krate::idl_build::s(#tail)),*
                ],
                wire: #krate::idl_build::__reexport::CompactWire::InlineFieldsThenTailHeadersThenTailPayloads,
            })
        }
    }
}

/// Convert a Rust type to a `proc_macro2::TokenStream` that constructs an
/// `Option<IdlCodec>` at runtime (used by IDL fragment emission).
///
/// Returns `None` for fixed types (inferred), and `Some(IdlCodec::SizePrefixed
/// { .. })` for dynamic types (PodString, PodVec, String, Vec with const
/// generics).
pub(crate) fn type_to_idl_codec_tokens(ty: &Type) -> proc_macro2::TokenStream {
    let dyn_field = classify_pod_dynamic(ty)
        .or_else(|| extract_generic_inner_type(ty, "Option").and_then(classify_pod_dynamic));

    match dyn_field {
        Some(dyn_field) => idl_codec_for_dynamic(dyn_field),
        None => quote! { None },
    }
}

fn idl_codec_for_dynamic(dyn_field: PodDynField) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => {
            let pfx_ty_str = prefix_type_name(prefix_bytes, "u8");
            quote! {
                Some(#krate::idl_build::__reexport::IdlCodec::SizePrefixed {
                    prefix: #krate::idl_build::__reexport::ScalarRepr {
                        ty: #krate::idl_build::s(#pfx_ty_str),
                        endian: #krate::idl_build::__reexport::Endian::Le,
                    },
                    storage: #krate::idl_build::__reexport::Storage::Tail,
                    max_bytes: Some(#max),
                    max_items: None,
                    encoding: Some(#krate::idl_build::s("utf8")),
                    item: None,
                })
            }
        }
        PodDynField::Vec {
            max, prefix_bytes, ..
        } => {
            let pfx_ty_str = prefix_type_name(prefix_bytes, "u16");
            quote! {
                Some(#krate::idl_build::__reexport::IdlCodec::SizePrefixed {
                    prefix: #krate::idl_build::__reexport::ScalarRepr {
                        ty: #krate::idl_build::s(#pfx_ty_str),
                        endian: #krate::idl_build::__reexport::Endian::Le,
                    },
                    storage: #krate::idl_build::__reexport::Storage::Tail,
                    max_bytes: None,
                    max_items: Some(#max),
                    encoding: None,
                    item: None,
                })
            }
        }
    }
}

fn prefix_type_name(prefix_bytes: usize, fallback: &'static str) -> &'static str {
    match prefix_bytes {
        1 => "u8",
        2 => "u16",
        4 => "u32",
        8 => "u64",
        _ => fallback,
    }
}

/// Convert a Rust type to a `proc_macro2::TokenStream` that constructs an
/// `IdlType` at runtime (used by IDL fragment emission).
pub(crate) fn type_to_idl_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    if let Type::Array(array) = ty {
        let inner_tokens = type_to_idl_type_tokens(&array.elem);
        let len = &array.len;
        return quote! {
            #krate::idl_build::__reexport::IdlType::Array {
                array: (
                    #krate::idl_build::Box::new(#inner_tokens),
                    #len,
                ),
            }
        };
    }

    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let name = seg.ident.to_string();
            let primitive = match name.as_str() {
                "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" | "u128" | "i128"
                | "bool" | "f32" | "f64" => Some(name.clone()),
                "Address" | "Pubkey" => Some("pubkey".to_owned()),
                _ => None,
            };
            if let Some(prim) = primitive {
                return quote! {
                    #krate::idl_build::__reexport::IdlType::Primitive(#krate::idl_build::s(#prim))
                };
            }
            // Option<T>
            if name == "Option" {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                        let inner_tokens = type_to_idl_type_tokens(inner);
                        return quote! {
                            #krate::idl_build::__reexport::IdlType::Option {
                                option: #krate::idl_build::Box::new(#inner_tokens),
                            }
                        };
                    }
                }
            }
            // Vec<T> / PodVec<T, N>
            if name == "Vec" || name == "PodVec" {
                if let PathArguments::AngleBracketed(ab) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = ab.args.first() {
                        let inner_tokens = type_to_idl_type_tokens(inner);
                        return quote! {
                            #krate::idl_build::__reexport::IdlType::Vec {
                                vec: #krate::idl_build::Box::new(#inner_tokens),
                            }
                        };
                    }
                }
            }
            // String / PodString
            if name == "String" || name == "PodString" {
                return quote! {
                    #krate::idl_build::__reexport::IdlType::Primitive(#krate::idl_build::s("string"))
                };
            }
            // Fall back to defined type reference
            return quote! {
                #krate::idl_build::__reexport::IdlType::Defined {
                    defined: #krate::idl_build::__reexport::IdlDefinedRef {
                        name: #krate::idl_build::s(#name),
                        generics: #krate::idl_build::Vec::new(),
                    },
                }
            };
        }
    }
    // Fallback: opaque bytes
    quote! {
        #krate::idl_build::__reexport::IdlType::Primitive(#krate::idl_build::s("bytes"))
    }
}
