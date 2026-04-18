//! Compact account mutation codegen.
//!
//! Two mutation paths:
//! - `as_mut()` returns a `{Name}CompactMut` guard with load-mutate-save
//!   semantics: all dynamic fields are loaded on construction, the user mutates
//!   whichever fields they want via the public `PodString`/`PodVec` fields,
//!   then the guard auto-saves on drop. `.save()` remains available for
//!   mid-flight flushes (e.g. before a CPI), and `.reload()` refreshes the
//!   cached fields from account data after external mutation.
//! - `compact_writer()` returns a `{Name}CompactWriter` (builder pattern) with
//!   explicit `.set_field()` + `.commit()` semantics — used by `set_inner()`
//!   for account initialization.
//!
//! Read accessors use zeropod's compact `Ref` for zero-copy borrowed views.

use {
    super::fixed::PodFieldInfo,
    crate::helpers::{map_to_pod_type, PodDynField},
    quote::{format_ident, quote},
};

pub(super) type DynFieldRef<'a> = (&'a syn::Field, &'a PodDynField);

pub(super) struct DynamicPieces<'a> {
    pub dyn_fields: Vec<DynFieldRef<'a>>,
    pub max_space_terms: Vec<proc_macro2::TokenStream>,
    pub read_accessors: Vec<proc_macro2::TokenStream>,
}

pub(super) fn build_dynamic_pieces<'a>(
    field_infos: &'a [PodFieldInfo<'a>],
    disc_len: usize,
    zc_mod: &syn::Ident,
) -> DynamicPieces<'a> {
    let dyn_fields: Vec<DynFieldRef<'a>> = field_infos
        .iter()
        .filter_map(|fi| fi.pod_dyn.as_ref().map(|pd| (fi.field, pd)))
        .collect();
    let max_space_terms = dyn_fields
        .iter()
        .map(|(_, pd)| dyn_max_space_term(pd))
        .collect();
    let read_accessors = dyn_fields
        .iter()
        .map(|(field, pd)| {
            let name = field.ident.as_ref().expect("field must be named");
            compact_read_accessor(name, pd, disc_len, zc_mod)
        })
        .collect();

    DynamicPieces {
        dyn_fields,
        max_space_terms,
        read_accessors,
    }
}

pub(super) fn emit_inner_field(
    name: &syn::Ident,
    dyn_field: &PodDynField,
) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { .. } => quote! { pub #name: &'a str },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { pub #name: &'a [#mapped] }
        }
    }
}

pub(super) fn emit_max_check(
    name: &syn::Ident,
    dyn_field: &PodDynField,
) -> proc_macro2::TokenStream {
    let max = match dyn_field {
        PodDynField::Str { max, .. } | PodDynField::Vec { max, .. } => max,
    };
    quote! {
        if #name.len() > #max { return Err(QuasarError::DynamicFieldTooLong.into()); }
    }
}

pub(super) fn emit_space_term(
    name: &syn::Ident,
    dyn_field: &PodDynField,
) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { .. } => quote! { + #name.len() },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { + #name.len() * core::mem::size_of::<#mapped>() }
        }
    }
}

pub(super) fn emit_dynamic_impl_block(
    name: &syn::Ident,
    has_dynamic: bool,
    disc_len: usize,
    zc_mod: &syn::Ident,
    pieces: &DynamicPieces<'_>,
) -> proc_macro2::TokenStream {
    if has_dynamic {
        let max_space_terms = &pieces.max_space_terms;
        let read_accessors = &pieces.read_accessors;
        quote! {
            impl #name {
                pub const MIN_SPACE: usize = #disc_len
                    + <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE;
                pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

                #(#read_accessors)*
            }
        }
    } else {
        quote! {}
    }
}

