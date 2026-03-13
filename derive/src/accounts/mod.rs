//! `#[derive(Accounts)]` — generates account parsing, validation, and PDA
//! derivation from a struct definition. This is the core macro that transforms
//! a declarative accounts struct into the zero-copy parsing pipeline.

mod attrs;
mod client;
mod fields;

use {
    crate::helpers::{
        classify_dynamic_string, classify_dynamic_vec, classify_tail, extract_generic_inner_type,
        is_composite_type, map_to_pod_type, strip_generics, zc_deserialize_expr, DynKind,
    },
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse::ParseStream, parse_macro_input, Data, DeriveInput, Fields, Ident, Token, Type},
};

struct InstructionArg {
    name: Ident,
    ty: Type,
}

fn parse_struct_instruction_args(input: &DeriveInput) -> Option<Vec<InstructionArg>> {
    input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("instruction"))
        .and_then(|attr| {
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
            })
            .ok()
        })
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

    // --- Generate parse_steps (hybrid: per-field dup-aware or no-dup) ---

    let mut parse_steps: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut buf_offset = quote! { 0usize };

    for (fi, ct) in composite_types.iter().enumerate() {
        if let Some(inner_ty) = ct {
            // Composite type - recursively call parse_accounts
            // (each inner type knows its own dup policy from its #[account(dup)] attribute)
            let cur_offset = buf_offset.clone();

            parse_steps.push(quote! {
                {
                    let mut __inner_buf = core::mem::MaybeUninit::<
                        [quasar_core::__internal::AccountView; <#inner_ty as AccountCount>::COUNT]
                    >::uninit();
                    input = <#inner_ty>::parse_accounts(input, &mut __inner_buf)?;
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
            let attrs = &pf.field_attrs[fi];
            let field = &fields[fi];

            if attrs.dup && buf_offset.to_string() == "0usize" {
                return syn::Error::new_spanned(
                    field,
                    "first account (index 0) cannot be marked with #[account(dup)] - it can never \
                     be a duplicate",
                )
                .to_compile_error()
                .into();
            }
            let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
            let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());
            let expected_header = fields::compute_header_expected(field, attrs, is_ref_mut);

            let is_optional = extract_generic_inner_type(&field.ty, "Option").is_some();
            let field_name = field.ident.as_ref().unwrap();
            let account_index = buf_offset.to_string();

            if is_optional || attrs.dup {
                // Dup-aware path: checks borrow state to detect duplicates
                let expected_signer = (expected_header >> 8) & 0x01;
                let expected_writable = (expected_header >> 16) & 0x01;
                let expected_exec = (expected_header >> 24) & 0x01;

                let dup_check = |cond: proc_macro2::TokenStream, msg: &str| {
                    quote! {
                        if quasar_core::utils::hint::unlikely(#cond) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!(
                                "Account '", stringify!(#field_name),
                                "' (index ", #account_index, "): ", #msg
                            ));
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }
                    }
                };

                let flag_check = match (expected_signer, expected_writable) {
                    (1, 1) => dup_check(
                        quote! { (actual_header >> 8) as u16 != 0x0101 },
                        "must be writable signer",
                    ),
                    (0, 1) => dup_check(
                        quote! { ((actual_header >> 16) & 0x01) == 0 },
                        "must be writable",
                    ),
                    (1, 0) => dup_check(
                        quote! { ((actual_header >> 8) & 0x01) == 0 },
                        "must be signer",
                    ),
                    _ => quote! {},
                };

                let exec_check = if is_optional {
                    quote! {}
                } else {
                    match expected_exec {
                        1 => dup_check(
                            quote! { ((actual_header >> 24) & 0x01) != 1 },
                            "must be executable program",
                        ),
                        _ => dup_check(
                            quote! { ((actual_header >> 24) & 0x01) != 0 },
                            "must not be executable",
                        ),
                    }
                };

                parse_steps.push(quote! {
                    {
                        let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                        let actual_header = unsafe { *(raw as *const u32) };

                        if (actual_header & 0xFF) == quasar_core::__internal::NOT_BORROWED as u32 {
                            #flag_check
                            #exec_check
                            unsafe {
                                core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                                input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                                input = input.add((input as usize).wrapping_neg() & 7);
                            }
                        } else {
                            unsafe {
                                let idx = (actual_header & 0xFF) as usize;
                                core::ptr::write(base.add(#cur_offset), core::ptr::read(base.add(idx)));
                                input = input.add(core::mem::size_of::<u64>());
                            }
                        }
                    }
                });
            } else {
                // No-dup path: single constant comparison
                let nodup_const = fields::determine_nodup_constant(field, attrs, is_ref_mut);
                let nodup_const_ident = format_ident!("{}", nodup_const);

                let (check_cond, debug_msg) = if attrs.init_if_needed {
                    (
                        quote! { (header & 0x000100FF) != 0x000100FF },
                        "init_if_needed requires writable, no duplicates",
                    )
                } else if nodup_const == "NODUP_SIGNER" {
                    // u16: borrow_state + is_signer only, permits writable/executable
                    (
                        quote! { (header as u16) != (quasar_core::__internal::#nodup_const_ident as u16) },
                        "must be signer, no duplicates",
                    )
                } else {
                    (
                        quote! { header != quasar_core::__internal::#nodup_const_ident },
                        match nodup_const {
                            "NODUP" => "no duplicates allowed",
                            "NODUP_MUT" => "must be writable, no duplicates",
                            "NODUP_MUT_SIGNER" => "must be writable signer, no duplicates",
                            "NODUP_EXECUTABLE" => "must be executable, no duplicates",
                            _ => "constraint violated",
                        },
                    )
                };

                parse_steps.push(quote! {
                    unsafe {
                        let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                        let header = *(raw as *const u32);

                        if quasar_core::utils::hint::unlikely(#check_cond) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!(
                                "Account '", stringify!(#field_name),
                                "' (index ", #account_index, "): ", #debug_msg
                            ));
                            return Err(quasar_core::decode_header_error(header, #expected_header));
                        }

                        core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                        input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                        input = input.add((input as usize).wrapping_neg() & 7);
                    }
                });
            }

            buf_offset = quote! { #buf_offset + 1usize };
        }
    }

    // --- Composite field_lets (pre-compute before bumps so pushes take effect) ---

    let has_pda_fields = !pf.bump_struct_fields.is_empty();

    let mut field_lets: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut non_composite_constructs: Vec<proc_macro2::TokenStream> = Vec::new();
    if has_composites {
        // Use split_at_mut to progressively carve off chunks from the &mut slice.
        // Each field takes its chunk, and __accounts_rest is the remainder.
        field_lets.push(quote! {
            let mut __accounts_rest = accounts;
        });
        for (fi, field) in fields.iter().enumerate() {
            let field_name = field.ident.as_ref().unwrap();
            if composite_types[fi].is_some() {
                let inner_ty = composite_types[fi].as_ref().unwrap();
                let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                field_lets.push(quote! {
                    let (__chunk, __rest) = __accounts_rest.split_at_mut(<#inner_ty as AccountCount>::COUNT);
                    __accounts_rest = __rest;
                    let (#field_name, #bumps_var) = <#inner_ty as ParseAccounts>::parse(
                        __chunk,
                        __program_id
                    )?;
                });
                pf.bump_struct_fields
                    .push(quote! { pub #field_name: <#inner_ty as ParseAccounts>::Bumps });
                pf.bump_struct_inits
                    .push(quote! { #field_name: #bumps_var });
            } else {
                field_lets.push(quote! {
                    let (__chunk, __rest) = __accounts_rest.split_at_mut(1);
                    __accounts_rest = __rest;
                    let #field_name = &mut __chunk[0];
                });
            }
        }
        field_lets.push(quote! {
            let _ = __accounts_rest;
        });

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
            fn epilogue(&mut self) -> Result<(), ProgramError> {
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
                fn parse(accounts: &'info mut [AccountView], program_id: &Address) -> Result<(Self, Self::Bumps), ProgramError> {
                    Self::parse_with_instruction_data(accounts, &[], program_id)
                }

                #[inline(always)]
                fn parse_with_instruction_data(
                    accounts: &'info mut [AccountView],
                    __ix_data: &'info [u8],
                    __program_id: &Address,
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
                fn parse(accounts: &'info mut [AccountView], __program_id: &Address) -> Result<(Self, Self::Bumps), ProgramError> {
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
            ) -> Result<*mut u8, ProgramError> {
                const __ACCOUNT_HEADER: usize =
                    core::mem::size_of::<quasar_core::__internal::RuntimeAccount>()
                    + quasar_core::__internal::MAX_PERMITTED_DATA_INCREASE
                    + core::mem::size_of::<u64>();

                let base = buf.as_mut_ptr() as *mut quasar_core::__internal::AccountView;

                #(#parse_steps)*

                Ok(input)
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

    let mut stmts: Vec<proc_macro2::TokenStream> = vec_align_asserts;

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
                DynKind::Tail { .. } => {
                    dyn_idx += 1;
                    // Tail: remaining data, no prefix
                    stmts.push(quote! {
                        let #name: &[u8] = &__data[__offset..];
                    });
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
