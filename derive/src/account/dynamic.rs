//! Codegen for dynamic-layout `#[account]` types.
//!
//! Dynamic accounts contain String/Vec fields with length prefixes.
//! Generates runtime codec helpers (read/write with offset tracking) and
//! accessor methods that operate directly on the account data buffer.

use {
    super::accessors,
    crate::helpers::{map_to_pod_type, zc_assign_from_value, DynFieldKind, DynKind, TailElement},
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::DeriveInput,
};

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
    let lt = &input.generics.lifetimes().next().unwrap().lifetime;
    let zc_name = format_ident!("{}Zc", name);

    let dyn_fields: Vec<(&syn::Field, DynFieldKind<'_>)> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter_map(|(f, k)| k.as_dynamic().map(|dk| (f, dk)))
        .collect();

    let num_dyn = dyn_fields.len();
    // N-1 cached offsets (first dynamic field starts at compile-time constant)
    let num_offsets = num_dyn.saturating_sub(1);

    // --- 1. set_inner field types (native types for fixed, slices/strs for
    // dynamic) ---
    let init_field_names: Vec<&Option<syn::Ident>> = fields_data.iter().map(|f| &f.ident).collect();
    let init_field_types: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| match kind {
            DynKind::Fixed => {
                let fty = &f.ty;
                quote! { #fty }
            }
            DynKind::Str { .. } | DynKind::Tail { .. } => {
                quote! { &str }
            }
            DynKind::Vec { elem, .. } => {
                quote! { &[#elem] }
            }
        })
        .collect();

    // --- 2. ZC companion fields (fixed fields only) ---
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

    // --- 3. ZC header serialize (fixed fields only, for set_inner) ---
    let zc_header_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| matches!(k, DynKind::Fixed))
        .map(|(f, _)| {
            let fname = f.ident.as_ref().unwrap();
            zc_assign_from_value(fname, &f.ty)
        })
        .collect();

    // --- 4. Variable tail serialize (inline prefix + data per dynamic field) ---
    let var_serialize_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynFieldKind::Str { prefix, .. } => {
                    let pb = prefix.bytes();
                    let write_prefix = prefix.gen_write_prefix(&quote! { #fname.len() });
                    quote! {
                        {
                            #write_prefix
                            __offset += #pb;
                            let __len = #fname.len();
                            __data[__offset..__offset + __len].copy_from_slice(#fname.as_bytes());
                            __offset += __len;
                        }
                    }
                }
                DynFieldKind::Tail { .. } => {
                    quote! {
                        {
                            let __len = #fname.len();
                            __data[__offset..__offset + __len].copy_from_slice(#fname.as_bytes());
                            __offset += __len;
                        }
                    }
                }
                DynFieldKind::Vec { elem, prefix, .. } => {
                    let pb = prefix.bytes();
                    let write_prefix = prefix.gen_write_prefix(&quote! { #fname.len() });
                    quote! {
                        {
                            #write_prefix
                            __offset += #pb;
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
                    }
                }
            }
        })
        .collect();

    // --- 5. Max length checks for set_inner ---
    let max_checks: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynFieldKind::Str { max, .. } | DynFieldKind::Vec { max, .. } => quote! {
                    if #fname.len() > #max {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                },
                DynFieldKind::Tail { .. } => quote! {
                    if #fname.len() > 1024 {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                },
            }
        })
        .collect();

    // --- 6. Dynamic space terms (prefix bytes + data bytes per field) ---
    let prefix_space: usize = dyn_fields.iter().map(|(_, k)| k.prefix_bytes()).sum();

    let space_terms: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynFieldKind::Str { .. } | DynFieldKind::Tail { .. } => {
                    quote! { + #fname.len() }
                }
                DynFieldKind::Vec { elem, .. } => {
                    quote! { + #fname.len() * core::mem::size_of::<#elem>() }
                }
            }
        })
        .collect();

    // --- 7. MAX_SPACE terms (prefix bytes + max data per field) ---
    let max_space_terms: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(_, kind)| match kind {
            DynFieldKind::Str { max, .. } => quote! { + #max },
            DynFieldKind::Tail { .. } => quote! { + 1024usize },
            DynFieldKind::Vec { elem, max, .. } => {
                quote! { + #max * core::mem::size_of::<#elem>() }
            }
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

    // --- 8. AccountCheck validation stmts (walks inline prefixes — runs once
    // during parse) ---
    let mut validation_stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    for (_f, kind) in &dyn_fields {
        match kind {
            DynFieldKind::Str { prefix, max, .. } => {
                let read = prefix.gen_read_len();
                let pb = prefix.bytes();
                validation_stmts.push(quote! {
                    {
                        if __offset + #pb > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        let __len = #read;
                        __offset += #pb;
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
                });
            }
            DynFieldKind::Tail { element } => {
                let validate_utf8 = matches!(element, TailElement::Str);
                if validate_utf8 {
                    validation_stmts.push(quote! {
                        {
                            let __tail = &__data[__offset..__data_len];
                            if core::str::from_utf8(__tail).is_err() {
                                return Err(ProgramError::InvalidAccountData);
                            }
                            __offset = __data_len;
                        }
                    });
                } else {
                    validation_stmts.push(quote! {
                        {
                            __offset = __data_len;
                        }
                    });
                }
            }
            DynFieldKind::Vec { elem, prefix, max } => {
                let read = prefix.gen_read_len();
                let pb = prefix.bytes();
                validation_stmts.push(quote! {
                    {
                        if __offset + #pb > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        let __count = #read;
                        __offset += #pb;
                        if __count > #max {
                            return Err(ProgramError::InvalidAccountData);
                        }
                        let __byte_len = __count * core::mem::size_of::<#elem>();
                        if __offset + __byte_len > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        __offset += __byte_len;
                    }
                });
            }
        }
    }

    // --- 9. Parse offset caching stmts (walk prefixes once, store cumulative
    // offsets) ---
    let mut parse_offset_stmts: Vec<proc_macro2::TokenStream> = Vec::new();
    for (dyn_idx, (_f, kind)) in dyn_fields.iter().enumerate() {
        match kind {
            DynFieldKind::Str { prefix, .. } => {
                let pb = prefix.bytes();
                let read = prefix.gen_read_len();
                if dyn_idx < num_offsets {
                    parse_offset_stmts.push(quote! {
                        {
                            let __len = #read;
                            __offset += #pb + __len;
                            __off[#dyn_idx] = __offset as u32;
                        }
                    });
                }
            }
            DynFieldKind::Vec { elem, prefix, .. } => {
                let pb = prefix.bytes();
                let read = prefix.gen_read_len();
                if dyn_idx < num_offsets {
                    parse_offset_stmts.push(quote! {
                        {
                            let __count = #read;
                            __offset += #pb + __count * core::mem::size_of::<#elem>();
                            __off[#dyn_idx] = __offset as u32;
                        }
                    });
                }
            }
            DynFieldKind::Tail { .. } => {
                // Tail is always last — no offset to store after it
            }
        }
    }

    // --- 10. Accessor methods (O(1) via cached offsets) ---
    let acc = accessors::generate_accessors(disc_len, fields_data, field_kinds, &zc_name);

    let accessor_methods = &acc.accessor_methods;
    let raw_methods = &acc.raw_methods;
    let write_methods = &acc.write_methods;

    // --- 11. Offset array type ---
    let off_array_type = quote! { [u32; #num_offsets] };
    let off_array_init = quote! { [0u32; #num_offsets] };

    // --- Combine ---
    quote! {
        #(#attrs)*
        #vis struct #name<#lt> {
            __view: &#lt mut AccountView,
            __off: #off_array_type,
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

        // --- View type trait impls ---

        impl Discriminator for #name<'_> {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Space for #name<'_> {
            const SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_space;
        }

        impl Owner for #name<'_> {
            const OWNER: Address = crate::ID;
        }

        impl AsAccountView for #name<'_> {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &*self.__view
            }
        }

        impl core::ops::Deref for #name<'_> {
            type Target = #zc_name;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_name) }
            }
        }

        impl core::ops::DerefMut for #name<'_> {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_name) }
            }
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
                #(#validation_stmts)*
                let _ = __offset;
                Ok(())
            }
        }

        // --- View type methods ---

        impl<#lt> #name<#lt> {
            pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>() + #prefix_space;
            pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

            /// Parse an AccountView into an offset-cached view, wrapped in Account<T>.
            ///
            /// Validates discriminator and walks inline prefixes ONCE to cache
            /// byte offsets for O(1) field access.
            #[inline(always)]
            pub fn from_account_view(view: &#lt mut AccountView) -> Result<Account<Self>, ProgramError> {
                <Self as CheckOwner>::check_owner(view)?;
                <Self as AccountCheck>::check(view)?;
                Self::__parse(view)
            }

            #[inline(always)]
            fn __parse(view: &#lt mut AccountView) -> Result<Account<Self>, ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                let mut __off = #off_array_init;
                #(#parse_offset_stmts)*
                let _ = __offset;
                Ok(Account::wrap(Self { __view: view, __off }))
            }

            #[inline(always)]
            pub fn close(&mut self, destination: &AccountView) -> Result<(), ProgramError> {
                if !destination.is_writable() {
                    return Err(ProgramError::Immutable);
                }

                let zero_len = self.__view.data_len().min(8);
                if zero_len > 0 {
                    unsafe {
                        core::ptr::write_bytes(self.__view.data_mut_ptr(), 0, zero_len);
                    }
                }

                let new_lamports = destination
                    .lamports()
                    .checked_add(self.__view.lamports())
                    .ok_or(ProgramError::InvalidArgument)?;
                quasar_core::accounts::account::set_lamports(destination, new_lamports);
                self.__view.set_lamports(0);
                unsafe { self.__view.assign(&quasar_core::cpi::system::SYSTEM_PROGRAM_ID) };
                quasar_core::accounts::account::resize(&mut *self.__view, 0)?;
                Ok(())
            }

            #[inline(always)]
            pub fn realloc(
                &mut self,
                new_space: usize,
                payer: &AccountView,
                rent: Option<&Rent>,
            ) -> Result<(), ProgramError> {
                quasar_core::accounts::account::realloc_account(&mut *self.__view, new_space, payer, rent)
            }

            #(#accessor_methods)*
            #(#raw_methods)*
            #(#write_methods)*
        }

        // --- set_inner on view type (writes all fields + reallocs if needed) ---

        impl #name<'_> {
            #[inline(always)]
            #[allow(clippy::too_many_arguments)]
            pub fn set_inner(&mut self, #(#init_field_names: #init_field_types,)* payer: &AccountView, rent: Option<&Rent>) -> Result<(), ProgramError> {
                #(#max_checks)*

                let __space = Self::MIN_SPACE #(#space_terms)*;

                if __space > self.__view.data_len() {
                    quasar_core::accounts::account::realloc_account(&mut *self.__view, __space, payer, rent)?;
                }

                let __len = self.__view.data_len();
                let __data = unsafe { core::slice::from_raw_parts_mut(self.__view.data_mut_ptr(), __len) };
                let __zc = unsafe { &mut *(__data[<#name as Discriminator>::DISCRIMINATOR.len()..].as_mut_ptr() as *mut #zc_name) };
                #(#zc_header_stmts)*
                let mut __offset = <#name as Discriminator>::DISCRIMINATOR.len() + core::mem::size_of::<#zc_name>();
                #(#var_serialize_stmts)*
                let _ = __offset;
                Ok(())
            }
        }
    }
    .into()
}
