use quote::{format_ident, quote};
use syn::{Expr, Ident, Type};

use super::attrs::parse_field_attrs;
use crate::helpers::{extract_generic_inner_type, seed_slice_expr_for_parse, strip_generics};

pub(super) struct ProcessedFields {
    pub field_constructs: Vec<proc_macro2::TokenStream>,
    pub has_one_checks: Vec<proc_macro2::TokenStream>,
    pub constraint_checks: Vec<proc_macro2::TokenStream>,
    pub mut_checks: Vec<proc_macro2::TokenStream>,
    pub pda_checks: Vec<proc_macro2::TokenStream>,
    pub bump_init_vars: Vec<proc_macro2::TokenStream>,
    pub bump_struct_fields: Vec<proc_macro2::TokenStream>,
    pub bump_struct_inits: Vec<proc_macro2::TokenStream>,
    pub seeds_methods: Vec<proc_macro2::TokenStream>,
    pub seed_addr_captures: Vec<proc_macro2::TokenStream>,
    pub field_attrs: Vec<super::attrs::AccountFieldAttrs>,
}

pub(super) fn process_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_name_strings: &[String],
) -> Result<ProcessedFields, proc_macro::TokenStream> {
    let field_attrs: Vec<super::attrs::AccountFieldAttrs> = fields
        .iter()
        .map(parse_field_attrs)
        .collect::<syn::Result<Vec<_>>>()
        .map_err(|e| -> proc_macro::TokenStream { e.to_compile_error().into() })?;

    let mut field_constructs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut has_one_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut constraint_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut mut_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut pda_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_init_vars: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_inits: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seeds_methods: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seed_addr_captures: Vec<proc_macro2::TokenStream> = Vec::new();

    for (field, attrs) in fields.iter().zip(field_attrs.iter()) {
        let field_name = field.ident.as_ref().unwrap();

        let is_optional = extract_generic_inner_type(&field.ty, "Option").is_some();
        let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
        let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());

        match effective_ty {
            Type::Reference(type_ref) => {
                let base_type = strip_generics(&type_ref.elem);
                let construct_expr = if type_ref.mutability.is_some() {
                    quote! { #base_type::from_account_view_mut(#field_name)? }
                } else {
                    quote! { #base_type::from_account_view(#field_name)? }
                };
                if is_optional {
                    field_constructs.push(quote! { #field_name: if *#field_name.address() == crate::ID { None } else { Some(#construct_expr) } });
                } else {
                    field_constructs.push(quote! { #field_name: #construct_expr });
                }
            }
            _ => {
                let base_type = strip_generics(effective_ty);
                if is_optional {
                    field_constructs.push(quote! { #field_name: if *#field_name.address() == crate::ID { None } else { Some(#base_type::from_account_view(#field_name)?) } });
                } else {
                    field_constructs
                        .push(quote! { #field_name: #base_type::from_account_view(#field_name)? });
                }
            }
        }

        if attrs.is_mut && !is_ref_mut {
            let check = quote! {
                if !#field_name.to_account_view().is_writable() {
                    return Err(ProgramError::Immutable);
                }
            };
            if is_optional {
                mut_checks.push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                mut_checks.push(check);
            }
        }

        for (target, custom_error) in &attrs.has_ones {
            let error = match custom_error {
                Some(err) => quote! { #err.into() },
                None => quote! { QuasarError::HasOneMismatch.into() },
            };
            let check = quote! {
                if #field_name.#target != *#target.to_account_view().address() {
                    return Err(#error);
                }
            };
            if is_optional {
                has_one_checks
                    .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                has_one_checks.push(check);
            }
        }

        for (expr, custom_error) in &attrs.constraints {
            let error = match custom_error {
                Some(err) => quote! { #err.into() },
                None => quote! { QuasarError::ConstraintViolation.into() },
            };
            let check = quote! {
                if !(#expr) {
                    return Err(#error);
                }
            };
            if is_optional {
                constraint_checks
                    .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                constraint_checks.push(check);
            }
        }

        if let Some((addr_expr, custom_error)) = &attrs.address {
            let error = match custom_error {
                Some(err) => quote! { #err.into() },
                None => quote! { QuasarError::AddressMismatch.into() },
            };
            let check = quote! {
                if *#field_name.to_account_view().address() != #addr_expr {
                    return Err(#error);
                }
            };
            if is_optional {
                constraint_checks
                    .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                constraint_checks.push(check);
            }
        }

        if let Some(seed_exprs) = &attrs.seeds {
            let bump_var = format_ident!("__bumps_{}", field_name);

            bump_init_vars.push(quote! { let mut #bump_var: u8 = 0; });
            bump_struct_fields.push(quote! { pub #field_name: u8 });
            bump_struct_inits.push(quote! { #field_name: #bump_var });

            let bump_arr_field = format_ident!("__{}_bump", field_name);
            bump_struct_fields.push(quote! { #bump_arr_field: [u8; 1] });
            bump_struct_inits.push(quote! { #bump_arr_field: [#bump_var] });

            let seed_slices: Vec<proc_macro2::TokenStream> = seed_exprs
                .iter()
                .map(|expr| seed_slice_expr_for_parse(expr, field_name_strings))
                .collect();

            let seed_idents: Vec<Ident> = seed_slices
                .iter()
                .enumerate()
                .map(|(idx, _)| format_ident!("__seed_{}_{}", field_name, idx))
                .collect();

            let seed_len_checks: Vec<proc_macro2::TokenStream> = seed_idents
                .iter()
                .zip(seed_slices.iter())
                .map(|(ident, seed)| {
                    quote! {
                        let #ident: &[u8] = #seed;
                        if #ident.len() > 32 {
                            return Err(QuasarError::InvalidSeeds.into());
                        }
                    }
                })
                .collect();

            match &attrs.bump {
                Some(Some(bump_expr)) => {
                    pda_checks.push(quote! {
                        {
                            #(#seed_len_checks)*
                            let __bump_val: u8 = #bump_expr;
                            let __bump_ref: &[u8] = &[__bump_val];
                            let __pda_seeds = [#(quasar_core::cpi::Seed::from(#seed_idents),)* quasar_core::cpi::Seed::from(__bump_ref)];
                            let __expected = quasar_core::pda::create_program_address(&__pda_seeds, &crate::ID)?;
                            if *#field_name.to_account_view().address() != __expected {
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump_val;
                        }
                    });
                }
                Some(None) => {
                    pda_checks.push(quote! {
                        {
                            #(#seed_len_checks)*
                            let __pda_seeds = [#(quasar_core::cpi::Seed::from(#seed_idents)),*];
                            let (__expected, __bump) = quasar_core::pda::find_program_address(&__pda_seeds, &crate::ID);
                            if *#field_name.to_account_view().address() != __expected {
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump;
                        }
                    });
                }
                None => {
                    return Err(syn::Error::new_spanned(
                        field_name,
                        "#[account(seeds = [...])] requires a `bump` or `bump = expr` directive",
                    )
                    .to_compile_error()
                    .into());
                }
            }

            let method_name = format_ident!("{}_seeds", field_name);
            let seed_count = seed_exprs.len() + 1;
            let mut seed_elements: Vec<proc_macro2::TokenStream> = Vec::new();

            for expr in seed_exprs {
                if let Expr::Path(ep) = expr {
                    if ep.qself.is_none() && ep.path.segments.len() == 1 {
                        let ident = &ep.path.segments[0].ident;
                        if field_name_strings.contains(&ident.to_string()) {
                            let addr_field = format_ident!("__seed_{}_{}", field_name, ident);
                            let capture_var = format_ident!("__seed_addr_{}_{}", field_name, ident);

                            seed_addr_captures.push(quote! {
                                let #capture_var = *#ident.address();
                            });
                            bump_struct_fields.push(quote! { #addr_field: Address });
                            bump_struct_inits.push(quote! { #addr_field: #capture_var });

                            seed_elements.push(
                                quote! { quasar_core::cpi::Seed::from(self.#addr_field.as_ref()) },
                            );
                            continue;
                        }
                    }
                }
                seed_elements.push(quote! { quasar_core::cpi::Seed::from((#expr) as &[u8]) });
            }

            seed_elements
                .push(quote! { quasar_core::cpi::Seed::from(&self.#bump_arr_field as &[u8]) });

            seeds_methods.push(quote! {
                #[inline(always)]
                pub fn #method_name(&self) -> [quasar_core::cpi::Seed<'_>; #seed_count] {
                    [#(#seed_elements),*]
                }
            });
        }
    }

    Ok(ProcessedFields {
        field_constructs,
        has_one_checks,
        constraint_checks,
        mut_checks,
        pda_checks,
        bump_init_vars,
        bump_struct_fields,
        bump_struct_inits,
        seeds_methods,
        seed_addr_captures,
        field_attrs,
    })
}
