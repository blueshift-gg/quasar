//! Codegen for `#[account(one_of)] pub enum ConsensusAccount { ... }`
//! polymorphic account types.
//!
//! Generates a `#[repr(transparent)]` wrapper over `AccountView` that validates
//! the discriminator matches one of the enum variants, delegates `Owner` to
//! the (asserted-equal) owner of all variants, and provides:
//! - `ConsensusAccountRef<'a>` ref enum for pattern matching
//! - `variant()` method for discriminator-dispatched matching
//! - `is_X()` / `X()` typed accessors per variant
//! - Pairwise discriminator prefix assertions (security)
//! - Optional `Deref<Target = dyn Trait>` via `implements()`

use {
    crate::helpers::pascal_to_snake,
    quote::{format_ident, quote},
};

/// Extract variant names and inner types from an enum declaration.
pub(crate) struct OneOfVariant {
    pub ident: syn::Ident,
    pub inner_ty: syn::Path,
}

pub(crate) fn extract_variants(data: &syn::DataEnum) -> syn::Result<Vec<OneOfVariant>> {
    let mut variants = Vec::new();
    for variant in &data.variants {
        let inner = match &variant.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let ty = &fields.unnamed[0].ty;
                match ty {
                    syn::Type::Path(tp) => tp.path.clone(),
                    _ => {
                        return Err(syn::Error::new_spanned(
                            ty,
                            "one_of variant must be a path type: `Variant(Type)`",
                        ));
                    }
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    variant,
                    "one_of variants must have exactly one unnamed field: `Variant(Type)`",
                ));
            }
        };
        variants.push(OneOfVariant {
            ident: variant.ident.clone(),
            inner_ty: inner,
        });
    }
    if variants.len() < 2 {
        return Err(syn::Error::new_spanned(
            &data.variants,
            "one_of requires at least two variants",
        ));
    }
    Ok(variants)
}

pub(crate) fn generate_one_of_account(
    name: &syn::Ident,
    variants: &[OneOfVariant],
    implements: Option<&syn::Path>,
) -> proc_macro2::TokenStream {
    let variant_paths: Vec<&syn::Path> = variants.iter().map(|v| &v.inner_ty).collect();

    // 1. #[repr(transparent)] struct with __view: AccountView
    let struct_def = quote! {
        #[repr(transparent)]
        pub struct #name {
            __view: quasar_lang::__internal::AccountView,
        }
    };

    // 2. AsAccountView
    let as_account_view = quote! {
        impl quasar_lang::traits::AsAccountView for #name {
            #[inline(always)]
            fn to_account_view(&self) -> &quasar_lang::__internal::AccountView {
                &self.__view
            }
        }
    };

    // 3. AccountCheck — delegates to each variant's full check(), not just disc
    let variant_checks: Vec<proc_macro2::TokenStream> = variant_paths
        .iter()
        .map(|v| {
            quote! {
                <#v as quasar_lang::traits::AccountCheck>::check(view).is_ok()
            }
        })
        .collect();

    let account_check = quote! {
        impl quasar_lang::traits::AccountCheck for #name {

            #[inline(always)]
            fn check(view: &quasar_lang::__internal::AccountView) -> Result<(), quasar_lang::prelude::ProgramError> {
                if #(#variant_checks)||* {
                    Ok(())
                } else {
                    Err(quasar_lang::prelude::ProgramError::InvalidAccountData)
                }
            }
        }
    };

    // 4. Owner — const assertion all variants share same owner
    let first_variant = &variant_paths[0];
    let owner_checks: Vec<proc_macro2::TokenStream> = variant_paths
        .iter()
        .skip(1)
        .map(|v| {
            quote! {
                assert!(
                    quasar_lang::keys_eq_const(
                        &<#first_variant as quasar_lang::traits::Owner>::OWNER,
                        &<#v as quasar_lang::traits::Owner>::OWNER,
                    ),
                    "all one_of variants must have the same program owner"
                );
            }
        })
        .collect();

    let owner_impl = quote! {
        const _: () = {
            #(#owner_checks)*
        };

        impl quasar_lang::traits::Owner for #name {
            const OWNER: quasar_lang::prelude::Address =
                <#first_variant as quasar_lang::traits::Owner>::OWNER;
        }
    };

    // 5. Pairwise discriminator prefix assertions (Security Finding #3)
    let disc_assertions = emit_pairwise_disc_assertions(&variant_paths);

    // 6. Space — max of all variants
    let space_impl = emit_max_space(name, &variant_paths);

    // 7. StaticView
    let static_view = quote! {
        unsafe impl quasar_lang::traits::StaticView for #name {}
    };

    // 8. Ref enum for pattern matching
    let ref_enum_name = format_ident!("{}Ref", name);
    let ref_variants: Vec<proc_macro2::TokenStream> = variants
        .iter()
        .map(|v| {
            let ident = &v.ident;
            let ty = &v.inner_ty;
            quote! { #ident(&'a quasar_lang::accounts::account::Account<#ty>) }
        })
        .collect();

    let ref_enum = quote! {
        pub enum #ref_enum_name<'a> {
            #(#ref_variants,)*
        }
    };

    // 9. variant() + typed accessors
    let accessors = emit_one_of_accessors(name, variants, &ref_enum_name);

    // 10. Compile-time trait bound assertion (if implements present)
    let trait_assertion = implements.map(|trait_path| {
        quote! {
            const _: fn() = || {
                fn __assert_impl<T: #trait_path>() {}
                #(
                    __assert_impl::<#variant_paths>();
                )*
            };
        }
    });

    // 11. Deref<Target = dyn Trait> (if implements present)
    let deref_impl = implements.map(|trait_path| emit_deref_impl(name, trait_path, &variant_paths));

    quote! {
        #struct_def
        #as_account_view
        #account_check
        #owner_impl
        #disc_assertions
        #space_impl
        #static_view
        #ref_enum
        #accessors
        #trait_assertion
        #deref_impl
    }
}

