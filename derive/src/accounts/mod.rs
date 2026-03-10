//! `#[derive(Accounts)]` — generates account parsing, validation, and PDA
//! derivation from a struct definition. This is the core macro that transforms
//! a declarative accounts struct into the zero-copy parsing pipeline.

mod attrs;
mod client;
mod fields;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::ParseStream, parse_macro_input, Data, DeriveInput, Fields, Ident, Token, Type};

use crate::helpers::{
    classify_dynamic_string, classify_dynamic_vec, classify_tail, extract_generic_inner_type,
    is_composite_type, map_to_pod_type, strip_generics, zc_deserialize_expr, DynKind,
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
            // Simple field - per-field dup-aware or no-dup
            let cur_offset = buf_offset.clone();
            let attrs = &pf.field_attrs[fi];
            let field = &fields[fi];

            // Validate: first account (index 0) cannot be marked with #[account(dup)]
            if attrs.dup && buf_offset.to_string() == "0usize" {
                return syn::Error::new_spanned(
                    field,
                    "first account (index 0) cannot be marked with #[account(dup)] - it can never be a duplicate",
                )
                .to_compile_error()
                .into();
            }
            let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
            let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());
            let expected_header = fields::compute_header_expected(field, attrs, is_ref_mut);

            // Detect if this field is Optional - optional fields always allow duplicates
            let is_optional = extract_generic_inner_type(&field.ty, "Option").is_some();

            // Choose parsing strategy: dup-aware (checks borrow state) or no-dup (optimized u32)
            if is_optional || attrs.dup {
                // Dup-aware path: optional accounts or accounts marked with #[account(dup)]
                // (init_if_needed now uses optimized nodup path with masked validation)

                // Regular accounts with dup allowed: optimized dup checking
                // Extract required flags for static compile-time checks
                let expected_signer = (expected_header >> 8) & 0x01;
                let expected_writable = (expected_header >> 16) & 0x01;
                let expected_exec = (expected_header >> 24) & 0x01;

                let field_name = field.ident.as_ref().unwrap();
                let account_index = buf_offset.to_string();

                // Generate optimal check based on requirements
                let flag_check = if expected_signer == 1 && expected_writable == 1 {
                    // Both mut+signer required: combined u16 check (bytes 1-2)
                    quote! {
                        if quasar_core::utils::hint::unlikely((actual_header >> 8) as u16 != 0x0101) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must be writable signer"));
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }
                    }
                } else if expected_writable == 1 {
                    // At least mut required (may or may not be signer)
                    quote! {
                        if quasar_core::utils::hint::unlikely(((actual_header >> 16) & 0x01) == 0) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must be writable"));
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }
                    }
                } else if expected_signer == 1 {
                    // At least signer required (not writable)
                    quote! {
                        if quasar_core::utils::hint::unlikely(((actual_header >> 8) & 0x01) == 0) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must be signer"));
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }
                    }
                } else {
                    // No signer/mut requirements
                    quote! {}
                };

                // Executable must match exactly if required
                // For optional accounts, skip executable check (program_id is executable)
                let exec_check = if is_optional {
                    quote! {}
                } else if expected_exec == 1 {
                    quote! {
                        if quasar_core::utils::hint::unlikely(((actual_header >> 24) & 0x01) != 1) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must be executable program"));
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }
                    }
                } else {
                    quote! {
                        if quasar_core::utils::hint::unlikely(((actual_header >> 24) & 0x01) != 0) {
                            #[cfg(feature = "debug")]
                            quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must not be executable"));
                            return Err(quasar_core::decode_header_error(actual_header, #expected_header));
                        }
                    }
                };

                parse_steps.push(quote! {
                    {
                        let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                        let actual_header = unsafe { *(raw as *const u32) };

                        // Check duplicate flag first
                        let is_dup = (actual_header & 0xFF) != quasar_core::__internal::NOT_BORROWED as u32;

                        // Validate flags for non-duplicates only
                        // (Duplicates don't have their own flags - they point to the original account)
                        if !is_dup {
                            #flag_check
                            #exec_check
                        }

                        // Handle dup
                        if !is_dup {
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
                // No-dup path: optimized u32 comparison for accounts without #[account(dup)]
                let field_name = field.ident.as_ref().unwrap();
                let account_index = buf_offset.to_string();

                // Use constant lookup table for no-dup validation
                let nodup_const = fields::determine_nodup_constant(field, attrs, is_ref_mut);
                let nodup_const_ident = format_ident!("{}", nodup_const);

                // Generate human-readable constraint description
                let constraint_desc = match nodup_const {
                    "NODUP" => "no duplicates",
                    "NODUP_SIGNER" => "signer with no duplicates",
                    "NODUP_MUT" => "writable with no duplicates",
                    "NODUP_MUT_SIGNER" => "writable signer with no duplicates",
                    "NODUP_EXECUTABLE" => "executable program with no duplicates",
                    _ => "unknown constraint",
                };

                // For init_if_needed, mask out signer bit and check writable + nodup
                if attrs.init_if_needed {
                    parse_steps.push(quote! {
                        unsafe {
                            let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                            let header = *(raw as *const u32);

                            // Single constant: 0x000100FF (NOT_BORROWED + writable bit)
                            // Masks out signer and upper writable bits + executable
                            if quasar_core::utils::hint::unlikely((header & 0x000100FF) != 0x000100FF) {
                                #[cfg(feature = "debug")]
                                quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): init_if_needed must be writable with no duplicates"));
                                return Err(quasar_core::decode_header_error(header, #expected_header));
                            }

                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                            input = input.add((input as usize).wrapping_neg() & 7);
                        }
                    });
                } else if nodup_const == "NODUP_SIGNER" {
                    // For NODUP_SIGNER, use u16 comparison (bytes 0-1 only) to allow mut/executable
                    parse_steps.push(quote! {
                        unsafe {
                            let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                            let header = *(raw as *const u32);

                            // u16 comparison: only check borrow_state (byte 0) and is_signer (byte 1)
                            // This allows the account to also be writable or executable
                            if quasar_core::utils::hint::unlikely((header as u16) != (quasar_core::__internal::#nodup_const_ident as u16)) {
                                #[cfg(feature = "debug")]
                                quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must be ", #constraint_desc));
                                return Err(quasar_core::decode_header_error(header, #expected_header));
                            }

                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                            input = input.add((input as usize).wrapping_neg() & 7);
                        }
                    });
                } else {
                    parse_steps.push(quote! {
                        unsafe {
                            let raw = input as *mut quasar_core::__internal::RuntimeAccount;
                            let header = *(raw as *const u32);

                            // Single u32 comparison against compile-time constant (no masking!)
                            if quasar_core::utils::hint::unlikely(header != quasar_core::__internal::#nodup_const_ident) {
                                #[cfg(feature = "debug")]
                                quasar_core::__internal::log_str(concat!("Account '", stringify!(#field_name), "' (index ", #account_index, "): must be ", #constraint_desc));
                                return Err(quasar_core::decode_header_error(header, #expected_header));
                            }

                            core::ptr::write(base.add(#cur_offset), quasar_core::__internal::AccountView::new_unchecked(raw));
                            input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                            input = input.add((input as usize).wrapping_neg() & 7);
                        }
                    });
                }
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
