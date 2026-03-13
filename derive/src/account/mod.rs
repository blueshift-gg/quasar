//! `#[account]` — generates the zero-copy companion struct, discriminator
//! validation, `Owner`/`Discriminator`/`Space` trait impls, and typed accessor
//! methods for on-chain account types.

mod accessors;
mod dynamic;
mod fixed;

use {
    crate::helpers::{
        classify_dynamic_string, classify_dynamic_vec, classify_tail,
        validate_discriminator_not_zero, DynKind, InstructionArgs,
    },
    proc_macro::TokenStream,
    syn::{parse_macro_input, Data, DeriveInput, Fields},
};

pub(crate) fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

    if let Err(e) = validate_discriminator_not_zero(disc_bytes) {
        return e.to_compile_error().into();
    }

    let disc_indices: Vec<usize> = (0..disc_len).collect();

    let fields_data = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "#[account] can only be used on structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "#[account] can only be used on structs")
                .to_compile_error()
                .into();
        }
    };

    let field_kinds: Vec<DynKind> = fields_data
        .iter()
        .map(|f| {
            if let Some((prefix, max)) = classify_dynamic_string(&f.ty) {
                DynKind::Str { prefix, max }
            } else if let Some(tail_elem) = classify_tail(&f.ty) {
                DynKind::Tail { element: tail_elem }
            } else if let Some((elem, prefix, max)) = classify_dynamic_vec(&f.ty) {
                DynKind::Vec {
                    elem: Box::new(elem),
                    prefix,
                    max,
                }
            } else {
                DynKind::Fixed
            }
        })
        .collect();

    let has_dynamic = field_kinds.iter().any(|k| !matches!(k, DynKind::Fixed));

    if !has_dynamic {
        return fixed::generate_fixed_account(
            name,
            disc_bytes,
            disc_len,
            &disc_indices,
            fields_data,
            &input,
        );
    }

    // Validate: fixed fields must precede all dynamic fields
    let first_dynamic = field_kinds
        .iter()
        .position(|k| !matches!(k, DynKind::Fixed));
    let last_fixed = field_kinds
        .iter()
        .rposition(|k| matches!(k, DynKind::Fixed));
    if let (Some(fd), Some(lf)) = (first_dynamic, last_fixed) {
        if lf > fd {
            return syn::Error::new_spanned(
                &fields_data[lf],
                "fixed fields must precede all dynamic fields (String/Vec)",
            )
            .to_compile_error()
            .into();
        }
    }

    // Validate: Vec element types must not be dynamic (no nested String/Vec)
    if let Some(f) = fields_data
        .iter()
        .zip(field_kinds.iter())
        .find_map(|(f, k)| match k {
            DynKind::Vec { elem, .. }
                if classify_dynamic_string(elem).is_some()
                    || classify_dynamic_vec(elem).is_some() =>
            {
                Some(f)
            }
            _ => None,
        })
    {
        return syn::Error::new_spanned(
            f,
            "Vec element type must be a fixed-size type; nested dynamic types (String/Vec) are \
             not supported",
        )
        .to_compile_error()
        .into();
    }

    // Validate: at most one tail field, and it must be the last field
    let tail_count = field_kinds
        .iter()
        .filter(|k| matches!(k, DynKind::Tail { .. }))
        .count();
    if tail_count > 1 {
        return syn::Error::new_spanned(
            name,
            "at most one tail field (&str / &[u8]) is allowed per struct",
        )
        .to_compile_error()
        .into();
    }
    if tail_count == 1 && !matches!(field_kinds.last(), Some(DynKind::Tail { .. })) {
        let tail_field = fields_data
            .iter()
            .zip(field_kinds.iter())
            .find_map(|(f, k)| matches!(k, DynKind::Tail { .. }).then_some(f))
            .unwrap();
        return syn::Error::new_spanned(
            tail_field,
            "tail field (&str / &[u8]) must be the last field in the struct",
        )
        .to_compile_error()
        .into();
    }

    // Validate: struct must have a lifetime parameter
    if input.generics.lifetimes().next().is_none() {
        return syn::Error::new_spanned(
            name,
            "structs with dynamic fields (String/Vec/tail) must have a lifetime parameter, e.g. \
             Profile<'a>",
        )
        .to_compile_error()
        .into();
    }

    dynamic::generate_dynamic_account(
        name,
        disc_bytes,
        disc_len,
        &disc_indices,
        fields_data,
        &field_kinds,
        &input,
    )
}
