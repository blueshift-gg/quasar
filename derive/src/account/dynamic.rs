use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::DeriveInput;

use super::accessors;
use crate::helpers::{map_to_pod_type, zc_serialize_field, DynKind};

pub(super) fn generate_dynamic_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    fields_data: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_kinds: &[DynKind],
    input: &DeriveInput,
) -> TokenStream {
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;
    let lt = &input.generics.lifetimes().next().unwrap().lifetime;
    let zc_name = format_ident!("{}Zc", name);
    let view_name = format_ident!("{}View", name);

    // --- 1. Transformed struct fields ---
    let transformed_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| {
            let fname = &f.ident;
            let fvis = &f.vis;
            match kind {
                DynKind::Fixed => {
                    let fty = &f.ty;
                    quote! { #fvis #fname: #fty }
                }
                DynKind::Str { .. } | DynKind::StrRef => {
                    quote! { #fvis #fname: &#lt str }
                }
                DynKind::Vec { elem, .. } => {
                    quote! { #fvis #fname: &#lt [#elem] }
                }
            }
        })
        .collect();

    // --- 2. ZC companion fields ---
    let zc_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| {
            let fvis = &f.vis;
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Fixed => {
                    let zc_ty = map_to_pod_type(&f.ty);
                    quote! { #fvis #fname: #zc_ty }
                }
                DynKind::Str { .. } | DynKind::StrRef | DynKind::Vec { .. } => {
                    let end_name = format_ident!("{}_end", fname);
                    quote! { #fvis #end_name: quasar_core::pod::PodU16 }
                }
            }
        })
        .collect();

    // --- 3. ZC header serialize (fixed fields + cumulative byte-offset descriptors) ---
    let mut first_dyn_seen = false;
    let zc_header_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Fixed => zc_serialize_field(fname, &f.ty),
                DynKind::Str { .. } | DynKind::StrRef => {
                    let end_name = format_ident!("{}_end", fname);
                    let init = if !first_dyn_seen {
                        first_dyn_seen = true;
                        quote! { let mut __cum: u16 = 0; }
                    } else {
                        quote! {}
                    };
                    quote! {
                        #init
                        __cum += self.#fname.len() as u16;
                        __zc.#end_name = quasar_core::pod::PodU16::from(__cum);
                    }
                }
                DynKind::Vec { elem, .. } => {
                    let end_name = format_ident!("{}_end", fname);
                    let init = if !first_dyn_seen {
                        first_dyn_seen = true;
                        quote! { let mut __cum: u16 = 0; }
                    } else {
                        quote! {}
                    };
                    quote! {
                        #init
                        __cum += (self.#fname.len() * core::mem::size_of::<#elem>()) as u16;
                        __zc.#end_name = quasar_core::pod::PodU16::from(__cum);
                    }
                }
            }
        })
        .collect();

    // --- 4. Variable tail serialize ---
    let var_serialize_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => quote! {
                    {
                        let __len = self.#fname.len();
                        __data[__var_offset..__var_offset + __len].copy_from_slice(self.#fname.as_bytes());
                        __var_offset += __len;
                    }
                },
                DynKind::Vec { elem, .. } => quote! {
                    {
                        let __bytes = self.#fname.len() * core::mem::size_of::<#elem>();
                        if __bytes > 0 {
                            // SAFETY: Source and destination do not overlap. Alignment 1 guaranteed.
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    self.#fname.as_ptr() as *const u8,
                                    __data[__var_offset..].as_mut_ptr(),
                                    __bytes,
                                );
                            }
                        }
                        __var_offset += __bytes;
                    }
                },
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 5. Max length checks for init ---
    let max_checks: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { max } | DynKind::Vec { max, .. } => quote! {
                    if self.#fname.len() > #max {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                },
                DynKind::StrRef => quote! {
                    if self.#fname.len() > 255 {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                },
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 6. Dynamic space terms ---
    let space_terms: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => quote! { + self.#fname.len() },
                DynKind::Vec { elem, .. } => {
                    quote! { + self.#fname.len() * core::mem::size_of::<#elem>() }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 7. MAX_SPACE terms ---
    let max_space_terms: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(_, kind)| match kind {
            DynKind::Str { max } => quote! { + #max },
            DynKind::StrRef => quote! { + 255usize },
            DynKind::Vec { elem, max } => {
                quote! { + #max * core::mem::size_of::<#elem>() }
            }
            _ => unreachable!(),
        })
        .collect();

    // --- 8. AccountCheck: last cumulative _end covers all dynamic bytes ---
    let last_dyn_fname = fields_data
        .iter()
        .zip(field_kinds.iter())
        .rev()
        .find(|(_, k)| !matches!(k, DynKind::Fixed))
        .unwrap()
        .0
        .ident
        .as_ref()
        .unwrap();
    let last_end_name = format_ident!("{}_end", last_dyn_fname);
    let var_check_term = quote! { + __zc.#last_end_name.get() as usize };

    // --- 9-12. Accessor methods, write setters, batch fields, set_dynamic_fields ---
    let acc = accessors::generate_accessors(name, disc_len, fields_data, field_kinds, &zc_name, lt);

    let accessor_methods = &acc.accessor_methods;
    let write_methods = &acc.write_methods;
    let fields_name = &acc.fields_name;
    let fields_struct_fields = &acc.fields_struct_fields;
    let fields_extract_stmts = &acc.fields_extract_stmts;
    let fields_field_names = &acc.fields_field_names;
    let set_dyn_params = &acc.set_dyn_params;
    let set_dyn_buf_stmts = &acc.set_dyn_buf_stmts;
    let set_dyn_zc_updates = &acc.set_dyn_zc_updates;

    // --- Combine ---
    quote! {
        #(#attrs)*
        #vis struct #name #generics {
            #(#transformed_fields,)*
        }

        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #zc_name {
            #(#zc_fields,)*
        }

        const _: () = assert!(
            core::mem::align_of::<#zc_name>() == 1,
            "ZC companion struct must have alignment 1; all fields must use Pod types or alignment-1 types"
        );

        #vis struct #fields_name<#lt> {
            #(#fields_struct_fields,)*
        }

        impl Discriminator for #name<'_> {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Space for #name<'_> {
            const SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>();
        }

        impl Owner for #name<'_> {
            const OWNER: Address = crate::ID;
        }

        impl AccountCheck for #name<'_> {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                let __min = #disc_len + core::mem::size_of::<#zc_name>();
                if __data.len() < __min {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                #(
                    if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                        return Err(ProgramError::InvalidAccountData);
                    }
                )*
                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                let __total = __min #var_check_term;
                if __total > __data.len() {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                Ok(())
            }
        }

        #[repr(transparent)]
        #vis struct #view_name {
            __view: AccountView,
        }

        impl AsAccountView for #view_name {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.__view
            }
        }

        impl #view_name {
            #[inline(always)]
            pub fn realloc(
                &self,
                new_space: usize,
                payer: &AccountView,
                rent: Option<&Rent>,
            ) -> Result<(), ProgramError> {
                quasar_core::accounts::account::realloc_account(&self.__view, new_space, payer, rent)
            }

            #(#accessor_methods)*
            #(#write_methods)*

            #[inline(always)]
            pub fn dynamic_fields(&self) -> #fields_name<'_> {
                let __data = unsafe { self.__view.borrow_unchecked() };
                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                let __tail_start = #disc_len + core::mem::size_of::<#zc_name>();
                let mut __offset = __tail_start;
                #(#fields_extract_stmts)*
                let _ = __offset;
                #fields_name { #(#fields_field_names),* }
            }

            #[inline(always)]
            pub fn set_dynamic_fields(&mut self, __payer: &impl AsAccountView, #(#set_dyn_params),*) -> Result<(), ProgramError> {
                let __view = &self.__view;
                let __data = unsafe { __view.borrow_unchecked() };
                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };

                let mut __buf = [0u8; 0 #(#max_space_terms)*];
                let mut __buf_offset = 0usize;
                let mut __old_offset = #disc_len + core::mem::size_of::<#zc_name>();

                #(#set_dyn_buf_stmts)*

                let _ = __old_offset;
                let __new_total = #disc_len + core::mem::size_of::<#zc_name>() + __buf_offset;
                let __old_total = __data.len();

                if __new_total > __old_total {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }

                let __data = unsafe { __view.borrow_unchecked_mut() };
                let __tail_start = #disc_len + core::mem::size_of::<#zc_name>();
                __data[__tail_start..__tail_start + __buf_offset]
                    .copy_from_slice(&__buf[..__buf_offset]);

                let __zc = unsafe { &mut *(__data[#disc_len..].as_mut_ptr() as *mut #zc_name) };
                #(#set_dyn_zc_updates)*

                if __new_total < __old_total {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }

                Ok(())
            }
        }

        impl core::ops::Deref for #view_name {
            type Target = #zc_name;

            /// SAFETY: Bounds validated by AccountCheck::check. ZC struct has alignment 1.
            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_name) }
            }
        }

        impl core::ops::DerefMut for #view_name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_ptr().add(#disc_len) as *mut #zc_name) }
            }
        }

        impl ZeroCopyDeref for #name<'_> {
            type Target = #view_name;

            /// SAFETY: #view_name is #[repr(transparent)] over AccountView.
            #[inline(always)]
            fn deref_from(view: &AccountView) -> &Self::Target {
                unsafe { &*(view as *const AccountView as *const #view_name) }
            }

            #[inline(always)]
            fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
                unsafe { &mut *(view as *const AccountView as *mut #view_name) }
            }
        }

        impl #name<'_> {
            pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>();
            pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

            #[inline(always)]
            fn __dynamic_space(&self) -> usize {
                Self::MIN_SPACE #(#space_terms)*
            }

            #[inline(always)]
            fn __serialize_dynamic(&self, __data: &mut [u8]) -> Result<(), ProgramError> {
                let __zc = unsafe { &mut *(__data.as_mut_ptr() as *mut #zc_name) };
                #(#zc_header_stmts)*
                let mut __var_offset = core::mem::size_of::<#zc_name>();
                #(#var_serialize_stmts)*
                Ok(())
            }

            #[inline(always)]
            pub fn init<'__init>(self, account: &mut Initialize<#name<'__init>>, payer: &AccountView, rent: Option<&Rent>) -> Result<(), ProgramError> {
                self.init_signed(account, payer, rent, &[])
            }

            #[inline(always)]
            pub fn init_signed<'__init>(self, account: &mut Initialize<#name<'__init>>, payer: &AccountView, rent: Option<&Rent>, signers: &[quasar_core::cpi::Signer]) -> Result<(), ProgramError> {
                #(#max_checks)*

                let view = account.to_account_view();
                let __space = self.__dynamic_space();

                {
                    let __existing = unsafe { view.borrow_unchecked() };
                    if __existing.len() >= #disc_len {
                        #(
                            if unsafe { *__existing.get_unchecked(#disc_indices) } != 0 {
                                return Err(QuasarError::AccountAlreadyInitialized.into());
                            }
                        )*
                    }
                }

                let lamports = match rent {
                    Some(rent_data) => rent_data.minimum_balance_unchecked(__space),
                    None => {
                        use quasar_core::sysvars::Sysvar;
                        quasar_core::sysvars::rent::Rent::get()?.minimum_balance_unchecked(__space)
                    }
                };

                if view.lamports() == 0 {
                    quasar_core::cpi::system::create_account(payer, view, lamports, __space as u64, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                } else {
                    let required = lamports.saturating_sub(view.lamports());
                    if required > 0 {
                        quasar_core::cpi::system::transfer(payer, view, required)
                            .invoke_with_signers(signers)?;
                    }
                    quasar_core::cpi::system::assign(view, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                    unsafe { view.resize_unchecked(__space) }?;
                }

                let __data = unsafe { view.borrow_unchecked_mut() };
                __data[..Self::DISCRIMINATOR.len()].copy_from_slice(Self::DISCRIMINATOR);
                self.__serialize_dynamic(&mut __data[Self::DISCRIMINATOR.len()..])?;
                Ok(())
            }
        }
    }
    .into()
}
