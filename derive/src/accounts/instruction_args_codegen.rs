//! Codegen for instruction args extraction. Reads parsed `InstructionArg`
//! values from `quasar-syntax` and emits the extraction snippet inserted
//! into generated `parse` bodies.

use {
    crate::helpers::{classify_pod_dynamic, PodDynField},
    quasar_syntax::accounts::InstructionArg,
    quote::quote,
    syn::{Ident, Type},
};

pub(crate) fn generate_instruction_arg_extraction(
    ix_args: &[InstructionArg],
) -> proc_macro2::TokenStream {
    if ix_args.is_empty() {
        return quote! {};
    }

    let pod_dyns: Vec<Option<PodDynField>> = ix_args
        .iter()
        .map(|arg| classify_pod_dynamic(&arg.ty))
        .collect();

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
            // SAFETY: `__IxArgsZc` has alignment 1 and the preceding length check
            // guarantees the fixed ZC block is present.
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
                    let pfx = *prefix_bytes;
                    stmts.push(quote! {
                        let __ix_dyn_prefix_end = __offset
                            .checked_add(#pfx)
                            .ok_or(ProgramError::InvalidInstructionData)?;
                        if __data.len() < __ix_dyn_prefix_end {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_len = {
                            let mut __buf = [0u8; 8];
                            __buf[..#pfx].copy_from_slice(&__data[__offset..__ix_dyn_prefix_end]);
                            u64::from_le_bytes(__buf) as usize
                        };
                    });
                    stmts.push(quote! {
                        __offset = __ix_dyn_prefix_end;
                    });
                    stmts.push(quote! {
                        if __ix_dyn_len > #max {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_end = __offset
                            .checked_add(__ix_dyn_len)
                            .ok_or(ProgramError::InvalidInstructionData)?;
                        if __data.len() < __ix_dyn_end {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let #name: &[u8] = &__data[__offset..__ix_dyn_end];
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __offset = __ix_dyn_end;
                        });
                    }
                }
                Some(PodDynField::Vec {
                    elem,
                    max,
                    prefix_bytes,
                }) => {
                    dyn_idx += 1;
                    let pfx = *prefix_bytes;
                    stmts.push(quote! {
                        let __ix_dyn_prefix_end = __offset
                            .checked_add(#pfx)
                            .ok_or(ProgramError::InvalidInstructionData)?;
                        if __data.len() < __ix_dyn_prefix_end {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_count = {
                            let mut __buf = [0u8; 8];
                            __buf[..#pfx].copy_from_slice(&__data[__offset..__ix_dyn_prefix_end]);
                            u64::from_le_bytes(__buf) as usize
                        };
                    });
                    stmts.push(quote! {
                        __offset = __ix_dyn_prefix_end;
                    });
                    stmts.push(quote! {
                        if __ix_dyn_count > #max {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_byte_len = __ix_dyn_count
                            .checked_mul(core::mem::size_of::<#elem>())
                            .ok_or(ProgramError::InvalidInstructionData)?;
                    });
                    stmts.push(quote! {
                        let __ix_dyn_end = __offset
                            .checked_add(__ix_dyn_byte_len)
                            .ok_or(ProgramError::InvalidInstructionData)?;
                        if __data.len() < __ix_dyn_end {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        // SAFETY: the byte range was bounds-checked above and
                        // vector element alignment is asserted to be 1.
                        let #name: &[#elem] = unsafe {
                            core::slice::from_raw_parts(
                                __data.as_ptr().add(__offset) as *const #elem,
                                __ix_dyn_count,
                            )
                        };
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __offset = __ix_dyn_end;
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
