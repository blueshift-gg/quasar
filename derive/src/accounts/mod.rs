mod attrs;
mod client;
mod fields;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::ParseStream, parse_macro_input, Data, DeriveInput, Fields, Ident, Token, Type};

use crate::helpers::{
    extract_generic_inner_type, is_composite_type, is_dynamic_string, is_dynamic_vec, is_str_ref,
    map_to_pod_type, strip_generics, zc_deserialize_expr, DynKind,
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

    // Parse struct-level attributes for #[account(dup)]
    let mut allow_dup = false;
    for attr in &input.attrs {
        if attr.path().is_ident("account") {
            if let Ok(meta_list) = attr.meta.require_list() {
                for nested in meta_list.tokens.clone() {
                    if nested.to_string() == "dup" {
                        allow_dup = true;
                    }
                }
            }
        }
    }

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

    // --- Generate parse_steps and parse_steps_no_dup ---

    let mut parse_steps: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut parse_steps_no_dup: Vec<proc_macro2::TokenStream> = Vec::new();
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

            parse_steps_no_dup.push(quote! {
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
            // Simple field - validate header for no-dup case
            let cur_offset = buf_offset.clone();
            let attrs = &pf.field_attrs[fi];
            let field = &fields[fi];
            let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
            let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());
            let expected_header = fields::compute_header_expected(field, attrs, is_ref_mut);

            // Standard parse_accounts (allows duplicates, validates flags)
            // For init_if_needed, also mask out is_signer byte since we can't know if account will be signer at parse time
            let mask = if attrs.init_if_needed {
                0xFFFF00FFu32  // Mask out borrow_state (byte 0) AND is_signer (byte 1)
            } else {
                0xFFFFFF00u32  // Mask out borrow_state (byte 0) only
            };
            let expected_flags = expected_header & mask;
            parse_steps.push(quote! {
                {
                    let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                    let actual_header = unsafe { *(raw as *const u32) };

                    // Validate flags (mask out borrow_state, and is_signer for init_if_needed)
                    let actual_flags = actual_header & #mask;
                    if quasar_core::utils::hint::unlikely(actual_flags != #expected_flags) {
                        return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                    }

                    // Handle dup
                    if (actual_header & 0xFF) == quasar_core::__internal::NOT_BORROWED {
                        unsafe {
                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER + (*raw).data_len as usize);
                            let align = (input as *const u8).align_offset(8);
                            input = input.add(align);
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

            // No-dup parse_accounts (validates u32 header)
            // For init_if_needed, mask out is_signer since we can't know if account will be signer at parse time
            if attrs.init_if_needed {
                let no_dup_mask = 0xFFFF00FFu32;  // Mask out borrow_state (byte 0) AND is_signer (byte 1)
                let expected_flags_no_dup = expected_header & no_dup_mask;
                parse_steps_no_dup.push(quote! {
                    {
                        let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                        let actual_header = unsafe { *(raw as *const u32) };

                        // Validate header (masking out borrow_state and is_signer for init_if_needed)
                        let actual_flags = actual_header & #no_dup_mask;
                        if quasar_core::utils::hint::unlikely(actual_flags != #expected_flags_no_dup) {
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }

                        // Ensure no duplicate (byte 0 must be 0xFF)
                        if quasar_core::utils::hint::unlikely((actual_header & 0xFF) != 0xFF) {
                            return Err(ProgramError::AccountBorrowFailed);
                        }

                        unsafe {
                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER + (*raw).data_len as usize);
                            let addr = input as usize;
                            input = ((addr + 7) & !7) as *mut u8;
                        }
                    }
                });
            } else {
                parse_steps_no_dup.push(quote! {
                    {
                        let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                        let actual_header = unsafe { *(raw as *const u32) };

                        if quasar_core::utils::hint::unlikely(actual_header != #expected_header) {
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }

                        unsafe {
                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER + (*raw).data_len as usize);
                            let addr = input as usize;
                            input = ((addr + 7) & !7) as *mut u8;
                        }
                    }
                });
            }

            buf_offset = quote! { #buf_offset + 1usize };
        }
    }

    // Choose which parse_steps to use based on allow_dup
    let active_parse_steps = if allow_dup {
        parse_steps  // Use dup-allowed version (still optimizes flags)
    } else {
        parse_steps_no_dup  // Use no-dup version (default, fully optimized)
    };

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
            ) -> Result<*mut u8, ProgramError> {
                const __ACCOUNT_HEADER: usize =
                    core::mem::size_of::<quasar_core::__internal::RuntimeAccount>()
                    + quasar_core::__internal::MAX_PERMITTED_DATA_INCREASE
                    + core::mem::size_of::<u64>();

                let base = buf.as_mut_ptr() as *mut quasar_core::__internal::AccountView;

                #(#active_parse_steps)*

                Ok(input)
            }
        }

        #client_macro
    };

    TokenStream::from(expanded)
}

