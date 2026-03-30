//! `#[derive(Accounts)]` — generates account parsing, validation, and PDA
//! derivation from a struct definition. This is the core macro that transforms
//! a declarative accounts struct into the zero-copy parsing pipeline.

mod attrs;
mod client;
mod field_kind;
mod fields;
mod init;

use {
    crate::helpers::{
        classify_dynamic_string, classify_dynamic_vec, classify_tail, extract_generic_inner_type,
        is_composite_type, strip_generics, DynKind,
    },
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse::ParseStream, parse_macro_input, Data, DeriveInput, Fields, Ident, Token, Type},
};

pub(super) struct InstructionArg {
    pub name: Ident,
    pub ty: Type,
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

    let instruction_args = parse_struct_instruction_args(&input);

    let mut pf = match fields::process_fields(fields, &field_name_strings, &instruction_args) {
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
    // Track buffer offset as a plain integer for non-composite structs (emits
    // clean literals like `3usize` instead of `0usize + 1usize + 1usize + 1usize`,
    // which avoids clippy::int_plus_one in generated code).
    // For composite structs, fall back to expression trees since composite
    // account counts aren't known at macro expansion time.
    let mut buf_offset_num: usize = 0;
    let mut buf_offset_expr: Option<proc_macro2::TokenStream> = if has_composites {
        Some(quote! { 0usize })
    } else {
        None
    };

    for (fi, ct) in composite_types.iter().enumerate() {
        if let Some(inner_ty) = ct {
            // Composite type - recursively call parse_accounts
            // (each inner type knows its own dup policy from its #[account(dup)] attribute)
            let cur_offset = buf_offset_expr.clone().expect("buf_offset_expr must be set before processing composite types");

            parse_steps.push(quote! {
                {
                    let mut __inner_buf = core::mem::MaybeUninit::<
                        [quasar_lang::__internal::AccountView; <#inner_ty as AccountCount>::COUNT]
                    >::uninit();
                    input = <#inner_ty>::parse_accounts(input, &mut __inner_buf, __program_id)?;
                    let __inner = unsafe { __inner_buf.assume_init() };
                    let mut __j = 0usize;
                    while __j < <#inner_ty as AccountCount>::COUNT {
                        unsafe { core::ptr::write(base.add(#cur_offset + __j), *__inner.as_ptr().add(__j)); }
                        __j += 1;
                    }
                }
            });

            buf_offset_expr = Some(quote! { #cur_offset + <#inner_ty as AccountCount>::COUNT });
        } else {
            let cur_offset = if let Some(ref expr) = buf_offset_expr {
                expr.clone()
            } else {
                quote! { #buf_offset_num }
            };
            let attrs = &pf.field_attrs[fi];
            let field = &fields[fi];

            if attrs.dup && buf_offset_num == 0 && buf_offset_expr.is_none() {
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
            let field_name = field.ident.as_ref().expect("account field must have an identifier");
            let account_index = if let Some(ref expr) = buf_offset_expr {
                expr.to_string()
            } else {
                buf_offset_num.to_string()
            };

            if is_optional || attrs.dup {
                // Dup-aware path: single masked u32 comparison replaces 2-3 separate flag
                // checks. For optional accounts: sentinel guard wraps ALL
                // checks (address == program_id means None, skip validation
                // entirely).
                let flag_mask: u32 = field_kind::FLAG_MASK;
                let expected_masked = expected_header & flag_mask;
                let flag_check = quote! {
                    if quasar_lang::utils::hint::unlikely((actual_header & #flag_mask) != #expected_masked) {
                        #[cfg(feature = "debug")]
                        quasar_lang::__internal::log_str(concat!(
                            "Account '", stringify!(#field_name),
                            "' (index ", #account_index, "): header flags mismatch"
                        ));
                        return Err(ProgramError::from(quasar_lang::decode_header_error(actual_header, #expected_header)));
                    }
                };

                // For optional: sentinel guard wraps ALL checks.
                // Use keys_eq for consistency — word-wise u64 comparison.
                let guarded_checks = if is_optional {
                    quote! {
                        if !quasar_lang::keys_eq(unsafe { &(*raw).address }, __program_id) {
                            #flag_check
                        }
                    }
                } else {
                    flag_check
                };

                parse_steps.push(quote! {
                    {
                        let raw = input as *mut quasar_lang::__internal::RuntimeAccount;
                        let actual_header = unsafe { *(raw as *const u32) };

                        if (actual_header & 0xFF) == quasar_lang::__internal::NOT_BORROWED as u32 {
                            #guarded_checks
                            unsafe {
                                core::ptr::write(base.add(#cur_offset), quasar_lang::__internal::AccountView::new_unchecked(raw));
                                input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                                input = input.add((input as usize).wrapping_neg() & 7);
                            }
                        } else {
                            // Security: bounds-check the dup index before using it to read
                            // from the AccountView buffer.
                            let idx = (actual_header & 0xFF) as usize;
                            if quasar_lang::utils::hint::unlikely(idx >= #cur_offset) {
                                return Err(ProgramError::InvalidAccountData);
                            }
                            unsafe {
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
                        quote! { (header as u16) != (quasar_lang::__internal::#nodup_const_ident as u16) },
                        "must be signer, no duplicates",
                    )
                } else {
                    (
                        quote! { header != quasar_lang::__internal::#nodup_const_ident },
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
                        let raw = input as *mut quasar_lang::__internal::RuntimeAccount;
                        let header = *(raw as *const u32);

                        if quasar_lang::utils::hint::unlikely(#check_cond) {
                            #[cfg(feature = "debug")]
                            quasar_lang::__internal::log_str(concat!(
                                "Account '", stringify!(#field_name),
                                "' (index ", #account_index, "): ", #debug_msg
                            ));
                            return Err(ProgramError::from(quasar_lang::decode_header_error(header, #expected_header)));
                        }

                        core::ptr::write(base.add(#cur_offset), quasar_lang::__internal::AccountView::new_unchecked(raw));
                        input = input.add(__ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
                        input = input.add((input as usize).wrapping_neg() & 7);
                    }
                });
            }

            buf_offset_num += 1;
            if let Some(ref expr) = buf_offset_expr {
                buf_offset_expr = Some(quote! { #expr + 1usize });
            }
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
            let field_name = field.ident.as_ref().expect("account field must have an identifier");
            if composite_types[fi].is_some() {
                let inner_ty = composite_types[fi].as_ref().expect("composite type must be Some when is_some() returned true");
                let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                field_lets.push(quote! {
                    // SAFETY: dispatch! guarantees the total slice has COUNT
                    // elements; each split_at_mut_unchecked carves off exactly
                    // the inner type's COUNT.
                    let (__chunk, __rest) = unsafe {
                        __accounts_rest.split_at_mut_unchecked(<#inner_ty as AccountCount>::COUNT)
                    };
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
                    // SAFETY: dispatch! guarantees sufficient elements remain.
                    let (__chunk, __rest) = unsafe { __accounts_rest.split_at_mut_unchecked(1) };
                    __accounts_rest = __rest;
                    let #field_name = unsafe { __chunk.get_unchecked_mut(0) };
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
                let field_name = field.ident.as_ref().expect("account field must have an identifier");
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

    let has_any_checks =
        !pf.field_checks.is_empty() || !pf.init_pda_checks.is_empty() || !pf.init_blocks.is_empty();

    let seed_addr_captures = &pf.seed_addr_captures;
    let bump_init_vars = &pf.bump_init_vars;
    let field_checks = &pf.field_checks;
    let field_constructs = &pf.field_constructs;
    let init_pda_checks = &pf.init_pda_checks;
    let init_blocks = &pf.init_blocks;

    let rent_fetch = if pf.needs_rent {
        if let Some(ref rent_field) = pf.rent_sysvar_field {
            // Read Rent from the Sysvar<Rent> account — avoids sol_get_rent_sysvar syscall.
            // SAFETY: At this point #rent_field is &mut AccountView. borrow_unchecked
            // returns the account data; from_bytes_unchecked casts it to &Rent.
            // The address is validated later in the normal check phase.
            quote! {
                let __shared_rent = unsafe {
                    core::clone::Clone::clone(
                        <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::from_bytes_unchecked(
                            #rent_field.borrow_unchecked()
                        )
                    )
                };
            }
        } else {
            quote! { let __shared_rent = <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::get()?; }
        }
    } else {
        quote! {}
    };

    // SAFETY: `dispatch!` in the entrypoint verifies `__num_accounts >= COUNT`
    // and creates an `[AccountView; COUNT]` buffer before calling `parse()`.
    // The slice always has exactly `COUNT` elements, so length checks and
    // pattern-match fallbacks are unreachable.
    let parse_body = if has_composites {
        if has_any_checks {
            quote! {
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
                    #(#field_checks)*
                }

                Ok((result, #bumps_init))
            }
        } else {
            quote! {
                #(#field_lets)*

                Ok((Self {
                    #(#non_composite_constructs,)*
                }, #bumps_init))
            }
        }
    } else if has_any_checks {
        quote! {
            // SAFETY: dispatch! guarantees accounts.len() == Self::COUNT.
            let [#(#field_names),*] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
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
                #(#field_checks)*
            }

            Ok((result, #bumps_init))
        }
    } else {
        quote! {
            // SAFETY: dispatch! guarantees accounts.len() == Self::COUNT.
            let [#(#field_names),*] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
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

    let has_epilogue = !pf.sweep_fields.is_empty() || !pf.close_fields.is_empty();
    let epilogue_method = if has_epilogue {
        // Sweep stmts run FIRST — transfer remaining tokens out.
        let sweep_stmts: Vec<proc_macro2::TokenStream> = pf
            .sweep_fields
            .iter()
            .map(|info| {
                let field = &info.field;
                let receiver = &info.receiver;
                let mint = &info.mint;
                let auth = &info.authority;
                let tp = &info.token_program;
                quote! {
                    {
                        use quasar_spl::TokenCpi as _;
                        let __sweep_amount = self.#field.amount();
                        if __sweep_amount > 0 {
                            let __sweep_decimals = self.#mint.decimals();
                            self.#tp.transfer_checked(
                                self.#field,
                                self.#mint,
                                self.#receiver,
                                self.#auth,
                                __sweep_amount,
                                __sweep_decimals,
                            ).invoke()?;
                        }
                    }
                }
            })
            .collect();

        // Close stmts run AFTER sweeps.
        let close_stmts: Vec<proc_macro2::TokenStream> = pf
            .close_fields
            .iter()
            .map(|info| {
                let field = &info.field;
                let dest = &info.destination;
                if let Some(cpi) = &info.cpi_close {
                    // Token/mint: CPI close via the token program.
                    let tp = &cpi.token_program;
                    let auth = &cpi.authority;
                    quote! {
                        {
                            use quasar_spl::TokenClose as _;
                            self.#field.close(self.#tp, self.#dest, self.#auth).invoke()?;
                        }
                    }
                } else {
                    // Framework close: zero + drain + reassign.
                    quote! { self.#field.close(self.#dest.to_account_view())?; }
                }
            })
            .collect();

        quote! {
            #[inline(always)]
            fn epilogue(&mut self) -> Result<(), ProgramError> {
                #(#sweep_stmts)*
                #(#close_stmts)*
                Ok(())
            }
        }
    } else {
        quote! {}
    };

    // --- Instruction arg extraction (struct-level #[instruction(...)]) ---

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
                    debug_assert_eq!(accounts.len(), Self::COUNT, "parse() must be called with exactly COUNT elements");
                    Self::parse_with_instruction_data(accounts, &[], program_id)
                }

                #[inline(always)]
                fn parse_with_instruction_data(
                    accounts: &'info mut [AccountView],
                    __ix_data: &'info [u8],
                    __program_id: &Address,
                ) -> Result<(Self, Self::Bumps), ProgramError> {
                    debug_assert_eq!(accounts.len(), Self::COUNT, "parse() must be called with exactly COUNT elements");
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
                    debug_assert_eq!(accounts.len(), Self::COUNT, "parse() must be called with exactly COUNT elements");
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
                buf: &mut core::mem::MaybeUninit<[quasar_lang::__internal::AccountView; #count_expr]>,
                __program_id: &quasar_lang::prelude::Address,
            ) -> Result<*mut u8, ProgramError> {
                const __ACCOUNT_HEADER: usize =
                    core::mem::size_of::<quasar_lang::__internal::RuntimeAccount>()
                    + quasar_lang::__internal::MAX_PERMITTED_DATA_INCREASE
                    + core::mem::size_of::<u64>();

                let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;

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

    // ZC struct with ONLY fixed fields, using InstructionArg::Zc
    if has_fixed {
        let mut zc_field_names: Vec<Ident> = Vec::new();
        let mut zc_field_types: Vec<proc_macro2::TokenStream> = Vec::new();
        let mut zc_field_orig_types: Vec<Type> = Vec::new();

        for (i, kind) in kinds.iter().enumerate() {
            if matches!(kind, DynKind::Fixed) {
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

        // Extract fixed fields via InstructionArg::from_zc
        let mut zc_idx = 0usize;
        for (i, kind) in kinds.iter().enumerate() {
            if matches!(kind, DynKind::Fixed) {
                let name = &ix_args[i].name;
                let ty = &zc_field_orig_types[zc_idx];
                zc_idx += 1;
                stmts.push(quote! {
                    let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ix_zc.#name);
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
