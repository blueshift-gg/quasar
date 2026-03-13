//! `#[instruction]` — generates instruction handler wrappers with context
//! deserialization, discriminator matching, and Borsh argument decoding.

use {
    crate::helpers::{
        classify_dynamic_string, classify_dynamic_vec, classify_tail, extract_generic_inner_type,
        is_unit_type, map_to_pod_type, zc_deserialize_expr, DynKind, InstructionArgs, TailElement,
    },
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, FnArg, Ident, ItemFn, Pat, ReturnType},
};

pub(crate) fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let mut func = parse_macro_input!(item as ItemFn);
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

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
        syn::parse_quote!(
            if !context.data.starts_with(&[#(#disc_bytes),*]) {
                return Err(ProgramError::InvalidInstructionData);
            }
        ),
        syn::parse_quote!(
            context.data = &context.data[#disc_len..];
        ),
        syn::parse_quote!(
            let mut #param_name: #param_type = <#param_type>::new(context)?;
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

        let kinds: Vec<DynKind> = remaining
            .iter()
            .map(|pt| {
                if let Some((prefix, max)) = classify_dynamic_string(&pt.ty) {
                    DynKind::Str { prefix, max }
                } else if let Some(tail_elem) = classify_tail(&pt.ty) {
                    DynKind::Tail { element: tail_elem }
                } else if let Some((elem, prefix, max)) = classify_dynamic_vec(&pt.ty) {
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

        let has_dynamic = kinds.iter().any(|k| !matches!(k, DynKind::Fixed));
        let has_fixed = kinds.iter().any(|k| matches!(k, DynKind::Fixed));

        // Build ZC struct with ONLY fixed fields
        let mut zc_field_names: Vec<Ident> = Vec::new();
        let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();

        for (i, kind) in kinds.iter().enumerate() {
            if matches!(kind, DynKind::Fixed) {
                zc_field_names.push(field_names[i].clone());
                zc_field_types.push(map_to_pod_type(&remaining[i].ty));
            }
        }

        let vec_align_asserts: Vec<proc_macro2::TokenStream> = kinds
            .iter()
            .filter_map(|kind| match kind {
                DynKind::Vec { elem, .. } => Some(quote! {
                    const _: () = assert!(
                        core::mem::align_of::<#elem>() == 1,
                        "instruction Vec element type must have alignment 1"
                    );
                }),
                _ => None,
            })
            .collect();

        for assert_stmt in vec_align_asserts {
            new_stmts.push(syn::parse2(assert_stmt).unwrap());
        }

        if has_fixed {
            new_stmts.push(syn::parse_quote!(
                #[repr(C)]
                #[derive(Copy, Clone)]
                struct InstructionDataZc {
                    #(#zc_field_names: #zc_field_types,)*
                }
            ));

            new_stmts.push(syn::parse_quote!(
                const _: () = assert!(
                    core::mem::align_of::<InstructionDataZc>() == 1,
                    "instruction data ZC struct must have alignment 1"
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

            // Extract fixed fields from ZC header
            for (i, kind) in kinds.iter().enumerate() {
                if matches!(kind, DynKind::Fixed) {
                    let name = &field_names[i];
                    let expr = zc_deserialize_expr(name, &remaining[i].ty);
                    new_stmts.push(syn::parse_quote!(
                        let #name = #expr;
                    ));
                }
            }
        }

        // Extract dynamic fields with inline prefix reads
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

            let dyn_count = kinds
                .iter()
                .filter(|k| !matches!(k, DynKind::Fixed))
                .count();
            let mut dyn_idx = 0usize;

            for (i, kind) in kinds.iter().enumerate() {
                let name = &field_names[i];
                match kind {
                    DynKind::Fixed => {}
                    DynKind::Str { prefix, max } => {
                        dyn_idx += 1;
                        let pb = prefix.bytes();
                        let max_lit = *max;
                        let read_len = prefix.gen_read_len();
                        new_stmts.push(syn::parse_quote!(
                            if __data.len() < __offset + #pb {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(
                            let __dyn_len = #read_len;
                        ));
                        new_stmts.push(syn::parse_quote!(
                            __offset += #pb;
                        ));
                        new_stmts.push(syn::parse_quote!(
                            if __dyn_len > #max_lit {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(if __data.len() < __offset + __dyn_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }));
                        new_stmts.push(syn::parse_quote!(
                            let #name: &str = {
                                let __bytes = &__data[__offset..__offset + __dyn_len];
                                match core::str::from_utf8(__bytes) {
                                    Ok(__s) => __s,
                                    Err(_) => return Err(ProgramError::InvalidInstructionData),
                                }
                            };
                        ));
                        if dyn_idx < dyn_count {
                            new_stmts.push(syn::parse_quote!(
                                __offset += __dyn_len;
                            ));
                        }
                    }
                    DynKind::Tail { element } => {
                        dyn_idx += 1;
                        // Tail field: remaining data, no prefix
                        match element {
                            TailElement::Str => {
                                new_stmts.push(syn::parse_quote!(
                                    let #name: &str = {
                                        let __bytes = &__data[__offset..];
                                        match core::str::from_utf8(__bytes) {
                                            Ok(__s) => __s,
                                            Err(_) => return Err(ProgramError::InvalidInstructionData),
                                        }
                                    };
                                ));
                            }
                            TailElement::Bytes => {
                                new_stmts.push(syn::parse_quote!(
                                    let #name: &[u8] = &__data[__offset..];
                                ));
                            }
                        }
                        // Tail consumes all remaining data — no offset advance
                        // needed
                    }
                    DynKind::Vec { elem, prefix, max } => {
                        dyn_idx += 1;
                        let pb = prefix.bytes();
                        let max_lit = *max;
                        let read_len = prefix.gen_read_len();
                        new_stmts.push(syn::parse_quote!(
                            if __data.len() < __offset + #pb {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(
                            let __dyn_count = #read_len;
                        ));
                        new_stmts.push(syn::parse_quote!(
                            __offset += #pb;
                        ));
                        new_stmts.push(syn::parse_quote!(
                            if __dyn_count > #max_lit {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(
                            let __dyn_byte_len = __dyn_count * core::mem::size_of::<#elem>();
                        ));
                        new_stmts.push(syn::parse_quote!(
                            if __data.len() < __offset + __dyn_byte_len {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(
                            let #name: &[#elem] = unsafe {
                                core::slice::from_raw_parts(
                                    __data.as_ptr().add(__offset) as *const #elem,
                                    __dyn_count,
                                )
                            };
                        ));
                        if dyn_idx < dyn_count {
                            new_stmts.push(syn::parse_quote!(
                                __offset += __dyn_byte_len;
                            ));
                        }
                    }
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
        let ok_ty = return_ok_type.unwrap();
        let user_body: proc_macro2::TokenStream = stmts.iter().map(|s| quote!(#s)).collect();
        new_stmts.push(syn::parse_quote!(
            const _: () = assert!(
                core::mem::align_of::<#ok_ty>() == 1,
                "return data type must have alignment 1 (use Pod types)"
            );
        ));
        new_stmts.push(syn::parse_quote!(
            {
                let __result: Result<#ok_ty, ProgramError> = (|| { #user_body })();
                match __result {
                    Ok(ref __val) => {
                        #param_ident.accounts.epilogue()?;
                        let __bytes = unsafe {
                            core::slice::from_raw_parts(
                                __val as *const #ok_ty as *const u8,
                                core::mem::size_of::<#ok_ty>(),
                            )
                        };
                        quasar_core::return_data::set_return_data(__bytes);
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
