use {super::fixed::PodFieldInfo, crate::helpers::map_to_pod_type, quote::quote};

pub(super) struct AccountCheckSpec<'a> {
    pub name: &'a syn::Ident,
    pub has_dynamic: bool,
    pub disc_len: usize,
    pub disc_indices: &'a [usize],
    pub disc_bytes: &'a [syn::LitInt],
    pub zc_path: &'a proc_macro2::TokenStream,
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

pub(super) fn emit_account_check_impl(spec: AccountCheckSpec<'_>) -> proc_macro2::TokenStream {
    let AccountCheckSpec {
        name,
        has_dynamic,
        disc_len,
        disc_indices,
        disc_bytes,
        zc_path,
        zc_mod,
    } = spec;

    if has_dynamic {
        // Single ZeroPodCompact::validate call replaces all manual prefix walks.
        // It validates: header size, all prefix values <= max, tail bounds, UTF-8.
        quote! {
            impl AccountCheck for #name {
                type Params = ();

                #[inline(always)]
                fn check(view: &AccountView) -> Result<(), ProgramError> {
                    let __data = unsafe { view.borrow_unchecked() };
                    let __min = #disc_len
                        + <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
                    if __data.len() < __min {
                        return Err(ProgramError::AccountDataTooSmall);
                    }
                    #(
                        if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                            return Err(ProgramError::InvalidAccountData);
                        }
                    )*
                    <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::validate(
                        &__data[#disc_len..]
                    ).map_err(|_| ProgramError::InvalidAccountData)?;
                    Ok(())
                }
            }
        }
    } else {
        quote! {
            impl AccountCheck for #name {
                type Params = ();

                #[inline(always)]
                fn check(view: &AccountView) -> Result<(), ProgramError> {
                    let __data = unsafe { view.borrow_unchecked() };
                    if __data.len() < #disc_len + core::mem::size_of::<#zc_path>() {
                        return Err(ProgramError::AccountDataTooSmall);
                    }
                    #(
                        if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                            return Err(ProgramError::InvalidAccountData);
                        }
                    )*
                    <#zc_mod::__Schema as quasar_lang::ZeroPodFixed>::validate(
                        &__data[#disc_len..#disc_len + core::mem::size_of::<#zc_path>()]
                    ).map_err(|_| ProgramError::InvalidAccountData)?;
                    Ok(())
                }
            }
        }
    }
}