pub(super) fn emit_dyn_writer(
    name: &syn::Ident,
    has_dynamic: bool,
    disc_len: usize,
    zc_mod: &syn::Ident,
    zc_path: &proc_macro2::TokenStream,
    pieces: &DynamicPieces<'_>,
) -> proc_macro2::TokenStream {
    if !has_dynamic {
        return quote! {};
    }

    let writer_name = format_ident!("{}CompactWriter", name);
    let setter_fields: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| dyn_view_field(field.ident.as_ref().expect("field must be named"), pd))
        .collect();
    let setter_inits: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, _)| {
            let name = field.ident.as_ref().expect("field must be named");
            let slot = format_ident!("__{}", name);
            quote! { #slot: None }
        })
        .collect();
    let setter_methods: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| dyn_view_setter(field.ident.as_ref().expect("field must be named"), pd))
        .collect();
    let binding_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, _)| {
            let name = field.ident.as_ref().expect("field must be named");
            let slot = format_ident!("__{}", name);
            quote! {
                let #name = self.#slot.ok_or(QuasarError::CompactWriterFieldNotSet)?;
            }
        })
        .collect();
    let size_terms: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            let name = field.ident.as_ref().expect("field must be named");
            writer_space_term(name, pd)
        })
        .collect();
    let compact_set_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            let name = field.ident.as_ref().expect("field must be named");
            writer_compact_set_stmt(name, pd)
        })
        .collect();

    quote! {
        pub struct #writer_name<'a> {
            __view: &'a mut AccountView,
            __payer: &'a AccountView,
            __rent_lpb: u64,
            __rent_threshold: u64,
            #(#setter_fields,)*
        }

        impl<'a> core::ops::Deref for #writer_name<'a> {
            type Target = #zc_path;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_path) }
            }
        }

        impl<'a> core::ops::DerefMut for #writer_name<'a> {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_path) }
            }
        }

        impl<'a> #writer_name<'a> {
            #(#setter_methods)*

            pub fn commit(&mut self) -> Result<(), ProgramError> {
                #(#binding_stmts)*

                let __new_total = #disc_len
                    + <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE
                    #(#size_terms)*;
                let __old_total = self.__view.data_len();
                if __new_total != __old_total {
                    quasar_lang::accounts::account::realloc_account_raw(
                        self.__view,
                        __new_total,
                        self.__payer,
                        self.__rent_lpb,
                        self.__rent_threshold,
                    )?;
                }

                let __compact_data = unsafe {
                    core::slice::from_raw_parts_mut(
                        self.__view.data_mut_ptr().add(#disc_len),
                        __new_total - #disc_len,
                    )
                };
                let mut __compact = unsafe { #zc_mod::__SchemaMut::new_unchecked(__compact_data) };
                #(#compact_set_stmts)*
                __compact.commit().map_err(|_| ProgramError::InvalidAccountData)?;
                Ok(())
            }
        }

        impl #name {
            #[inline(always)]
            pub fn compact_writer<'a>(
                &'a mut self,
                payer: &'a AccountView,
                rent_lpb: u64,
                rent_threshold: u64,
            ) -> #writer_name<'a> {
                // SAFETY: `self.__view` is the transparent account backing store for this
                // wrapper. Reborrowing it as `&mut AccountView` is sound here because the
                // writer exclusively owns `&'a mut self` for its full lifetime and does not
                // create any competing mutable references.
                let __view = unsafe { &mut *(&mut self.__view as *mut AccountView) };
                #writer_name {
                    __view,
                    __payer: payer,
                    __rent_lpb: rent_lpb,
                    __rent_threshold: rent_threshold,
                    #(#setter_inits,)*
                }
            }
        }
    }
}

