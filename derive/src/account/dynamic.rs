use {
    super::fixed::PodFieldInfo,
    crate::helpers::PodDynField,
    quote::{format_ident, quote},
};

pub(super) type DynFieldRef<'a> = (&'a syn::Field, &'a PodDynField);

pub(super) struct DynamicPieces<'a> {
    pub dyn_fields: Vec<DynFieldRef<'a>>,
    pub align_asserts: Vec<proc_macro2::TokenStream>,
    pub prefix_total: usize,
    pub max_space_terms: Vec<proc_macro2::TokenStream>,
    pub validation_stmts: Vec<proc_macro2::TokenStream>,
    pub read_accessors: Vec<proc_macro2::TokenStream>,
}

pub(super) fn build_dynamic_pieces<'a>(
    field_infos: &'a [PodFieldInfo<'a>],
    disc_len: usize,
    zc_path: &proc_macro2::TokenStream,
) -> DynamicPieces<'a> {
    let dyn_fields: Vec<DynFieldRef<'a>> = field_infos
        .iter()
        .filter_map(|fi| fi.pod_dyn.as_ref().map(|pd| (fi.field, pd)))
        .collect();
    let align_asserts = dyn_fields
        .iter()
        .filter_map(|(_, pd)| dyn_align_assert(pd))
        .collect();
    let prefix_total = dyn_fields.iter().map(|(_, pd)| dyn_prefix_bytes(pd)).sum();
    let max_space_terms = dyn_fields
        .iter()
        .map(|(_, pd)| dyn_max_space_term(pd))
        .collect();
    let validation_stmts = dyn_fields
        .iter()
        .map(|(_, pd)| dyn_validation_stmt(pd))
        .collect();
    let dyn_start = quote! { #disc_len + core::mem::size_of::<#zc_path>() };
    let read_accessors = dyn_fields
        .iter()
        .enumerate()
        .map(|(dyn_idx, (field, pd))| {
            let name = field.ident.as_ref().expect("field must be named");
            let walk_stmts: Vec<proc_macro2::TokenStream> = dyn_fields[..dyn_idx]
                .iter()
                .map(|(_, prev_pd)| dyn_walk_stmt(prev_pd))
                .collect();
            dyn_read_accessor(name, pd, &dyn_start, &walk_stmts)
        })
        .collect();

    DynamicPieces {
        dyn_fields,
        align_asserts,
        prefix_total,
        max_space_terms,
        validation_stmts,
        read_accessors,
    }
}

pub(super) fn emit_inner_field(
    name: &syn::Ident,
    dyn_field: &PodDynField,
) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { .. } => quote! { pub #name: &'a str },
        PodDynField::Vec { elem, .. } => quote! { pub #name: &'a [#elem] },
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
            quote! { + #name.len() * core::mem::size_of::<#elem>() }
        }
    }
}

pub(super) fn emit_write_stmt(
    name: &syn::Ident,
    dyn_field: &PodDynField,
) -> proc_macro2::TokenStream {
    let prefix_bytes = dyn_prefix_bytes(dyn_field);
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            {
                let __len_bytes = (#name.len() as u64).to_le_bytes();
                __data[__offset..__offset + #prefix_bytes].copy_from_slice(&__len_bytes[..#prefix_bytes]);
                __offset += #prefix_bytes;
                __data[__offset..__offset + #name.len()].copy_from_slice(#name.as_bytes());
                __offset += #name.len();
            }
        },
        PodDynField::Vec { elem, .. } => quote! {
            {
                let __count_bytes = (#name.len() as u64).to_le_bytes();
                __data[__offset..__offset + #prefix_bytes].copy_from_slice(&__count_bytes[..#prefix_bytes]);
                __offset += #prefix_bytes;
                let __bytes = #name.len() * core::mem::size_of::<#elem>();
                if __bytes > 0 {
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            #name.as_ptr() as *const u8,
                            __data[__offset..].as_mut_ptr(),
                            __bytes,
                        );
                    }
                }
                __offset += __bytes;
            }
        },
    }
}

pub(super) fn emit_dynamic_impl_block(
    name: &syn::Ident,
    has_dynamic: bool,
    disc_len: usize,
    zc_path: &proc_macro2::TokenStream,
    pieces: &DynamicPieces<'_>,
) -> proc_macro2::TokenStream {
    if has_dynamic {
        let prefix_total = pieces.prefix_total;
        let max_space_terms = &pieces.max_space_terms;
        let read_accessors = &pieces.read_accessors;
        quote! {
            impl #name {
                pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_path>() + #prefix_total;
                pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

                #(#read_accessors)*
            }
        }
    } else {
        quote! {}
    }
}

