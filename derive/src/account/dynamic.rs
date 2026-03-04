use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::DeriveInput;

use super::accessors;
use crate::helpers::{map_to_pod_type, zc_serialize_field, DynKind, PrefixType};

fn kind_prefix(kind: &DynKind) -> &PrefixType {
    match kind {
        DynKind::Str { prefix, .. } => prefix,
        DynKind::Vec { prefix, .. } => prefix,
        _ => unreachable!(),
    }
}

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

    let dyn_fields: Vec<(&syn::Field, &DynKind)> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .collect();

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

    // --- 2. ZC companion fields (ONLY fixed fields — no _end descriptors) ---
    let zc_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| matches!(k, DynKind::Fixed))
        .map(|(f, _)| {
            let fvis = &f.vis;
            let fname = f.ident.as_ref().unwrap();
            let zc_ty = map_to_pod_type(&f.ty);
            quote! { #fvis #fname: #zc_ty }
        })
        .collect();

    // --- 3. ZC header serialize (fixed fields only) ---
    let zc_header_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| matches!(k, DynKind::Fixed))
        .map(|(f, _)| {
            let fname = f.ident.as_ref().unwrap();
            zc_serialize_field(fname, &f.ty)
        })
        .collect();

    // --- 4. Variable tail serialize (inline prefix + data per dynamic field) ---
    let var_serialize_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            let prefix = kind_prefix(kind);
            let pb = prefix.bytes();
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => {
                    let write_prefix = prefix.gen_write_prefix(&quote! { self.#fname.len() });
                    quote! {
                        {
                            #write_prefix
                            __offset += #pb;
                            let __len = self.#fname.len();
                            __data[__offset..__offset + __len].copy_from_slice(self.#fname.as_bytes());
                            __offset += __len;
                        }
                    }
                }
                DynKind::Vec { elem, .. } => {
                    let write_prefix = prefix.gen_write_prefix(&quote! { self.#fname.len() });
                    quote! {
                        {
                            #write_prefix
                            __offset += #pb;
                            let __bytes = self.#fname.len() * core::mem::size_of::<#elem>();
                            if __bytes > 0 {
                                unsafe {
                                    core::ptr::copy_nonoverlapping(
                                        self.#fname.as_ptr() as *const u8,
                                        __data[__offset..].as_mut_ptr(),
                                        __bytes,
                                    );
                                }
                            }
                            __offset += __bytes;
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 5. Max length checks for init ---
    let max_checks: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { max, .. } | DynKind::Vec { max, .. } => quote! {
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

    // --- 6. Dynamic space terms (prefix bytes + data bytes per field) ---
    let prefix_space: usize = dyn_fields.iter().map(|(_, k)| kind_prefix(k).bytes()).sum();

    let space_terms: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
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

    // --- 7. MAX_SPACE terms (prefix bytes + max data per field) ---
    let max_space_terms: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(_, kind)| match kind {
            DynKind::Str { max, .. } => quote! { + #max },
            DynKind::StrRef => quote! { + 255usize },
            DynKind::Vec { elem, max, .. } => {
                quote! { + #max * core::mem::size_of::<#elem>() }
            }
            _ => unreachable!(),
        })
        .collect();

    let vec_align_asserts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter_map(|(_, kind)| match kind {
            DynKind::Vec { elem, .. } => Some(quote! {
                const _: () = assert!(
                    core::mem::align_of::<#elem>() == 1,
                    "dynamic Vec element type must have alignment 1"
                );
            }),
            _ => None,
        })
        .collect();

    // --- 8. AccountCheck: walk inline prefixes to validate bounds ---
    let dyn_validation_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            let prefix = kind_prefix(kind);
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();
            let _ = fname;
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => {
                    let max_val = match kind {
                        DynKind::Str { max, .. } => *max,
                        DynKind::StrRef => 255,
                        _ => unreachable!(),
                    };
                    quote! {
                        {
                            if __offset + #pb > __data_len {
                                return Err(ProgramError::AccountDataTooSmall);
                            }
                            let __len = #read;
                            __offset += #pb;
                            if __len > #max_val {
                                return Err(ProgramError::InvalidAccountData);
                            }
                            if __offset + __len > __data_len {
                                return Err(ProgramError::AccountDataTooSmall);
                            }
                            if core::str::from_utf8(&__data[__offset..__offset + __len]).is_err() {
                                return Err(ProgramError::InvalidAccountData);
                            }
                            __offset += __len;
                        }
                    }
                }
                DynKind::Vec { elem, max, .. } => {
                    let max_val = *max;
                    quote! {
                        {
                            if __offset + #pb > __data_len {
                                return Err(ProgramError::AccountDataTooSmall);
                            }
                            let __count = #read;
                            __offset += #pb;
                            if __count > #max_val {
                                return Err(ProgramError::InvalidAccountData);
                            }
                            let __byte_len = __count * core::mem::size_of::<#elem>();
                            if __offset + __byte_len > __data_len {
                                return Err(ProgramError::AccountDataTooSmall);
                            }
                            __offset += __byte_len;
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 9-12. Accessor methods, write setters, batch fields, set_dynamic_fields ---
    let acc = accessors::generate_accessors(name, disc_len, fields_data, field_kinds, &zc_name, lt);

    let accessor_methods = &acc.accessor_methods;
    let raw_methods = &acc.raw_methods;
    let write_methods = &acc.write_methods;
    let fields_name = &acc.fields_name;
    let fields_struct_fields = &acc.fields_struct_fields;
    let fields_extract_stmts = &acc.fields_extract_stmts;
    let fields_field_names = &acc.fields_field_names;
    let set_dyn_params = &acc.set_dyn_params;
    let set_dyn_buf_stmts = &acc.set_dyn_buf_stmts;

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

        #(#vec_align_asserts)*

        #vis struct #fields_name<#lt> {
            #(#fields_struct_fields,)*
        }

        impl Discriminator for #name<'_> {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Space for #name<'_> {
            const SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_space;
        }

        impl Owner for #name<'_> {
            const OWNER: Address = crate::ID;
        }

        impl AccountCheck for #name<'_> {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                let __data_len = __data.len();
                let __min = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_space;
                if __data_len < __min {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                #(
                    if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                        return Err(ProgramError::InvalidAccountData);
                    }
                )*
                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                #(#dyn_validation_stmts)*
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
            #(#raw_methods)*
            #(#write_methods)*

            #[inline(always)]
            pub fn dynamic_fields(&self) -> #fields_name<'_> {
                let __data = unsafe { self.__view.borrow_unchecked() };
                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                #(#fields_extract_stmts)*
                let _ = __offset;
                #fields_name { #(#fields_field_names),* }
            }

            #[inline(always)]
            pub fn set_dynamic_fields(&mut self, __payer: &impl AsAccountView, #(#set_dyn_params),*) -> Result<(), ProgramError> {
                let __view = &self.__view;
                let __data = unsafe { __view.borrow_unchecked() };

                const __MAX_TAIL: usize = #prefix_space #(#max_space_terms)*;
                #[cfg(not(feature = "alloc"))]
                const _: () = assert!(
                    __MAX_TAIL <= quasar_core::dynamic::MAX_DYNAMIC_TAIL,
                    "dynamic fields max size exceeds stack buffer; enable alloc feature or reduce limits"
                );

                #[cfg(feature = "alloc")]
                let mut __buf_vec = alloc::vec![0u8; __MAX_TAIL];
                #[cfg(feature = "alloc")]
                let mut __buf: &mut [u8] = __buf_vec.as_mut_slice();

                #[cfg(not(feature = "alloc"))]
                let mut __buf = [0u8; __MAX_TAIL];
                #[cfg(not(feature = "alloc"))]
                let mut __buf: &mut [u8] = __buf.as_mut_slice();
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

                if __new_total < __old_total {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }

                Ok(())
            }
        }

        impl core::ops::Deref for #view_name {
            type Target = #zc_name;

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
            pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_space;
            pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

            #[inline(always)]
            fn __dynamic_space(&self) -> usize {
                Self::MIN_SPACE #(#space_terms)*
            }

            #[inline(always)]
            fn __serialize_dynamic(&self, __data: &mut [u8]) -> Result<(), ProgramError> {
                let __zc = unsafe { &mut *(__data.as_mut_ptr() as *mut #zc_name) };
                #(#zc_header_stmts)*
                let mut __offset = core::mem::size_of::<#zc_name>();
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
