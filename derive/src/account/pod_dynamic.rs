//! Codegen for Pod-based dynamic-layout `#[account]` types.
//!
//! Replaces the old `dynamic.rs` + `accessors.rs` system (~1200 lines) with
//! ~200 lines. Accounts with `PodString<N>` / `PodVec<T, N>` fields use
//! dynamic sizing (only actual content allocated), walk-from-header accessors
//! (no offset cache), and a runtime memmove helper for writes that grow.

use {
    crate::helpers::{map_to_pod_type, zc_assign_from_value, PodDynField},
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::DeriveInput,
};

/// Info about each field needed for codegen.
pub(super) struct PodFieldInfo<'a> {
    pub field: &'a syn::Field,
    pub pod_dyn: Option<PodDynField>,
}

pub(super) fn generate_pod_dynamic_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    field_infos: &[PodFieldInfo<'_>],
    input: &DeriveInput,
    gen_set_inner: bool,
) -> TokenStream {
    let vis = &input.vis;
    let attrs = &input.attrs;
    let zc_name = format_ident!("{}Zc", name);

    // --- Fixed fields → ZC companion ---
    let zc_fields: Vec<proc_macro2::TokenStream> = field_infos
        .iter()
        .filter(|fi| fi.pod_dyn.is_none())
        .map(|fi| {
            let f = fi.field;
            let fvis = &f.vis;
            let fname = f.ident.as_ref().unwrap();
            let zc_ty = map_to_pod_type(&f.ty);
            quote! { #fvis #fname: #zc_ty }
        })
        .collect();

    // --- Dynamic fields info ---
    let dyn_fields: Vec<(&syn::Field, &PodDynField)> = field_infos
        .iter()
        .filter_map(|fi| fi.pod_dyn.as_ref().map(|pd| (fi.field, pd)))
        .collect();

    // --- Alignment assertions for PodVec element types ---
    let align_asserts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .filter_map(|(_, pd)| match pd {
            PodDynField::Vec { elem, .. } => Some(quote! {
                const _: () = assert!(
                    core::mem::align_of::<#elem>() == 1,
                    "PodVec element type must have alignment 1"
                );
            }),
            _ => None,
        })
        .collect();

    // --- AccountCheck: validate dynamic field prefixes ---
    let validation_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(_f, pd)| {
            match pd {
                PodDynField::Str { max } => quote! {
                    {
                        if __offset + 1 > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        let __len = __data[__offset] as usize;
                        __offset += 1;
                        if __len > #max {
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
                },
                PodDynField::Vec { elem, max } => quote! {
                    {
                        if __offset + 2 > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        let __count = u16::from_le_bytes([__data[__offset], __data[__offset + 1]]) as usize;
                        __offset += 2;
                        if __count > #max {
                            return Err(ProgramError::InvalidAccountData);
                        }
                        let __byte_len = __count * core::mem::size_of::<#elem>();
                        if __offset + __byte_len > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        __offset += __byte_len;
                    }
                },
            }
        })
        .collect();

    // --- MIN_SPACE: disc + ZC header + sum of prefix sizes ---
    let prefix_total: usize = dyn_fields
        .iter()
        .map(|(_, pd)| match pd {
            PodDynField::Str { .. } => 1usize,
            PodDynField::Vec { .. } => 2usize,
        })
        .sum();

    // --- MAX_SPACE terms: max data per field ---
    let max_space_terms: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(_, pd)| match pd {
            PodDynField::Str { max } => quote! { + #max },
            PodDynField::Vec { elem, max } => {
                quote! { + #max * core::mem::size_of::<#elem>() }
            }
        })
        .collect();

    // --- Walk codegen: for each dynamic field, generate the skip-past expr ---
    // Each accessor for field N walks past fields 0..N-1 from the dynamic start.
    let dyn_start = quote! { #disc_len + core::mem::size_of::<#zc_name>() };

    let read_accessors: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (f, pd))| {
            let fname = f.ident.as_ref().unwrap();

            // Generate walk-past stmts for fields 0..dyn_idx
            let walk_stmts: Vec<proc_macro2::TokenStream> = dyn_fields[..dyn_idx]
                .iter()
                .map(|(_, prev_pd)| match prev_pd {
                    PodDynField::Str { .. } => quote! {
                        __off += 1 + __data[__off] as usize;
                    },
                    PodDynField::Vec { elem, .. } => quote! {
                        __off += 2 + u16::from_le_bytes([__data[__off], __data[__off + 1]]) as usize
                            * core::mem::size_of::<#elem>();
                    },
                })
                .collect();

            match pd {
                PodDynField::Str { .. } => quote! {
                    #[inline(always)]
                    pub fn #fname(&self) -> &str {
                        let __data = unsafe { self.__view.borrow_unchecked() };
                        let mut __off = #dyn_start;
                        #(#walk_stmts)*
                        let __len = __data[__off] as usize;
                        unsafe { core::str::from_utf8_unchecked(&__data[__off + 1..__off + 1 + __len]) }
                    }
                },
                PodDynField::Vec { elem, .. } => quote! {
                    #[inline(always)]
                    pub fn #fname(&self) -> &[#elem] {
                        let __data = unsafe { self.__view.borrow_unchecked() };
                        let mut __off = #dyn_start;
                        #(#walk_stmts)*
                        let __count = u16::from_le_bytes([__data[__off], __data[__off + 1]]) as usize;
                        unsafe { core::slice::from_raw_parts(__data[__off + 2..].as_ptr() as *const #elem, __count) }
                    }
                },
            }
        })
        .collect();

    // --- Write methods: walk + pod_field_rewrite ---
    let write_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (f, pd))| {
            let fname = f.ident.as_ref().unwrap();
            let setter_name = format_ident!("set_{}", fname);

            let walk_stmts: Vec<proc_macro2::TokenStream> = dyn_fields[..dyn_idx]
                .iter()
                .map(|(_, prev_pd)| match prev_pd {
                    PodDynField::Str { .. } => quote! {
                        __off += 1 + unsafe { *__ptr.add(__off) } as usize;
                    },
                    PodDynField::Vec { elem, .. } => quote! {
                        __off += 2 + u16::from_le_bytes(unsafe {
                            [*__ptr.add(__off), *__ptr.add(__off + 1)]
                        }) as usize * core::mem::size_of::<#elem>();
                    },
                })
                .collect();

            match pd {
                PodDynField::Str { max } => quote! {
                    pub fn #setter_name(
                        &mut self,
                        value: &str,
                        payer: &AccountView,
                        rent_lpb: u64,
                        rent_threshold: u64,
                    ) -> Result<(), ProgramError> {
                        if value.len() > #max {
                            return Err(QuasarError::DynamicFieldTooLong.into());
                        }
                        let __ptr = self.__view.data_ptr();
                        let mut __off = #dyn_start;
                        #(#walk_stmts)*
                        let __old_len = unsafe { *__ptr.add(__off) } as usize;
                        // SAFETY: #name is #[repr(transparent)] over AccountView.
                        let __view = unsafe { &mut *(self as *mut Self as *mut AccountView) };
                        quasar_lang::pod_dynamic::pod_field_rewrite(
                            __view, __off, 1, __old_len,
                            &[value.len() as u8],
                            value.as_bytes(),
                            payer, rent_lpb, rent_threshold,
                        )
                    }
                },
                PodDynField::Vec { elem, max } => quote! {
                    pub fn #setter_name(
                        &mut self,
                        values: &[#elem],
                        payer: &AccountView,
                        rent_lpb: u64,
                        rent_threshold: u64,
                    ) -> Result<(), ProgramError> {
                        if values.len() > #max {
                            return Err(QuasarError::DynamicFieldTooLong.into());
                        }
                        let __ptr = self.__view.data_ptr();
                        let mut __off = #dyn_start;
                        #(#walk_stmts)*
                        let __old_count = u16::from_le_bytes(unsafe {
                            [*__ptr.add(__off), *__ptr.add(__off + 1)]
                        }) as usize;
                        let __old_bytes = __old_count * core::mem::size_of::<#elem>();
                        let __new_bytes = values.len() * core::mem::size_of::<#elem>();
                        // SAFETY: #name is #[repr(transparent)] over AccountView.
                        let __view = unsafe { &mut *(self as *mut Self as *mut AccountView) };
                        quasar_lang::pod_dynamic::pod_field_rewrite(
                            __view, __off, 2, __old_bytes,
                            &(values.len() as u16).to_le_bytes(),
                            unsafe { core::slice::from_raw_parts(values.as_ptr() as *const u8, __new_bytes) },
                            payer, rent_lpb, rent_threshold,
                        )
                    }
                },
            }
        })
        .collect();

    // --- set_inner (opt-in) ---
    let set_inner_impl = if gen_set_inner {
        let inner_name = format_ident!("{}Inner", name);

        let inner_fields: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .map(|fi| {
                let fname = fi.field.ident.as_ref().unwrap();
                match &fi.pod_dyn {
                    None => {
                        let fty = &fi.field.ty;
                        quote! { pub #fname: #fty }
                    }
                    Some(PodDynField::Str { .. }) => quote! { pub #fname: &'a str },
                    Some(PodDynField::Vec { elem, .. }) => quote! { pub #fname: &'a [#elem] },
                }
            })
            .collect();

        let max_checks: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter_map(|fi| {
                let fname = fi.field.ident.as_ref().unwrap();
                match &fi.pod_dyn {
                    Some(PodDynField::Str { max }) => Some(quote! {
                        if #fname.len() > #max { return Err(QuasarError::DynamicFieldTooLong.into()); }
                    }),
                    Some(PodDynField::Vec { max, .. }) => Some(quote! {
                        if #fname.len() > #max { return Err(QuasarError::DynamicFieldTooLong.into()); }
                    }),
                    None => None,
                }
            })
            .collect();

        // Space computation terms for dynamic fields
        let space_terms: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter_map(|fi| {
                let fname = fi.field.ident.as_ref().unwrap();
                match &fi.pod_dyn {
                    Some(PodDynField::Str { .. }) => Some(quote! { + #fname.len() }),
                    Some(PodDynField::Vec { elem, .. }) => Some(quote! {
                        + #fname.len() * core::mem::size_of::<#elem>()
                    }),
                    None => None,
                }
            })
            .collect();

        let zc_header_stmts: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter(|fi| fi.pod_dyn.is_none())
            .map(|fi| zc_assign_from_value(fi.field.ident.as_ref().unwrap(), &fi.field.ty))
            .collect();

        let var_write_stmts: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter_map(|fi| {
                let fname = fi.field.ident.as_ref().unwrap();
                match &fi.pod_dyn {
                    Some(PodDynField::Str { .. }) => Some(quote! {
                        {
                            __data[__offset] = #fname.len() as u8;
                            __offset += 1;
                            __data[__offset..__offset + #fname.len()].copy_from_slice(#fname.as_bytes());
                            __offset += #fname.len();
                        }
                    }),
                    Some(PodDynField::Vec { elem, .. }) => Some(quote! {
                        {
                            let __count_bytes = (#fname.len() as u16).to_le_bytes();
                            __data[__offset] = __count_bytes[0];
                            __data[__offset + 1] = __count_bytes[1];
                            __offset += 2;
                            let __bytes = #fname.len() * core::mem::size_of::<#elem>();
                            if __bytes > 0 {
                                unsafe {
                                    core::ptr::copy_nonoverlapping(
                                        #fname.as_ptr() as *const u8,
                                        __data[__offset..].as_mut_ptr(),
                                        __bytes,
                                    );
                                }
                            }
                            __offset += __bytes;
                        }
                    }),
                    None => None,
                }
            })
            .collect();

        let init_field_names: Vec<&syn::Ident> = field_infos
            .iter()
            .map(|fi| fi.field.ident.as_ref().unwrap())
            .collect();

        quote! {
            #vis struct #inner_name<'a> {
                #(#inner_fields,)*
            }

            impl #name {
                #[inline(always)]
                pub fn set_inner(&mut self, inner: #inner_name<'_>, payer: &AccountView, rent_lpb: u64, rent_threshold: u64) -> Result<(), ProgramError> {
                    #(let #init_field_names = inner.#init_field_names;)*
                    #(#max_checks)*

                    let __space = Self::MIN_SPACE #(#space_terms)*;
                    // SAFETY: #name is #[repr(transparent)] over AccountView.
                        let __view = unsafe { &mut *(self as *mut Self as *mut AccountView) };

                    if __space != __view.data_len() {
                        quasar_lang::accounts::account::realloc_account_raw(__view, __space, payer, rent_lpb, rent_threshold)?;
                    }

                    // Derive __zc from raw pointer (not from __data slice) to avoid
                    // overlapping &mut references (Stacked Borrows violation).
                    let __ptr = __view.data_mut_ptr();
                    let __zc = unsafe { &mut *(__ptr.add(#disc_len) as *mut #zc_name) };
                    #(#zc_header_stmts)*
                    let __dyn_start = #disc_len + core::mem::size_of::<#zc_name>();
                    let __len = __view.data_len();
                    let __data = unsafe { core::slice::from_raw_parts_mut(__ptr.add(__dyn_start), __len - __dyn_start) };
                    let mut __offset = 0usize;
                    #(#var_write_stmts)*
                    let _ = __offset;
                    Ok(())
                }
            }
        }
    } else {
        quote! {}
    };

    // --- Combine ---
    quote! {
        #(#attrs)*
        #[repr(transparent)]
        #vis struct #name {
            __view: AccountView,
        }

        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #zc_name {
            #(#zc_fields,)*
        }

        const _: () = assert!(
            core::mem::align_of::<#zc_name>() == 1,
            "ZC companion struct must have alignment 1"
        );

        #(#align_asserts)*

        // SAFETY: #name is #[repr(transparent)] over AccountView.
        // This assertion guards the pointer cast in write methods.
        const _: () = assert!(
            core::mem::size_of::<#name>() == core::mem::size_of::<AccountView>(),
            "Pod-dynamic struct must be #[repr(transparent)] over AccountView"
        );

        unsafe impl StaticView for #name {}

        impl AsAccountView for #name {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.__view
            }
        }

        impl core::ops::Deref for #name {
            type Target = #zc_name;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_name) }
            }
        }

        impl core::ops::DerefMut for #name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_name) }
            }
        }

        impl Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }

        impl Space for #name {
            const SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_total;
        }

        impl AccountCheck for #name {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                let __data_len = __data.len();
                let __min = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_total;
                if __data_len < __min {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                #(
                    if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                        return Err(ProgramError::InvalidAccountData);
                    }
                )*
                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                #(#validation_stmts)*
                let _ = __offset;
                Ok(())
            }
        }

        impl #name {
            pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_total;
            pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

            #(#read_accessors)*
            #(#write_methods)*
        }

        #set_inner_impl
    }
    .into()
}
