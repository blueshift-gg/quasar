use {
    crate::helpers::{is_signer_type, pascal_to_snake, seed_slice_expr_for_parse, strip_generics},
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{
        parse::{Parse, ParseStream},
        parse_macro_input, Data, DeriveInput, Expr, ExprArray, Fields, Ident, Token, Type,
    },
};

// --- Account field attribute parsing ---

enum AccountDirective {
    Mut,
    HasOne(Ident, Option<Expr>),
    Constraint(Expr, Option<Expr>),
    Seeds(Vec<Expr>),
    Bump(Option<Expr>),
    Address(Expr, Option<Expr>),
}

impl Parse for AccountDirective {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![mut]) {
            let _: Token![mut] = input.parse()?;
            return Ok(Self::Mut);
        }
        let key: Ident = input.parse()?;
        match key.to_string().as_str() {
            "has_one" => {
                let _: Token![=] = input.parse()?;
                let ident: Ident = input.parse()?;
                let error = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                Ok(Self::HasOne(ident, error))
            }
            "constraint" => {
                let _: Token![=] = input.parse()?;
                let expr: Expr = input.parse()?;
                let error = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                Ok(Self::Constraint(expr, error))
            }
            "address" => {
                let _: Token![=] = input.parse()?;
                let expr: Expr = input.parse()?;
                let error = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;
                    Some(input.parse::<Expr>()?)
                } else {
                    None
                };
                Ok(Self::Address(expr, error))
            }
            "seeds" => {
                let _: Token![=] = input.parse()?;
                let arr: ExprArray = input.parse()?;
                Ok(Self::Seeds(arr.elems.into_iter().collect()))
            }
            "bump" => {
                if input.peek(Token![=]) {
                    let _: Token![=] = input.parse()?;
                    Ok(Self::Bump(Some(input.parse()?)))
                } else {
                    Ok(Self::Bump(None))
                }
            }
            _ => Err(syn::Error::new(
                key.span(),
                format!("unknown account attribute: `{}`", key),
            )),
        }
    }
}

struct AccountFieldAttrs {
    is_mut: bool,
    has_ones: Vec<(Ident, Option<Expr>)>,
    constraints: Vec<(Expr, Option<Expr>)>,
    seeds: Option<Vec<Expr>>,
    bump: Option<Option<Expr>>,
    address: Option<(Expr, Option<Expr>)>,
}

impl Parse for AccountFieldAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let directives = input.parse_terminated(AccountDirective::parse, Token![,])?;
        let mut is_mut = false;
        let mut has_ones = Vec::new();
        let mut constraints = Vec::new();
        let mut seeds = None;
        let mut bump = None;
        let mut address = None;
        for d in directives {
            match d {
                AccountDirective::Mut => is_mut = true,
                AccountDirective::HasOne(ident, err) => has_ones.push((ident, err)),
                AccountDirective::Constraint(expr, err) => constraints.push((expr, err)),
                AccountDirective::Seeds(s) => seeds = Some(s),
                AccountDirective::Bump(b) => bump = Some(b),
                AccountDirective::Address(expr, err) => address = Some((expr, err)),
            }
        }
        Ok(Self {
            is_mut,
            has_ones,
            constraints,
            seeds,
            bump,
            address,
        })
    }
}

fn parse_field_attrs(field: &syn::Field) -> AccountFieldAttrs {
    for attr in &field.attrs {
        if attr.path().is_ident("account") {
            return attr
                .parse_args::<AccountFieldAttrs>()
                .expect("failed to parse #[account(...)] attribute");
        }
    }
    AccountFieldAttrs {
        is_mut: false,
        has_ones: vec![],
        constraints: vec![],
        seeds: None,
        bump: None,
        address: None,
    }
}

fn is_composite_type(ty: &Type) -> bool {
    if matches!(ty, Type::Reference(_)) {
        return false;
    }
    if extract_option_inner(ty).is_some() {
        return false;
    }
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                return args
                    .args
                    .iter()
                    .any(|arg| matches!(arg, syn::GenericArgument::Lifetime(_)));
            }
        }
    }
    false
}

fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            if last.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