pub(super) fn emit_compact_mut(
    name: &syn::Ident,
    has_dynamic: bool,
    disc_len: usize,
    zc_mod: &syn::Ident,
    zc_path: &proc_macro2::TokenStream,
    pieces: &DynamicPieces<'_>,
) -> proc_macro2::TokenStream {
    if !has_dynamic {
        return quote! {};
    }

    let guard_name = format_ident!("{}CompactMut", name);
    let guard_fields: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            compact_mut_field(field.ident.as_ref().expect("field must be named"), pd)
        })
        .collect();
    let load_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| compact_mut_load(field.ident.as_ref().expect("field must be named"), pd))
        .collect();
    let field_names: Vec<&syn::Ident> = pieces
        .dyn_fields
        .iter()
        .map(|(field, _)| field.ident.as_ref().expect("field must be named"))
        .collect();
    let save_size_terms: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            let fname = field.ident.as_ref().expect("field must be named");
            compact_mut_size_term(fname, pd)
        })
        .collect();
    let compact_set_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            let fname = field.ident.as_ref().expect("field must be named");
            compact_mut_set_stmt(fname, pd)
        })
        .collect();
    quote! {
        #[must_use = "bind the as_mut() guard to mutate cached fields; changes auto-save on drop"]
        pub struct #guard_name<'a> {
            __view: &'a mut AccountView,
            __payer: &'a AccountView,
            #(#guard_fields,)*
        }

        impl<'a> core::ops::Deref for #guard_name<'a> {
            type Target = #zc_path;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_path) }
            }
        }

        impl<'a> #guard_name<'a> {
            pub fn save(&mut self) -> Result<(), ProgramError> {
                let __tail_size: usize = 0 #(#save_size_terms)*;
                let __new_total = #disc_len
                    + <#zc_mod::__Schema as quasar_lang::ZeroPodCompact>::HEADER_SIZE
                    + __tail_size;

                let __old_total = self.__view.data_len();
                if __new_total != __old_total {
                    quasar_lang::accounts::account::realloc_account(
                        self.__view, __new_total, self.__payer,
                        None,
                    )?;
                }

                let __compact_data = unsafe {
                    core::slice::from_raw_parts_mut(
                        self.__view.data_mut_ptr().add(#disc_len),
                        __new_total - #disc_len,
                    )
                };
                let mut __compact = unsafe { #zc_mod::__SchemaMut::new_unchecked(__compact_data) };
                #(#compact_set_stmts)*
                __compact.commit().map_err(|_| ProgramError::InvalidAccountData)?;
                Ok(())
            }

            pub fn reload(&mut self) {
                let (#(#field_names,)*) = {
                    let __data = unsafe { self.__view.borrow_unchecked() };
                    let __r = unsafe { #zc_mod::__SchemaRef::new_unchecked(&__data[#disc_len..]) };
                    #(#load_stmts)*
                    (#(#field_names,)*)
                };
                #(self.#field_names = #field_names;)*
            }
        }

        impl<'a> Drop for #guard_name<'a> {
            fn drop(&mut self) {
                self.save().expect("as_mut auto-save failed");
            }
        }

        impl #name {
            #[inline(always)]
            pub fn as_mut<'a>(
                &'a mut self,
                payer: &'a AccountView,
            ) -> #guard_name<'a> {
                let (#(#field_names,)*) = {
                    let __data = unsafe { self.__view.borrow_unchecked() };
                    let __r = unsafe { #zc_mod::__SchemaRef::new_unchecked(&__data[#disc_len..]) };
                    #(#load_stmts)*
                    (#(#field_names,)*)
                };
                // SAFETY: `self.__view` is the transparent account backing store for this
                // wrapper. Reborrowing it as `&mut AccountView` is sound here because the
                // guard exclusively owns `&'a mut self` for its full lifetime and does not
                // create any competing mutable references.
                let __view = unsafe { &mut *(&mut self.__view as *mut AccountView) };
                #guard_name {
                    __view,
                    __payer: payer,
                    #(#field_names,)*
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dyn_max_space_term(dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, .. } => quote! { + #max },
        PodDynField::Vec { elem, max, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { + #max * core::mem::size_of::<#mapped>() }
        }
    }
}

/// Read accessor: construct a CompactRef, delegate to its accessor.
fn compact_read_accessor(
    name: &syn::Ident,
    dyn_field: &PodDynField,
    disc_len: usize,
    zc_mod: &syn::Ident,
) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            #[inline(always)]
            pub fn #name(&self) -> &str {
                let __data = unsafe { self.__view.borrow_unchecked() };
                let __r = unsafe { #zc_mod::__SchemaRef::new_unchecked(&__data[#disc_len..]) };
                __r.#name()
            }
        },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! {
                #[inline(always)]
                pub fn #name(&self) -> &[#mapped] {
                    let __data = unsafe { self.__view.borrow_unchecked() };
                    let __r = unsafe { #zc_mod::__SchemaRef::new_unchecked(&__data[#disc_len..]) };
                    __r.#name()
                }
            }
        }
    }
}

fn dyn_view_field(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let slot = format_ident!("__{}", name);
    match dyn_field {
        PodDynField::Str { .. } => quote! { #slot: Option<&'a str> },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { #slot: Option<&'a [#mapped]> }
        }
    }
}

fn dyn_view_setter(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let slot = format_ident!("__{}", name);
    let setter = format_ident!("set_{}", name);
    let max = match dyn_field {
        PodDynField::Str { max, .. } | PodDynField::Vec { max, .. } => max,
    };

    match dyn_field {
        PodDynField::Str { .. } => quote! {
            #[inline(always)]
            pub fn #setter(&mut self, value: &'a str) -> Result<(), ProgramError> {
                if value.len() > #max {
                    return Err(QuasarError::DynamicFieldTooLong.into());
                }
                self.#slot = Some(value);
                Ok(())
            }
        },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! {
                #[inline(always)]
                pub fn #setter(&mut self, value: &'a [#mapped]) -> Result<(), ProgramError> {
                    if value.len() > #max {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                    self.#slot = Some(value);
                    Ok(())
                }
            }
        }
    }
}

