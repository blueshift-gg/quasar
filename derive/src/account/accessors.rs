use quote::{format_ident, quote};

use crate::helpers::{DynKind, PrefixType};

pub(super) struct DynamicAccessors {
    pub accessor_methods: Vec<proc_macro2::TokenStream>,
    pub raw_methods: Vec<proc_macro2::TokenStream>,
    pub write_methods: Vec<proc_macro2::TokenStream>,
    pub fields_name: syn::Ident,
    pub fields_struct_fields: Vec<proc_macro2::TokenStream>,
    pub fields_extract_stmts: Vec<proc_macro2::TokenStream>,
    pub fields_field_names: Vec<syn::Ident>,
    pub set_dyn_params: Vec<proc_macro2::TokenStream>,
    pub set_dyn_buf_stmts: Vec<proc_macro2::TokenStream>,
}

fn kind_prefix(kind: &DynKind) -> &PrefixType {
    match kind {
        DynKind::Str { prefix, .. } => prefix,
        DynKind::Vec { prefix, .. } => prefix,
        _ => unreachable!(),
    }
}

fn gen_skip(kind: &DynKind) -> proc_macro2::TokenStream {
    match kind {
        DynKind::Str { prefix, .. } => {
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();
            quote! { { let __s = #read; __offset += #pb + __s; } }
        }
        DynKind::Vec { elem, prefix, .. } => {
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();
            quote! { { let __s = #read; __offset += #pb + __s * core::mem::size_of::<#elem>(); } }
        }
        _ => unreachable!(),
    }
}

fn gen_buf_write_prefix(
    prefix: &PrefixType,
    value_expr: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let pb = prefix.bytes();
    match prefix {
        PrefixType::U8 => quote! {
            __buf[__buf_offset] = #value_expr as u8;
            __buf_offset += #pb;
        },
        PrefixType::U16 => quote! {
            {
                let __pb = (#value_expr as u16).to_le_bytes();
                __buf[__buf_offset] = __pb[0];
                __buf[__buf_offset + 1] = __pb[1];
            }
            __buf_offset += #pb;
        },
        PrefixType::U32 => quote! {
            {
                let __pb = (#value_expr as u32).to_le_bytes();
                __buf[__buf_offset] = __pb[0];
                __buf[__buf_offset + 1] = __pb[1];
                __buf[__buf_offset + 2] = __pb[2];
                __buf[__buf_offset + 3] = __pb[3];
            }
            __buf_offset += #pb;
        },
    }
}

fn gen_old_read_prefix(prefix: &PrefixType) -> proc_macro2::TokenStream {
    match prefix {
        PrefixType::U8 => quote! { __data[__old_offset] as usize },
        PrefixType::U16 => quote! {
            u16::from_le_bytes([__data[__old_offset], __data[__old_offset + 1]]) as usize
        },
        PrefixType::U32 => quote! {
            u32::from_le_bytes([
                __data[__old_offset],
                __data[__old_offset + 1],
                __data[__old_offset + 2],
                __data[__old_offset + 3],
            ]) as usize
        },
    }
}

pub(super) fn generate_accessors(
    name: &syn::Ident,
    disc_len: usize,
    fields_data: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_kinds: &[DynKind],
    zc_name: &syn::Ident,
    lt: &syn::Lifetime,
) -> DynamicAccessors {
    let dyn_fields: Vec<(&syn::Field, &DynKind)> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .collect();

    // --- Read accessor methods ---
    let accessor_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let skip_stmts: Vec<_> = dyn_fields[..i].iter().map(|(_, k)| gen_skip(k)).collect();
            let prefix = kind_prefix(kind);
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();

            match kind {
                DynKind::Str { .. } => {
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &str {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                            #(#skip_stmts)*
                            let __len = #read;
                            __offset += #pb;
                            {
                                let __bytes = &__data[__offset..__offset + __len];
                                #[cfg(target_os = "solana")]
                                { unsafe { core::str::from_utf8_unchecked(__bytes) } }
                                #[cfg(not(target_os = "solana"))]
                                { core::str::from_utf8(__bytes).expect("account string field contains invalid UTF-8") }
                            }
                        }
                    }
                }
                DynKind::Vec { elem, .. } => {
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &[#elem] {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                            #(#skip_stmts)*
                            let __count = #read;
                            __offset += #pb;
                            // SAFETY: Bounds validated by AccountCheck::check. Alignment 1 guaranteed.
                            unsafe { core::slice::from_raw_parts(__data[__offset..].as_ptr() as *const #elem, __count) }
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- Raw accessor methods (_raw() for zero-copy CPI pass-through) ---
    let raw_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let raw_name = format_ident!("{}_raw", fname);
            let skip_stmts: Vec<_> = dyn_fields[..i].iter().map(|(_, k)| gen_skip(k)).collect();
            let prefix = kind_prefix(kind);
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();

            match kind {
                DynKind::Str { .. } => {
                    quote! {
                        #[inline(always)]
                        pub fn #raw_name(&self) -> quasar_core::dynamic::RawEncoded<'_, #pb> {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                            #(#skip_stmts)*
                            let __len = #read;
                            let __total = #pb + __len;
                            quasar_core::dynamic::RawEncoded::new(&__data[__offset..__offset + __total])
                        }
                    }
                }
                DynKind::Vec { elem, .. } => {
                    quote! {
                        #[inline(always)]
                        pub fn #raw_name(&self) -> quasar_core::dynamic::RawEncoded<'_, #pb> {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                            #(#skip_stmts)*
                            let __count = #read;
                            let __total = #pb + __count * core::mem::size_of::<#elem>();
                            quasar_core::dynamic::RawEncoded::new(&__data[__offset..__offset + __total])
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- Write setter methods ---
    let write_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let setter_name = format_ident!("set_{}", fname);
            let skip_stmts: Vec<_> = dyn_fields[..i].iter().map(|(_, k)| gen_skip(k)).collect();
            let prefix = kind_prefix(kind);
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();

            match kind {
                DynKind::Str { max, .. } => {
                    let max_val = *max;
                    let write_stmt = prefix.gen_write_prefix(&quote! { __new_data_len });
                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: &str) -> Result<(), ProgramError> {
                            if __value.len() > #max_val {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __view = self.to_account_view();
                            let __prefix_offset;
                            let __old_data_len;
                            let __old_total;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                                #(#skip_stmts)*
                                __prefix_offset = __offset;
                                __old_data_len = #read;
                                __old_total = __data.len();
                            }
                            let __new_data_len = __value.len();
                            if __old_data_len != __new_data_len {
                                let __new_total = __old_total + __new_data_len - __old_data_len;
                                let __tail_start = __prefix_offset + #pb + __old_data_len;
                                let __tail_len = __old_total - __tail_start;
                                if __new_data_len > __old_data_len {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                                if __tail_len > 0 {
                                    let __new_tail = __prefix_offset + #pb + __new_data_len;
                                    let __data = unsafe { __view.borrow_unchecked_mut() };
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
                            }
                            {
                                let __data = unsafe { __view.borrow_unchecked_mut() };
                                let mut __offset = __prefix_offset;
                                #write_stmt
                                __offset += #pb;
                                __data[__offset..__offset + __new_data_len].copy_from_slice(__value.as_bytes());
                            }
                            Ok(())
                        }
                    }
                }
                DynKind::Vec { elem, max, prefix: vec_prefix } => {
                    let max_val = *max;
                    let mut_name = format_ident!("{}_mut", fname);
                    let write_count_stmt = vec_prefix.gen_write_prefix(&quote! { __value.len() });

                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: &[#elem]) -> Result<(), ProgramError> {
                            if __value.len() > #max_val {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __elem_size = core::mem::size_of::<#elem>();
                            let __view = self.to_account_view();
                            let __prefix_offset;
                            let __old_count;
                            let __old_total;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                                #(#skip_stmts)*
                                __prefix_offset = __offset;
                                __old_count = #read;
                                __old_total = __data.len();
                            }
                            let __old_data_len = __old_count * __elem_size;
                            let __new_data_len = __value.len() * __elem_size;
                            if __old_data_len != __new_data_len {
                                let __new_total = __old_total + __new_data_len - __old_data_len;
                                let __tail_start = __prefix_offset + #pb + __old_data_len;
                                let __tail_len = __old_total - __tail_start;
                                if __new_data_len > __old_data_len {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                                if __tail_len > 0 {
                                    let __new_tail = __prefix_offset + #pb + __new_data_len;
                                    let __data = unsafe { __view.borrow_unchecked_mut() };
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
                            }
                            {
                                let __data = unsafe { __view.borrow_unchecked_mut() };
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
                            let __data = unsafe { self.to_account_view().borrow_unchecked_mut() };
                            let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                            #(#skip_stmts)*
                            let __count = #read;
                            __offset += #pb;
                            unsafe { core::slice::from_raw_parts_mut(__data[__offset..].as_mut_ptr() as *mut #elem, __count) }
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- Batch fields struct ---
    let fields_name = format_ident!("{}DynamicFields", name);

    let fields_struct_fields: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = &f.ident;
            let fvis = &f.vis;
            match kind {
                DynKind::Str { .. } => quote! { #fvis #fname: &#lt str },
                DynKind::Vec { elem, .. } => quote! { #fvis #fname: &#lt [#elem] },
                _ => unreachable!(),
            }
        })
        .collect();

    let fields_extract_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            let prefix = kind_prefix(kind);
            let read = prefix.gen_read_len();
            let pb = prefix.bytes();

            match kind {
                DynKind::Str { .. } => {
                    quote! {
                        let #fname = {
                            let __len = #read;
                            __offset += #pb;
                            let __s = {
                                let __bytes = &__data[__offset..__offset + __len];
                                #[cfg(target_os = "solana")]
                                { unsafe { core::str::from_utf8_unchecked(__bytes) } }
                                #[cfg(not(target_os = "solana"))]
                                { core::str::from_utf8(__bytes).expect("account string field contains invalid UTF-8") }
                            };
                            __offset += __len;
                            __s
                        };
                    }
                }
                DynKind::Vec { elem, .. } => {
                    quote! {
                        let #fname = {
                            let __count = #read;
                            __offset += #pb;
                            let __slice = unsafe {
                                core::slice::from_raw_parts(
                                    __data[__offset..].as_ptr() as *const #elem,
                                    __count,
                                )
                            };
                            __offset += __count * core::mem::size_of::<#elem>();
                            __slice
                        };
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    let fields_field_names: Vec<syn::Ident> = dyn_fields
        .iter()
        .map(|(f, _)| f.ident.as_ref().unwrap().clone())
        .collect();

    // --- Batch set_dynamic_fields ---
    let set_dyn_params: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => quote! { #fname: Option<&str> },
                DynKind::Vec { elem, .. } => quote! { #fname: Option<&[#elem]> },
                _ => unreachable!(),
            }
        })
        .collect();

    let set_dyn_buf_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            let prefix = kind_prefix(kind);
            let pb = prefix.bytes();
            let old_read = gen_old_read_prefix(prefix);

            match kind {
                DynKind::Str { max, .. } => {
                    let max_val = *max;
                    let write_prefix = gen_buf_write_prefix(prefix, quote! { __val.len() });
                    quote! {
                        {
                            let __old_val = #old_read;
                            let __old_field = #pb + __old_val;
                            match #fname {
                                Some(__val) => {
                                    if __val.len() > #max_val {
                                        return Err(QuasarError::DynamicFieldTooLong.into());
                                    }
                                    #write_prefix
                                    __buf[__buf_offset..__buf_offset + __val.len()]
                                        .copy_from_slice(__val.as_bytes());
                                    __buf_offset += __val.len();
                                }
                                None => {
                                    __buf[__buf_offset..__buf_offset + __old_field]
                                        .copy_from_slice(&__data[__old_offset..__old_offset + __old_field]);
                                    __buf_offset += __old_field;
                                }
                            }
                            __old_offset += __old_field;
                        }
                    }
                }
                DynKind::Vec { elem, max, .. } => {
                    let max_val = *max;
                    let write_prefix = gen_buf_write_prefix(prefix, quote! { __val.len() });
                    quote! {
                        {
                            let __old_count = #old_read;
                            let __old_data_bytes = __old_count * core::mem::size_of::<#elem>();
                            let __old_field = #pb + __old_data_bytes;
                            match #fname {
                                Some(__val) => {
                                    if __val.len() > #max_val {
                                        return Err(QuasarError::DynamicFieldTooLong.into());
                                    }
                                    #write_prefix
                                    let __new_data_bytes = __val.len() * core::mem::size_of::<#elem>();
                                    if __new_data_bytes > 0 {
                                        unsafe {
                                            core::ptr::copy_nonoverlapping(
                                                __val.as_ptr() as *const u8,
                                                __buf[__buf_offset..].as_mut_ptr(),
                                                __new_data_bytes,
                                            );
                                        }
                                    }
                                    __buf_offset += __new_data_bytes;
                                }
                                None => {
                                    __buf[__buf_offset..__buf_offset + __old_field]
                                        .copy_from_slice(&__data[__old_offset..__old_offset + __old_field]);
                                    __buf_offset += __old_field;
                                }
                            }
                            __old_offset += __old_field;
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    DynamicAccessors {
        accessor_methods,
        raw_methods,
        write_methods,
        fields_name,
        fields_struct_fields,
        fields_extract_stmts,
        fields_field_names,
        set_dyn_params,
        set_dyn_buf_stmts,
    }
}
