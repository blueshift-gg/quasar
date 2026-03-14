//! Generates typed accessor methods for dynamic account fields.
//!
//! Each dynamic field (String, Vec, tail) gets a getter that reads the length
//! prefix and returns a slice/reference into the raw account data buffer.

use {
    crate::helpers::{DynFieldKind, DynKind, TailElement},
    quote::{format_ident, quote},
};

pub(super) struct DynamicAccessors {
    pub accessor_methods: Vec<proc_macro2::TokenStream>,
    pub raw_methods: Vec<proc_macro2::TokenStream>,
    pub write_methods: Vec<proc_macro2::TokenStream>,
}

/// Generate the offset expression for a dynamic field at `dyn_idx`.
///
/// - Field 0: compile-time constant `disc_len + sizeof(ZcHeader)`
/// - Field i (i > 0): `self.__off[i-1] as usize` (cached offset)
fn offset_expr(dyn_idx: usize, disc_len: usize, zc_name: &syn::Ident) -> proc_macro2::TokenStream {
    if dyn_idx == 0 {
        quote! { #disc_len + core::mem::size_of::<#zc_name>() }
    } else {
        let idx = dyn_idx - 1;
        quote! { self.__off[#idx] as usize }
    }
}

pub(super) fn generate_accessors(
    disc_len: usize,
    fields_data: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_kinds: &[DynKind],
    zc_name: &syn::Ident,
) -> DynamicAccessors {
    let dyn_fields: Vec<(usize, &syn::Field, DynFieldKind<'_>)> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .enumerate()
        .filter_map(|(i, (f, k))| k.as_dynamic().map(|dk| (i, f, dk)))
        .collect();

    let num_dyn = dyn_fields.len();
    let num_offsets = num_dyn.saturating_sub(1);

    // --- Read accessor methods (O(1) via cached offsets) ---
    let accessor_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (_, f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let off_expr = offset_expr(dyn_idx, disc_len, zc_name);

            match kind {
                DynFieldKind::Str { prefix, .. } => {
                    let read = prefix.gen_read_len();
                    let pb = prefix.bytes();
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &str {
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let __offset = #off_expr;
                            let __len = #read;
                            let __start = __offset + #pb;
                            let __bytes = &__data[__start..__start + __len];
                            #[cfg(target_os = "solana")]
                            { unsafe { core::str::from_utf8_unchecked(__bytes) } }
                            #[cfg(not(target_os = "solana"))]
                            { core::str::from_utf8(__bytes).expect("account string field contains invalid UTF-8") }
                        }
                    }
                }
                DynFieldKind::Vec { elem, prefix, .. } => {
                    let read = prefix.gen_read_len();
                    let pb = prefix.bytes();
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &[#elem] {
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let __offset = #off_expr;
                            let __count = #read;
                            let __start = __offset + #pb;
                            unsafe { core::slice::from_raw_parts(__data[__start..].as_ptr() as *const #elem, __count) }
                        }
                    }
                }
                DynFieldKind::Tail { element } => {
                    match element {
                        TailElement::Str => {
                            quote! {
                                #[inline(always)]
                                pub fn #fname(&self) -> &str {
                                    let __data = unsafe { self.__view.borrow_unchecked() };
                                    let __offset = #off_expr;
                                    let __bytes = &__data[__offset..];
                                    #[cfg(target_os = "solana")]
                                    { unsafe { core::str::from_utf8_unchecked(__bytes) } }
                                    #[cfg(not(target_os = "solana"))]
                                    { core::str::from_utf8(__bytes).expect("account tail field contains invalid UTF-8") }
                                }
                            }
                        }
                        TailElement::Bytes => {
                            quote! {
                                #[inline(always)]
                                pub fn #fname(&self) -> &[u8] {
                                    let __data = unsafe { self.__view.borrow_unchecked() };
                                    let __offset = #off_expr;
                                    &__data[__offset..]
                                }
                            }
                        }
                    }
                }
            }
        })
        .collect();

    // --- Raw accessor methods (_raw() for zero-copy CPI pass-through) ---
    let raw_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (_, f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let raw_name = format_ident!("{}_raw", fname);
            let off_expr = offset_expr(dyn_idx, disc_len, zc_name);

            match kind {
                DynFieldKind::Str { prefix, .. } => {
                    let read = prefix.gen_read_len();
                    let pb = prefix.bytes();
                    quote! {
                        #[inline(always)]
                        pub fn #raw_name(&self) -> quasar_core::dynamic::RawEncoded<'_, #pb> {
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let __offset = #off_expr;
                            let __len = #read;
                            let __total = #pb + __len;
                            quasar_core::dynamic::RawEncoded::new(&__data[__offset..__offset + __total])
                        }
                    }
                }
                DynFieldKind::Vec { elem, prefix, .. } => {
                    let read = prefix.gen_read_len();
                    let pb = prefix.bytes();
                    quote! {
                        #[inline(always)]
                        pub fn #raw_name(&self) -> quasar_core::dynamic::RawEncoded<'_, #pb> {
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let __offset = #off_expr;
                            let __count = #read;
                            let __total = #pb + __count * core::mem::size_of::<#elem>();
                            quasar_core::dynamic::RawEncoded::new(&__data[__offset..__offset + __total])
                        }
                    }
                }
                DynFieldKind::Tail { .. } => {
                    quote! {
                        #[inline(always)]
                        pub fn #raw_name(&self) -> quasar_core::dynamic::RawEncoded<'_, 0> {
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let __offset = #off_expr;
                            quasar_core::dynamic::RawEncoded::new(&__data[__offset..])
                        }
                    }
                }
            }
        })
        .collect();

    // --- Write setter methods ---

    // Shared realloc + memmove block for prefixed fields (Str, Vec).
    // Handles grow/shrink, tail shift, and offset cache fixup.
    let realloc_memmove = |pb: usize, offset_fixup: &[proc_macro2::TokenStream]| {
        quote! {
            if __old_data_len != __new_data_len {
                let __new_total = __old_total + __new_data_len - __old_data_len;
                let __tail_start = __prefix_offset + #pb + __old_data_len;
                let __tail_len = __old_total - __tail_start;
                if __new_data_len > __old_data_len {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }
                if __tail_len > 0 {
                    let __new_tail = __prefix_offset + #pb + __new_data_len;
                    let __len = self.__view.data_len();
                    let __data = unsafe { core::slice::from_raw_parts_mut(self.__view.data_mut_ptr(), __len) };
                    unsafe {
                        core::ptr::copy(
                            __data.as_ptr().add(__tail_start),
                            __data.as_mut_ptr().add(__new_tail),
                            __tail_len,
                        );
                    }
                }
                if __new_data_len < __old_data_len {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }
                let __delta = __new_data_len as i64 - __old_data_len as i64;
                #(#offset_fixup)*
            }
        }
    };

    let write_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (_, f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let setter_name = format_ident!("set_{}", fname);
            let off_expr = offset_expr(dyn_idx, disc_len, zc_name);

            let offset_fixup_stmts: Vec<proc_macro2::TokenStream> = (dyn_idx..num_offsets)
                .map(|i| {
                    quote! {
                        self.__off[#i] = (self.__off[#i] as i64 + __delta) as u32;
                    }
                })
                .collect();

            match kind {
                DynFieldKind::Str { max, prefix } => {
                    let pb = prefix.bytes();
                    let read = prefix.gen_read_len();
                    let write_stmt = prefix.gen_write_prefix(&quote! { __new_data_len });
                    let realloc_block = realloc_memmove(pb, &offset_fixup_stmts);

                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: &str) -> Result<(), ProgramError> {
                            if __value.len() > #max {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __view = &*self.__view;
                            let __prefix_offset;
                            let __old_data_len;
                            let __old_total;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let __offset = #off_expr;
                                __prefix_offset = __offset;
                                __old_data_len = #read;
                                __old_total = __data.len();
                            }
                            let __new_data_len = __value.len();
                            #realloc_block
                            {
                                let __len = self.__view.data_len();
                                let __data = unsafe { core::slice::from_raw_parts_mut(self.__view.data_mut_ptr(), __len) };
                                let mut __offset = __prefix_offset;
                                #write_stmt
                                __offset += #pb;
                                __data[__offset..__offset + __new_data_len].copy_from_slice(__value.as_bytes());
                            }
                            Ok(())
                        }
                    }
                }
                DynFieldKind::Vec { elem, max, prefix: vec_prefix } => {
                    let pb = vec_prefix.bytes();
                    let read = vec_prefix.gen_read_len();
                    let mut_name = format_ident!("{}_mut", fname);
                    let write_count_stmt = vec_prefix.gen_write_prefix(&quote! { __value.len() });
                    let realloc_block = realloc_memmove(pb, &offset_fixup_stmts);

                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: &[#elem]) -> Result<(), ProgramError> {
                            if __value.len() > #max {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __elem_size = core::mem::size_of::<#elem>();
                            let __view = &*self.__view;
                            let __prefix_offset;
                            let __old_count;
                            let __old_total;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let __offset = #off_expr;
                                __prefix_offset = __offset;
                                __old_count = #read;
                                __old_total = __data.len();
                            }
                            let __old_data_len = __old_count * __elem_size;
                            let __new_data_len = __value.len() * __elem_size;
                            #realloc_block
                            {
                                let __len = self.__view.data_len();
                                let __data = unsafe { core::slice::from_raw_parts_mut(self.__view.data_mut_ptr(), __len) };
                                let mut __offset = __prefix_offset;
                                #write_count_stmt
                                __offset += #pb;
                                if !__value.is_empty() {
                                    unsafe {
                                        core::ptr::copy_nonoverlapping(
                                            __value.as_ptr() as *const u8,
                                            __data[__offset..].as_mut_ptr(),
                                            __new_data_len,
                                        );
                                    }
                                }
                            }
                            Ok(())
                        }

                        #[inline(always)]
                        pub fn #mut_name(&mut self) -> &mut [#elem] {
                            let __len = self.__view.data_len();
                            let __data = unsafe { core::slice::from_raw_parts_mut(self.__view.data_mut_ptr(), __len) };
                            let __offset = #off_expr;
                            let __count = #read;
                            let __start = __offset + #pb;
                            unsafe { core::slice::from_raw_parts_mut(__data[__start..].as_mut_ptr() as *mut #elem, __count) }
                        }
                    }
                }
                DynFieldKind::Tail { element } => {
                    let max_val = 1024usize;
                    let (param_type, copy_src) = match element {
                        TailElement::Str => (quote! { &str }, quote! { __value.as_bytes() }),
                        TailElement::Bytes => (quote! { &[u8] }, quote! { __value }),
                    };
                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: #param_type) -> Result<(), ProgramError> {
                            if __value.len() > #max_val {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __view = &*self.__view;
                            let __start_offset = #off_expr;
                            let __old_len = unsafe { __view.borrow_unchecked() }.len() - __start_offset;
                            let __new_len = __value.len();
                            let __new_total = __start_offset + __new_len;
                            if __new_len > __old_len {
                                self.realloc(__new_total, __payer.to_account_view(), None)?;
                            }
                            let __len = self.__view.data_len();
                            let __data = unsafe { core::slice::from_raw_parts_mut(self.__view.data_mut_ptr(), __len) };
                            __data[__start_offset..__start_offset + __new_len].copy_from_slice(#copy_src);
                            if __new_len < __old_len {
                                self.realloc(__new_total, __payer.to_account_view(), None)?;
                            }
                            Ok(())
                        }
                    }
                }
            }
        })
        .collect();

    DynamicAccessors {
        accessor_methods,
        raw_methods,
        write_methods,
    }
}
