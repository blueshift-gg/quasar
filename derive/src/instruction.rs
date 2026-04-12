//! `#[instruction]` — generates instruction handler wrappers with context
//! deserialization, discriminator matching, and Borsh argument decoding.

use {
    crate::helpers::{
        classify_pod_string, classify_pod_vec, extract_generic_inner_type, is_unit_type,
        InstructionArgs, PodDynField,
    },
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, FnArg, Ident, ItemFn, Pat, ReturnType},
};

pub(crate) fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let mut func = parse_macro_input!(item as ItemFn);
    let disc_bytes = match &args.discriminator {
        Some(d) => d,
        None => {
            return syn::Error::new_spanned(
                &func.sig.ident,
                "#[instruction] requires `discriminator = [...]`",
            )
            .to_compile_error()
            .into();
        }
    };
    let disc_len = disc_bytes.len();

    // Reject multi-byte all-zero discriminators — zeroed instruction data could
    // accidentally match. Single-byte discriminators are fine (the dispatch
    // macro's length check rejects empty instruction data).
    if disc_len > 1
        && disc_bytes
            .iter()
            .all(|lit| matches!(lit.base10_parse::<u8>(), Ok(0)))
    {
        return syn::Error::new_spanned(
            &disc_bytes[0],
            "instruction discriminator must contain at least one non-zero byte; all-zero \
             multi-byte discriminators are dangerous because zeroed instruction data would match",
        )
        .to_compile_error()
        .into();
    }

    let first_arg = match func.sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt.clone(),
        _ => {
            return syn::Error::new_spanned(
                &func.sig.ident,
                "#[instruction] requires ctx: Ctx<T> as first parameter",
            )
            .to_compile_error()
            .into();
        }
    };

    let param_name = &first_arg.pat;
    let param_ident = match &*first_arg.pat {
        Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        _ => {
            return syn::Error::new_spanned(
                &first_arg.pat,
                "#[instruction] ctx parameter must be an identifier",
            )
            .to_compile_error()
            .into();
        }
    };
    let param_type = &first_arg.ty;

    let return_ok_type = match &func.sig.output {
        ReturnType::Type(_, ty) => extract_generic_inner_type(ty, "Result").cloned(),
        _ => None,
    };
    let has_return_data = return_ok_type
        .as_ref()
        .is_some_and(|ok_ty| !is_unit_type(ok_ty));

    if has_return_data {
        func.sig.output = syn::parse_quote!(-> Result<(), ProgramError>);
    }

    let remaining: Vec<_> = func
        .sig
        .inputs
        .iter()
        .skip(1)
        .filter_map(|arg| match arg {
            FnArg::Typed(pt) => Some(pt.clone()),
            _ => None,
        })
        .collect();

    func.sig.inputs = syn::punctuated::Punctuated::new();
    func.sig
        .inputs
        .push(syn::parse_quote!(mut context: Context));

    let stmts = std::mem::take(&mut func.block.stmts);
    let mut new_stmts: Vec<syn::Stmt> = vec![
        // Skip past the discriminator prefix. The dispatch! macro in the
        // entrypoint already verified the discriminator matches via a
        // fixed-size array comparison, so no need to re-check here.
        syn::parse_quote!(
            context.data = &context.data[#disc_len..];
        ),
        syn::parse_quote!(
            let mut #param_name: #param_type = <#param_type>::new(context)?;
        ),
        // Call validate() only when the user overrides it. The const bool
        // is known at compile time so this branch is fully elided when false,
        // avoiding a dead Result branch that sBPF doesn't optimize away.
        syn::parse_quote!(
            if #param_ident.has_validate() {
                #param_ident.accounts.validate()?;
            }
        ),
    ];

    if !remaining.is_empty() {
        let mut field_names: Vec<Ident> = Vec::with_capacity(remaining.len());
        for pt in &remaining {
            match &*pt.pat {
                Pat::Ident(pat_ident) => field_names.push(pat_ident.ident.clone()),
                _ => {
                    return syn::Error::new_spanned(
                        &pt.pat,
                        "#[instruction] parameters must be simple identifiers",
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }

        // Classify each arg as Pod-dynamic or fixed
        let mut pod_dyns: Vec<Option<PodDynField>> = Vec::with_capacity(remaining.len());
        for pt in &remaining {
            let pd = if let Some(max) = classify_pod_string(&pt.ty) {
                Some(PodDynField::Str { max })
            } else if let Some((elem, max)) = classify_pod_vec(&pt.ty) {
                Some(PodDynField::Vec {
                    elem: Box::new(elem),
                    max,
                })
            } else {
                None
            };
            pod_dyns.push(pd);
        }

        let has_dynamic = pod_dyns.iter().any(|pd| pd.is_some());
        let has_fixed = pod_dyns.iter().any(|pd| pd.is_none());

        // Build ZC struct with ONLY fixed fields, using InstructionArg::Zc
        // for each field type to guarantee alignment 1.
        let mut zc_field_names: Vec<Ident> = Vec::new();
        let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut zc_field_orig_types: Vec<syn::Type> = Vec::new();

        for (i, pd) in pod_dyns.iter().enumerate() {
            if pd.is_none() {
                zc_field_names.push(field_names[i].clone());
                let ty = &remaining[i].ty;
                zc_field_types
                    .push(quote! { <#ty as quasar_lang::instruction_arg::InstructionArg>::Zc });
                zc_field_orig_types.push((*remaining[i].ty).clone());
            }
        }

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

        for assert_stmt in vec_align_asserts {
            new_stmts.push(
                syn::parse2(assert_stmt)
                    .expect("failed to parse generated Vec alignment assert statement"),
            );
        }

        if has_fixed {
            new_stmts.push(syn::parse_quote!(
                #[repr(C)]
                struct InstructionDataZc {
                    #(#zc_field_names: #zc_field_types,)*
                }
            ));

            new_stmts.push(syn::parse_quote!(
                const _: () = assert!(
                    core::mem::align_of::<InstructionDataZc>() == 1,
                    "instruction data ZC struct must have alignment 1 — all instruction arg types \
                     must implement InstructionArg with an alignment-1 Zc companion"
                );
            ));

            new_stmts.push(syn::parse_quote!(
                if #param_ident.data.len() < core::mem::size_of::<InstructionDataZc>() {
                    return Err(ProgramError::InvalidInstructionData);
                }
            ));

            new_stmts.push(syn::parse_quote!(
                let __zc = unsafe { &*(#param_ident.data.as_ptr() as *const InstructionDataZc) };
            ));

            // Validate and extract fixed fields via InstructionArg
            {
                let mut zc_idx = 0usize;
                for (i, pd) in pod_dyns.iter().enumerate() {
                    if pd.is_none() {
                        let name = &field_names[i];
                        let ty = &zc_field_orig_types[zc_idx];
                        zc_idx += 1;
                        new_stmts.push(syn::parse_quote!(
                            <#ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__zc.#name)?;
                        ));
                        new_stmts.push(syn::parse_quote!(
                            let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__zc.#name);
                        ));
                    }
                }
            }
        }

        // Extract dynamic fields with inline prefix reads
        // String<N> → u8 prefix (1 byte), Vec<T, N> → u16 prefix (2 bytes)
        if has_dynamic {
            new_stmts.push(syn::parse_quote!(
                let __data = #param_ident.data;
            ));
            if has_fixed {
                new_stmts.push(syn::parse_quote!(
                    let mut __offset = core::mem::size_of::<InstructionDataZc>();
                ));
            } else {
                new_stmts.push(syn::parse_quote!(
                    let mut __offset: usize = 0;
                ));
            }

            let dyn_count = pod_dyns.iter().filter(|pd| pd.is_some()).count();
            let mut dyn_idx = 0usize;

            for (i, pd) in pod_dyns.iter().enumerate() {
                let name = &field_names[i];
                let (read_call, max_lit) = match pd {
                    None => continue,
                    Some(PodDynField::Str { max }) => (
                        quote!(quasar_lang::instruction_data::read_dynamic_str::<1>),
                        *max,
                    ),
                    Some(PodDynField::Vec { elem, max }) => (
                        quote!(quasar_lang::instruction_data::read_dynamic_vec::<#elem, 2>),
                        *max,
                    ),
                };
                dyn_idx += 1;
                if dyn_idx < dyn_count {
                    new_stmts.push(syn::parse_quote!(
                        let (#name, __new_offset) = #read_call(__data, __offset, #max_lit)?;
                    ));
                    new_stmts.push(syn::parse_quote!(
                        __offset = __new_offset;
                    ));
                } else {
                    new_stmts.push(syn::parse_quote!(
                        let (#name, _) = #read_call(__data, __offset, #max_lit)?;
                    ));
                }
            }

            new_stmts.push(syn::parse_quote!(
                let _ = __offset;
            ));
        }

        // Clear ctx.data after extraction
        new_stmts.push(syn::parse_quote!(
            #param_ident.data = &[];
        ));
    }

    if has_return_data {
        let ok_ty =
            return_ok_type.expect("return_ok_type must be set when has_return_data is true");
        let user_body: proc_macro2::TokenStream = stmts.iter().map(|s| quote!(#s)).collect();
        new_stmts.push(syn::parse_quote!(
            const _: () = assert!(
                core::mem::align_of::<<#ok_ty as quasar_lang::instruction_arg::InstructionArg>::Zc>() == 1,
                "return data type must implement InstructionArg with an alignment-1 Zc companion"
            );
        ));
        new_stmts.push(syn::parse_quote!(
            {
                let __result: Result<#ok_ty, ProgramError> = (|| { #user_body })();
                match __result {
                    Ok(ref __val) => {
                        #param_ident.accounts.epilogue()?;
                        let __zc =
                            <#ok_ty as quasar_lang::instruction_arg::InstructionArg>::to_zc(__val);
                        let __bytes = unsafe {
                            core::slice::from_raw_parts(
                                &__zc as *const <#ok_ty as quasar_lang::instruction_arg::InstructionArg>::Zc as *const u8,
                                core::mem::size_of::<<#ok_ty as quasar_lang::instruction_arg::InstructionArg>::Zc>(),
                            )
                        };
                        quasar_lang::return_data::set_return_data(__bytes);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
        ));
        func.block.stmts = new_stmts;
    } else {
        let user_body: proc_macro2::TokenStream = stmts.iter().map(|s| quote!(#s)).collect();
        new_stmts.push(syn::parse_quote!({
            let __user_result: Result<(), ProgramError> = { #user_body };
            __user_result?;
            #param_ident.accounts.epilogue()
        }));
        func.block.stmts = new_stmts;
    }

    quote!(#func).into()
}
