use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Expr, ExprArray, FnArg, Fields, Ident, Item, ItemFn, ItemMod, LitInt, Pat, Token, Type,
};

// --- Account field attribute parsing ---

enum AccountDirective {
    HasOne(Ident),
    Constraint(Expr),
    Seeds(Vec<Expr>),
    Bump(Option<Expr>),
}

impl Parse for AccountDirective {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: Ident = input.parse()?;
        match key.to_string().as_str() {
            "has_one" => {
                let _: Token![=] = input.parse()?;
                Ok(Self::HasOne(input.parse()?))
            }
            "constraint" => {
                let _: Token![=] = input.parse()?;
                Ok(Self::Constraint(input.parse()?))
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
    has_ones: Vec<Ident>,
    constraints: Vec<Expr>,
    seeds: Option<Vec<Expr>>,
    bump: Option<Option<Expr>>,
}

impl Parse for AccountFieldAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let directives =
            input.parse_terminated(AccountDirective::parse, Token![,])?;
        let mut has_ones = Vec::new();
        let mut constraints = Vec::new();
        let mut seeds = None;
        let mut bump = None;
        for d in directives {
            match d {
                AccountDirective::HasOne(ident) => has_ones.push(ident),
                AccountDirective::Constraint(expr) => constraints.push(expr),
                AccountDirective::Seeds(s) => seeds = Some(s),
                AccountDirective::Bump(b) => bump = Some(b),
            }
        }
        Ok(Self { has_ones, constraints, seeds, bump })
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
        has_ones: vec![],
        constraints: vec![],
        seeds: None,
        bump: None,
    }
}

// --- Derive Accounts ---

