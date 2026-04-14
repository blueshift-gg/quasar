//! Unified codegen for `#[account]` types (fixed and Pod-dynamic layouts).
//!
//! Fixed accounts have all fields known at compile time (no PodString/PodVec).
//! Pod-dynamic accounts have `PodString<N>` / `PodVec<T, N>` fields that use
//! dynamic sizing, walk-from-header accessors, and a runtime memmove helper for
//! writes that grow.
//!
//! This single function handles both cases: when all fields have `pod_dyn:
//! None` it produces pure fixed-layout output; when any field has `pod_dyn:
//! Some(...)` it generates the dynamic parts (MIN_SPACE/MAX_SPACE, accessors,
//! setters, and modified AccountCheck validation).

use {
    crate::helpers::{map_to_pod_type, pascal_to_snake, zc_assign_from_value, PodDynField},
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::DeriveInput,
};

/// Info about each field needed for codegen.
pub(super) struct PodFieldInfo<'a> {
    pub field: &'a syn::Field,
    pub pod_dyn: Option<PodDynField>,
}

pub(super) fn generate_account(
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

    let has_dynamic = field_infos.iter().any(|fi| fi.pod_dyn.is_some());

    // --- ZC companion struct ---
    // Fixed: all fields. Dynamic: only non-dynamic fields go in ZC.
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

    // Names for ZC struct and its containing module.
    // Dynamic: ZC is public (no wrapping module).
    // Fixed: ZC is inside a hidden module.
    let zc_name = format_ident!("{}Zc", name);
    let zc_mod = format_ident!("__{}_zc", pascal_to_snake(&name.to_string()));

    // Qualified path to the ZC struct for use in impls.
    let zc_path = if has_dynamic {
        quote! { #zc_name }
    } else {
        quote! { #zc_mod::#zc_name }
    };

    // --- Detect bump: u8 field for PDA bump auto-detection (fixed only) ---
    let has_bump_u8 = !has_dynamic
        && field_infos.iter().any(|fi| {
            fi.field.ident.as_ref().is_some_and(|id| id == "bump")
                && matches!(&fi.field.ty, syn::Type::Path(tp) if tp.path.is_ident("u8"))
        });

    let bump_offset_impl = if has_bump_u8 {
        quote! {
            const BUMP_OFFSET: Option<usize> = Some(
                #disc_len + core::mem::offset_of!(#zc_path, bump)
            );
        }
    } else {
        quote! {}
    };

    // =========================================================================
    // Dynamic-only pieces
    // =========================================================================

    let dyn_fields: Vec<(&syn::Field, &PodDynField)> = field_infos
        .iter()
        .filter_map(|fi| fi.pod_dyn.as_ref().map(|pd| (fi.field, pd)))
        .collect();

    // Alignment assertions for PodVec element types
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

    // Prefix total (for MIN_SPACE)
    let prefix_total: usize = dyn_fields
        .iter()
        .map(|(_, pd)| match pd {
            PodDynField::Str { prefix_bytes, .. } => *prefix_bytes,
            PodDynField::Vec { prefix_bytes, .. } => *prefix_bytes,
        })
        .sum();

    // MAX_SPACE terms
    let max_space_terms: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(_, pd)| match pd {
            PodDynField::Str { max, .. } => quote! { + #max },
            PodDynField::Vec { elem, max, .. } => {
                quote! { + #max * core::mem::size_of::<#elem>() }
            }
        })
        .collect();

    // AccountCheck validation for dynamic field prefixes
    let validation_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(_f, pd)| match pd {
            PodDynField::Str { max, prefix_bytes } => {
                let pfx = *prefix_bytes;
                quote! {
                    {
                        if __offset + #pfx > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        let __len = {
                            let mut __buf = [0u8; 8];
                            __buf[..#pfx].copy_from_slice(&__data[__offset..__offset + #pfx]);
                            u64::from_le_bytes(__buf) as usize
                        };
                        __offset += #pfx;
                        if __len > #max {
                            return Err(ProgramError::InvalidAccountData);
                        }
                        if __offset + __len > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        // UTF-8 re-validation skipped: the owner check
                        // (check_owner) already proved this account belongs to
                        // this program, and all PodString write paths (set_xxx,
                        // set_inner) accept &str, which is valid UTF-8 by
                        // Rust's type system. DerefMut only exposes the fixed
                        // ZC header, not the dynamic region.
                        __offset += __len;
                    }
                }
            }
            PodDynField::Vec {
                elem,
                max,
                prefix_bytes,
            } => {
                let pfx = *prefix_bytes;
                quote! {
                    {
                        if __offset + #pfx > __data_len {
                            return Err(ProgramError::AccountDataTooSmall);
                        }
                        let __count = {
                            let mut __buf = [0u8; 8];
                            __buf[..#pfx].copy_from_slice(&__data[__offset..__offset + #pfx]);
                            u64::from_le_bytes(__buf) as usize
                        };
                        __offset += #pfx;
                        if __count > #max {
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
        })
        .collect();

    // Walk codegen: read accessors
    let dyn_start = quote! { #disc_len + core::mem::size_of::<#zc_path>() };

    let read_accessors: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (f, pd))| {
            let fname = f.ident.as_ref().unwrap();
            let walk_stmts: Vec<proc_macro2::TokenStream> = dyn_fields[..dyn_idx]
                .iter()
                .map(|(_, prev_pd)| match prev_pd {
                    PodDynField::Str { prefix_bytes, .. } => {
                        let pfx = *prefix_bytes;
                        quote! {
                            {
                                let mut __buf = [0u8; 8];
                                __buf[..#pfx].copy_from_slice(&__data[__off..__off + #pfx]);
                                let __field_len = u64::from_le_bytes(__buf) as usize;
                                __off += #pfx + __field_len;
                            }
                        }
                    },
                    PodDynField::Vec { elem, prefix_bytes, .. } => {
                        let pfx = *prefix_bytes;
                        quote! {
                            {
                                let mut __buf = [0u8; 8];
                                __buf[..#pfx].copy_from_slice(&__data[__off..__off + #pfx]);
                                let __field_count = u64::from_le_bytes(__buf) as usize;
                                __off += #pfx + __field_count * core::mem::size_of::<#elem>();
                            }
                        }
                    },
                })
                .collect();

            match pd {
                PodDynField::Str { prefix_bytes, .. } => {
                    let pfx = *prefix_bytes;
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &str {
                            // SAFETY: self.__view is a valid AccountView for this program's
                            // account. borrow_unchecked returns the raw data slice.
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let mut __off = #dyn_start;
                            #(#walk_stmts)*
                            let __len = {
                                let mut __buf = [0u8; 8];
                                __buf[..#pfx].copy_from_slice(&__data[__off..__off + #pfx]);
                                u64::from_le_bytes(__buf) as usize
                            };
                            // SAFETY: data[off+pfx..off+pfx+len] was written by this program's
                            // set()/set_inner() which accept &str (valid UTF-8). The owner
                            // check in AccountCheck::check proves this program owns the data.
                            // SVM zeros uninitialized account memory.
                            unsafe { core::str::from_utf8_unchecked(&__data[__off + #pfx..__off + #pfx + __len]) }
                        }
                    }
                },
                PodDynField::Vec { elem, prefix_bytes, .. } => {
                    let pfx = *prefix_bytes;
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &[#elem] {
                            // SAFETY: see PodDynField::Str case above.
                            let __data = unsafe { self.__view.borrow_unchecked() };
                            let mut __off = #dyn_start;
                            #(#walk_stmts)*
                            let __count = {
                                let mut __buf = [0u8; 8];
                                __buf[..#pfx].copy_from_slice(&__data[__off..__off + #pfx]);
                                u64::from_le_bytes(__buf) as usize
                            };
                            // SAFETY: T has alignment 1 (enforced by PodVec compile-time
                            // assertion). count was validated by AccountCheck::check.
                            // Pointer is into the account data buffer which is valid for
                            // the instruction's lifetime.
                            unsafe { core::slice::from_raw_parts(__data[__off + #pfx..].as_ptr() as *const #elem, __count) }
                        }
                    }
                },
            }
        })
        .collect();

    // =========================================================================
    // set_inner — differs between fixed and dynamic
    // =========================================================================

    let set_inner_impl = if gen_set_inner {
        if has_dynamic {
            // Dynamic set_inner: takes payer + rent args, handles realloc
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
                        Some(PodDynField::Str { max, .. }) => Some(quote! {
                            if #fname.len() > #max { return Err(QuasarError::DynamicFieldTooLong.into()); }
                        }),
                        Some(PodDynField::Vec { max, .. }) => Some(quote! {
                            if #fname.len() > #max { return Err(QuasarError::DynamicFieldTooLong.into()); }
                        }),
                        None => None,
                    }
                })
                .collect();

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
                        Some(PodDynField::Str { prefix_bytes, .. }) => {
                            let pfx = *prefix_bytes;
                            Some(quote! {
                                {
                                    let __len_bytes = (#fname.len() as u64).to_le_bytes();
                                    __data[__offset..__offset + #pfx].copy_from_slice(&__len_bytes[..#pfx]);
                                    __offset += #pfx;
                                    __data[__offset..__offset + #fname.len()].copy_from_slice(#fname.as_bytes());
                                    __offset += #fname.len();
                                }
                            })
                        },
                        Some(PodDynField::Vec { elem, prefix_bytes, .. }) => {
                            let pfx = *prefix_bytes;
                            Some(quote! {
                                {
                                    let __count_bytes = (#fname.len() as u64).to_le_bytes();
                                    __data[__offset..__offset + #pfx].copy_from_slice(&__count_bytes[..#pfx]);
                                    __offset += #pfx;
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
                            })
                        },
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
            // Fixed set_inner: simple field assignment, no realloc
            let inner_name = format_ident!("{}Inner", name);
            let field_names: Vec<_> = field_infos.iter().map(|fi| &fi.field.ident).collect();
            let field_types: Vec<_> = field_infos.iter().map(|fi| &fi.field.ty).collect();

            let set_inner_stmts: Vec<proc_macro2::TokenStream> = field_infos
                .iter()
                .map(|fi| {
                    zc_assign_from_value(
                        fi.field
                            .ident
                            .as_ref()
                            .expect("field must have an identifier"),
                        &fi.field.ty,
                    )
                })
                .collect();

            quote! {
                #vis struct #inner_name {
                    #(pub #field_names: #field_types,)*
                }

                impl #name {
                    #[inline(always)]
                    pub fn set_inner(&mut self, inner: #inner_name) {
                        let __zc = unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_path) };
                        #(let #field_names = inner.#field_names;)*
                        #(#set_inner_stmts)*
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    // =========================================================================
    // Space impl — fixed vs dynamic
    // =========================================================================

    let space_impl = if has_dynamic {
        // For dynamic accounts, Space::SPACE = MIN_SPACE (disc + ZC header + prefix
        // sizes)
        quote! {
            impl Space for #name {
                const SPACE: usize = #disc_len + core::mem::size_of::<#zc_path>() + #prefix_total;
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
    };

    // =========================================================================
    // AccountCheck impl — fixed vs dynamic
    // =========================================================================

    let account_check_impl = if has_dynamic {
        quote! {
            impl AccountCheck for #name {
                #[inline(always)]
                fn check(view: &AccountView) -> Result<(), ProgramError> {
                    let __data = unsafe { view.borrow_unchecked() };
                    let __data_len = __data.len();
                    let __min = #disc_len + core::mem::size_of::<#zc_path>() + #prefix_total;
                    if __data_len < __min {
                        return Err(ProgramError::AccountDataTooSmall);
                    }
                    #(
                        if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                            return Err(ProgramError::InvalidAccountData);
                        }
                    )*
                    let mut __offset = #disc_len + core::mem::size_of::<#zc_path>();
                    #(#validation_stmts)*
                    let _ = __offset;
                    Ok(())
                }
            }
        }
    } else {
        quote! {
            impl AccountCheck for #name {
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
                    Ok(())
                }
            }
        }
    };

    // =========================================================================
    // ZC struct definition — fixed uses hidden module, dynamic is public
    // =========================================================================

    let zc_definition = if has_dynamic {
        quote! {
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
        }
    } else {
        quote! {
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
        }
    };

    // =========================================================================
    // Dynamic-only: RAII guard for mutable access with auto-save on drop
    // =========================================================================

    let dyn_guard = if has_dynamic {
        let guard_name = format_ident!("{}DynGuard", name);

        let guard_fields: Vec<proc_macro2::TokenStream> = dyn_fields
            .iter()
            .map(|(f, pd)| {
                let fname = f.ident.as_ref().unwrap();
                match pd {
                    PodDynField::Str { max, prefix_bytes } => quote! {
                        pub #fname: quasar_lang::pod::PodString<#max, #prefix_bytes>
                    },
                    PodDynField::Vec {
                        elem,
                        max,
                        prefix_bytes,
                    } => quote! {
                        pub #fname: quasar_lang::pod::PodVec<#elem, #max, #prefix_bytes>
                    },
                }
            })
            .collect();

        let load_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
            .iter()
            .map(|(f, pd)| {
                let fname = f.ident.as_ref().unwrap();
                match pd {
                    PodDynField::Str { max, prefix_bytes } => quote! {
                        let mut #fname =
                            quasar_lang::pod::PodString::<#max, #prefix_bytes>::default();
                        __off += #fname.load_from_bytes(&__data[__off..]);
                    },
                    PodDynField::Vec {
                        elem,
                        max,
                        prefix_bytes,
                    } => quote! {
                        let mut #fname =
                            quasar_lang::pod::PodVec::<#elem, #max, #prefix_bytes>::default();
                        __off += #fname.load_from_bytes(&__data[__off..]);
                    },
                }
            })
            .collect();

        let field_names: Vec<&syn::Ident> = dyn_fields
            .iter()
            .map(|(f, _)| f.ident.as_ref().unwrap())
            .collect();

        let save_size_terms: Vec<proc_macro2::TokenStream> = dyn_fields
            .iter()
            .map(|(f, _)| {
                let fname = f.ident.as_ref().unwrap();
                quote! { + self.#fname.serialized_len() }
            })
            .collect();

        let save_write_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
            .iter()
            .map(|(f, _)| {
                let fname = f.ident.as_ref().unwrap();
                quote! { __off += self.#fname.write_to_bytes(&mut __data[__off..]); }
            })
            .collect();

        quote! {
            /// RAII guard for mutable dynamic field access.
            ///
            /// Created via [`#name::as_dynamic_mut()`]. Dynamic fields are loaded
            /// into stack-local PodString/PodVec copies on creation. Fixed fields
            /// are accessed via `Deref`/`DerefMut` to the ZC struct (zero-copy).
            /// On drop, all dynamic fields are flushed back to account data in a
            /// single batched write with at most one realloc.
            pub struct #guard_name<'a> {
                __view: &'a mut AccountView,
                __payer: &'a AccountView,
                __rent_lpb: u64,
                __rent_threshold: u64,
                #(#guard_fields,)*
            }

            impl<'a> core::ops::Deref for #guard_name<'a> {
                type Target = #zc_name;

                #[inline(always)]
                fn deref(&self) -> &Self::Target {
                    unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_name) }
                }
            }

            impl<'a> core::ops::DerefMut for #guard_name<'a> {
                #[inline(always)]
                fn deref_mut(&mut self) -> &mut Self::Target {
                    unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_name) }
                }
            }

            impl<'a> #guard_name<'a> {
                /// Explicitly save dynamic fields. Also called automatically on drop.
                pub fn save(&mut self) -> Result<(), ProgramError> {
                    let __new_total = #disc_len + core::mem::size_of::<#zc_name>()
                        #(#save_size_terms)*;

                    let __old_total = self.__view.data_len();
                    if __new_total != __old_total {
                        quasar_lang::accounts::account::realloc_account_raw(
                            self.__view, __new_total, self.__payer,
                            self.__rent_lpb, self.__rent_threshold,
                        )?;
                    }

                    let __dyn_start = #disc_len + core::mem::size_of::<#zc_name>();
                    let __ptr = self.__view.data_mut_ptr();
                    let __data = unsafe {
                        core::slice::from_raw_parts_mut(
                            __ptr.add(__dyn_start),
                            __new_total - __dyn_start,
                        )
                    };
                    let mut __off = 0usize;
                    #(#save_write_stmts)*
                    let _ = __off;
                    Ok(())
                }

                /// Re-load dynamic fields from account data (e.g. after CPI).
                pub fn reload(&mut self) {
                    let __data = unsafe { self.__view.borrow_unchecked() };
                    let mut __off = #disc_len + core::mem::size_of::<#zc_name>();
                    #(
                        __off += self.#field_names.load_from_bytes(&__data[__off..]);
                    )*
                    let _ = __off;
                }
            }

            impl<'a> Drop for #guard_name<'a> {
                fn drop(&mut self) {
                    // Auto-save on drop. Panic on failure — Solana transactions
                    // are atomic, so a panic just aborts the whole instruction.
                    self.save().expect("dynamic field auto-save failed");
                }
            }

            impl #name {
                /// Create a mutable guard for dynamic field access.
                ///
                /// Loads dynamic fields into stack-local PodString/PodVec copies.
                /// Fixed fields are still accessed zero-copy via `Deref`/`DerefMut`.
                /// On drop, all dynamic fields are flushed back with a single
                /// batched write.
                #[inline(always)]
                pub fn as_dynamic_mut<'a>(
                    &'a mut self,
                    payer: &'a AccountView,
                    rent_lpb: u64,
                    rent_threshold: u64,
                ) -> #guard_name<'a> {
                    let (#(#field_names,)*) = {
                        let __data = unsafe { self.__view.borrow_unchecked() };
                        let mut __off = #disc_len + core::mem::size_of::<#zc_name>();
                        #(#load_stmts)*
                        let _ = __off;
                        (#(#field_names,)*)
                        // __data is definitively dropped here — no shared borrow
                        // of self.__view is live past this point.
                    };
                    // SAFETY: `__data` (the shared borrow of `self.__view`) was
                    // dropped at the end of the enclosing block. No shared reference
                    // to `self.__view` is live at this point. The raw-pointer reborrow
                    // produces a `&mut AccountView` from `&mut self` which is the
                    // unique owner — no aliasing occurs. `#name` is
                    // `#[repr(transparent)]` over `AccountView` so the cast is
                    // layout-compatible.
                    let __view = unsafe { &mut *(&mut self.__view as *mut AccountView) };
                    #guard_name {
                        __view,
                        __payer: payer,
                        __rent_lpb: rent_lpb,
                        __rent_threshold: rent_threshold,
                        #(#field_names,)*
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    // =========================================================================
    // Dynamic-only: MIN_SPACE/MAX_SPACE constants + accessors + setters
    // =========================================================================

    let dynamic_impl_block = if has_dynamic {
        quote! {
            impl #name {
                pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_path>() + #prefix_total;
                pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

                #(#read_accessors)*
            }
        }
    } else {
        quote! {}
    };

    // =========================================================================
    // Combine everything
    // =========================================================================

    quote! {
        #(#attrs)*
        #[repr(transparent)]
        #vis struct #name {
            __view: AccountView,
        }

        #zc_definition

        unsafe impl StaticView for #name {}

        impl AsAccountView for #name {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.__view
            }
        }

        impl core::ops::Deref for #name {
            type Target = #zc_path;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_path) }
            }
        }

        impl core::ops::DerefMut for #name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_path) }
            }
        }

        impl Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
            #bump_offset_impl
        }

        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }

        #space_impl

        #account_check_impl

        #dynamic_impl_block

        #dyn_guard

        #set_inner_impl
    }
    .into()
}
