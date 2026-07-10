//! Struct-level `#[instruction(..)]` argument extraction codegen.
//!
//! The extraction used to be spliced verbatim into three sites (both
//! `parse_*_unchecked` bodies and the `{field}_signer` helper). It is now
//! emitted ONCE as a generated `#[inline(always)] fn __extract_ix_args` on the
//! accounts struct; each site calls it and destructures the returned args. The
//! helper is `#[inline(always)]`, so the generated SBF (and CU) is unchanged;
//! only the front-end token volume shrinks.
//!
//! The declared arg list must match the handler's parameters through the last
//! dynamic arg: both decoders read the same buffer, so a divergent prefix would
//! desync every arg after it.
//!
//! When every declared arg is fixed, they are read via a zero-copy `#[repr(C)]`
//! struct pointer cast. When any arg is dynamic (`String<N>` / `Vec<T, N>`),
//! the whole list is decoded through the canonical zeropod **compact** layout —
//! inline fixed fields, then all tail length-prefixes, then all tail payloads —
//! which is exactly what the handler macro (`derive/src/instruction.rs`) and
//! the generated client emit.

use {
    crate::{
        accounts::InstructionArg,
        helpers::{classify_pod_dynamic, PodDynField},
    },
    quote::quote,
    syn::{Ident, Type},
};

/// Emit the single `#[inline(always)] fn __extract_ix_args` definition placed
/// on the accounts struct's inherent impl. Empty when there are no ix args.
pub(crate) fn emit_extract_ix_args_fn(ix_args: &[InstructionArg]) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    if ix_args.is_empty() {
        return quote! {};
    }

    let pod_dyns: Vec<Option<PodDynField>> = ix_args
        .iter()
        .map(|arg| classify_pod_dynamic(&arg.ty))
        .collect();

    let ret_types: Vec<proc_macro2::TokenStream> = ix_args
        .iter()
        .zip(pod_dyns.iter())
        .map(|(arg, pd)| arg_return_type(arg, pd))
        .collect();
    let names: Vec<&Ident> = ix_args.iter().map(|arg| &arg.name).collect();

    let body = extract_body(ix_args, &pod_dyns);

    quote! {
        #[inline(always)]
        #[allow(unused_variables)]
        fn __extract_ix_args<'a>(
            __ix_data: &'a [u8],
        ) -> Result<(#(#ret_types),*), #krate::__solana_program_error::ProgramError> {
            #body
            Ok((#(#names),*))
        }
    }
}

