use {super::fixed::PodFieldInfo, crate::helpers::map_to_pod_type, quote::quote};

pub(super) struct AccountLoadSpec<'a> {
    pub name: &'a syn::Ident,
    pub disc_len: usize,
    pub disc_indices: &'a [usize],
    pub disc_bytes: &'a [syn::LitInt],
    pub zc_mod: &'a syn::Ident,
}

pub(super) fn emit_discriminator_impl(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    bump_offset_impl: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        impl Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
            #bump_offset_impl
        }
    }
}

pub(super) fn emit_owner_impl(name: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }
    }
}

pub(super) fn emit_space_impl(
    name: &syn::Ident,
    field_infos: &[PodFieldInfo<'_>],
    has_dynamic: bool,
    disc_len: usize,
    zc_mod: &syn::Ident,
) -> proc_macro2::TokenStream {
    if has_dynamic {
        // Space = discriminator + compact header size (includes length prefixes).
        quote! {
            impl Space for #name {
                const SPACE: usize = #disc_len
                    + <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
            }
        }
    } else {
        let field_pod_types: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .map(|fi| map_to_pod_type(&fi.field.ty))
            .collect();
        quote! {
            impl Space for #name {
                const SPACE: usize = #disc_len #(+ core::mem::size_of::<#field_pod_types>())*;
            }
        }
    }
}

pub(super) fn emit_dynamic_account_load(spec: AccountLoadSpec<'_>) -> proc_macro2::TokenStream {
    let AccountLoadSpec {
        name,
        disc_len,
        disc_indices,
        disc_bytes,
        zc_mod,
    } = spec;

    let body = emit_account_load_check_body(false, disc_len, disc_indices, disc_bytes, zc_mod);
    let checked_body =
        emit_account_load_check_body(true, disc_len, disc_indices, disc_bytes, zc_mod);

    quote! {
        impl quasar_lang::account_load::AccountLoad for #name {
            #[inline(always)]
            fn check(view: &quasar_lang::__internal::AccountView) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
                #body
            }

            #[inline(always)]
            fn check_checked(view: &quasar_lang::__internal::AccountView) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
                #checked_body
            }
        }
    }
}

fn emit_account_load_check_body(
    checked: bool,
    disc_len: usize,
    disc_indices: &[usize],
    disc_bytes: &[syn::LitInt],
    zc_mod: &syn::Ident,
) -> proc_macro2::TokenStream {
    let borrow = if checked {
        quote! {
            let __data_ref = view.try_borrow()?;
            let __data: &[u8] = &__data_ref;
        }
    } else {
        quote! {
            // SAFETY: generated account parsing calls unchecked validation only
            // when no checked borrow is live.
            let __data = unsafe { view.borrow_unchecked() };
        }
    };

    let validate = quote! {
        let __min = #disc_len
            + <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
        if __data.len() < __min {
            return Err(ProgramError::AccountDataTooSmall);
        }
        #(
            // SAFETY: `__data.len() >= __min` and every discriminator index is
            // strictly less than `disc_len`.
            if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                return Err(ProgramError::InvalidAccountData);
            }
        )*
        <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::validate(
            // SAFETY: `__data.len() >= __min`, so the compact payload range
            // starting at `disc_len` is in bounds.
            unsafe { __data.get_unchecked(#disc_len..) }
        ).map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(())
    };

    quote! {
        #borrow
        #validate
    }
}
