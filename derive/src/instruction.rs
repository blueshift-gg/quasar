use {
    crate::helpers::{
        is_ix_dynamic_string, is_ix_dynamic_vec, map_to_pod_type, zc_deserialize_expr,
        InstructionArgs,
    },
    proc_macro::TokenStream,
    quote::quote,
    syn::{
        parse_macro_input, FnArg, GenericArgument, Ident, ItemFn, Pat, PathArguments, ReturnType,
        Type,
    },
};

fn extract_result_ok_type(output: &ReturnType) -> Option<&Type> {
    if let ReturnType::Type(_, ty) = output {
        if let Type::Path(type_path) = ty.as_ref() {
            if let Some(last) = type_path.path.segments.last() {
                if last.ident == "Result" {
                    if let PathArguments::AngleBracketed(args) = &last.arguments {
                        if let Some(GenericArgument::Type(ok_ty)) = args.args.first() {
                            return Some(ok_ty);
                        }
                    }
                }
            }
        }
    }
    None
}

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(t) if t.elems.is_empty())
}

pub(crate) fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let mut func = parse_macro_input!(item as ItemFn);
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

    let first_arg = match func.sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt.clone(),
        _ => panic!("#[instruction] requires ctx: Ctx<T> as first parameter"),
    };

    let param_name = &first_arg.pat;
    let param_ident = match &*first_arg.pat {
        Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        _ => panic!("#[instruction] ctx parameter must be an identifier"),
    };
    let param_type = &first_arg.ty;

    let has_return_data =
        extract_result_ok_type(&func.sig.output).is_some_and(|ok_ty| !is_unit_type(ok_ty));
    let return_ok_type = extract_result_ok_type(&func.sig.output).cloned();

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
        let field_names: Vec<Ident> = remaining
            .iter()
            .map(|pt| match &*pt.pat {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => panic!("#[instruction] parameters must be simple identifiers"),
            })
            .collect();

        enum IxDynKind {
            Fixed,
            Str { max: usize },
            Vec { elem: Box<Type>, max: usize },
        }

        let kinds: Vec<IxDynKind> = remaining
            .iter()
            .map(|pt| {
                if let Some(max) = is_ix_dynamic_string(&pt.ty) {
                    IxDynKind::Str { max }
                } else if let Some((elem, max)) = is_ix_dynamic_vec(&pt.ty) {
                    IxDynKind::Vec {
                        elem: Box::new(elem),
                        max,
                    }
                } else {
                    IxDynKind::Fixed
                }
            })
            .collect();

        let has_dynamic = kinds.iter().any(|k| !matches!(k, IxDynKind::Fixed));

        // Build ZC struct: fixed fields as Pod types + PodU16 descriptors for dynamic fields
        let mut zc_field_names: Vec<Ident> = Vec::new();
        let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();

        for (i, kind) in kinds.iter().enumerate() {
            match kind {
                IxDynKind::Fixed => {
                    zc_field_names.push(field_names[i].clone());
                    zc_field_types.push(map_to_pod_type(&remaining[i].ty));
                }
                IxDynKind::Str { .. } => {
                    let len_name =
                        Ident::new(&format!("{}_len", field_names[i]), field_names[i].span());
                    zc_field_names.push(len_name);
                    zc_field_types.push(quote! { quasar_core::pod::PodU16 });
                }
                IxDynKind::Vec { .. } => {
                    let count_name =
                        Ident::new(&format!("{}_count", field_names[i]), field_names[i].span());
                    zc_field_names.push(count_name);
                    zc_field_types.push(quote! { quasar_core::pod::PodU16 });
                }
            }
        }

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
            if matches!(kind, IxDynKind::Fixed) {
                let name = &field_names[i];
                let expr = zc_deserialize_expr(name, &remaining[i].ty);
                new_stmts.push(syn::parse_quote!(
                    let #name = #expr;
                ));
            }
        }

        // Extract dynamic fields from variable tail
        if has_dynamic {
            new_stmts.push(syn::parse_quote!(
                let __tail = &#param_ident.data[core::mem::size_of::<InstructionDataZc>()..];
            ));
            new_stmts.push(syn::parse_quote!(
                let mut __offset: usize = 0;
            ));

            // Count dynamic fields to avoid unused offset update on last one
            let dyn_count = kinds
                .iter()
                .filter(|k| !matches!(k, IxDynKind::Fixed))
                .count();
            let mut dyn_idx = 0usize;

            for (i, kind) in kinds.iter().enumerate() {
                let name = &field_names[i];
                match kind {
                    IxDynKind::Fixed => {}
                    IxDynKind::Str { max } => {
                        dyn_idx += 1;
                        let len_name = Ident::new(&format!("{}_len", name), name.span());
                        let max_lit = *max;
                        new_stmts.push(syn::parse_quote!(
                            let __dyn_len = __zc.#len_name.get() as usize;
                        ));
                        new_stmts.push(syn::parse_quote!(
                            if __dyn_len > #max_lit {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(if __tail.len() < __offset + __dyn_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }));
                        new_stmts.push(syn::parse_quote!(
                            let #name: &str = unsafe {
                                core::str::from_utf8_unchecked(
                                    &__tail[__offset..__offset + __dyn_len]
                                )
                            };
                        ));
                        if dyn_idx < dyn_count {
                            new_stmts.push(syn::parse_quote!(
                                __offset += __dyn_len;
                            ));
                        }
                    }
                    IxDynKind::Vec { elem, max } => {
                        dyn_idx += 1;
                        let count_name = Ident::new(&format!("{}_count", name), name.span());
                        let max_lit = *max;
                        new_stmts.push(syn::parse_quote!(
                            let __dyn_count = __zc.#count_name.get() as usize;
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
                            if __tail.len() < __offset + __dyn_byte_len {
                                return Err(ProgramError::InvalidInstructionData);
                            }
                        ));
                        new_stmts.push(syn::parse_quote!(
                            let #name: &[#elem] = unsafe {
                                core::slice::from_raw_parts(
                                    __tail.as_ptr().add(__offset) as *const #elem,
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

            // Suppress unused warning on __offset after last dynamic field
            new_stmts.push(syn::parse_quote!(
                let _ = __offset;
            ));
        }

        // Clear ctx.data after extraction — prevents accidental access to raw bytes
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
        func.block.stmts = new_stmts.into_iter().chain(stmts).collect();
    }

    quote!(#func).into()
}