fn emit_pairwise_disc_assertions(variants: &[&syn::Path]) -> proc_macro2::TokenStream {
    let mut assertions = Vec::new();
    for i in 0..variants.len() {
        for j in (i + 1)..variants.len() {
            let a = &variants[i];
            let b = &variants[j];
            assertions.push(quote! {
                {
                    let a = <#a as quasar_lang::traits::Discriminator>::DISCRIMINATOR;
                    let b = <#b as quasar_lang::traits::Discriminator>::DISCRIMINATOR;
                    let min_len = if a.len() < b.len() { a.len() } else { b.len() };
                    let mut k = 0;
                    let mut prefix_match = true;
                    while k < min_len {
                        if a[k] != b[k] { prefix_match = false; }
                        k += 1;
                    }
                    assert!(
                        !prefix_match,
                        "one_of variant discriminators must not be prefixes of each other"
                    );
                }
            });
        }
    }
    quote! {
        const _: () = {
            #(#assertions)*
        };
    }
}

fn emit_max_space(name: &syn::Ident, variants: &[&syn::Path]) -> proc_macro2::TokenStream {
    let first = &variants[0];
    let mut max_expr = quote! { <#first as quasar_lang::traits::Space>::SPACE };
    for v in &variants[1..] {
        let prev = max_expr;
        max_expr = quote! {
            {
                let __a = #prev;
                let __b = <#v as quasar_lang::traits::Space>::SPACE;
                if __a > __b { __a } else { __b }
            }
        };
    }
    quote! {
        impl quasar_lang::traits::Space for #name {
            const SPACE: usize = #max_expr;
        }
    }
}

