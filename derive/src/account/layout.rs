use {
    super::fixed::PodFieldInfo,
    crate::{
        helpers::{map_to_pod_type, pascal_to_snake},
        schema_ir::{LayoutClass, SchemaField, SchemaIR},
    },
    quote::{format_ident, quote},
};

pub(super) struct ZcSpec {
    pub zc_name: syn::Ident,
    pub zc_mod: syn::Ident,
    pub zc_path: proc_macro2::TokenStream,
    /// Native-typed fields for the fixed (non-compact) zeropod schema struct.
    /// Empty for dynamic accounts (which carry a `compact_ir` instead).
    pub schema_fields: Vec<proc_macro2::TokenStream>,
    /// The compact schema IR for dynamic accounts (emitted via the
    /// single-source `emit_compact_schema`); `None` for fixed accounts.
    pub compact_ir: Option<SchemaIR>,
}

pub(super) fn build_zc_spec(
    name: &syn::Ident,
    field_infos: &[PodFieldInfo<'_>],
    has_dynamic: bool,
) -> ZcSpec {
    let (schema_fields, compact_ir) = if has_dynamic {
        // Dynamic accounts: all fields (fixed with native types + dynamic with
        // compact pod types) via the shared compact-schema IR. `PodVec` uses the
        // mapped `ZcField` companion element to match the read accessors. Field
        // ordering was validated in `account_inner` before codegen.
        let fields: Vec<SchemaField> = field_infos
            .iter()
            .map(|fi| {
                let field = fi.field;
                let ident = field
                    .ident
                    .clone()
                    .unwrap_or_else(|| ice!("field must be named"));
                let vis = field.vis.clone();
                match &fi.pod_dyn {
                    None => {
                        let ty = &field.ty;
                        let attrs: Vec<syn::Attribute> = field
                            .attrs
                            .iter()
                            .filter(|a| a.path().is_ident("zeropod"))
                            .cloned()
                            .collect();
                        SchemaField {
                            ident,
                            vis,
                            attrs,
                            class: LayoutClass::Fixed { ty: quote!(#ty) },
                        }
                    }
                    Some(pd) => SchemaField {
                        ident,
                        vis,
                        attrs: Vec::new(),
                        class: LayoutClass::from_pod_dyn(pd, map_to_pod_type),
                    },
                }
            })
            .collect();
        let ir = SchemaIR::new(fields)
            .unwrap_or_else(|_| ice!("account field ordering validated before codegen"));
        (Vec::new(), Some(ir))
    } else {
        // Fixed accounts: only the fixed fields with native types.
        let fields = field_infos
            .iter()
            .filter(|fi| fi.pod_dyn.is_none())
            .map(|fi| {
                let field = fi.field;
                let vis = &field.vis;
                let name = field
                    .ident
                    .as_ref()
                    .unwrap_or_else(|| ice!("field must be named"));
                let ty = &field.ty;
                // Pass through #[zeropod(...)] attributes (e.g. skip_accessor).
                let zeropod_attrs: Vec<_> = field
                    .attrs
                    .iter()
                    .filter(|a| a.path().is_ident("zeropod"))
                    .collect();
                quote! { #(#zeropod_attrs)* #vis #name: #ty }
            })
            .collect();
        (fields, None)
    };

    let zc_name = format_ident!("{}Zc", name);
    let zc_mod = format_ident!("__{}_zc", pascal_to_snake(&name.to_string()));
    let zc_path = quote! { #zc_mod::#zc_name };

    ZcSpec {
        zc_name,
        zc_mod,
        zc_path,
        schema_fields,
        compact_ir,
    }
}

pub(super) fn emit_bump_offset_impl(
    field_infos: &[PodFieldInfo<'_>],
    disc_len: usize,
    zc_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let has_bump_u8 = field_infos.iter().any(|fi| {
        fi.field.ident.as_ref().is_some_and(|id| id == "bump")
            && matches!(&fi.field.ty, syn::Type::Path(tp) if tp.path.is_ident("u8"))
    });

    if has_bump_u8 {
        quote! {
            const BUMP_OFFSET: Option<usize> = Some(
                #disc_len + core::mem::offset_of!(#zc_path, bump)
            );
        }
    } else {
        quote! {}
    }
}

pub(super) fn emit_zc_definition(
    name: &syn::Ident,
    has_dynamic: bool,
    zc: &ZcSpec,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let zc_name = &zc.zc_name;
    let zc_mod = &zc.zc_mod;
    let schema_fields = &zc.schema_fields;

    if has_dynamic {
        // Compact schema: all fields (fixed + dynamic). zeropod generates
        // __SchemaHeader, __SchemaRef, __SchemaMut at the module scope. The
        // the compact `pub struct __Schema` is emitted by the shared
        // single-source emitter.
        let ir = zc
            .compact_ir
            .as_ref()
            .unwrap_or_else(|| ice!("dynamic account must carry a compact schema IR"));
        let schema_struct = crate::schema_ir::emit_compact_schema(
            &format_ident!("__Schema"),
            ir,
            &syn::parse_quote!(pub),
        );
        quote! {
            #[doc(hidden)]
            pub mod #zc_mod {
                use super::*;
                use #krate::__zeropod as zeropod;

                #schema_struct

                pub type #zc_name = __SchemaHeader;
            }

            const _: () = assert!(
                core::mem::size_of::<#name>() == core::mem::size_of::<#krate::__internal::AccountView>(),
                "Pod-dynamic struct must be #[repr(transparent)] over AccountView"
            );
        }
    } else {
        quote! {
            #[doc(hidden)]
            pub mod #zc_mod {
                use super::*;
                use #krate::__zeropod as zeropod;

                #[derive(zeropod::ZeroPod)]
                pub struct __Schema {
                    #(#schema_fields,)*
                }

                pub type #zc_name = __SchemaZc;
            }
        }
    }
}

pub(super) fn emit_account_wrapper(
    attrs: &[syn::Attribute],
    vis: &syn::Visibility,
    name: &syn::Ident,
    disc_len: usize,
    zc_path: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let data_alias = quote::format_ident!("{}Data", name);
    let data_doc = format!(
        "Raw `#[repr(C)]` data layout for [`{name}`].\n\nUse this type when constructing account \
         data values (e.g., for `Migrate` implementations)."
    );

    quote! {
        #(#attrs)*
        #[repr(transparent)]
        #vis struct #name {
            __view: #krate::__internal::AccountView,
        }

        #[doc = #data_doc]
        #vis type #data_alias = #zc_path;

        unsafe impl #krate::traits::StaticView for #name {}

        impl #krate::traits::AsAccountView for #name {
            #[inline(always)]
            fn to_account_view(&self) -> &#krate::__internal::AccountView {
                &self.__view
            }
        }

        impl core::ops::Deref for #name {
            type Target = #zc_path;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                // SAFETY: AccountLoad validated that the account data contains a
                // ZC value immediately after the discriminator.
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_path) }
            }
        }

        impl core::ops::DerefMut for #name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                // SAFETY: `&mut self` gives exclusive access to the transparent
                // account wrapper and its backing account data.
                unsafe { &mut *(self.__view.data_mut_ptr().add(#disc_len) as *mut #zc_path) }
            }
        }
    }
}