#[proc_macro_derive(Accounts, attributes(account))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
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

    let field_constructs: Vec<proc_macro2::TokenStream> = fields.iter().map(|f| {
        let name = &f.ident;
        match &f.ty {
            Type::Reference(type_ref) => {
                let base_type = strip_generics(&type_ref.elem);
                if type_ref.mutability.is_some() {
                    quote! { #name: #base_type::from_account_view_mut(#name)? }
                } else {
                    quote! { #name: #base_type::from_account_view(#name)? }
                }
            }
            _ => {
                let base_type = strip_generics(&f.ty);
                quote! { #name: #base_type::from_account_view(#name)? }
            }
        }
    }).collect();

    let field_name_strings: Vec<String> = fields.iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect();

    let mut has_one_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut constraint_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut pda_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_init_vars: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_inits: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seeds_methods: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seed_addr_captures: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields.iter() {
        let attrs = parse_field_attrs(field);
        let field_name = field.ident.as_ref().unwrap();

        for target in &attrs.has_ones {
            has_one_checks.push(quote! {
                if #field_name.#target != *#target.to_account_view().address() {
                    return Err(QuasarError::HasOneMismatch.into());
                }
            });
        }

        for expr in &attrs.constraints {
            constraint_checks.push(quote! {
                if !(#expr) {
                    return Err(QuasarError::ConstraintViolation.into());
                }
            });
        }

        if let Some(ref seed_exprs) = attrs.seeds {
            let bump_var = format_ident!("__bumps_{}", field_name);

            bump_init_vars.push(quote! { let mut #bump_var: u8 = 0; });
            bump_struct_fields.push(quote! { pub #field_name: u8 });
            bump_struct_inits.push(quote! { #field_name: #bump_var });

            let bump_arr_field = format_ident!("__{}_bump", field_name);
            bump_struct_fields.push(quote! { #bump_arr_field: [u8; 1] });
            bump_struct_inits.push(quote! { #bump_arr_field: [#bump_var] });

            let seed_slices: Vec<proc_macro2::TokenStream> = seed_exprs.iter().map(|expr| {
                seed_slice_expr_for_parse(expr, &field_name_strings)
            }).collect();

            let seed_idents: Vec<Ident> = seed_slices.iter().enumerate().map(|(idx, _)| {
                format_ident!("__seed_{}_{}", field_name, idx)
            }).collect();

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
                    panic!("#[account(seeds = [...])] requires a `bump` or `bump = expr` directive");
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

                            seed_elements.push(quote! { quasar_core::cpi::Seed::from(self.#addr_field.as_ref()) });
                            continue;
                        }
                    }
                }
                seed_elements.push(quote! { quasar_core::cpi::Seed::from((#expr) as &[u8]) });
            }

            seed_elements.push(quote! { quasar_core::cpi::Seed::from(&self.#bump_arr_field as &[u8]) });

            seeds_methods.push(quote! {
                #[inline(always)]
                pub fn #method_name(&self) -> [quasar_core::cpi::Seed<'_>; #seed_count] {
                    [#(#seed_elements),*]
                }
            });
        }
    }

    let field_count = field_names.len();
    let field_indices: Vec<usize> = (0..field_count).collect();

    let parse_steps: Vec<proc_macro2::TokenStream> = field_indices.iter().map(|&i| {
        quote! {
            {
                let raw = input as *mut quasar_core::__private::RuntimeAccount;
                if unsafe { (*raw).borrow_state } == quasar_core::__private::NOT_BORROWED {
                    unsafe {
                        core::ptr::write(base.add(#i), quasar_core::__private::AccountView::new_unchecked(raw));
                        input = input.add(__ACCOUNT_HEADER + (*raw).data_len as usize);
                        let addr = input as usize;
                        input = ((addr + 7) & !7) as *mut u8;
                    }
                } else {
                    unsafe {
                        let idx = (*raw).borrow_state as usize;
                        core::ptr::write(base.add(#i), core::ptr::read(base.add(idx)));
                        input = input.add(core::mem::size_of::<u64>());
                    }
                }
            }
        }
    }).collect();

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
        || !pda_checks.is_empty();

    let parse_body = if has_any_checks {
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

    let account_fields_str: String = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap().to_string();
        format!("pub {}: solana_address::Address,", field_name)
    }).collect::<Vec<_>>().join("\n                ");

    let account_metas_str: String = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap().to_string();
        let writable = matches!(&f.ty, Type::Reference(r) if r.mutability.is_some());
        let signer = is_signer_type(&f.ty);
        if writable {
            format!("quasar_core::client::AccountMeta::new(ix.{}, {}),", field_name, signer)
        } else {
            format!("quasar_core::client::AccountMeta::new_readonly(ix.{}, {}),", field_name, signer)
        }
    }).collect::<Vec<_>>().join("\n                        ");

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
                            program_id: crate::ID,
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

    let client_macro: proc_macro2::TokenStream = macro_def_str.parse()
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
            const COUNT: usize = #field_count;
        }

        impl<'info> #name<'info> {
            #[inline(always)]
            pub unsafe fn parse_accounts(
                mut input: *mut u8,
                buf: &mut core::mem::MaybeUninit<[quasar_core::__private::AccountView; #field_count]>,
            ) -> *mut u8 {
                const __ACCOUNT_HEADER: usize =
                    core::mem::size_of::<quasar_core::__private::RuntimeAccount>()
                    + quasar_core::__private::MAX_PERMITTED_DATA_INCREASE
                    + core::mem::size_of::<u64>();

                let base = buf.as_mut_ptr() as *mut quasar_core::__private::AccountView;

                #(#parse_steps)*

                input
            }
        }

        #client_macro
    };

    TokenStream::from(expanded)
}

// --- Instruction macro ---

struct InstructionArgs {
    discriminator: Vec<LitInt>,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident != "discriminator" {
            return Err(syn::Error::new(ident.span(), "expected `discriminator`"));
        }
        let _: Token![=] = input.parse()?;
        if input.peek(syn::token::Bracket) {
            let content;
            syn::bracketed!(content in input);
            let lits = content.parse_terminated(LitInt::parse, Token![,])?;
            let discriminator: Vec<LitInt> = lits.into_iter().collect();
            if discriminator.is_empty() {
                return Err(syn::Error::new(input.span(), "discriminator must have at least one byte"));
            }
            Ok(Self { discriminator })
        } else {
            let lit: LitInt = input.parse()?;
            Ok(Self { discriminator: vec![lit] })
        }
    }
}

#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
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

    let remaining: Vec<_> = func.sig.inputs.iter().skip(1).filter_map(|arg| {
        match arg {
            FnArg::Typed(pt) => Some(pt.clone()),
            _ => None,
        }
    }).collect();

    func.sig.inputs = syn::punctuated::Punctuated::new();
    func.sig.inputs.push(syn::parse_quote!(mut context: Context));

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
            let mut #param_name: #param_type = Ctx::new(context)?;
        ),
    ];

    if !remaining.is_empty() {
        let field_names: Vec<Ident> = remaining.iter().map(|pt| {
            match &*pt.pat {
                Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => panic!("#[instruction] parameters must be simple identifiers"),
            }
        }).collect();

        let field_types: Vec<&Type> = remaining.iter().map(|pt| &*pt.ty).collect();

        new_stmts.push(syn::parse_quote!(
            #[repr(C)]
            struct InstructionData {
                #(#field_names: #field_types,)*
            }
        ));

        new_stmts.push(syn::parse_quote!(
            if #param_ident.data.len() < core::mem::size_of::<InstructionData>() {
                return Err(ProgramError::InvalidInstructionData);
            }
        ));

        new_stmts.push(syn::parse_quote!(
            let __instruction_data = unsafe { core::ptr::read_unaligned(#param_ident.data.as_ptr() as *const InstructionData) };
        ));

        for name in &field_names {
            new_stmts.push(syn::parse_quote!(
                let #name = __instruction_data.#name;
            ));
        }
    }

    func.block.stmts = new_stmts.into_iter().chain(stmts).collect();

    quote!(#func).into()
}

// --- Account attribute macro ---

#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

    let disc_values: Vec<u8> = disc_bytes.iter()
        .map(|lit| lit.base10_parse::<u8>().expect("discriminator byte must be 0-255"))
        .collect();
    if disc_values.iter().all(|&b| b == 0) {
        return syn::Error::new_spanned(
            &args.discriminator[0],
            "discriminator must contain at least one non-zero byte; all-zero discriminators are indistinguishable from uninitialized account data",
        ).to_compile_error().into();
    }

    let disc_indices: Vec<usize> = (0..disc_len).collect();

    let fields_data = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("#[account] can only be used on structs with named fields"),
        },
        _ => panic!("#[account] can only be used on structs"),
    };

    let field_types: Vec<_> = fields_data.iter().map(|f| &f.ty).collect();

    let zc_name = format_ident!("{}Zc", name);
    let zc_fields: Vec<proc_macro2::TokenStream> = fields_data.iter().map(|f| {
        let fname = &f.ident;
        let vis = &f.vis;
        let zc_ty = map_to_pod_type(&f.ty);
        quote! { #vis #fname: #zc_ty }
    }).collect();

    quote! {
        #[repr(C)]
        #[derive(::wincode::SchemaRead, ::wincode::SchemaWrite)]
        #input

        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #zc_name {
            #(#zc_fields,)*
        }

        const _: () = assert!(
            core::mem::align_of::<#zc_name>() == 1,
            "ZC companion struct must have alignment 1; all fields must use Pod types or alignment-1 types"
        );

        impl Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Space for #name {
            const SPACE: usize = #disc_len #(+ core::mem::size_of::<#field_types>())*;
        }

        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }

        impl AccountCheck for #name {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                if __data.len() < #disc_len + core::mem::size_of::<#zc_name>() {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                #(
                    if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                        return Err(ProgramError::InvalidAccountData);
                    }
                )*
                Ok(())
            }
        }

        impl ZeroCopyDeref for #name {
            type Target = #zc_name;
            const DATA_OFFSET: usize = Self::DISCRIMINATOR.len();
        }

        impl QuasarAccount for #name {
            #[inline(always)]
            fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
                ::wincode::deserialize(data).map_err(|_| ProgramError::InvalidAccountData)
            }

            #[inline(always)]
            fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError> {
                ::wincode::serialize_into(data, self).map_err(|_| ProgramError::InvalidAccountData)
            }
        }

        impl #name {
            #[inline(always)]
            pub fn init(self, account: &mut Initialize<Self>, payer: &AccountView, rent: Option<&Rent>) -> Result<(), ProgramError> {
                self.init_signed(account, payer, rent, &[])
            }

            #[inline(always)]
            pub fn init_signed(self, account: &mut Initialize<Self>, payer: &AccountView, rent: Option<&Rent>, signers: &[quasar_core::cpi::Signer]) -> Result<(), ProgramError> {
                let view = account.to_account_view();

                {
                    let __existing = unsafe { view.borrow_unchecked() };
                    if __existing.len() >= Self::DISCRIMINATOR.len()
                        && __existing[..Self::DISCRIMINATOR.len()] == *Self::DISCRIMINATOR
                    {
                        return Err(QuasarError::AccountAlreadyInitialized.into());
                    }
                }

                use quasar_core::sysvars::Sysvar;
                let lamports = match rent {
                    Some(rent_account) => rent_account.get()?.try_minimum_balance(Self::SPACE)?,
                    None => quasar_core::sysvars::rent::Rent::get()?.try_minimum_balance(Self::SPACE)?,
                };

                if view.lamports() == 0 {
                    quasar_core::cpi::system::create_account(payer, view, lamports, Self::SPACE as u64, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                } else {
                    let required = lamports.saturating_sub(view.lamports());
                    if required > 0 {
                        quasar_core::cpi::system::transfer(payer, view, required)
                            .invoke_with_signers(signers)?;
                    }
                    quasar_core::cpi::system::assign(view, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                    unsafe { view.resize_unchecked(Self::SPACE) }?;
                }

                let mut data = view.try_borrow_mut()?;
                data[..Self::DISCRIMINATOR.len()].copy_from_slice(Self::DISCRIMINATOR);
                self.serialize(&mut data[Self::DISCRIMINATOR.len()..])?;
                Ok(())
            }
        }
    }.into()
}

fn map_to_pod_type(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let ident_str = seg.ident.to_string();
            return match ident_str.as_str() {
                "u128" => quote! { quasar_core::pod::PodU128 },
                "u64" => quote! { quasar_core::pod::PodU64 },
                "u32" => quote! { quasar_core::pod::PodU32 },
                "u16" => quote! { quasar_core::pod::PodU16 },
                "i128" => quote! { quasar_core::pod::PodI128 },
                "i64" => quote! { quasar_core::pod::PodI64 },
                "i32" => quote! { quasar_core::pod::PodI32 },
                "i16" => quote! { quasar_core::pod::PodI16 },
                "bool" => quote! { quasar_core::pod::PodBool },
                _ => quote! { #ty },
            };
        }
    }
    quote! { #ty }
}

// --- Program macro ---

/// Extracts the inner type `T` from a `Ctx<T>` first parameter.
fn extract_ctx_inner_type(sig: &syn::Signature) -> proc_macro2::TokenStream {
    let first_arg = match sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt,
        _ => panic!("#[program]: instruction function must have ctx: Ctx<T> as first parameter"),
    };

    match &*first_arg.ty {
        Type::Path(type_path) => {
            let last_seg = type_path.path.segments.last()
                .expect("Ctx type must have segments");
            match &last_seg.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    match args.args.first() {
                        Some(syn::GenericArgument::Type(ty)) => quote!(#ty),
                        _ => panic!("Ctx must have a type argument"),
                    }
                }
                _ => panic!("Ctx must have angle-bracketed arguments"),
            }
        }
        _ => panic!("First parameter must be Ctx<T>"),
    }
}

#[proc_macro_attribute]
pub fn program(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as ItemMod);
    let mod_name = module.ident.clone();
    let program_type_name = format_ident!("{}", snake_to_pascal(&mod_name.to_string()));

    let (_, items) = module.content
        .as_ref()
        .expect("#[program] must be used on a module with a body");

    // Scan for #[instruction(discriminator = ...)] functions
    let mut dispatch_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut client_items: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seen_discriminators: Vec<(Vec<u8>, String)> = Vec::new();
    let mut disc_len: Option<usize> = None;

    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("instruction") {
                    let args: InstructionArgs = attr.parse_args()
                        .expect("failed to parse #[instruction] attribute");
                    let disc_bytes = &args.discriminator;
                    let fn_name = &func.sig.ident;
                    let accounts_type = extract_ctx_inner_type(&func.sig);

                    // Validate same length across all instructions
                    match disc_len {
                        Some(len) => {
                            if disc_bytes.len() != len {
                                return syn::Error::new_spanned(
                                    attr,
                                    format!(
                                        "all instruction discriminators must have the same length: expected {} byte(s), found {}",
                                        len, disc_bytes.len()
                                    ),
                                ).to_compile_error().into();
                            }
                        }
                        None => disc_len = Some(disc_bytes.len()),
                    }

                    // Check for duplicates
                    let disc_values: Vec<u8> = disc_bytes.iter()
                        .map(|lit| lit.base10_parse::<u8>().expect("discriminator byte must be 0-255"))
                        .collect();
                    if let Some((_, prev_fn)) = seen_discriminators.iter().find(|(v, _)| *v == disc_values) {
                        return syn::Error::new_spanned(
                            attr,
                            format!(
                                "duplicate discriminator {:?}: already used by `{}`",
                                disc_values, prev_fn
                            ),
                        ).to_compile_error().into();
                    }
                    seen_discriminators.push((disc_values.clone(), fn_name.to_string()));

                    dispatch_arms.push(quote! {
                        [#(#disc_bytes),*] => #fn_name(#accounts_type)
                    });

                    // Collect data for client module generation — invoke the macro_rules
                    // bridge emitted by derive(Accounts)
                    let struct_name = format_ident!("{}Instruction", snake_to_pascal(&fn_name.to_string()));
                    let accounts_type_str = accounts_type.to_string().replace(' ', "");
                    let macro_ident = format_ident!("__{}_instruction", pascal_to_snake(&accounts_type_str));

                    let remaining_args: Vec<(Ident, Type)> = func.sig.inputs.iter().skip(1).filter_map(|arg| {
                        match arg {
                            FnArg::Typed(pt) => {
                                let name = match &*pt.pat {
                                    Pat::Ident(pi) => pi.ident.clone(),
                                    _ => return None,
                                };
                                Some((name, (*pt.ty).clone()))
                            }
                            _ => None,
                        }
                    }).collect();

                    let arg_names: Vec<&Ident> = remaining_args.iter().map(|(n, _)| n).collect();
                    let arg_types: Vec<&Type> = remaining_args.iter().map(|(_, t)| t).collect();

                    let disc_byte_lits: Vec<proc_macro2::TokenStream> = disc_values.iter().map(|b| {
                        let lit = proc_macro2::Literal::u8_unsuffixed(*b);
                        quote! { #lit }
                    }).collect();

                    client_items.push(quote! {
                        #macro_ident!(#struct_name, [#(#disc_byte_lits),*], {#(#arg_names : #arg_types),*});
                    });

                    break;
                }
            }
        }
    }

    let disc_len_lit = disc_len.unwrap_or(1);

    // Check reserved __event discriminator (0xFF) doesn't collide with user instructions
    let event_disc: Vec<u8> = vec![0xFF; disc_len_lit];
    if let Some((_, fn_name)) = seen_discriminators.iter().find(|(v, _)| *v == event_disc) {
        return syn::Error::new_spanned(
            &module.ident,
            format!(
                "instruction `{}` uses discriminator {:?} which is reserved for __event",
                fn_name, event_disc
            ),
        ).to_compile_error().into();
    }

    // Generate event discriminator check bytes for __dispatch
    let event_check_bytes: Vec<proc_macro2::TokenStream> = (0..disc_len_lit)
        .map(|i| quote! { instruction_data[#i] == 0xFF })
        .collect();

    // Append dispatch + entrypoint to the module
    if let Some((_, ref mut items)) = module.content {
        items.push(syn::parse_quote! {
            fn __handle_event(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                let num_accounts = unsafe { *(ptr as *const u64) };
                if num_accounts < 1 {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }

                let __accounts_start = unsafe { (ptr as *mut u8).add(core::mem::size_of::<u64>()) };

                let mut __buf = core::mem::MaybeUninit::<[AccountView; 1]>::uninit();
                unsafe {
                    let raw = __accounts_start as *mut quasar_core::__private::RuntimeAccount;
                    let base = __buf.as_mut_ptr() as *mut AccountView;
                    if (*raw).borrow_state == quasar_core::__private::NOT_BORROWED {
                        core::ptr::write(base, AccountView::new_unchecked(raw));
                    } else {
                        return Err(ProgramError::AccountBorrowFailed);
                    }
                }
                let __accounts = unsafe { __buf.assume_init() };
                let event_authority = &__accounts[0];

                if !event_authority.is_signer() {
                    return Err(ProgramError::MissingRequiredSignature);
                }

                if instruction_data.len() <= #disc_len_lit {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let bump = instruction_data[instruction_data.len() - 1];

                let bump_ref: &[u8] = &[bump];
                let seeds = [
                    quasar_core::cpi::Seed::from(b"__event_authority" as &[u8]),
                    quasar_core::cpi::Seed::from(bump_ref),
                ];
                let expected = quasar_core::pda::create_program_address(&seeds, &crate::ID)?;
                if *event_authority.address() != expected {
                    return Err(ProgramError::InvalidSeeds);
                }

                let event_data = &instruction_data[#disc_len_lit..instruction_data.len() - 1];
                quasar_core::log::log_data(&[event_data]);

                Ok(())
            }
        });

        items.push(syn::parse_quote! {
            #[inline(always)]
            fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                if instruction_data.len() >= #disc_len_lit && #(#event_check_bytes)&&* {
                    return __handle_event(ptr, instruction_data);
                }
                dispatch!(ptr, instruction_data, #disc_len_lit, {
                    #(#dispatch_arms),*
                })
            }
        });

        items.push(syn::parse_quote! {
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn entrypoint(ptr: *mut u8, instruction_data: *const u8) -> u64 {
                let instruction_data = unsafe {
                    core::slice::from_raw_parts(
                        instruction_data,
                        *(instruction_data.sub(8) as *const u64) as usize,
                    )
                };
                match __dispatch(ptr, instruction_data) {
                    Ok(_) => 0,
                    Err(e) => e.into(),
                }
            }
        });

        // Add client module inside the program module
        let client_mod: syn::Item = syn::parse2(quote! {
            #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
            pub mod client {
                use alloc::vec;
                use super::*;

                #(#client_items)*
            }
        }).expect("failed to parse client module");
        items.push(client_mod);
    }

    // Generate the named program type outside the module
    let program_type = quote! {
        quasar_core::define_account!(pub struct #program_type_name => [quasar_core::checks::Executable, quasar_core::checks::Address]);

        impl Program for #program_type_name {
            const ID: Address = crate::ID;
        }

        impl #program_type_name {
            #[inline(always)]
            pub fn emit_event<E: quasar_core::traits::Event>(
                &self,
                event: &E,
                event_authority: &impl AsAccountView,
                bump: u8,
            ) -> Result<(), ProgramError> {
                const __DISC_LEN: usize = #disc_len_lit;
                let event_disc_len = E::DISCRIMINATOR.len();
                let total = __DISC_LEN + event_disc_len + E::DATA_SIZE + 1;
                if total > 1232 {
                    return Err(ProgramError::InvalidArgument);
                }

                let mut buf = [0u8; 1232];
                let mut i = 0;
                while i < __DISC_LEN { buf[i] = 0xFF; i += 1; }
                buf[__DISC_LEN..__DISC_LEN + event_disc_len].copy_from_slice(E::DISCRIMINATOR);
                event.write_data(&mut buf[__DISC_LEN + event_disc_len..__DISC_LEN + event_disc_len + E::DATA_SIZE]);
                buf[__DISC_LEN + event_disc_len + E::DATA_SIZE] = bump;

                quasar_core::event::emit_event_cpi(
                    self.to_account_view(),
                    event_authority.to_account_view(),
                    &buf[..total],
                )
            }
        }
    };

    quote! {
        #program_type

        #module

        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        extern crate alloc;

        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        pub use #mod_name::client;
    }.into()
}