/// Emit the call that destructures the extracted args at a splice site. Empty
/// when there are no ix args (so the site emits nothing, as before).
pub(crate) fn emit_extract_ix_args_call(ix_args: &[InstructionArg]) -> proc_macro2::TokenStream {
    if ix_args.is_empty() {
        return quote! {};
    }
    let names: Vec<&Ident> = ix_args.iter().map(|arg| &arg.name).collect();
    quote! {
        let (#(#names),*) = Self::__extract_ix_args(__ix_data)?;
    }
}

/// The decoded return type of one arg: fixed args decode to their declared
/// type; dynamic args to the zero-copy view returned by the compact ref
/// accessor (`&'a str` / `&'a [<pod elem>]`).
fn arg_return_type(arg: &InstructionArg, pd: &Option<PodDynField>) -> proc_macro2::TokenStream {
    match pd {
        None => {
            let ty = &arg.ty;
            quote! { #ty }
        }
        Some(PodDynField::Str { .. }) => quote! { &'a str },
        Some(PodDynField::Vec { elem, .. }) => {
            let pod_elem = pod_elem_type(elem);
            quote! { &'a [#pod_elem] }
        }
    }
}

/// The pod element type the compact `Vec` accessor yields, mirroring zeropod's
/// `map_to_pod_type`: align-1 primitives map to their `Pod*` companions, and
/// every other element delegates through `ZcField::Pod`.
fn pod_elem_type(elem: &Type) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    if let Type::Path(tp) = elem {
        if let Some(seg) = tp.path.segments.last() {
            if seg.arguments.is_none() {
                let mapped = match seg.ident.to_string().as_str() {
                    "u8" => Some(quote! { u8 }),
                    "i8" => Some(quote! { i8 }),
                    "u16" => Some(quote! { #krate::__zeropod::pod::PodU16 }),
                    "u32" => Some(quote! { #krate::__zeropod::pod::PodU32 }),
                    "u64" => Some(quote! { #krate::__zeropod::pod::PodU64 }),
                    "u128" => Some(quote! { #krate::__zeropod::pod::PodU128 }),
                    "i16" => Some(quote! { #krate::__zeropod::pod::PodI16 }),
                    "i32" => Some(quote! { #krate::__zeropod::pod::PodI32 }),
                    "i64" => Some(quote! { #krate::__zeropod::pod::PodI64 }),
                    "i128" => Some(quote! { #krate::__zeropod::pod::PodI128 }),
                    "bool" => Some(quote! { #krate::__zeropod::pod::PodBool }),
                    _ => None,
                };
                if let Some(mapped) = mapped {
                    return mapped;
                }
            }
        }
    }
    quote! { <#elem as #krate::__zeropod::ZcField>::Pod }
}

/// The extraction statements that bind every declared arg from `__ix_data`.
/// Ends with the args in scope for the trailing `Ok((..))`.
fn extract_body(
    ix_args: &[InstructionArg],
    pod_dyns: &[Option<PodDynField>],
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let has_dynamic = pod_dyns.iter().any(|pd| pd.is_some());

    let vec_align_asserts: Vec<proc_macro2::TokenStream> = pod_dyns
        .iter()
        .filter_map(|pd| match pd {
            Some(PodDynField::Vec { elem, .. }) => Some(quote! {
                const _: () = assert!(
                    core::mem::align_of::<#elem>() == 1,
                    "instruction Vec element type must have alignment 1"
                );
            }),
            _ => None,
        })
        .collect();

    let mut stmts: Vec<proc_macro2::TokenStream> = vec_align_asserts;

    if !has_dynamic {
        // Pure-fixed path: read every arg from a `#[repr(C)]` ZC struct cast.
        let zc_field_names: Vec<Ident> = ix_args.iter().map(|arg| arg.name.clone()).collect();
        let zc_field_types: Vec<proc_macro2::TokenStream> = ix_args
            .iter()
            .map(|arg| {
                let ty = &arg.ty;
                quote! { <#ty as #krate::instruction_arg::InstructionArg>::Zc }
            })
            .collect();

        stmts.push(quote! {
            #[repr(C)]
            struct __IxArgsZc {
                #(#zc_field_names: #zc_field_types,)*
            }
        });

        stmts.push(quote! {
            const _: () = assert!(
                core::mem::align_of::<__IxArgsZc>() == 1,
                "instruction args ZC struct must have alignment 1"
            );
        });

        stmts.push(quote! {
            if __ix_data.len() < core::mem::size_of::<__IxArgsZc>() {
                return Err(#krate::__solana_program_error::ProgramError::InvalidInstructionData);
            }
        });

        stmts.push(quote! {
            // SAFETY: `__IxArgsZc` has alignment 1 and the preceding length check
            // guarantees the fixed ZC block is present.
            let __ix_zc = unsafe { &*(__ix_data.as_ptr() as *const __IxArgsZc) };
        });

        for arg in ix_args {
            let name = &arg.name;
            let ty = &arg.ty;
            stmts.push(quote! {
                <#ty as #krate::instruction_arg::InstructionArg>::validate_zc(&__ix_zc.#name)?;
                let #name = <#ty as #krate::instruction_arg::InstructionArg>::from_zc(&__ix_zc.#name);
            });
        }

        return quote! { #(#stmts)* };
    }

    // Mixed/dynamic path: decode via the canonical zeropod compact layout, the
    // same schema the handler macro and the generated client use. Fixed args
    // stay inline; dynamic args (String<N>/Vec<T, N>) become compact fields
    // whose length prefixes group ahead of their payloads. `validate` enforces
    // the `#[max]` bounds, so no separate bound checks are emitted here.
    //
    // Alias quasar_lang's re-export so `zeropod::*` paths emitted by the ZeroPod
    // derive resolve without a direct crate dependency.
    stmts.push(quote! {
        use #krate::__zeropod as zeropod;
    });

    // Build the compact schema IR (raw `PodVec` element, matching the `&[elem]`
    // accessor these args decode to) and emit via the single-source emitters.
    let schema_fields: Vec<crate::schema_ir::SchemaField> = ix_args
        .iter()
        .zip(pod_dyns.iter())
        .map(|(arg, pd)| {
            let class = match pd {
                Some(pd) => crate::schema_ir::LayoutClass::from_pod_dyn(pd, |e| quote!(#e)),
                None => {
                    let ty = &arg.ty;
                    crate::schema_ir::LayoutClass::Fixed { ty: quote!(#ty) }
                }
            };
            crate::schema_ir::SchemaField::private(arg.name.clone(), class)
        })
        .collect();
    let ir = match crate::schema_ir::SchemaIR::new(schema_fields) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error(),
    };
    let schema_name: Ident = syn::parse_quote!(__IxArgsCompact);
    let ref_name: Ident = syn::parse_quote!(__IxArgsCompactRef);

    stmts.push(crate::schema_ir::emit_compact_schema(
        &schema_name,
        &ir,
        &syn::Visibility::Inherited,
    ));
    let decode = crate::schema_ir::emit_compact_decode(
        &ir,
        &crate::schema_ir::DecodeOpts {
            schema_name,
            ref_name,
            data: quote!(__ix_data),
            err: quote!(#krate::__solana_program_error::ProgramError::InvalidInstructionData),
            validate_fixed: true,
        },
    );
    stmts.push(quote! { #(#decode)* });

    quote! { #(#stmts)* }
}
