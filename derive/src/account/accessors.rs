use quote::{format_ident, quote};

use crate::helpers::DynKind;

pub(super) struct DynamicAccessors {
    pub accessor_methods: Vec<proc_macro2::TokenStream>,
    pub write_methods: Vec<proc_macro2::TokenStream>,
    pub fields_name: syn::Ident,
    pub fields_struct_fields: Vec<proc_macro2::TokenStream>,
    pub fields_extract_stmts: Vec<proc_macro2::TokenStream>,
    pub fields_field_names: Vec<syn::Ident>,
    pub set_dyn_params: Vec<proc_macro2::TokenStream>,
    pub set_dyn_buf_stmts: Vec<proc_macro2::TokenStream>,
    pub set_dyn_zc_updates: Vec<proc_macro2::TokenStream>,
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

    // --- 9. Read accessor methods ---
    let accessor_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let end_name = format_ident!("{}_end", fname);

            let start_expr = if i > 0 {
                let prev_end =
                    format_ident!("{}_end", dyn_fields[i - 1].0.ident.as_ref().unwrap());
                quote! { #disc_len + core::mem::size_of::<#zc_name>() + __zc.#prev_end.get() as usize }
            } else {
                quote! { #disc_len + core::mem::size_of::<#zc_name>() }
            };
            let end_expr =
                quote! { #disc_len + core::mem::size_of::<#zc_name>() + __zc.#end_name.get() as usize };

            match kind {
                DynKind::Str { .. } | DynKind::StrRef => {
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &str {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                            let __start = #start_expr;
                            let __end = #end_expr;
                            {
                                let __bytes = &__data[__start..__end];
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
                            let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                            let __start = #start_expr;
                            let __end = #end_expr;
                            let __count = (__end - __start) / core::mem::size_of::<#elem>();
                            // SAFETY: Bounds validated by AccountCheck::check. Alignment 1 guaranteed.
                            unsafe { core::slice::from_raw_parts(__data[__start..].as_ptr() as *const #elem, __count) }
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 10. Write setter methods ---
    let write_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let setter_name = format_ident!("set_{}", fname);
            let end_name = format_ident!("{}_end", fname);

            let (field_offset_expr, old_bytes_expr) = if i > 0 {
                let prev_end =
                    format_ident!("{}_end", dyn_fields[i - 1].0.ident.as_ref().unwrap());
                (
                    quote! { __field_offset = #disc_len + core::mem::size_of::<#zc_name>() + __zc.#prev_end.get() as usize; },
                    quote! { __old_bytes = (__zc.#end_name.get() - __zc.#prev_end.get()) as usize; },
                )
            } else {
                (
                    quote! { __field_offset = #disc_len + core::mem::size_of::<#zc_name>(); },
                    quote! { __old_bytes = __zc.#end_name.get() as usize; },
                )
            };

            let fields_to_bump: Vec<syn::Ident> = dyn_fields[i..]
                .iter()
                .map(|(bf, _)| format_ident!("{}_end", bf.ident.as_ref().unwrap()))
                .collect();

            let str_max = match kind {
                DynKind::Str { max } => *max,
                DynKind::StrRef => 255,
                _ => 0,
            };
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => {
                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: &str) -> Result<(), ProgramError> {
                            if __value.len() > #str_max {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __view = self.to_account_view();
                            let __old_bytes;
                            let __old_total;
                            let __field_offset;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                                #field_offset_expr
                                #old_bytes_expr
                                __old_total = __data.len();
                            }
                            let __new_bytes = __value.len();
                            if __old_bytes != __new_bytes {
                                let __new_total = __old_total + __new_bytes - __old_bytes;
                                let __tail_start = __field_offset + __old_bytes;
                                let __tail_len = __old_total - __tail_start;
                                if __new_bytes > __old_bytes {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                                if __tail_len > 0 {
                                    let __new_tail = __field_offset + __new_bytes;
                                    let __data = unsafe { __view.borrow_unchecked_mut() };
                                    // SAFETY: copy handles overlapping source/dest.
                                    unsafe {
                                        core::ptr::copy(
                                            __data.as_ptr().add(__tail_start),
                                            __data.as_mut_ptr().add(__new_tail),
                                            __tail_len,
                                        );
                                    }
                                }
                                if __new_bytes < __old_bytes {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                            }
                            let __data = unsafe { __view.borrow_unchecked_mut() };
                            __data[__field_offset..__field_offset + __new_bytes].copy_from_slice(__value.as_bytes());
                            let __zc = unsafe { &mut *(__data[#disc_len..].as_mut_ptr() as *mut #zc_name) };
                            let __delta = __new_bytes as i32 - __old_bytes as i32;
                            if __delta != 0 {
                                #(
                                    __zc.#fields_to_bump = quasar_core::pod::PodU16::from((__zc.#fields_to_bump.get() as i32 + __delta) as u16);
                                )*
                            }
                            Ok(())
                        }
                    }
                }
                DynKind::Vec { elem, max } => {
                    let mut_name = format_ident!("{}_mut", fname);

                    let start_expr = if i > 0 {
                        let prev_end =
                            format_ident!("{}_end", dyn_fields[i - 1].0.ident.as_ref().unwrap());
                        quote! { #disc_len + core::mem::size_of::<#zc_name>() + __zc.#prev_end.get() as usize }
                    } else {
                        quote! { #disc_len + core::mem::size_of::<#zc_name>() }
                    };
                    let end_expr = quote! { #disc_len + core::mem::size_of::<#zc_name>() + __zc.#end_name.get() as usize };

                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&mut self, __payer: &impl AsAccountView, __value: &[#elem]) -> Result<(), ProgramError> {
                            if __value.len() > #max {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __elem_size = core::mem::size_of::<#elem>();
                            let __view = self.to_account_view();
                            let __old_bytes;
                            let __old_total;
                            let __field_offset;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                                #field_offset_expr
                                #old_bytes_expr
                                __old_total = __data.len();
                            }
                            let __new_bytes = __value.len() * __elem_size;
                            if __old_bytes != __new_bytes {
                                let __new_total = __old_total + __new_bytes - __old_bytes;
                                let __tail_start = __field_offset + __old_bytes;
                                let __tail_len = __old_total - __tail_start;
                                if __new_bytes > __old_bytes {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                                if __tail_len > 0 {
                                    let __new_tail = __field_offset + __new_bytes;
                                    let __data = unsafe { __view.borrow_unchecked_mut() };
                                    unsafe {
                                        core::ptr::copy(
                                            __data.as_ptr().add(__tail_start),
                                            __data.as_mut_ptr().add(__new_tail),
                                            __tail_len,
                                        );
                                    }
                                }
                                if __new_bytes < __old_bytes {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                            }
                            let __data = unsafe { __view.borrow_unchecked_mut() };
                            if !__value.is_empty() {
                                // SAFETY: Source and dest do not overlap. Alignment 1 guaranteed.
                                unsafe {
                                    core::ptr::copy_nonoverlapping(
                                        __value.as_ptr() as *const u8,
                                        __data[__field_offset..].as_mut_ptr(),
                                        __new_bytes,
                                    );
                                }
                            }
                            let __zc = unsafe { &mut *(__data[#disc_len..].as_mut_ptr() as *mut #zc_name) };
                            let __delta = __new_bytes as i32 - __old_bytes as i32;
                            if __delta != 0 {
                                #(
                                    __zc.#fields_to_bump = quasar_core::pod::PodU16::from((__zc.#fields_to_bump.get() as i32 + __delta) as u16);
                                )*
                            }
                            Ok(())
                        }

                        #[inline(always)]
                        pub fn #mut_name(&mut self) -> &mut [#elem] {
                            let __data = unsafe { self.to_account_view().borrow_unchecked_mut() };
                            let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                            let __start = #start_expr;
                            let __end = #end_expr;
                            let __count = (__end - __start) / core::mem::size_of::<#elem>();
                            // SAFETY: Bounds validated by AccountCheck::check. Alignment 1 guaranteed.
                            unsafe { core::slice::from_raw_parts_mut(__data[__start..].as_mut_ptr() as *mut #elem, __count) }
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 11. Batch fields struct for single-pass access ---
    let fields_name = format_ident!("{}DynamicFields", name);

    let fields_struct_fields: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = &f.ident;
            let fvis = &f.vis;
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => quote! { #fvis #fname: &#lt str },
                DynKind::Vec { elem, .. } => quote! { #fvis #fname: &#lt [#elem] },
                _ => unreachable!(),
            }
        })
        .collect();

    let fields_extract_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            let end_name = format_ident!("{}_end", fname);
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => {
                    quote! {
                        let #fname = {
                            let __end = __tail_start + __zc.#end_name.get() as usize;
                            let __s = {
                                let __bytes = &__data[__offset..__end];
                                #[cfg(target_os = "solana")]
                                { unsafe { core::str::from_utf8_unchecked(__bytes) } }
                                #[cfg(not(target_os = "solana"))]
                                { core::str::from_utf8(__bytes).expect("account string field contains invalid UTF-8") }
                            };
                            __offset = __end;
                            __s
                        };
                    }
                }
                DynKind::Vec { elem, .. } => {
                    quote! {
                        let #fname = {
                            let __end = __tail_start + __zc.#end_name.get() as usize;
                            let __count = (__end - __offset) / core::mem::size_of::<#elem>();
                            let __slice = unsafe {
                                core::slice::from_raw_parts(
                                    __data[__offset..].as_ptr() as *const #elem,
                                    __count,
                                )
                            };
                            __offset = __end;
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

    // --- 12. Batch set_dynamic_fields method (Option params, stack buffer) ---
    let set_dyn_params: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => quote! { #fname: Option<&str> },
                DynKind::Vec { elem, .. } => quote! { #fname: Option<&[#elem]> },
                _ => unreachable!(),
            }
        })
        .collect();

    let set_dyn_buf_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let end_name = format_ident!("{}_end", fname);
            let cum_end_var = format_ident!("__{}_cum_end", fname);

            let old_bytes_expr = if i > 0 {
                let prev_end =
                    format_ident!("{}_end", dyn_fields[i - 1].0.ident.as_ref().unwrap());
                quote! { (__zc.#end_name.get() - __zc.#prev_end.get()) as usize }
            } else {
                quote! { __zc.#end_name.get() as usize }
            };

            let buf_max = match kind {
                DynKind::Str { max } => *max,
                DynKind::StrRef => 255,
                _ => 0,
            };
            match kind {
                DynKind::Str { .. } | DynKind::StrRef => {
                    quote! {
                        let #cum_end_var: usize;
                        {
                            let __old_bytes = #old_bytes_expr;
                            match #fname {
                                Some(__val) => {
                                    if __val.len() > #buf_max {
                                        return Err(QuasarError::DynamicFieldTooLong.into());
                                    }
                                    let __new_bytes = __val.len();
                                    __buf[__buf_offset..__buf_offset + __new_bytes]
                                        .copy_from_slice(__val.as_bytes());
                                    __buf_offset += __new_bytes;
                                }
                                None => {
                                    __buf[__buf_offset..__buf_offset + __old_bytes]
                                        .copy_from_slice(&__data[__old_offset..__old_offset + __old_bytes]);
                                    __buf_offset += __old_bytes;
                                }
                            }
                            #cum_end_var = __buf_offset;
                            __old_offset += __old_bytes;
                        }
                    }
                }
                DynKind::Vec { elem, max } => {
                    quote! {
                        let #cum_end_var: usize;
                        {
                            let __old_bytes = #old_bytes_expr;
                            let __elem_size = core::mem::size_of::<#elem>();
                            match #fname {
                                Some(__val) => {
                                    if __val.len() > #max {
                                        return Err(QuasarError::DynamicFieldTooLong.into());
                                    }
                                    let __new_bytes = __val.len() * __elem_size;
                                    if __new_bytes > 0 {
                                        unsafe {
                                            core::ptr::copy_nonoverlapping(
                                                __val.as_ptr() as *const u8,
                                                __buf[__buf_offset..].as_mut_ptr(),
                                                __new_bytes,
                                            );
                                        }
                                    }
                                    __buf_offset += __new_bytes;
                                }
                                None => {
                                    __buf[__buf_offset..__buf_offset + __old_bytes]
                                        .copy_from_slice(&__data[__old_offset..__old_offset + __old_bytes]);
                                    __buf_offset += __old_bytes;
                                }
                            }
                            #cum_end_var = __buf_offset;
                            __old_offset += __old_bytes;
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    let set_dyn_zc_updates: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, _kind)| {
            let fname = f.ident.as_ref().unwrap();
            let end_name = format_ident!("{}_end", fname);
            let cum_end_var = format_ident!("__{}_cum_end", fname);
            quote! { __zc.#end_name = quasar_core::pod::PodU16::from(#cum_end_var as u16); }
        })
        .collect();

    DynamicAccessors {
        accessor_methods,
        write_methods,
        fields_name,
        fields_struct_fields,
        fields_extract_stmts,
        fields_field_names,
        set_dyn_params,
        set_dyn_buf_stmts,
        set_dyn_zc_updates,
    }
}
