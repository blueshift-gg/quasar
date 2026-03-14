//! Codegen for fixed-layout `#[account]` types.
//!
//! Fixed accounts have all fields known at compile time (no dynamic
//! String/Vec). Generates the `#[repr(C)]` ZC companion struct with Pod fields,
//! plus `Owner`, `Discriminator`, `Space`, and `AccountSerialize` impls.

use {
    crate::helpers::{map_to_pod_type, pascal_to_snake, zc_assign_from_value},
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::DeriveInput,
};

pub(super) fn generate_fixed_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    fields_data: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    input: &DeriveInput,
) -> TokenStream {
    let vis = &input.vis;
    let attrs = &input.attrs;
    let zc_name = format_ident!("{}Zc", name);
    let zc_mod = format_ident!("__{}_zc", pascal_to_snake(&name.to_string()));

    let field_names: Vec<_> = fields_data.iter().map(|f| &f.ident).collect();
    let field_types: Vec<_> = fields_data.iter().map(|f| &f.ty).collect();

    let zc_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .map(|f| {
            let fname = &f.ident;
            let fvis = &f.vis;
            let zc_ty = map_to_pod_type(&f.ty);
            quote! { #fvis #fname: #zc_ty }
        })
        .collect();

    let set_inner_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .map(|f| zc_assign_from_value(f.ident.as_ref().unwrap(), &f.ty))
        .collect();

    quote! {
        // --- View type: repr(transparent) over AccountView ---

        #(#attrs)*
        #[repr(transparent)]
        #vis struct #name {
            __view: AccountView,
        }

        unsafe impl StaticView for #name {}

        impl AsAccountView for #name {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.__view
            }
        }

        impl core::ops::Deref for #name {
            type Target = #zc_mod::#zc_name;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_mod::#zc_name) }
            }
        }

        impl core::ops::DerefMut for #name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_mod::#zc_name) }
            }
        }

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
                if __data.len() < #disc_len + core::mem::size_of::<#zc_mod::#zc_name>() {
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

        // --- ZC companion struct (hidden module — not importable as state::EscrowZc) ---

        #[doc(hidden)]
        pub mod #zc_mod {
            use super::*;

            #[repr(C)]
            #[derive(Copy, Clone)]
            pub struct #zc_name {
                #(#zc_fields,)*
            }

            const _: () = assert!(
                core::mem::align_of::<#zc_name>() == 1,
                "ZC companion struct must have alignment 1; all fields must use Pod types or alignment-1 types"
            );
        }

        // --- set_inner on view type (for mutations after init) ---

        impl #name {
            #[inline(always)]
            pub fn set_inner(&mut self, #(#field_names: #field_types),*) {
                let __zc = unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_mod::#zc_name) };
                #(#set_inner_stmts)*
            }
        }

        // --- Keep ZeroCopyDeref for backward compat (InterfaceAccount, SPL types) ---

        impl ZeroCopyDeref for #name {
            type Target = #zc_mod::#zc_name;

            #[inline(always)]
            fn deref_from(view: &AccountView) -> &Self::Target {
                unsafe { &*(view.data_ptr().add(#disc_len) as *const #zc_mod::#zc_name) }
            }

            #[inline(always)]
            fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
                unsafe { &mut *(view.data_mut_ptr().add(#disc_len) as *mut #zc_mod::#zc_name) }
            }
        }
    }
    .into()
}
