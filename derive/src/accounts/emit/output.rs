//! Final TokenStream assembly for ParseAccounts / ParseAccountsUnchecked.
//! Adapted from v1 — same output shape, same trait impls.

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
    pub needs_event_cpi_expr: proc_macro2::TokenStream,
    pub parse_steps: Vec<proc_macro2::TokenStream>,
    pub typed_seed_asserts: proc_macro2::TokenStream,
    pub parse_body: proc_macro2::TokenStream,
    pub direct_parse_body: proc_macro2::TokenStream,
    pub bumps_struct: proc_macro2::TokenStream,
    pub account_seeds_impl: proc_macro2::TokenStream,
    pub epilogue_method: proc_macro2::TokenStream,
    pub has_epilogue_expr: proc_macro2::TokenStream,
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
        needs_event_cpi_expr,
        parse_steps,
        typed_seed_asserts,
        parse_body,
        direct_parse_body,
        bumps_struct,
        account_seeds_impl,
        epilogue_method,
        has_epilogue_expr,
        client_macro,
        ix_arg_extraction,
    } = output;

    let exact_len_guard = quote! {
        quasar_lang::traits::check_account_count(accounts.len(), Self::COUNT)?;
    };

    let has_epilogue_const = quote! {
        const HAS_EPILOGUE: bool = #has_epilogue_expr;
    };

    let has_validate_const = quote! {};

    let parse_accounts_impl = quote! {
        impl #parse_impl_generics ParseAccounts<'input> for #name #ty_generics #parse_where_clause {
            type Bumps = #bumps_name;
            #has_epilogue_const
            #has_validate_const

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
        #account_seeds_impl

        #parse_accounts_impl

        impl #impl_generics AccountCount for #name #ty_generics #where_clause {
            const COUNT: usize = #count_expr;
            const NEEDS_EVENT_CPI: bool = #needs_event_cpi_expr;
        }

        impl #impl_generics #name #ty_generics #where_clause {
            #[inline(always)]
            #[doc(hidden)]
            pub unsafe fn parse_accounts(
                mut input: *mut u8,
                buf: &mut core::mem::MaybeUninit<[quasar_lang::__internal::AccountView; #count_expr]>,
                __program_id: &quasar_lang::prelude::Address,
            ) -> Result<*mut u8, ProgramError> {
                let base = buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView;

                #(#parse_steps)*

                Ok(input)
            }

            #[inline(always)]
            #[doc(hidden)]
            pub unsafe fn parse_direct_with_instruction_data_unchecked(
                mut input: *mut u8,
                __ix_data: &[u8],
                __program_id: &quasar_lang::prelude::Address,
            ) -> Result<(Self, #bumps_name), ProgramError> {
                #ix_arg_extraction
                #direct_parse_body
            }
        }

        unsafe impl #impl_generics quasar_lang::traits::ParseAccountsRaw for #name #ty_generics #where_clause {
            #[inline(always)]
            unsafe fn parse_accounts_raw(
                input: *mut u8,
                base: *mut quasar_lang::__internal::AccountView,
                offset: usize,
                __program_id: &quasar_lang::prelude::Address,
            ) -> Result<*mut u8, ProgramError> {
                let mut __inner_buf = core::mem::MaybeUninit::<
                    [quasar_lang::__internal::AccountView; #count_expr]
                >::uninit();
                let input = Self::parse_accounts(input, &mut __inner_buf, __program_id)?;
                let __inner = core::mem::ManuallyDrop::new(__inner_buf.assume_init());
                let mut __j = 0usize;
                while __j < #count_expr {
                    core::ptr::write(
                        base.add(offset + __j),
                        core::ptr::read(__inner.as_ptr().add(__j)),
                    );
                    __j += 1;
                }
                Ok(input)
            }
        }

        #client_macro
    }
}