pub(super) fn emit_dyn_guard(
    name: &syn::Ident,
    has_dynamic: bool,
    disc_len: usize,
    zc_name: &syn::Ident,
    pieces: &DynamicPieces<'_>,
) -> proc_macro2::TokenStream {
    if !has_dynamic {
        return quote! {};
    }

    let guard_name = format_ident!("{}DynGuard", name);
    let guard_fields: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| dyn_guard_field(field.ident.as_ref().expect("field must be named"), pd))
        .collect();
    let load_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| dyn_guard_load(field.ident.as_ref().expect("field must be named"), pd))
        .collect();
    let field_names: Vec<&syn::Ident> = pieces
        .dyn_fields
        .iter()
        .map(|(field, _)| field.ident.as_ref().expect("field must be named"))
        .collect();
    let save_size_terms: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, _)| {
            let name = field.ident.as_ref().expect("field must be named");
            quote! { + self.#name.serialized_len() }
        })
        .collect();
    let save_write_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, _)| {
            let name = field.ident.as_ref().expect("field must be named");
            quote! { __off += self.#name.write_to_bytes(&mut __data[__off..]); }
        })
        .collect();

    quote! {
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
                self.save().expect("dynamic field auto-save failed");
            }
        }

        impl #name {
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
                };
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
}

pub(super) fn emit_dyn_writer(
    name: &syn::Ident,
    has_dynamic: bool,
    disc_len: usize,
    zc_name: &syn::Ident,
    pieces: &DynamicPieces<'_>,
) -> proc_macro2::TokenStream {
    if !has_dynamic {
        return quote! {};
    }

    let writer_name = format_ident!("{}DynWriter", name);
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
                let #name = self.#slot.ok_or(QuasarError::DynWriterFieldNotSet)?;
            }
        })
        .collect();
    let size_terms: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            let name = field.ident.as_ref().expect("field must be named");
            dynamic_view_space_term(name, pd)
        })
        .collect();
    let write_stmts: Vec<proc_macro2::TokenStream> = pieces
        .dyn_fields
        .iter()
        .map(|(field, pd)| {
            let name = field.ident.as_ref().expect("field must be named");
            dynamic_view_write_stmt(name, pd)
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
            type Target = #zc_name;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_name) }
            }
        }

        impl<'a> core::ops::DerefMut for #writer_name<'a> {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_name) }
            }
        }

        impl<'a> #writer_name<'a> {
            #(#setter_methods)*

            pub fn commit(&mut self) -> Result<(), ProgramError> {
                #(#binding_stmts)*

                let __new_total = #disc_len + core::mem::size_of::<#zc_name>()
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

                let __dyn_start = #disc_len + core::mem::size_of::<#zc_name>();
                let __ptr = self.__view.data_mut_ptr();
                let __data = unsafe {
                    core::slice::from_raw_parts_mut(
                        __ptr.add(__dyn_start),
                        __new_total - __dyn_start,
                    )
                };
                let mut __offset = 0usize;
                #(#write_stmts)*
                let _ = __offset;
                Ok(())
            }
        }

        impl #name {
            #[inline(always)]
            pub fn as_dynamic_writer<'a>(
                &'a mut self,
                payer: &'a AccountView,
                rent_lpb: u64,
                rent_threshold: u64,
            ) -> #writer_name<'a> {
                // SAFETY: `self.__view` is the transparent account backing store for this
                // wrapper. Reborrowing it as `&mut AccountView` is sound here because the
                // writer exclusively owns `&'a mut self` for its full lifetime and does not
                // create any competing mutable references. This follows the same Tree Borrows
                // pattern used by the dynamic stack-cache guard path.
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