/// Generate code that extracts `#[instruction(..)]` args from `__ix_data`.
///
/// Follows the same ZC header + dynamic tail pattern as instruction.rs.
/// Fixed types are read via a zero-copy `#[repr(C)]` struct pointer cast.
/// Dynamic strings are read from a variable-length tail region using
/// PodU16 length descriptors in the ZC header.
fn generate_instruction_arg_extraction(ix_args: &[InstructionArg]) -> proc_macro2::TokenStream {
    if ix_args.is_empty() {
        return quote! {};
    }

    let kinds: Vec<DynKind> = ix_args
        .iter()
        .map(|arg| {
            if let Some(max) = is_dynamic_string(&arg.ty, false) {
                DynKind::Str { max }
            } else if is_str_ref(&arg.ty) {
                DynKind::StrRef
            } else if let Some((elem, max)) = is_dynamic_vec(&arg.ty, false) {
                DynKind::Vec {
                    elem: Box::new(elem),
                    max,
                }
            } else {
                DynKind::Fixed
            }
        })
        .collect();

    let has_dynamic = kinds.iter().any(|k| !matches!(k, DynKind::Fixed));

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

    let mut zc_field_names: Vec<Ident> = Vec::new();
    let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();

    for (i, kind) in kinds.iter().enumerate() {
        match kind {
            DynKind::Fixed => {
                zc_field_names.push(ix_args[i].name.clone());
                zc_field_types.push(map_to_pod_type(&ix_args[i].ty));
            }
            DynKind::Str { .. } => {
                let len_name = format_ident!("{}_len", ix_args[i].name);
                zc_field_names.push(len_name);
                zc_field_types.push(quote! { quasar_core::pod::PodU16 });
            }
            DynKind::StrRef => {
                let len_name = format_ident!("{}_len", ix_args[i].name);
                zc_field_names.push(len_name);
                zc_field_types.push(quote! { u8 });
            }
            DynKind::Vec { .. } => {
                let count_name = format_ident!("{}_count", ix_args[i].name);
                zc_field_names.push(count_name);
                zc_field_types.push(quote! { quasar_core::pod::PodU16 });
            }
        }
    }

    let mut stmts: Vec<proc_macro2::TokenStream> = Vec::new();

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

    for assert_stmt in vec_align_asserts {
        stmts.push(assert_stmt);
    }

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
            // Prefix with __ix_zc instead of __zc
            let prefixed_expr = quote! { {
                let __zc = __ix_zc;
                #expr
            } };
            stmts.push(quote! {
                let #name = #prefixed_expr;
            });
        }
    }

    // Extract dynamic fields from tail
    if has_dynamic {
        stmts.push(quote! {
            let __ix_tail = &__ix_data[core::mem::size_of::<__IxArgsZc>()..];
        });
        stmts.push(quote! {
            let mut __ix_offset: usize = 0;
        });

        let dyn_count = kinds
            .iter()
            .filter(|k| !matches!(k, DynKind::Fixed))
            .count();
        let mut dyn_idx = 0usize;

        for (i, kind) in kinds.iter().enumerate() {
            let name = &ix_args[i].name;
            match kind {
                DynKind::Fixed => {}
                DynKind::Str { max } => {
                    dyn_idx += 1;
                    let len_name = format_ident!("{}_len", name);
                    let max_lit = *max;
                    stmts.push(quote! {
                        let __ix_dyn_len = __ix_zc.#len_name.get() as usize;
                    });
                    stmts.push(quote! {
                        if __ix_dyn_len > #max_lit {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        if __ix_tail.len() < __ix_offset + __ix_dyn_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let #name: &[u8] = &__ix_tail[__ix_offset..__ix_offset + __ix_dyn_len];
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __ix_offset += __ix_dyn_len;
                        });
                    }
                }
                DynKind::StrRef => {
                    dyn_idx += 1;
                    let len_name = format_ident!("{}_len", name);
                    stmts.push(quote! {
                        let __ix_dyn_len = __ix_zc.#len_name as usize;
                    });
                    stmts.push(quote! {
                        if __ix_tail.len() < __ix_offset + __ix_dyn_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let #name: &[u8] = &__ix_tail[__ix_offset..__ix_offset + __ix_dyn_len];
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __ix_offset += __ix_dyn_len;
                        });
                    }
                }
                DynKind::Vec { elem, max } => {
                    dyn_idx += 1;
                    let count_name = format_ident!("{}_count", name);
                    let max_lit = *max;
                    stmts.push(quote! {
                        let __ix_dyn_count = __ix_zc.#count_name.get() as usize;
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
                        if __ix_tail.len() < __ix_offset + __ix_dyn_byte_len {
                            return Err(ProgramError::InvalidInstructionData);
                        }
                    });
                    stmts.push(quote! {
                        let #name: &[#elem] = unsafe {
                            core::slice::from_raw_parts(
                                __ix_tail.as_ptr().add(__ix_offset) as *const #elem,
                                __ix_dyn_count,
                            )
                        };
                    });
                    if dyn_idx < dyn_count {
                        stmts.push(quote! {
                            __ix_offset += __ix_dyn_byte_len;
                        });
                    }
                }
            }
        }

        stmts.push(quote! {
            let _ = __ix_offset;
        });
    }

    quote! { #(#stmts)* }
}