fn writer_space_term(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { .. } => quote! { + #name.len() },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { + #name.len() * core::mem::size_of::<#mapped>() }
        }
    }
}

fn writer_compact_set_stmt(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let setter = format_ident!("set_{}", name);
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            __compact.#setter(#name).map_err(|_| ProgramError::InvalidAccountData)?;
        },
        PodDynField::Vec { .. } => quote! {
            __compact.#setter(#name).map_err(|_| ProgramError::InvalidAccountData)?;
        },
    }
}

// ---------------------------------------------------------------------------
// CompactMut guard helpers
// ---------------------------------------------------------------------------

fn compact_mut_field(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => quote! {
            pub #name: quasar_lang::pod::PodString<#max, #prefix_bytes>
        },
        PodDynField::Vec {
            elem,
            max,
            prefix_bytes,
        } => {
            let mapped = map_to_pod_type(elem);
            quote! {
                pub #name: quasar_lang::pod::PodVec<#mapped, #max, #prefix_bytes>
            }
        }
    }
}

/// Load a dynamic field from a CompactRef into a PodString/PodVec.
/// Assumes `__r` (a `__SchemaRef`) is in scope.
fn compact_mut_load(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => quote! {
            let mut #name = quasar_lang::pod::PodString::<#max, #prefix_bytes>::default();
            let _ = #name.set(__r.#name());
        },
        PodDynField::Vec {
            elem,
            max,
            prefix_bytes,
        } => {
            let mapped = map_to_pod_type(elem);
            quote! {
                let mut #name = quasar_lang::pod::PodVec::<#mapped, #max, #prefix_bytes>::default();
                let _ = #name.set_from_slice(__r.#name());
            }
        }
    }
}

/// Size contribution of a guard field's current content (for tail region).
fn compact_mut_size_term(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { .. } => quote! { + self.#name.len() },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { + self.#name.len() * core::mem::size_of::<#mapped>() }
        }
    }
}

/// Set a dynamic field on a CompactMut from the guard's PodString/PodVec.
fn compact_mut_set_stmt(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let setter = format_ident!("set_{}", name);
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            __compact.#setter(self.#name.as_str()).map_err(|_| ProgramError::InvalidAccountData)?;
        },
        PodDynField::Vec { .. } => quote! {
            __compact.#setter(self.#name.as_slice()).map_err(|_| ProgramError::InvalidAccountData)?;
        },
    }
}
