//! Final TokenStream assembly for the generated accounts impls.
//!
//! This module keeps trait wiring in one place; parsing, planning, lifecycle,
//! and client macro generation are built before this step.

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
    pub parse_body: proc_macro2::TokenStream,
    pub bumps_struct: proc_macro2::TokenStream,
    pub signer_helpers_impl: proc_macro2::TokenStream,
    pub epilogue_method: proc_macro2::TokenStream,
    pub has_epilogue_expr: proc_macro2::TokenStream,
    pub client_macro: proc_macro2::TokenStream,
    /// The `Self::__extract_ix_args(..)` destructuring call spliced at each
    /// parse/signer site (empty when there are no ix args).
    pub ix_arg_extraction: proc_macro2::TokenStream,
    /// The single `#[inline(always)] fn __extract_ix_args` definition, placed
    /// on the inherent impl (empty when there are no ix args).
    pub extract_ix_args_fn: proc_macro2::TokenStream,
}

pub(crate) fn emit_accounts_output(output: AccountsOutput<'_>) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
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
        parse_body,
        bumps_struct,
        signer_helpers_impl,
        epilogue_method,
        has_epilogue_expr,
        client_macro,
        ix_arg_extraction,
        extract_ix_args_fn,
    } = output;

    // `ParseAccounts::HAS_EPILOGUE` already defaults to false, which is exactly
    // what the expression folds to for a struct with no lifecycle steps.
    let has_epilogue_const = if has_epilogue_expr.to_string() == "false" {
        quote! {}
    } else {
        quote! {
            const HAS_EPILOGUE: bool = #has_epilogue_expr;
        }
    };

    let inherent_impl = if extract_ix_args_fn.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #impl_generics #name #ty_generics #where_clause {
                #extract_ix_args_fn
            }
        }
    };

    let parse_accounts_impl = quote! {
        impl #parse_impl_generics #krate::traits::ParseAccounts<'input> for #name #ty_generics #parse_where_clause {
            type Bumps = #bumps_name;
            #has_epilogue_const

            #epilogue_method
        }

        unsafe impl #parse_impl_generics #krate::traits::ParseAccountsUnchecked<'input>
            for #name #ty_generics
            #parse_where_clause
        {
            #[inline(always)]
            unsafe fn parse_with_instruction_data_unchecked(
                accounts: &'input mut [#krate::__internal::AccountView],
                __ix_data: &[u8],
                __program_id: &#krate::prelude::Address,
            ) -> Result<(Self, Self::Bumps), #krate::__solana_program_error::ProgramError> {
                #ix_arg_extraction
                #parse_body
            }
        }
    };

    quote! {
        #bumps_struct
        #signer_helpers_impl

        #parse_accounts_impl

        impl #impl_generics #krate::traits::AccountCount for #name #ty_generics #where_clause {
            const COUNT: usize = #count_expr;
            const NEEDS_EVENT_CPI: bool = #needs_event_cpi_expr;
        }

        #inherent_impl

        unsafe impl #impl_generics #krate::traits::ParseAccountsRaw for #name #ty_generics #where_clause {
            #[inline(always)]
            unsafe fn parse_accounts_raw(
                mut input: *mut u8,
                base: *mut #krate::__internal::AccountView,
                __offset: usize,
                __program_id: &#krate::prelude::Address,
            ) -> Result<*mut u8, #krate::__solana_program_error::ProgramError> {
                #(#parse_steps)*

                Ok(input)
            }
        }

        impl #parse_impl_generics #krate::remaining::RemainingItem<'input>
            for #name #ty_generics
            #parse_where_clause
        {
            const COUNT: usize = <Self as #krate::traits::AccountCount>::COUNT;

            #[inline(always)]
            unsafe fn parse_remaining_chunk(
                accounts: &'input mut [#krate::__internal::AccountView],
                program_id: Option<&#krate::prelude::Address>,
                data: &[u8],
            ) -> Result<Self, #krate::__solana_program_error::ProgramError> {
                // SAFETY: forwards the caller's exact-count contract.
                unsafe { #krate::remaining::parse_group_chunk::<Self>(accounts, program_id, data) }
            }
        }

        #client_macro
    }
}
