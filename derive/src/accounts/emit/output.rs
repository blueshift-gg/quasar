use quote::quote;

pub(crate) struct AccountsOutput<'a> {
    pub name: &'a syn::Ident,
    pub bumps_name: &'a syn::Ident,
    pub impl_generics: proc_macro2::TokenStream,
    pub ty_generics: proc_macro2::TokenStream,
    pub where_clause: proc_macro2::TokenStream,
    pub parse_impl_generics: proc_macro2::TokenStream,
    pub parse_where_clause: proc_macro2::TokenStream,
    pub count_expr: proc_macro2::TokenStream,
    pub parse_steps: Vec<proc_macro2::TokenStream>,
    pub typed_seed_asserts: proc_macro2::TokenStream,
    pub parse_body: proc_macro2::TokenStream,
    pub bumps_struct: proc_macro2::TokenStream,
    pub epilogue_method: proc_macro2::TokenStream,
    pub seeds_methods: proc_macro2::TokenStream,
    pub client_macro: proc_macro2::TokenStream,
    pub ix_arg_extraction: proc_macro2::TokenStream,
}

pub(crate) fn emit_accounts_output(output: AccountsOutput<'_>) -> proc_macro2::TokenStream {
    let AccountsOutput {
        name,
        bumps_name,
        impl_generics,
        ty_generics,
        where_clause,
        parse_impl_generics,
        parse_where_clause,
        count_expr,
        parse_steps,
        typed_seed_asserts,
        parse_body,
        bumps_struct,
        epilogue_method,
        seeds_methods,
        client_macro,
        ix_arg_extraction,
    } = output;

    let exact_len_guard = quote! {
        quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
    };

    let seeds_impl = if seeds_methods.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #seeds_methods
            }
        }
    };

    let parse_accounts_impl = quote! {
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
            unsafe fn parse_unchecked(
                accounts: &'input mut [AccountView],
                program_id: &Address,
            ) -> Result<(Self, Self::Bumps), ProgramError> {
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
                #typed_seed_asserts
                #ix_arg_extraction
                #parse_body
            }
        }
    };

    quote! {
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
                let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;

                #(#parse_steps)*

                Ok(input)
            }
        }

        #client_macro
    }
}
