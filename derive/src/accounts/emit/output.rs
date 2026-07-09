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
    pub direct_parse_body: proc_macro2::TokenStream,
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
    /// The single `__assert_builder` helper, placed on the inherent impl (empty
    /// when the struct has no behavior groups).
    pub assert_builder_fn: proc_macro2::TokenStream,
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
        direct_parse_body,
        bumps_struct,
        signer_helpers_impl,
        epilogue_method,
        has_epilogue_expr,
        client_macro,
        ix_arg_extraction,
        extract_ix_args_fn,
        assert_builder_fn,
    } = output;

    let exact_len_guard = quote! {
        #krate::traits::check_account_count(accounts.len(), Self::COUNT)?;
    };

    let has_epilogue_const = quote! {
        const HAS_EPILOGUE: bool = #has_epilogue_expr;
    };

    let parse_accounts_impl = quote! {
        impl #parse_impl_generics #krate::traits::ParseAccounts<'input> for #name #ty_generics #parse_where_clause {
            type Bumps = #bumps_name;
            #has_epilogue_const

            #[inline(always)]
            fn parse(accounts: &'input mut [#krate::__internal::AccountView], program_id: &#krate::prelude::Address) -> Result<(Self, Self::Bumps), #krate::__solana_program_error::ProgramError> {
                #exact_len_guard
                // SAFETY: the exact-count guard above proves the unchecked parser
                // receives the account count it was generated for.
                unsafe {
                    <Self as #krate::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                        accounts,
                        &[],
                        program_id,
                    )
                }
            }

            #[inline(always)]
            fn parse_with_instruction_data(
                accounts: &'input mut [#krate::__internal::AccountView],
                __ix_data: &[u8],
                __program_id: &#krate::prelude::Address,
            ) -> Result<(Self, Self::Bumps), #krate::__solana_program_error::ProgramError> {
                #exact_len_guard
                // SAFETY: the exact-count guard above proves the unchecked parser
                // receives the account count it was generated for.
                unsafe {
                    <Self as #krate::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                        accounts,
                        __ix_data,
                        __program_id,
                    )
                }
            }

            #epilogue_method
        }

        unsafe impl #parse_impl_generics #krate::traits::ParseAccountsUnchecked<'input>
            for #name #ty_generics
            #parse_where_clause
        {
            #[inline(always)]
            unsafe fn parse_unchecked(
                accounts: &'input mut [#krate::__internal::AccountView],
                program_id: &#krate::prelude::Address,
            ) -> Result<(Self, Self::Bumps), #krate::__solana_program_error::ProgramError> {
                <Self as #krate::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                    accounts,
                    &[],
                    program_id,
                )
            }

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

        impl #impl_generics #name #ty_generics #where_clause {
            #extract_ix_args_fn
            #assert_builder_fn

            #[inline(always)]
            #[doc(hidden)]
            pub unsafe fn parse_accounts(
                mut input: *mut u8,
                buf: &mut core::mem::MaybeUninit<[#krate::__internal::AccountView; #count_expr]>,
                __program_id: &#krate::prelude::Address,
            ) -> Result<*mut u8, #krate::__solana_program_error::ProgramError> {
                let base = buf.as_mut_ptr() as *mut #krate::__internal::AccountView;

                #(#parse_steps)*

                Ok(input)
            }

            #[inline(always)]
            #[doc(hidden)]
            pub unsafe fn parse_direct_with_instruction_data_unchecked(
                mut input: *mut u8,
                __ix_data: &[u8],
                __program_id: &#krate::prelude::Address,
            ) -> Result<(Self, #bumps_name), #krate::__solana_program_error::ProgramError> {
                #ix_arg_extraction
                #direct_parse_body
            }
        }

        unsafe impl #impl_generics #krate::traits::ParseAccountsRaw for #name #ty_generics #where_clause {
            #[inline(always)]
            unsafe fn parse_accounts_raw(
                input: *mut u8,
                base: *mut #krate::__internal::AccountView,
                offset: usize,
                __program_id: &#krate::prelude::Address,
            ) -> Result<*mut u8, #krate::__solana_program_error::ProgramError> {
                let mut __inner_buf = core::mem::MaybeUninit::<
                    [#krate::__internal::AccountView; #count_expr]
                >::uninit();
                let input = Self::parse_accounts(input, &mut __inner_buf, __program_id)?;
                // SAFETY: parse_accounts initializes every element before
                // returning Ok.
                let __inner = core::mem::ManuallyDrop::new(__inner_buf.assume_init());
                let mut __j = 0usize;
                while __j < #count_expr {
                    // SAFETY: `__j < count_expr`; the caller's `base + offset`
                    // points into the preallocated outer account buffer.
                    core::ptr::write(
                        base.add(offset + __j),
                        // SAFETY: `__inner` owns `count_expr` initialized
                        // AccountView values.
                        core::ptr::read(__inner.as_ptr().add(__j)),
                    );
                    __j += 1;
                }
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
                let program_id = program_id.ok_or(#krate::__solana_program_error::ProgramError::InvalidInstructionData)?;
                let (item, _bumps) =
                    <Self as #krate::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                        accounts,
                        data,
                        program_id,
                    )?;
                Ok(item)
            }
        }

        #client_macro
    }
}