// --- Event attribute macro ---

fn event_field_size(ty: &Type) -> usize {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" | "i8" | "bool" => 1,
                "u16" | "i16" => 2,
                "u32" | "i32" => 4,
                "u64" | "i64" => 8,
                "u128" | "i128" => 16,
                "Address" => 32,
                _ => panic!("unsupported event field type `{}`; only primitive integers, bool, and Address are supported", seg.ident),
            };
        }
    }
    panic!("unsupported event field type");
}

fn event_field_write(name: &Ident, ty: &Type, offset: usize) -> proc_macro2::TokenStream {
    let size = event_field_size(ty);
    let end = offset + size;
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" => quote! { buf[#offset] = self.#name; },
                "i8" => quote! { buf[#offset] = self.#name as u8; },
                "bool" => quote! { buf[#offset] = self.#name as u8; },
                "Address" => quote! { buf[#offset..#end].copy_from_slice(self.#name.as_ref()); },
                _ => quote! { buf[#offset..#end].copy_from_slice(&self.#name.to_le_bytes()); },
            };
        }
    }
    unreachable!()
}

#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

    let disc_values: Vec<u8> = disc_bytes.iter()
        .map(|lit| lit.base10_parse::<u8>().expect("discriminator byte must be 0-255"))
        .collect();
    if disc_values.iter().all(|&b| b == 0) {
        return syn::Error::new_spanned(
            &args.discriminator[0],
            "event discriminator must contain at least one non-zero byte",
        ).to_compile_error().into();
    }

    let fields_data = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("#[event] requires named fields"),
        },
        _ => panic!("#[event] can only be used on structs"),
    };

    let mut data_size: usize = 0;
    let mut write_stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields_data.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let size = event_field_size(&field.ty);
        write_stmts.push(event_field_write(field_name, &field.ty, data_size));
        data_size += size;
    }

    if data_size > 1232 {
        return syn::Error::new_spanned(
            name,
            format!("event DATA_SIZE ({} bytes) exceeds 1232-byte limit", data_size),
        ).to_compile_error().into();
    }

    let total_buf_size = disc_len + data_size;
    let emit_log_method = quote! {
        impl #name {
            #[inline(always)]
            pub fn emit_log(&self) {
                let mut buf = [0u8; #total_buf_size];
                buf[..#disc_len].copy_from_slice(<Self as quasar_core::traits::Event>::DISCRIMINATOR);
                <Self as quasar_core::traits::Event>::write_data(self, &mut buf[#disc_len..]);
                quasar_core::log::log_data(&[&buf]);
            }
        }
    };

    quote! {
        #input

        impl quasar_core::traits::Event for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
            const DATA_SIZE: usize = #data_size;

            #[inline(always)]
            fn write_data(&self, buf: &mut [u8]) {
                #(#write_stmts)*
            }
        }

        #emit_log_method
    }.into()
}

// --- Helpers ---

/// Expand a seed expression into a byte slice for use inside parse (fields are local variables).
fn seed_slice_expr_for_parse(expr: &Expr, field_names: &[String]) -> proc_macro2::TokenStream {
    if let Expr::Path(ep) = expr {
        if ep.path.segments.len() == 1 && ep.qself.is_none() {
            let ident = &ep.path.segments[0].ident;
            if field_names.contains(&ident.to_string()) {
                return quote! { #ident.to_account_view().address().as_ref() };
            }
        }
    }
    quote! { #expr as &[u8] }
}

/// Check if a field type's base type is `Signer`.
fn is_signer_type(ty: &Type) -> bool {
    let inner = match ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    if let Type::Path(p) = inner {
        if let Some(last) = p.path.segments.last() {
            return last.ident == "Signer";
        }
    }
    false
}

fn strip_generics(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::Path(type_path) => {
            let segments: Vec<_> = type_path
                .path
                .segments
                .iter()
                .map(|seg| &seg.ident)
                .collect();
            quote! { #(#segments)::* }
        }
        _ => panic!("Unsupported field type"),
    }
}

fn pascal_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + &chars.collect::<String>(),
            }
        })
        .collect()
}

// --- Error code macro ---

#[proc_macro_attribute]
pub fn error_code(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("#[error_code] can only be used on enums"),
    };

    let mut next_discriminant: u32 = 0;
    let match_arms: Vec<_> = variants.iter().map(|v| {
        let ident = &v.ident;
        if let Some((_, expr)) = &v.discriminant {
            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit_int), .. }) = expr {
                next_discriminant = lit_int.base10_parse::<u32>()
                    .expect("#[error_code] discriminant must be a valid u32");
            } else {
                panic!("#[error_code] discriminant must be an integer literal");
            }
        }
        let value = next_discriminant;
        next_discriminant += 1;
        quote! { #value => Ok(#name::#ident) }
    }).collect();

    quote! {
        #[repr(u32)]
        #input

        impl From<#name> for ProgramError {
            #[inline(always)]
            fn from(e: #name) -> Self {
                ProgramError::Custom(e as u32)
            }
        }

        impl TryFrom<u32> for #name {
            type Error = ProgramError;

            #[inline(always)]
            fn try_from(error: u32) -> Result<Self, Self::Error> {
                match error {
                    #(#match_arms,)*
                    _ => Err(ProgramError::InvalidArgument),
                }
            }
        }
    }.into()
}
