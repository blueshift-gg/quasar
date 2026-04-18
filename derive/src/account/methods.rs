use {
    super::{dynamic, fixed::PodFieldInfo},
    crate::helpers::zc_assign_from_value,
    quote::{format_ident, quote},
};

pub(super) struct SetInnerSpec<'a> {
    pub name: &'a syn::Ident,
    pub vis: &'a syn::Visibility,
    pub field_infos: &'a [PodFieldInfo<'a>],
    pub has_dynamic: bool,
    pub disc_len: usize,
    pub zc_mod: &'a syn::Ident,
    pub zc_path: &'a proc_macro2::TokenStream,
    pub gen_set_inner: bool,
}

pub(super) fn emit_set_inner_impl(spec: SetInnerSpec<'_>) -> proc_macro2::TokenStream {
    let SetInnerSpec {
        name,
        vis,
        field_infos,
        has_dynamic,
        disc_len,
        zc_mod,
        zc_path,
        gen_set_inner,
    } = spec;

    if !gen_set_inner {
        return quote! {};
    }

    if has_dynamic {
        let inner_name = format_ident!("{}Inner", name);
        let inner_fields: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .map(|fi| {
                let field_name = fi.field.ident.as_ref().expect("field must be named");
                match &fi.pod_dyn {
                    None => {
                        let field_ty = &fi.field.ty;
                        quote! { pub #field_name: #field_ty }
                    }
                    Some(dyn_field) => dynamic::emit_inner_field(field_name, dyn_field),
                }
            })
            .collect();
        let max_checks: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter_map(|fi| {
                let field_name = fi.field.ident.as_ref().expect("field must be named");
                fi.pod_dyn
                    .as_ref()
                    .map(|dyn_field| dynamic::emit_max_check(field_name, dyn_field))
            })
            .collect();
        let space_terms: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter_map(|fi| {
                let field_name = fi.field.ident.as_ref().expect("field must be named");
                fi.pod_dyn
                    .as_ref()
                    .map(|dyn_field| dynamic::emit_space_term(field_name, dyn_field))
            })
            .collect();
        let zc_header_stmts: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter(|fi| fi.pod_dyn.is_none())
            .map(|fi| {
                zc_assign_from_value(
                    fi.field.ident.as_ref().expect("field must be named"),
                    &fi.field.ty,
                )
            })
            .collect();
        let compact_set_stmts: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .filter_map(|fi| {
                let field_name = fi.field.ident.as_ref().expect("field must be named");
                fi.pod_dyn.as_ref().map(|dyn_field| {
                    let setter = format_ident!("set_{}", field_name);
                    match dyn_field {
                        crate::helpers::PodDynField::Str { .. } => quote! {
                            __compact.#setter(#field_name)
                                .map_err(|_| ProgramError::InvalidAccountData)?;
                        },
                        crate::helpers::PodDynField::Vec { .. } => quote! {
                            __compact.#setter(#field_name)
                                .map_err(|_| ProgramError::InvalidAccountData)?;
                        },
                    }
                })
            })
            .collect();
        let init_field_names: Vec<&syn::Ident> = field_infos
            .iter()
            .map(|fi| fi.field.ident.as_ref().expect("field must be named"))
            .collect();

        quote! {
            #vis struct #inner_name<'a> {
                #(#inner_fields,)*
            }

            impl #name {
                #[inline(always)]
                pub fn set_inner(
                    &mut self,
                    inner: #inner_name<'_>,
                    payer: &AccountView,
                    rent_lpb: u64,
                    rent_threshold: u64,
                ) -> Result<(), ProgramError> {
                    #(let #init_field_names = inner.#init_field_names;)*
                    #(#max_checks)*

                    let __space = Self::MIN_SPACE #(#space_terms)*;
                    let __view = unsafe { &mut *(self as *mut Self as *mut AccountView) };

                    if __space != __view.data_len() {
                        quasar_lang::accounts::account::realloc_account_raw(
                            __view,
                            __space,
                            payer,
                            rent_lpb,
                            rent_threshold,
                        )?;
                    }

                    // Write fixed header fields.
                    let __ptr = __view.data_mut_ptr();
                    let __zc = unsafe { &mut *(__ptr.add(#disc_len) as *mut #zc_path) };
                    #(#zc_header_stmts)*

                    // Write dynamic fields via CompactMut.
                    let __compact_data = unsafe {
                        core::slice::from_raw_parts_mut(
                            __ptr.add(#disc_len),
                            __view.data_len() - #disc_len,
                        )
                    };
                    let mut __compact = unsafe {
                        #zc_mod::__SchemaMut::new_unchecked(__compact_data)
                    };
                    #(#compact_set_stmts)*
                    __compact.commit().map_err(|_| ProgramError::InvalidAccountData)?;
                    Ok(())
                }
            }
        }
    } else {
        let inner_name = format_ident!("{}Inner", name);
        let field_names: Vec<_> = field_infos.iter().map(|fi| &fi.field.ident).collect();
        let field_types: Vec<_> = field_infos.iter().map(|fi| &fi.field.ty).collect();
        let set_inner_stmts: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .map(|fi| {
                zc_assign_from_value(
                    fi.field.ident.as_ref().expect("field must be named"),
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
}