fn dyn_align_assert(dyn_field: &PodDynField) -> Option<proc_macro2::TokenStream> {
    match dyn_field {
        PodDynField::Vec { elem, .. } => Some(quote! {
            const _: () = assert!(
                core::mem::align_of::<#elem>() == 1,
                "PodVec element type must have alignment 1"
            );
        }),
        PodDynField::Str { .. } => None,
    }
}

fn dyn_prefix_bytes(dyn_field: &PodDynField) -> usize {
    match dyn_field {
        PodDynField::Str { prefix_bytes, .. } | PodDynField::Vec { prefix_bytes, .. } => {
            *prefix_bytes
        }
    }
}

fn dyn_max_space_term(dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, .. } => quote! { + #max },
        PodDynField::Vec { elem, max, .. } => {
            quote! { + #max * core::mem::size_of::<#elem>() }
        }
    }
}

fn dyn_validation_stmt(dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let prefix_bytes = dyn_prefix_bytes(dyn_field);
    match dyn_field {
        PodDynField::Str { max, .. } => quote! {
            {
                if __offset + #prefix_bytes > __data_len {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                let __len = {
                    let mut __buf = [0u8; 8];
                    __buf[..#prefix_bytes].copy_from_slice(&__data[__offset..__offset + #prefix_bytes]);
                    u64::from_le_bytes(__buf) as usize
                };
                __offset += #prefix_bytes;
                if __len > #max {
                    return Err(ProgramError::InvalidAccountData);
                }
                if __offset + __len > __data_len {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                __offset += __len;
            }
        },
        PodDynField::Vec { elem, max, .. } => quote! {
            {
                if __offset + #prefix_bytes > __data_len {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                let __count = {
                    let mut __buf = [0u8; 8];
                    __buf[..#prefix_bytes].copy_from_slice(&__data[__offset..__offset + #prefix_bytes]);
                    u64::from_le_bytes(__buf) as usize
                };
                __offset += #prefix_bytes;
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
}

fn dyn_walk_stmt(dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let prefix_bytes = dyn_prefix_bytes(dyn_field);
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            {
                let mut __buf = [0u8; 8];
                __buf[..#prefix_bytes].copy_from_slice(&__data[__off..__off + #prefix_bytes]);
                let __field_len = u64::from_le_bytes(__buf) as usize;
                __off += #prefix_bytes + __field_len;
            }
        },
        PodDynField::Vec { elem, .. } => quote! {
            {
                let mut __buf = [0u8; 8];
                __buf[..#prefix_bytes].copy_from_slice(&__data[__off..__off + #prefix_bytes]);
                let __field_count = u64::from_le_bytes(__buf) as usize;
                __off += #prefix_bytes + __field_count * core::mem::size_of::<#elem>();
            }
        },
    }
}

fn dyn_read_accessor(
    name: &syn::Ident,
    dyn_field: &PodDynField,
    dyn_start: &proc_macro2::TokenStream,
    walk_stmts: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    let prefix_bytes = dyn_prefix_bytes(dyn_field);
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            #[inline(always)]
            pub fn #name(&self) -> &str {
                let __data = unsafe { self.__view.borrow_unchecked() };
                let mut __off = #dyn_start;
                #(#walk_stmts)*
                let __len = {
                    let mut __buf = [0u8; 8];
                    __buf[..#prefix_bytes].copy_from_slice(&__data[__off..__off + #prefix_bytes]);
                    u64::from_le_bytes(__buf) as usize
                };
                unsafe { core::str::from_utf8_unchecked(&__data[__off + #prefix_bytes..__off + #prefix_bytes + __len]) }
            }
        },
        PodDynField::Vec { elem, .. } => quote! {
            #[inline(always)]
            pub fn #name(&self) -> &[#elem] {
                let __data = unsafe { self.__view.borrow_unchecked() };
                let mut __off = #dyn_start;
                #(#walk_stmts)*
                let __count = {
                    let mut __buf = [0u8; 8];
                    __buf[..#prefix_bytes].copy_from_slice(&__data[__off..__off + #prefix_bytes]);
                    u64::from_le_bytes(__buf) as usize
                };
                unsafe {
                    core::slice::from_raw_parts(
                        __data[__off + #prefix_bytes..].as_ptr() as *const #elem,
                        __count,
                    )
                }
            }
        },
    }
}

fn dyn_guard_field(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => quote! {
            pub #name: quasar_lang::pod::PodString<#max, #prefix_bytes>
        },
        PodDynField::Vec {
            elem,
            max,
            prefix_bytes,
        } => quote! {
            pub #name: quasar_lang::pod::PodVec<#elem, #max, #prefix_bytes>
        },
    }
}

fn dyn_guard_load(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { max, prefix_bytes } => quote! {
            let mut #name = quasar_lang::pod::PodString::<#max, #prefix_bytes>::default();
            __off += #name.load_from_bytes(&__data[__off..]);
        },
        PodDynField::Vec {
            elem,
            max,
            prefix_bytes,
        } => quote! {
            let mut #name = quasar_lang::pod::PodVec::<#elem, #max, #prefix_bytes>::default();
            __off += #name.load_from_bytes(&__data[__off..]);
        },
    }
}

fn dyn_view_field(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let slot = format_ident!("__{}", name);
    match dyn_field {
        PodDynField::Str { .. } => quote! { #slot: Option<&'a str> },
        PodDynField::Vec { elem, .. } => quote! { #slot: Option<&'a [#elem]> },
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
        PodDynField::Vec { elem, .. } => quote! {
            #[inline(always)]
            pub fn #setter(&mut self, value: &'a [#elem]) -> Result<(), ProgramError> {
                if value.len() > #max {
                    return Err(QuasarError::DynamicFieldTooLong.into());
                }
                self.#slot = Some(value);
                Ok(())
            }
        },
    }
}

fn dynamic_view_space_term(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    match dyn_field {
        PodDynField::Str { prefix_bytes, .. } => quote! { + #prefix_bytes + #name.len() },
        PodDynField::Vec {
            elem, prefix_bytes, ..
        } => {
            quote! { + #prefix_bytes + #name.len() * core::mem::size_of::<#elem>() }
        }
    }
}

fn dynamic_view_write_stmt(name: &syn::Ident, dyn_field: &PodDynField) -> proc_macro2::TokenStream {
    let prefix_bytes = dyn_prefix_bytes(dyn_field);
    match dyn_field {
        PodDynField::Str { .. } => quote! {
            {
                let __len_bytes = (#name.len() as u64).to_le_bytes();
                __data[__offset..__offset + #prefix_bytes].copy_from_slice(&__len_bytes[..#prefix_bytes]);
                __offset += #prefix_bytes;
                __data[__offset..__offset + #name.len()].copy_from_slice(#name.as_bytes());
                __offset += #name.len();
            }
        },
        PodDynField::Vec { elem, .. } => quote! {
            {
                let __count_bytes = (#name.len() as u64).to_le_bytes();
                __data[__offset..__offset + #prefix_bytes].copy_from_slice(&__count_bytes[..#prefix_bytes]);
                __offset += #prefix_bytes;
                let __bytes = #name.len() * core::mem::size_of::<#elem>();
                if __bytes > 0 {
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            #name.as_ptr() as *const u8,
                            __data[__offset..].as_mut_ptr(),
                            __bytes,
                        );
                    }
                }
                __offset += __bytes;
            }
        },
    }
}
