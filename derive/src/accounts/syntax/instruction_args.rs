use {
    crate::helpers::{classify_pod_dynamic, PodDynField},
    quote::quote,
    syn::{parse::ParseStream, DeriveInput, Ident, Token, Type},
};

pub(crate) struct InstructionArg {
    pub name: Ident,
    pub ty: Type,
}

pub(crate) fn parse_struct_instruction_args(
    input: &DeriveInput,
) -> syn::Result<Option<Vec<InstructionArg>>> {
    let attr = match input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("instruction"))
    {
        Some(attr) => attr,
        None => return Ok(None),
    };

    let args = attr.parse_args_with(|stream: ParseStream| {
        let mut args = Vec::new();
        while !stream.is_empty() {
            let name: Ident = stream.parse()?;
            let _: Token![:] = stream.parse()?;
            let ty: Type = stream.parse()?;
            args.push(InstructionArg { name, ty });
            if !stream.is_empty() {
                let _: Token![,] = stream.parse()?;
            }
        }
        Ok(args)
    })?;

    Ok(Some(args))
}

/// Generate code that extracts `#[instruction(..)]` args from `__ix_data`.
///
/// Fixed types are read via a zero-copy `#[repr(C)]` struct pointer cast.
/// Dynamic fields (String<N>/Vec<T,N>) use inline prefix reads from the data
/// buffer after the fixed ZC block. String uses u8 prefix, Vec uses u16 prefix.
pub(crate) fn generate_instruction_arg_extraction(
    ix_args: &[InstructionArg],
) -> proc_macro2::TokenStream {
    if ix_args.is_empty() {
        return quote! {};
    }

    let mut pod_dyns: Vec<Option<PodDynField>> = Vec::with_capacity(ix_args.len());
    for arg in ix_args {
        pod_dyns.push(classify_pod_dynamic(&arg.ty));
    }

    let has_dynamic = pod_dyns.iter().any(|pd| pd.is_some());
    let has_fixed = pod_dyns.iter().any(|pd| pd.is_none());

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

    if has_fixed {
        let mut zc_field_names: Vec<Ident> = Vec::new();
        let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut zc_field_orig_types: Vec<Type> = Vec::new();

        for (i, pd) in pod_dyns.iter().enumerate() {
            if pd.is_none() {
                zc_field_names.push(ix_args[i].name.clone());
                let ty = &ix_args[i].ty;
                zc_field_types
                    .push(quote! { <#ty as quasar_lang::instruction_arg::InstructionArg>::Zc });
                zc_field_orig_types.push(ix_args[i].ty.clone());
            }
        }

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
                return Err(ProgramError::InvalidInstructionData);
            }
        });

        stmts.push(quote! {
            let __ix_zc = unsafe { &*(__ix_data.as_ptr() as *const __IxArgsZc) };
        });

        let mut zc_idx = 0usize;
        for (i, pd) in pod_dyns.iter().enumerate() {
            if pd.is_none() {
                let name = &ix_args[i].name;
                let ty = &zc_field_orig_types[zc_idx];
                zc_idx += 1;
                stmts.push(quote! {
                    <#ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__ix_zc.#name)?;
                    let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ix_zc.#name);
                });
            }
        }
    }

    if has_dynamic {
        stmts.push(quote! { let __data = __ix_data; });
        if has_fixed {
            stmts.push(quote! {
                let mut __offset = core::mem::size_of::<__IxArgsZc>();
            });
        } else {
            stmts.push(quote! {
                let mut __offset: usize = 0;
            });
        }

        let dyn_count = pod_dyns.iter().filter(|pd| pd.is_some()).count();
        let mut dyn_idx = 0usize;

        for (i, pd) in pod_dyns.iter().enumerate() {
            let name = &ix_args[i].name;
            match pd {
                None => {}
                Some(PodDynField::Str { max, prefix_bytes }) => {
                    dyn_idx += 1;
                    let max_lit = *max;
                    let pfx = *prefix_bytes;
                    stmts.push(quote! {
                        if __data.len() < __offset + #pfx {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_len = {
                            let mut __buf = [0u8; 8];
                            __buf[..#pfx].copy_from_slice(&__data[__offset..__offset + #pfx]);
                            u64::from_le_bytes(__buf) as usize
                        };
                    });
                    stmts.push(quote! {
                        __offset += #pfx;
                    });
                    stmts.push(quote! {
                        if __ix_dyn_len > #max_lit {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        if __data.len() < __offset + __ix_dyn_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let #name: &[u8] = &__data[__offset..__offset + __ix_dyn_len];
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __offset += __ix_dyn_len;
                        });
                    }
                }
                Some(PodDynField::Vec {
                    elem,
                    max,
                    prefix_bytes,
                }) => {
                    dyn_idx += 1;
                    let max_lit = *max;
                    let pfx = *prefix_bytes;
                    stmts.push(quote! {
                        if __data.len() < __offset + #pfx {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_count = {
                            let mut __buf = [0u8; 8];
                            __buf[..#pfx].copy_from_slice(&__data[__offset..__offset + #pfx]);
                            u64::from_le_bytes(__buf) as usize
                        };
                    });
                    stmts.push(quote! {
                        __offset += #pfx;
                    });
                    stmts.push(quote! {
                        if __ix_dyn_count > #max_lit {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_byte_len = __ix_dyn_count
                            .checked_mul(core::mem::size_of::<#elem>())
                            .ok_or(ProgramError::InvalidInstructionData)?;
                    });
                    stmts.push(quote! {
                        if __data.len() < __offset + __ix_dyn_byte_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let #name: &[#elem] = unsafe {
                            core::slice::from_raw_parts(
                                __data.as_ptr().add(__offset) as *const #elem,
                                __ix_dyn_count,
                            )
                        };
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __offset += __ix_dyn_byte_len;
                        });
                    }
                }
            }
        }

        stmts.push(quote! {
            let _ = __offset;
        });
    }

    quote! { #(#stmts)* }
}
