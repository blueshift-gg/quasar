mod attrs;
mod client;
mod fields;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::ParseStream, parse_macro_input, Data, DeriveInput, Fields, Ident, Token, Type};

use crate::helpers::{
    classify_dynamic_string, classify_dynamic_vec, classify_tail, is_composite_type, map_to_pod_type,
    strip_generics, zc_deserialize_expr, DynKind,
};

struct InstructionArg {
    name: Ident,
    ty: Type,
}

fn parse_struct_instruction_args(input: &DeriveInput) -> Option<Vec<InstructionArg>> {
    for attr in &input.attrs {
        if attr.path().is_ident("instruction") {
            let result: syn::Result<Vec<InstructionArg>> =
                attr.parse_args_with(|stream: ParseStream| {
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
                });
            return result.ok();
        }
    }
    None
}

pub(crate) fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let bumps_name = format_ident!("{}Bumps", name);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "Accounts can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Accounts can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();

    let field_name_strings: Vec<String> = fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect();

    let mut pf = match fields::process_fields(fields, &field_name_strings) {
        Ok(pf) => pf,
        Err(ts) => return ts,
    };

    // --- Composite type handling ---

    let mut has_composites = false;
    let mut composite_types: Vec<Option<proc_macro2::TokenStream>> = Vec::new();
    for field in fields.iter() {
        if is_composite_type(&field.ty) {
            has_composites = true;
            composite_types.push(Some(strip_generics(&field.ty)));
        } else {
            composite_types.push(None);
        }
    }

    let count_expr: proc_macro2::TokenStream = if has_composites {
        let addends: Vec<proc_macro2::TokenStream> = composite_types
            .iter()
            .map(|ct| match ct {
                Some(ty) => quote! { <#ty as AccountCount>::COUNT },
                None => quote! { 1usize },
            })
            .collect();
        quote! { #(#addends)+* }
    } else {
        let field_count = field_names.len();
        quote! { #field_count }
    };

    let mut parse_steps: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut buf_offset = quote! { 0usize };
    for ct in &composite_types {
        if let Some(inner_ty) = ct {
            let cur_offset = buf_offset.clone();
            parse_steps.push(quote! {
                {
                    let mut __inner_buf = core::mem::MaybeUninit::<
                        [quasar_core::__internal::AccountView; <#inner_ty as AccountCount>::COUNT]
                    >::uninit();
                    input = <#inner_ty>::parse_accounts(input, &mut __inner_buf);
                    let __inner = unsafe { __inner_buf.assume_init() };
                    let mut __j = 0usize;
                    while __j < <#inner_ty as AccountCount>::COUNT {
                        unsafe { core::ptr::write(base.add(#cur_offset + __j), *__inner.as_ptr().add(__j)); }
                        __j += 1;
                    }
                }
            });
            buf_offset = quote! { #buf_offset + <#inner_ty as AccountCount>::COUNT };
        } else {
            let cur_offset = buf_offset.clone();
            parse_steps.push(quote! {
                {
                    let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                    if unsafe { (*raw).borrow_state } == quasar_core::__internal::NOT_BORROWED {
                        unsafe {
                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER + (*raw).data_len as usize);
                            let align = (input as *const u8).align_offset(8);
                            input = input.add(align);
                        }
                    } else {
                        unsafe {
                            let idx = (*raw).borrow_state as usize;
                            core::ptr::write(base.add(#cur_offset), core::ptr::read(base.add(idx)));
                            input = input.add(core::mem::size_of::<u64>());
                        }
                    }
                }
            });
            buf_offset = quote! { #buf_offset + 1usize };
        }
    }

    // --- Composite field_lets (pre-compute before bumps so pushes take effect) ---

    let has_pda_fields = !pf.bump_struct_fields.is_empty();

    let mut field_lets: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut non_composite_constructs: Vec<proc_macro2::TokenStream> = Vec::new();
    if has_composites {
        let mut idx_offset = quote! { 0usize };
        for (fi, field) in fields.iter().enumerate() {
            let field_name = field.ident.as_ref().unwrap();
            if composite_types[fi].is_some() {
                let inner_ty = composite_types[fi].as_ref().unwrap();
                let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                let cur_offset = idx_offset.clone();
                field_lets.push(quote! {
                    let (#field_name, #bumps_var) = <#inner_ty as ParseAccounts>::parse(
                        &accounts[#cur_offset..#cur_offset + <#inner_ty as AccountCount>::COUNT]
                    )?;
                });
                pf.bump_struct_fields
                    .push(quote! { pub #field_name: <#inner_ty as ParseAccounts>::Bumps });
                pf.bump_struct_inits
                    .push(quote! { #field_name: #bumps_var });
                idx_offset = quote! { #idx_offset + <#inner_ty as AccountCount>::COUNT };
            } else {
                let cur_offset = idx_offset.clone();
                field_lets.push(quote! {
                    let #field_name = &accounts[#cur_offset];
                });
                idx_offset = quote! { #idx_offset + 1usize };
            }
        }

        non_composite_constructs = fields
            .iter()
            .enumerate()
            .map(|(fi, field)| {
                let field_name = field.ident.as_ref().unwrap();
                if composite_types[fi].is_some() {
                    quote! { #field_name }
                } else {
                    pf.field_constructs[fi].clone()
                }
            })
            .collect();
    }

    // --- Bumps (after all modifications including composites) ---

    let bump_struct_fields = &pf.bump_struct_fields;
    let bump_struct_inits = &pf.bump_struct_inits;

    let bumps_struct = if has_pda_fields || !bump_struct_fields.is_empty() {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name { #(#bump_struct_fields,)* } }
    } else {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name; }
    };

    let bumps_init = if has_pda_fields || !bump_struct_inits.is_empty() {
        quote! { #bumps_name { #(#bump_struct_inits,)* } }
    } else {
        quote! { #bumps_name }
    };

    // --- Parse body generation (3 code paths) ---

    let has_any_checks = !pf.has_one_checks.is_empty()
        || !pf.constraint_checks.is_empty()
        || !pf.mut_checks.is_empty()
        || !pf.pda_checks.is_empty()
        || !pf.init_pda_checks.is_empty()
        || !pf.init_blocks.is_empty();

    let seed_addr_captures = &pf.seed_addr_captures;
    let bump_init_vars = &pf.bump_init_vars;
    let mut_checks = &pf.mut_checks;
    let has_one_checks = &pf.has_one_checks;
    let constraint_checks = &pf.constraint_checks;
    let pda_checks = &pf.pda_checks;
    let field_constructs = &pf.field_constructs;
    let init_pda_checks = &pf.init_pda_checks;
    let init_blocks = &pf.init_blocks;

    let rent_fetch = if pf.needs_rent {
        quote! { let __shared_rent = <quasar_core::sysvars::rent::Rent as quasar_core::sysvars::Sysvar>::get()?; }
    } else {
        quote! {}
    };

    let parse_body = if has_composites {
        if has_any_checks {
            quote! {
                if accounts.len() < Self::COUNT {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                #(#field_lets)*
                #(#seed_addr_captures)*
                #(#bump_init_vars)*
                #(#init_pda_checks)*
                #rent_fetch
                #(#init_blocks)*

                let result = Self {
                    #(#non_composite_constructs,)*
                };

                {
                    let Self { #(ref #field_names,)* } = result;
                    #(#mut_checks)*
                    #(#has_one_checks)*
                    #(#constraint_checks)*
                    #(#pda_checks)*
                }

                Ok((result, #bumps_init))
            }
        } else {
            quote! {
                if accounts.len() < Self::COUNT {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                #(#field_lets)*

                Ok((Self {
                    #(#non_composite_constructs,)*
                }, #bumps_init))
            }
        }
    } else if has_any_checks {
        quote! {
            let [#(#field_names),*] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };

            #(#seed_addr_captures)*
            #(#bump_init_vars)*
            #(#init_pda_checks)*
            #rent_fetch
            #(#init_blocks)*

            let result = Self {
                #(#field_constructs,)*
            };

            {
                let Self { #(ref #field_names,)* } = result;
                #(#mut_checks)*
                #(#has_one_checks)*
                #(#constraint_checks)*
                #(#pda_checks)*
            }

            Ok((result, #bumps_init))
        }
    } else {
        quote! {
            let [#(#field_names),*] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };

            Ok((Self {
                #(#field_constructs,)*
            }, #bumps_init))
        }
    };

    // --- Seeds impl ---

    let seeds_methods = &pf.seeds_methods;
    let seeds_impl = if seeds_methods.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #bumps_name {
                #(#seeds_methods)*
            }
        }
    };

    // --- Client macro ---

    let client_macro = client::generate_client_macro(name, fields, &pf.field_attrs);

    // --- Epilogue generation ---

    let epilogue_method = if !pf.close_fields.is_empty() {
        let close_stmts: Vec<proc_macro2::TokenStream> = pf
            .close_fields
            .iter()
            .map(|(field, dest)| {
                quote! { self.#field.close(self.#dest.to_account_view())?; }
            })
            .collect();
        quote! {
            #[inline(always)]
            fn epilogue(&self) -> Result<(), ProgramError> {
                #(#close_stmts)*
                Ok(())
            }
        }
    } else {
        quote! {}
    };

    // --- Instruction arg extraction (struct-level #[instruction(...)]) ---

    let instruction_args = parse_struct_instruction_args(&input);
    let has_instruction_args = instruction_args.is_some();

    let ix_arg_extraction = if let Some(ref ix_args) = instruction_args {
        generate_instruction_arg_extraction(ix_args)
    } else {
        quote! {}
    };

    // --- Final output ---

    let parse_accounts_impl = if has_instruction_args {
        quote! {
            impl<'info> ParseAccounts<'info> for #name<'info> {
                type Bumps = #bumps_name;

                #[inline(always)]
                fn parse(accounts: &'info [AccountView]) -> Result<(Self, Self::Bumps), ProgramError> {
                    Self::parse_with_instruction_data(accounts, &[])
                }

                #[inline(always)]
                fn parse_with_instruction_data(
                    accounts: &'info [AccountView],
                    __ix_data: &'info [u8],
                ) -> Result<(Self, Self::Bumps), ProgramError> {
                    #ix_arg_extraction
                    #parse_body
                }

                #epilogue_method
            }
        }
    } else {
        quote! {
            impl<'info> ParseAccounts<'info> for #name<'info> {
                type Bumps = #bumps_name;

                #[inline(always)]
                fn parse(accounts: &'info [AccountView]) -> Result<(Self, Self::Bumps), ProgramError> {
                    #parse_body
                }

                #epilogue_method
            }
        }
    };

    let expanded = quote! {
        #bumps_struct

        #parse_accounts_impl

        #seeds_impl

        impl<'info> AccountCount for #name<'info> {
            const COUNT: usize = #count_expr;
        }

        impl<'info> #name<'info> {
            #[inline(always)]
            pub unsafe fn parse_accounts(
                mut input: *mut u8,
                buf: &mut core::mem::MaybeUninit<[quasar_core::__internal::AccountView; #count_expr]>,
            ) -> *mut u8 {
                const __ACCOUNT_HEADER: usize =
                    core::mem::size_of::<quasar_core::__internal::RuntimeAccount>()
                    + quasar_core::__internal::MAX_PERMITTED_DATA_INCREASE
                    + core::mem::size_of::<u64>();

                let base = buf.as_mut_ptr() as *mut quasar_core::__internal::AccountView;

                #(#parse_steps)*

                input
            }
        }

        #client_macro
    };

    TokenStream::from(expanded)
}

/// Generate code that extracts `#[instruction(..)]` args from `__ix_data`.
///
/// Fixed types are read via a zero-copy `#[repr(C)]` struct pointer cast.
/// Dynamic fields use inline prefix reads from the data buffer after the
/// fixed ZC block.
fn generate_instruction_arg_extraction(ix_args: &[InstructionArg]) -> proc_macro2::TokenStream {
    if ix_args.is_empty() {
        return quote! {};
    }

    let kinds: Vec<DynKind> = ix_args
        .iter()
        .map(|arg| {
            if let Some((prefix, max)) = classify_dynamic_string(&arg.ty) {
                DynKind::Str { prefix, max }
            } else if let Some(tail_elem) = classify_tail(&arg.ty) {
                DynKind::Tail { element: tail_elem }
            } else if let Some((elem, prefix, max)) = classify_dynamic_vec(&arg.ty) {
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

    let mut stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    for assert_stmt in vec_align_asserts {
        stmts.push(assert_stmt);
    }

    // ZC struct with ONLY fixed fields
    if has_fixed {
        let mut zc_field_names: Vec<Ident> = Vec::new();
        let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();

        for (i, kind) in kinds.iter().enumerate() {
            if matches!(kind, DynKind::Fixed) {
                zc_field_names.push(ix_args[i].name.clone());
                zc_field_types.push(map_to_pod_type(&ix_args[i].ty));
            }
        }

        stmts.push(quote! {
            #[repr(C)]
            #[derive(Copy, Clone)]
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

        // Extract fixed fields
        for (i, kind) in kinds.iter().enumerate() {
            if matches!(kind, DynKind::Fixed) {
                let name = &ix_args[i].name;
                let expr = zc_deserialize_expr(name, &ix_args[i].ty);
                let prefixed_expr = quote! { {
                    let __zc = __ix_zc;
                    #expr
                } };
                stmts.push(quote! {
                    let #name = #prefixed_expr;
                });
            }
        }
    }

    // Extract dynamic fields with inline prefix reads
    if has_dynamic {
        if has_fixed {
            stmts.push(quote! {
                let __data = __ix_data;
            });
            stmts.push(quote! {
                let mut __offset = core::mem::size_of::<__IxArgsZc>();
            });
        } else {
            stmts.push(quote! {
                let __data = __ix_data;
            });
            stmts.push(quote! {
                let mut __offset: usize = 0;
            });
        }

        let dyn_count = kinds
            .iter()
            .filter(|k| !matches!(k, DynKind::Fixed))
            .count();
        let mut dyn_idx = 0usize;

        for (i, kind) in kinds.iter().enumerate() {
            let name = &ix_args[i].name;
            match kind {
                DynKind::Fixed => {}
                DynKind::Str { prefix, max } => {
                    dyn_idx += 1;
                    let pb = prefix.bytes();
                    let max_lit = *max;
                    let read_len = prefix.gen_read_len();
                    stmts.push(quote! {
                        if __data.len() < __offset + #pb {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_len = #read_len;
                    });
                    stmts.push(quote! {
                        __offset += #pb;
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
                DynKind::Tail { element } => {
                    dyn_idx += 1;
                    // Tail: remaining data, no prefix
                    match element {
                        crate::helpers::TailElement::Str => {
                            stmts.push(quote! {
                                let #name: &[u8] = &__data[__offset..];
                            });
                        }
                        crate::helpers::TailElement::Bytes => {
                            stmts.push(quote! {
                                let #name: &[u8] = &__data[__offset..];
                            });
                        }
                    }
                    // Tail consumes all remaining data — no offset advance
                }
                DynKind::Vec { elem, prefix, max } => {
                    dyn_idx += 1;
                    let pb = prefix.bytes();
                    let max_lit = *max;
                    let read_len = prefix.gen_read_len();
                    stmts.push(quote! {
                        if __data.len() < __offset + #pb {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_count = #read_len;
                    });
                    stmts.push(quote! {
                        __offset += #pb;
                    });
                    stmts.push(quote! {
                        if __ix_dyn_count > #max_lit {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let __ix_dyn_byte_len = __ix_dyn_count * core::mem::size_of::<#elem>();
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
