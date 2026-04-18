//! `#[account]` — generates the zero-copy companion struct, discriminator
//! validation, `Owner`/`Discriminator`/`Space` trait impls, and typed accessor
//! methods for on-chain account types.

mod dynamic;
mod fixed;
mod layout;
mod methods;
pub mod seeds;
mod traits;

use {
    crate::helpers::{classify_pod_dynamic, validate_discriminator_not_zero, AccountAttr},
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

    // --- Classify fields: String<N>/PodString<N> -> PodDynField::Str,
    //     Vec<T,N>/PodVec<T,N> -> PodDynField::Vec, everything else -> fixed ---
    // When `fixed_capacity` is set, ALL fields are treated as fixed — PodVec and
    // PodString go directly into the ZC struct at full capacity. No dynamic
    // region, no CompactWriter, no walk-from-header.
    let pod_field_infos: Vec<fixed::PodFieldInfo<'_>> = fields_data
        .iter()
        .map(|f| {
            let pod_dyn = if args.fixed_capacity {
                None // fixed_capacity: everything goes in the ZC struct
            } else {
                classify_pod_dynamic(&f.ty)
            };
            fixed::PodFieldInfo { field: f, pod_dyn }
        })
        .collect();

    let has_pod_dynamic = pod_field_infos.iter().any(|fi| fi.pod_dyn.is_some());

    if has_pod_dynamic {
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
    }

    let mut output = fixed::generate_account(
        name,
        &disc_bytes,
        disc_len,
        &disc_indices,
        &pod_field_infos,
        &input,
        gen_set_inner,
    );
    if let Some(seeds_tokens) = &seeds_impl {
        output.extend(TokenStream::from(seeds_tokens.clone()));
    }
    output
}
