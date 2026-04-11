//! `#[derive(Accounts)]` — generates account parsing, validation, and PDA
//! derivation from a struct definition. This is the core macro that transforms
//! a declarative accounts struct into the zero-copy parsing pipeline.

mod attrs;
mod client;
mod field_kind;
mod fields;
mod init;
mod instruction_args;

pub(crate) use instruction_args::InstructionArg;
use {
    crate::helpers::{extract_generic_inner_type, is_composite_type, strip_generics},
    instruction_args::{generate_instruction_arg_extraction, parse_struct_instruction_args},
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Type},
};

pub(crate) fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let bumps_name = format_ident!("{}Bumps", name);

    // Currently only custom lifetime parameters are supported, so validate that
    // we don't have any type or const generics.
    if let Some(param) = input
        .generics
        .params
        .iter()
        .find(|param| !matches!(param, GenericParam::Lifetime(_)))
    {
        let message = match param {
            GenericParam::Type(_) => {
                "#[derive(Accounts)] only supports lifetime parameters; type parameters are not \
                 supported"
            }
            GenericParam::Const(_) => {
                "#[derive(Accounts)] only supports lifetime parameters; const parameters are not \
                 supported"
            }
            // Filtered by the `find` predicate above — lifetimes are skipped.
            GenericParam::Lifetime(_) => "",
        };
        return syn::Error::new_spanned(param, message)
            .to_compile_error()
            .into();
    }
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut parse_generics = input.generics.clone();
    // 'input is the default lifetime used for account references in the generated
    // traits, so we need to make sure that it lives longer than any
    // user-defined lifetimes.
    parse_generics.params.push(parse_quote!('input));
    {
        let parse_where = parse_generics.make_where_clause();
        for lifetime in input.generics.lifetimes() {
            let lifetime = &lifetime.lifetime;
            parse_where
                .predicates
                .push(syn::parse_quote!('input: #lifetime));
        }
    }
    // These generics are used for the ParseAccounts impl, which may have different
    // lifetime requirements than the original struct.
    let (parse_impl_generics, _, parse_where_clause) = parse_generics.split_for_impl();

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

    let mut pf =
        match fields::process_fields(fields, &field_name_strings, &instruction_args, &bumps_name) {
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

    // --- Generate parse_steps ---

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
            let cur_offset = buf_offset_expr
                .clone()
                .expect("buf_offset_expr must be set before processing composite types");

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
            let field_name = field
                .ident
                .as_ref()
                .expect("account field must have an identifier");
            let account_index = if let Some(ref expr) = buf_offset_expr {
                expr.to_string()
            } else {
                buf_offset_num.to_string()
            };

            if is_optional || attrs.dup {
                // Dup-aware path: handles optional sentinel guards and
                // duplicate accounts (same key in multiple slots).
                //
                // Uses mask-based minimum-requirements flag checks so that
                // extra permissions are silently accepted.
                let effective_ty =
                    extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
                let stripped = field_kind::strip_ref(effective_ty);
                let kind = field_kind::FieldKind::classify(stripped);
                let flags = field_kind::FieldFlags::compute(&kind, attrs, is_ref_mut);
                let req_flag_mask: u32 = flags.required_flag_mask();
                let expected_flag_value: u32 = expected_header & req_flag_mask;
                let required_mask: u32 = flags.required_mask();

                let flag_check = quote! {
                    if quasar_lang::utils::hint::unlikely((actual_header & #req_flag_mask) != #expected_flag_value) {
                        #[cfg(feature = "debug")]
                        quasar_lang::prelude::log(concat!(
                            "Account '", stringify!(#field_name),
                            "' (index ", #account_index, "): missing required flags"
                        ));
                        return Err(ProgramError::from(quasar_lang::decode_header_error(actual_header, #expected_header, #required_mask)));
                    }
                };

                // For optional: sentinel guard wraps ALL checks.
                let guarded_checks = if is_optional {
                    quote! {
                        if !quasar_lang::keys_eq(unsafe { &(*raw).address }, __program_id) {
                            #flag_check
                        }
                    }
                } else {
                    flag_check
                };

                // Dup branch: when borrow_state != NOT_BORROWED, the SVM
                // has deduplicated this account slot.
                //
                // Mutable dups without #[account(dup)] are rejected to
                // prevent unguarded aliased writes.
                //
                // Borrow-state tracking engages AccountView's RefCell-style
                // counter so try_borrow/try_borrow_mut detect conflicts.
                let dup_branch = if is_ref_mut && !attrs.dup {
                    quote! {
                        #[cfg(feature = "debug")]
                        quasar_lang::prelude::log(concat!(
                            "Account '", stringify!(#field_name),
                            "' (index ", #account_index,
                            "): duplicate mutable account rejected — use \
                             #[account(dup)] to opt in"
                        ));
                        return Err(ProgramError::AccountBorrowFailed);
                    }
                } else {
                    let borrow_state_update = if is_ref_mut {
                        // Mutable dup: claim exclusive access.
                        quote! {
                            let __orig_view = core::ptr::read(base.add(idx));
                            let __bs_ptr = __orig_view.account_ptr() as *mut u8;
                            let __bs = *__bs_ptr;
                            if quasar_lang::utils::hint::unlikely(
                                __bs != quasar_lang::__internal::NOT_BORROWED
                            ) {
                                #[cfg(feature = "debug")]
                                quasar_lang::prelude::log(concat!(
                                    "Account '", stringify!(#field_name),
                                    "' (index ", #account_index,
                                    "): mutable dup failed — original already borrowed"
                                ));
                                return Err(ProgramError::AccountBorrowFailed);
                            }
                            *__bs_ptr = 0;
                        }
                    } else {
                        // Immutable dup: consume one immutable borrow slot.
                        quote! {
                            let __orig_view = core::ptr::read(base.add(idx));
                            let __bs_ptr = __orig_view.account_ptr() as *mut u8;
                            let __bs = *__bs_ptr;
                            if quasar_lang::utils::hint::unlikely(__bs <= 1) {
                                #[cfg(feature = "debug")]
                                quasar_lang::prelude::log(concat!(
                                    "Account '", stringify!(#field_name),
                                    "' (index ", #account_index,
                                    "): immutable dup failed — no borrows remaining"
                                ));
                                return Err(ProgramError::AccountBorrowFailed);
                            }
                            *__bs_ptr = __bs - 1;
                        }
                    };

                    quote! {
                        let idx = (actual_header & 0xFF) as usize;
                        if quasar_lang::utils::hint::unlikely(idx >= #cur_offset) {
                            return Err(ProgramError::InvalidAccountData);
                        }
                        unsafe {
                            #borrow_state_update
                        }
                        #[cfg(feature = "debug")]
                        quasar_lang::prelude::log(concat!(
                            "Account '", stringify!(#field_name),
                            "' (index ", #account_index,
                            "): duplicate account — reusing view from original slot"
                        ));
                        unsafe {
                            core::ptr::write(base.add(#cur_offset), core::ptr::read(base.add(idx)));
                            input = input.add(core::mem::size_of::<u64>());
                        }
                    }
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
                            #dup_branch
                        }
                    }
                });
            } else {
                // Nodup path: single u32 comparison on the hot path.
                //
                // When the header matches exactly, falls straight through to
                // parse with zero overhead. On mismatch (unlikely), the cold
                // decode_header_error does a mask-based minimum-requirements
                // check and returns 0 for acceptable extra flags (e.g. a
                // signer passed to a non-signer field). Dups are rejected
                // with AccountBorrowFailed.
                let flags = fields::compute_field_flags(field, attrs, is_ref_mut);
                let expected = flags.header_constant();
                let mask = flags.required_mask();

                parse_steps.push(quote! {
                    unsafe {
                        let raw = input as *mut quasar_lang::__internal::RuntimeAccount;
                        let header = *(raw as *const u32);

                        if quasar_lang::utils::hint::unlikely(header != #expected) {
                            let __err = quasar_lang::decode_header_error(header, #expected, #mask);
                            if __err != 0 {
                                #[cfg(feature = "debug")]
                                quasar_lang::prelude::log(concat!(
                                    "Account '", stringify!(#field_name),
                                    "' (index ", #account_index, "): validation failed"
                                ));
                                return Err(ProgramError::from(__err));
                            }
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
            let field_name = field
                .ident
                .as_ref()
                .expect("account field must have an identifier");
            if composite_types[fi].is_some() {
                let inner_ty = composite_types[fi]
                    .as_ref()
                    .expect("composite type must be Some when is_some() returned true");
                let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                field_lets.push(quote! {
                    // SAFETY: dispatch! guarantees the total slice has COUNT
                    // elements; each split_at_mut_unchecked carves off exactly
                    // the inner type's COUNT.
                    let (__chunk, __rest) = unsafe {
                        __accounts_rest.split_at_mut_unchecked(<#inner_ty as AccountCount>::COUNT)
                    };
                    __accounts_rest = __rest;
                    let (#field_name, #bumps_var) = unsafe { <#inner_ty as quasar_lang::traits::ParseAccountsUnchecked>::parse_unchecked(
                        __chunk,
                        __program_id
                    ) }?;
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
                let field_name = field
                    .ident
                    .as_ref()
                    .expect("account field must have an identifier");
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
            // Extract just the two u64 values we need instead of carrying the struct.
            // SAFETY: At this point #rent_field is &mut AccountView. borrow_unchecked
            // returns the account data; from_bytes_unchecked casts it to &Rent.
            // The address is validated later in the normal check phase.
            quote! {
                let (__rent_lpb, __rent_threshold): (u64, u64) = {
                    let __r = unsafe {
                        <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::from_bytes_unchecked(
                            #rent_field.borrow_unchecked()
                        )
                    };
                    (__r.lamports_per_byte(), __r.exemption_threshold_raw())
                };
            }
        } else {
            quote! {
                let (__rent_lpb, __rent_threshold): (u64, u64) = {
                    let __r = <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::get()?;
                    (__r.lamports_per_byte(), __r.exemption_threshold_raw())
                };
            }
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
            impl #impl_generics #name #ty_generics #where_clause {
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
                                &self.#field,
                                &self.#mint,
                                &self.#receiver,
                                &self.#auth,
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
                            use quasar_spl::TokenCpi as _;
                            self.#tp.close_account(&self.#field, &self.#dest, &self.#auth).invoke()?;
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

    let exact_len_guard = quote! {
        quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
    };

    let parse_accounts_impl = if has_instruction_args {
        quote! {
            impl #parse_impl_generics ParseAccounts<'input> for #name #ty_generics #parse_where_clause {
                type Bumps = #bumps_name;

                #[inline(always)]
                fn parse(accounts: &'input mut [AccountView], program_id: &Address) -> Result<(Self, Self::Bumps), ProgramError> {
                    #exact_len_guard
                    unsafe {
                        <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                            accounts,
                            &[],
                            program_id,
                        )
                    }
                }

                #[inline(always)]
                fn parse_with_instruction_data(
                    accounts: &'input mut [AccountView],
                    __ix_data: &[u8],
                    __program_id: &Address,
                ) -> Result<(Self, Self::Bumps), ProgramError> {
                    #exact_len_guard
                    unsafe {
                        <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                            accounts,
                            __ix_data,
                            __program_id,
                        )
                    }
                }

                #epilogue_method
            }

            unsafe impl #parse_impl_generics quasar_lang::traits::ParseAccountsUnchecked<'input>
                for #name #ty_generics
                #parse_where_clause
            {
                #[inline(always)]
                unsafe fn parse_unchecked(accounts: &'input mut [AccountView], program_id: &Address) -> Result<(Self, Self::Bumps), ProgramError> {
                    <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                        accounts,
                        &[],
                        program_id,
                    )
                }

                #[inline(always)]
                unsafe fn parse_with_instruction_data_unchecked(
                    accounts: &'input mut [AccountView],
                    __ix_data: &[u8],
                    __program_id: &Address,
                ) -> Result<(Self, Self::Bumps), ProgramError> {
                    #ix_arg_extraction
                    #parse_body
                }
            }
        }
    } else {
        quote! {
            impl #parse_impl_generics ParseAccounts<'input> for #name #ty_generics #parse_where_clause {
                type Bumps = #bumps_name;

                #[inline(always)]
                fn parse(accounts: &'input mut [AccountView], __program_id: &Address) -> Result<(Self, Self::Bumps), ProgramError> {
                    #exact_len_guard
                    unsafe {
                        <Self as quasar_lang::traits::ParseAccountsUnchecked>::parse_unchecked(
                            accounts,
                            __program_id,
                        )
                    }
                }

                #epilogue_method
            }

            unsafe impl #parse_impl_generics quasar_lang::traits::ParseAccountsUnchecked<'input>
                for #name #ty_generics
                #parse_where_clause
            {
                #[inline(always)]
                unsafe fn parse_unchecked(
                    accounts: &'input mut [AccountView],
                    __program_id: &Address,
                ) -> Result<(Self, Self::Bumps), ProgramError> {
                    #parse_body
                }
            }
        }
    };

    let expanded = quote! {
        #bumps_struct

        #parse_accounts_impl

        #seeds_impl

        impl #impl_generics AccountCount for #name #ty_generics #where_clause {
            const COUNT: usize = #count_expr;
        }

        impl #impl_generics #name #ty_generics #where_clause {
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