// --- Derive Accounts ---

pub(crate) fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let bumps_name = format_ident!("{}Bumps", name);

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Accounts can only be derived for structs with named fields"),
        },
        _ => panic!("Accounts can only be derived for structs"),
    };

    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();

    let field_name_strings: Vec<String> = fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect();

    let mut field_constructs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut has_one_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut constraint_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut mut_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut pda_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_init_vars: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_inits: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seeds_methods: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seed_addr_captures: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields.iter() {
        let attrs = parse_field_attrs(field);
        let field_name = field.ident.as_ref().unwrap();

        let is_optional = extract_option_inner(&field.ty).is_some();
        let effective_ty = extract_option_inner(&field.ty).unwrap_or(&field.ty);
        let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());

        match effective_ty {
            Type::Reference(type_ref) => {
                let base_type = strip_generics(&type_ref.elem);
                let construct_expr = if type_ref.mutability.is_some() {
                    quote! { #base_type::from_account_view_mut(#field_name)? }
                } else {
                    quote! { #base_type::from_account_view(#field_name)? }
                };
                if is_optional {
                    field_constructs.push(quote! { #field_name: if *#field_name.address() == crate::ID { None } else { Some(#construct_expr) } });
                } else {
                    field_constructs.push(quote! { #field_name: #construct_expr });
                }
            }
            _ => {
                let base_type = strip_generics(effective_ty);
                if is_optional {
                    field_constructs.push(quote! { #field_name: if *#field_name.address() == crate::ID { None } else { Some(#base_type::from_account_view(#field_name)?) } });
                } else {
                    field_constructs
                        .push(quote! { #field_name: #base_type::from_account_view(#field_name)? });
                }
            }
        }

        if attrs.is_mut && !is_ref_mut {
            let check = quote! {
                if !#field_name.to_account_view().is_writable() {
                    return Err(ProgramError::Immutable);
                }
            };
            if is_optional {
                mut_checks.push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                mut_checks.push(check);
            }
        }

        for (target, custom_error) in &attrs.has_ones {
            let error = match custom_error {
                Some(err) => quote! { #err.into() },
                None => quote! { QuasarError::HasOneMismatch.into() },
            };
            let check = quote! {
                if #field_name.#target != *#target.to_account_view().address() {
                    return Err(#error);
                }
            };
            if is_optional {
                has_one_checks
                    .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                has_one_checks.push(check);
            }
        }

        for (expr, custom_error) in &attrs.constraints {
            let error = match custom_error {
                Some(err) => quote! { #err.into() },
                None => quote! { QuasarError::ConstraintViolation.into() },
            };
            let check = quote! {
                if !(#expr) {
                    return Err(#error);
                }
            };
            if is_optional {
                constraint_checks
                    .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                constraint_checks.push(check);
            }
        }

        if let Some((addr_expr, custom_error)) = &attrs.address {
            let error = match custom_error {
                Some(err) => quote! { #err.into() },
                None => quote! { QuasarError::AddressMismatch.into() },
            };
            let check = quote! {
                if *#field_name.to_account_view().address() != #addr_expr {
                    return Err(#error);
                }
            };
            if is_optional {
                constraint_checks
                    .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                constraint_checks.push(check);
            }
        }

        if let Some(ref seed_exprs) = attrs.seeds {
            let bump_var = format_ident!("__bumps_{}", field_name);

            bump_init_vars.push(quote! { let mut #bump_var: u8 = 0; });
            bump_struct_fields.push(quote! { pub #field_name: u8 });
            bump_struct_inits.push(quote! { #field_name: #bump_var });

            let bump_arr_field = format_ident!("__{}_bump", field_name);
            bump_struct_fields.push(quote! { #bump_arr_field: [u8; 1] });
            bump_struct_inits.push(quote! { #bump_arr_field: [#bump_var] });

            let seed_slices: Vec<proc_macro2::TokenStream> = seed_exprs
                .iter()
                .map(|expr| seed_slice_expr_for_parse(expr, &field_name_strings))
                .collect();

            let seed_idents: Vec<Ident> = seed_slices
                .iter()
                .enumerate()
                .map(|(idx, _)| format_ident!("__seed_{}_{}", field_name, idx))
                .collect();

            let seed_len_checks: Vec<proc_macro2::TokenStream> = seed_idents
                .iter()
                .zip(seed_slices.iter())
                .map(|(ident, seed)| {
                    quote! {
                        let #ident: &[u8] = #seed;
                        if #ident.len() > 32 {
                            return Err(QuasarError::InvalidSeeds.into());
                        }
                    }
                })
                .collect();

            match &attrs.bump {
                Some(Some(bump_expr)) => {
                    pda_checks.push(quote! {
                        {
                            #(#seed_len_checks)*
                            let __bump_val: u8 = #bump_expr;
                            let __bump_ref: &[u8] = &[__bump_val];
                            let __pda_seeds = [#(quasar_core::cpi::Seed::from(#seed_idents),)* quasar_core::cpi::Seed::from(__bump_ref)];
                            let __expected = quasar_core::pda::create_program_address(&__pda_seeds, &crate::ID)?;
                            if *#field_name.to_account_view().address() != __expected {
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump_val;
                        }
                    });
                }
                Some(None) => {
                    pda_checks.push(quote! {
                        {
                            #(#seed_len_checks)*
                            let __pda_seeds = [#(quasar_core::cpi::Seed::from(#seed_idents)),*];
                            let (__expected, __bump) = quasar_core::pda::find_program_address(&__pda_seeds, &crate::ID);
                            if *#field_name.to_account_view().address() != __expected {
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump;
                        }
                    });
                }
                None => {
                    panic!(
                        "#[account(seeds = [...])] requires a `bump` or `bump = expr` directive"
                    );
                }
            }

            let method_name = format_ident!("{}_seeds", field_name);
            let seed_count = seed_exprs.len() + 1;
            let mut seed_elements: Vec<proc_macro2::TokenStream> = Vec::new();

            for expr in seed_exprs {
                if let Expr::Path(ep) = expr {
                    if ep.qself.is_none() && ep.path.segments.len() == 1 {
                        let ident = &ep.path.segments[0].ident;
                        if field_name_strings.contains(&ident.to_string()) {
                            let addr_field = format_ident!("__seed_{}_{}", field_name, ident);
                            let capture_var = format_ident!("__seed_addr_{}_{}", field_name, ident);

                            seed_addr_captures.push(quote! {
                                let #capture_var = *#ident.address();
                            });
                            bump_struct_fields.push(quote! { #addr_field: Address });
                            bump_struct_inits.push(quote! { #addr_field: #capture_var });

                            seed_elements.push(
                                quote! { quasar_core::cpi::Seed::from(self.#addr_field.as_ref()) },
                            );
                            continue;
                        }
                    }
                }
                seed_elements.push(quote! { quasar_core::cpi::Seed::from((#expr) as &[u8]) });
            }

            seed_elements
                .push(quote! { quasar_core::cpi::Seed::from(&self.#bump_arr_field as &[u8]) });

            seeds_methods.push(quote! {
                #[inline(always)]
                pub fn #method_name(&self) -> [quasar_core::cpi::Seed<'_>; #seed_count] {
                    [#(#seed_elements),*]
                }
            });
        }
    }

    let field_attrs: Vec<AccountFieldAttrs> = fields.iter().map(parse_field_attrs).collect();

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
                            let addr = input as usize;
                            input = ((addr + 7) & !7) as *mut u8;
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

    let has_pda_fields = !bump_struct_fields.is_empty();

    let bumps_struct = if has_pda_fields {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name { #(#bump_struct_fields,)* } }
    } else {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name; }
    };

    let bumps_init = if has_pda_fields {
        quote! { #bumps_name { #(#bump_struct_inits,)* } }
    } else {
        quote! { #bumps_name }
    };

    let has_any_checks = !has_one_checks.is_empty()
        || !constraint_checks.is_empty()
        || !mut_checks.is_empty()
        || !pda_checks.is_empty();

    let parse_body = if has_composites {
        let mut field_lets: Vec<proc_macro2::TokenStream> = Vec::new();
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
                bump_struct_fields
                    .push(quote! { pub #field_name: <#inner_ty as ParseAccounts>::Bumps });
                bump_struct_inits.push(quote! { #field_name: #bumps_var });
                idx_offset = quote! { #idx_offset + <#inner_ty as AccountCount>::COUNT };
            } else {
                let cur_offset = idx_offset.clone();
                field_lets.push(quote! {
                    let #field_name = &accounts[#cur_offset];
                });
                idx_offset = quote! { #idx_offset + 1usize };
            }
        }

        let non_composite_constructs: Vec<proc_macro2::TokenStream> = fields
            .iter()
            .enumerate()
            .map(|(fi, field)| {
                let field_name = field.ident.as_ref().unwrap();
                if composite_types[fi].is_some() {
                    quote! { #field_name }
                } else {
                    field_constructs[fi].clone()
                }
            })
            .collect();

        if has_any_checks {
            quote! {
                if accounts.len() < Self::COUNT {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                #(#field_lets)*
                #(#seed_addr_captures)*

                let result = Self {
                    #(#non_composite_constructs,)*
                };

                #(#bump_init_vars)*

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

            let result = Self {
                #(#field_constructs,)*
            };

            #(#bump_init_vars)*

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

    let seeds_impl = if seeds_methods.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #bumps_name {
                #(#seeds_methods)*
            }
        }
    };

    // --- Client instruction macro (off-chain only) ---
    // Generates a #[macro_export] macro that the #[program] macro invokes
    // to produce flat instruction structs with account + arg fields.
    let snake_name = pascal_to_snake(&name.to_string());
    let macro_name_str = format!("__{}_instruction", snake_name);

    let account_fields_str: String = fields
        .iter()
        .map(|f| {
            let field_name = f.ident.as_ref().unwrap().to_string();
            format!("pub {}: solana_address::Address,", field_name)
        })
        .collect::<Vec<_>>()
        .join("\n                ");

    let account_metas_str: String = fields
        .iter()
        .enumerate()
        .map(|(fi, f)| {
            let field_name = f.ident.as_ref().unwrap().to_string();
            let writable = field_attrs[fi].is_mut
                || matches!(&f.ty, Type::Reference(r) if r.mutability.is_some());
            let signer = is_signer_type(&f.ty);
            if writable {
                format!(
                    "quasar_core::client::AccountMeta::new(ix.{}, {}),",
                    field_name, signer
                )
            } else {
                format!(
                    "quasar_core::client::AccountMeta::new_readonly(ix.{}, {}),",
                    field_name, signer
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n                        ");

    let macro_def_str = format!(
        r#"
        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        #[doc(hidden)]
        #[macro_export]
        macro_rules! {macro_name} {{
            ($struct_name:ident, [$($disc:expr),*], {{$($arg_name:ident : $arg_ty:ty),*}}) => {{
                pub struct $struct_name {{
                    {account_fields}
                    $(pub $arg_name: $arg_ty,)*
                }}

                impl From<$struct_name> for quasar_core::client::Instruction {{
                    fn from(ix: $struct_name) -> quasar_core::client::Instruction {{
                        let accounts = vec![
                            {account_metas}
                        ];
                        let data = quasar_core::client::build_instruction_data(
                            &[$($disc),*],
                            |_data| {{ $(quasar_core::client::WriteBytes::write_bytes(&ix.$arg_name, _data);)* }}
                        );
                        quasar_core::client::Instruction {{
                            program_id: $crate::ID,
                            accounts,
                            data,
                        }}
                    }}
                }}
            }};
        }}
        "#,
        macro_name = macro_name_str,
        account_fields = account_fields_str,
        account_metas = account_metas_str,
    );

    let client_macro: proc_macro2::TokenStream = macro_def_str
        .parse()
        .expect("failed to parse client instruction macro");

    let expanded = quote! {
        #bumps_struct

        impl<'info> ParseAccounts<'info> for #name<'info> {
            type Bumps = #bumps_name;

            #[inline(always)]
            fn parse(accounts: &'info [AccountView]) -> Result<(Self, Self::Bumps), ProgramError> {
                #parse_body
            }
        }

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
