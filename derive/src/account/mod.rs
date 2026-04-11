//! `#[account]` — generates the zero-copy companion struct, discriminator
//! validation, `Owner`/`Discriminator`/`Space` trait impls, and typed accessor
//! methods for on-chain account types.

mod accessors;
mod dynamic;
mod fixed;
mod pod_dynamic;
pub mod seeds;

use {
    crate::helpers::{
        classify_dynamic_string, classify_dynamic_vec, classify_pod_string, classify_pod_vec,
        classify_tail, validate_discriminator_not_zero, validate_prefix_capacity, AccountAttr,
        DynKind, PodDynField,
    },
    proc_macro::TokenStream,
    syn::{parse_macro_input, Data, DeriveInput, Fields},
};

pub(crate) fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AccountAttr);
    let mut input = parse_macro_input!(item as DeriveInput);

    // Parse #[seeds(...)] if present, then strip it before downstream processing.
    let seeds_parsed = seeds::parse_seeds_attr(&input.attrs);
    let seeds_impl = match seeds_parsed {
        Some(Ok(ref attr)) => Some(seeds::generate_seeds_impl(
            &input.ident,
            &input.generics,
            attr,
        )),
        Some(Err(e)) => return e.to_compile_error().into(),
        None => None,
    };
    input.attrs.retain(|a| !a.path().is_ident("seeds"));

    let name = &input.ident;

    let gen_set_inner = args.set_inner;
    let unsafe_no_disc = args.unsafe_no_disc;
    let disc_bytes = if !args.disc_bytes.is_empty() {
        if let Err(e) = validate_discriminator_not_zero(&args.disc_bytes) {
            return e.to_compile_error().into();
        }
        args.disc_bytes
    } else {
        vec![]
    };

    let disc_len = disc_bytes.len();
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

    let mut field_kinds = Vec::with_capacity(fields_data.len());
    for f in fields_data {
        let kind = if let Some((prefix, max)) = classify_dynamic_string(&f.ty) {
            if let Err(e) = validate_prefix_capacity(&f.ty, prefix, max, "String") {
                return e.to_compile_error().into();
            }
            DynKind::Str { prefix, max }
        } else if let Some(tail_elem) = classify_tail(&f.ty) {
            DynKind::Tail { element: tail_elem }
        } else if let Some((elem, prefix, max)) = classify_dynamic_vec(&f.ty) {
            if let Err(e) = validate_prefix_capacity(&f.ty, prefix, max, "Vec") {
                return e.to_compile_error().into();
            }
            DynKind::Vec {
                elem: Box::new(elem),
                prefix,
                max,
            }
        } else {
            DynKind::Fixed
        };
        field_kinds.push(kind);
    }

    let has_dynamic = field_kinds.iter().any(|k| !matches!(k, DynKind::Fixed));

    // --- Pod-dynamic detection: check for PodString<N> / PodVec<T, N> fields ---
    let pod_field_infos: Vec<pod_dynamic::PodFieldInfo<'_>> = fields_data
        .iter()
        .map(|f| {
            let pod_dyn = if let Some(max) = classify_pod_string(&f.ty) {
                Some(PodDynField::Str { max })
            } else if let Some((elem, max)) = classify_pod_vec(&f.ty) {
                Some(PodDynField::Vec {
                    elem: Box::new(elem),
                    max,
                })
            } else {
                None
            };
            pod_dynamic::PodFieldInfo { field: f, pod_dyn }
        })
        .collect();

    let has_pod_dynamic = pod_field_infos.iter().any(|fi| fi.pod_dyn.is_some());

    if has_pod_dynamic {
        // Validate: cannot mix old String/Vec with PodString/PodVec
        if has_dynamic {
            return syn::Error::new_spanned(
                name,
                "cannot mix dynamic String/Vec fields with PodString/PodVec fields in the same struct",
            )
            .to_compile_error()
            .into();
        }
        // Validate: fixed fields must precede Pod-dynamic fields
        let first_pod_dyn = pod_field_infos.iter().position(|fi| fi.pod_dyn.is_some());
        let last_fixed = pod_field_infos.iter().rposition(|fi| fi.pod_dyn.is_none());
        if let (Some(fd), Some(lf)) = (first_pod_dyn, last_fixed) {
            if lf > fd {
                return syn::Error::new_spanned(
                    &fields_data[lf],
                    "fixed fields must precede all PodString/PodVec fields",
                )
                .to_compile_error()
                .into();
            }
        }
        if unsafe_no_disc {
            return syn::Error::new_spanned(
                name,
                "unsafe_no_disc accounts cannot have PodString/PodVec fields",
            )
            .to_compile_error()
            .into();
        }

        let mut output = pod_dynamic::generate_pod_dynamic_account(
            name,
            &disc_bytes,
            disc_len,
            &disc_indices,
            fields_data,
            &pod_field_infos,
            &input,
            gen_set_inner,
        );
        if let Some(seeds_tokens) = &seeds_impl {
            output.extend(TokenStream::from(seeds_tokens.clone()));
        }
        return output;
    }

    if unsafe_no_disc && has_dynamic {
        return syn::Error::new_spanned(
            name,
            "unsafe_no_disc accounts cannot have dynamic fields (String/Vec/tail)",
        )
        .to_compile_error()
        .into();
    }

    if !has_dynamic {
        let mut output = fixed::generate_fixed_account(
            name,
            &disc_bytes,
            disc_len,
            &disc_indices,
            fields_data,
            &input,
            gen_set_inner,
        );
        if let Some(seeds_tokens) = &seeds_impl {
            output.extend(TokenStream::from(seeds_tokens.clone()));
        }
        return output;
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
            .expect("tail field must exist when tail_count == 1");
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

    let mut output = dynamic::generate_dynamic_account(
        name,
        &disc_bytes,
        disc_len,
        &disc_indices,
        fields_data,
        &field_kinds,
        &input,
        gen_set_inner,
    );
    if let Some(seeds_tokens) = &seeds_impl {
        output.extend(TokenStream::from(seeds_tokens.clone()));
    }
    output
}