fn emit_one_of_accessors(
    name: &syn::Ident,
    variants: &[OneOfVariant],
    ref_enum_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let mut methods = Vec::new();

    // variant() method
    let mut variant_arms = Vec::new();
    for v in variants {
        let ident = &v.ident;
        let ty = &v.inner_ty;
        variant_arms.push(quote! {
            if __data.starts_with(<#ty as quasar_lang::traits::Discriminator>::DISCRIMINATOR) {
                return #ref_enum_name::#ident(unsafe {
                    &*(&self.__view as *const quasar_lang::__internal::AccountView
                        as *const quasar_lang::accounts::account::Account<#ty>)
                });
            }
        });
    }

    methods.push(quote! {
        #[inline(always)]
        pub fn variant(&self) -> #ref_enum_name<'_> {
            let __data = unsafe { self.__view.borrow_unchecked() };
            #(#variant_arms)*
            // AccountCheck already validated one matches — unreachable.
            unsafe { core::hint::unreachable_unchecked() }
        }
    });

    // Per-variant is_X() and X() accessors
    for v in variants {
        let snake = pascal_to_snake(&v.ident.to_string());
        let is_accessor = format_ident!("is_{}", snake);
        let accessor = format_ident!("{}", snake);
        let accessor_mut = format_ident!("{}_mut", snake);
        let ty = &v.inner_ty;

        methods.push(quote! {
            #[inline(always)]
            pub fn #is_accessor(&self) -> bool {
                let __data = unsafe { self.__view.borrow_unchecked() };
                __data.starts_with(<#ty as quasar_lang::traits::Discriminator>::DISCRIMINATOR)
            }

            #[inline(always)]
            pub fn #accessor(&self) -> Option<&quasar_lang::accounts::account::Account<#ty>> {
                let __data = unsafe { self.__view.borrow_unchecked() };
                if __data.starts_with(<#ty as quasar_lang::traits::Discriminator>::DISCRIMINATOR) {
                    Some(unsafe {
                        &*(&self.__view as *const quasar_lang::__internal::AccountView
                            as *const quasar_lang::accounts::account::Account<#ty>)
                    })
                } else {
                    None
                }
            }

            #[inline(always)]
            pub fn #accessor_mut(&mut self) -> Option<&mut quasar_lang::accounts::account::Account<#ty>> {
                let __data = unsafe { self.__view.borrow_unchecked() };
                if __data.starts_with(<#ty as quasar_lang::traits::Discriminator>::DISCRIMINATOR) {
                    Some(unsafe {
                        &mut *(&mut self.__view as *mut quasar_lang::__internal::AccountView
                            as *mut quasar_lang::accounts::account::Account<#ty>)
                    })
                } else {
                    None
                }
            }
        });
    }

    quote! {
        impl #name {
            #(#methods)*
        }
    }
}

/// Generate `Deref<Target = dyn Trait>` + `DerefMut` for a
/// `#[repr(transparent)]` wrapper over `AccountView`.
fn emit_deref_impl(
    name: &syn::Ident,
    trait_path: &syn::Path,
    variants: &[&syn::Path],
) -> proc_macro2::TokenStream {
    let last_idx = variants.len() - 1;
    let last_variant = variants[last_idx];

    // --- Deref (shared ref) ---
    let mut deref_body = quote! {
        unsafe {
            &*(&self.__view as *const quasar_lang::__internal::AccountView
                as *const #last_variant) as &(dyn #trait_path)
        }
    };
    for variant in variants[..last_idx].iter().rev() {
        deref_body = quote! {
            if __data.starts_with(<#variant as quasar_lang::traits::Discriminator>::DISCRIMINATOR) {
                unsafe {
                    &*(&self.__view as *const quasar_lang::__internal::AccountView
                        as *const #variant) as &(dyn #trait_path)
                }
            } else {
                #deref_body
            }
        };
    }

    // --- DerefMut (exclusive ref) ---
    let mut deref_mut_body = quote! {
        unsafe {
            &mut *(&mut self.__view as *mut quasar_lang::__internal::AccountView
                as *mut #last_variant) as &mut (dyn #trait_path)
        }
    };
    for variant in variants[..last_idx].iter().rev() {
        deref_mut_body = quote! {
            if __data.starts_with(<#variant as quasar_lang::traits::Discriminator>::DISCRIMINATOR) {
                unsafe {
                    &mut *(&mut self.__view as *mut quasar_lang::__internal::AccountView
                        as *mut #variant) as &mut (dyn #trait_path)
                }
            } else {
                #deref_mut_body
            }
        };
    }

    quote! {
        impl core::ops::Deref for #name {
            type Target = dyn #trait_path + 'static;

            #[inline(always)]
            fn deref(&self) -> &(dyn #trait_path + 'static) {
                let __data = unsafe { self.__view.borrow_unchecked() };
                #deref_body
            }
        }

        impl core::ops::DerefMut for #name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut (dyn #trait_path + 'static) {
                let __data = unsafe { self.__view.borrow_unchecked() };
                #deref_mut_body
            }
        }
    }
}
