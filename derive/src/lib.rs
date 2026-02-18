use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Data, DeriveInput, Expr, ExprArray, FnArg, Fields, Ident, ItemFn, LitInt, Pat, Token, Type,
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
    };

    TokenStream::from(expanded)
}

// --- Instruction macro ---

struct InstructionArgs {
    discriminator: LitInt,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        if ident != "discriminator" {
            return Err(syn::Error::new(ident.span(), "expected `discriminator`"));
        }
        let _: Token![=] = input.parse()?;
        let discriminator: LitInt = input.parse()?;
        Ok(Self { discriminator })
    }
}

#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let mut func = parse_macro_input!(item as ItemFn);
    let discriminator = &args.discriminator;

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
            if context.data.first() != Some(&#discriminator) {
                return Err(ProgramError::InvalidInstructionData);
            }
        ),
        syn::parse_quote!(
            context.data = &context.data[1..];
        ),
        syn::parse_quote!(
            let #param_name: #param_type = Ctx::new(context)?;
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
            let __instruction_data = unsafe { &*(#param_ident.data.as_ptr() as *const InstructionData) };
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
    let discriminator = &args.discriminator;

    let field_types: Vec<_> = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields.named.iter().map(|f| &f.ty).collect(),
            _ => panic!("#[account] can only be used on structs with named fields"),
        },
        _ => panic!("#[account] can only be used on structs"),
    };

    quote! {
        #[repr(C)]
        #[derive(::wincode::SchemaRead, ::wincode::SchemaWrite)]
        #input

        impl Discriminator for #name {
            const DISCRIMINATOR: u8 = #discriminator;
        }

        impl Space for #name {
            const SPACE: usize = 1 #(+ core::mem::size_of::<#field_types>())*;
        }

        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }

        impl AccountCheck for #name {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                if unsafe { *view.borrow_unchecked().get_unchecked(0) } != #discriminator {
                    return Err(ProgramError::InvalidAccountData);
                }
                Ok(())
            }
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
                data[0] = Self::DISCRIMINATOR;
                self.serialize(&mut data[1..])?;
                Ok(())
            }
        }
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
