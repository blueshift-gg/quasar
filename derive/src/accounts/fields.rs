use quote::{format_ident, quote};
use syn::{Expr, Ident, Type};

use super::attrs::{parse_field_attrs, AccountFieldAttrs};
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
    pub field_attrs: Vec<AccountFieldAttrs>,
    pub init_pda_checks: Vec<proc_macro2::TokenStream>,
    pub init_blocks: Vec<proc_macro2::TokenStream>,
    pub close_fields: Vec<(Ident, Ident)>,
    pub needs_rent: bool,
}

/// Extract the base name (last segment) of a type, stripping references and generics.
fn type_base_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(type_path) => type_path.path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(type_ref) => type_base_name(&type_ref.elem),
        _ => None,
    }
}

/// Find a field by type base name. Returns the field ident if found.
fn find_field_by_type<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    type_names: &[&str],
) -> Option<&'a Ident> {
    for field in fields.iter() {
        if let Some(base) = type_base_name(&field.ty) {
            if type_names.contains(&base.as_str()) {
                return field.ident.as_ref();
            }
        }
    }
    None
}

/// Find a field by name.
fn find_field_by_name<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    name: &str,
) -> Option<&'a Ident> {
    fields
        .iter()
        .find(|f| f.ident.as_ref().is_some_and(|i| i == name))
        .and_then(|f| f.ident.as_ref())
}

/// Find a field whose type is `Account<T>` (or `&Account<T>` / `&mut Account<T>`)
/// where `T` matches one of the given inner type names.
fn find_field_by_account_inner_type<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    inner_type_names: &[&str],
) -> Option<&'a Ident> {
    for field in fields.iter() {
        let ty = match &field.ty {
            Type::Reference(r) => &*r.elem,
            other => other,
        };
        if let Some(inner) = extract_generic_inner_type(ty, "Account") {
            if let Some(base) = type_base_name(inner) {
                if inner_type_names.contains(&base.as_str()) {
                    return field.ident.as_ref();
                }
            }
        }
    }
    None
}

/// Extract the inner type T from Account<T> or similar wrapper, handling references.
fn extract_account_inner_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    let deref_ty = match ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    extract_generic_inner_type(deref_ty, "Account").map(|inner| quote!(#inner))
}

// ---------------------------------------------------------------------------
// Auto-detected fields — populated once, consumed by init/CPI codegen
// ---------------------------------------------------------------------------

struct DetectedFields<'a> {
    // Programs
    system_program: Option<&'a Ident>,
    token_program: Option<&'a Ident>,
    associated_token_program: Option<&'a Ident>,
    metadata_program: Option<&'a Ident>,

    // Accounts (by inner type of Account<T>)
    metadata_account: Option<&'a Ident>,
    master_edition_account: Option<&'a Ident>,

    // Payer (explicit attr > field named "payer")
    payer: Option<&'a Ident>,
    realloc_payer: Option<&'a Ident>,

    // Authorities (by name, since these are just Signers)
    mint_authority: Option<&'a Ident>,
    update_authority: Option<&'a Ident>,

    // Rent sysvar (needed by metadata/master_edition CPI)
    rent: Option<&'a Ident>,
}

impl<'a> DetectedFields<'a> {
    fn detect(
        fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
        field_attrs: &[AccountFieldAttrs],
    ) -> Self {
        // --- Type-based detection (single pass) ---
        let system_program = find_field_by_type(fields, &["SystemProgram"]);
        let token_program = find_field_by_type(
            fields,
            &["TokenProgram", "Token2022Program", "TokenInterface"],
        );
        let associated_token_program = find_field_by_type(fields, &["AssociatedTokenProgram"]);
        let metadata_program = find_field_by_type(fields, &["MetadataProgram"]);

        // --- Inner-type detection for Account<T> wrappers ---
        let metadata_account = find_field_by_account_inner_type(fields, &["MetadataAccount"]);
        let master_edition_account =
            find_field_by_account_inner_type(fields, &["MasterEditionAccount"]);

        // --- Payer detection: explicit attr > field named "payer" ---
        let explicit_payer = field_attrs.iter().find_map(|a| a.payer.as_ref());
        let payer = explicit_payer
            .and_then(|name| find_field_by_name(fields, &name.to_string()))
            .or_else(|| find_field_by_name(fields, "payer"));

        // --- Realloc payer: explicit attr > init payer attr > field named "payer" ---
        let explicit_realloc_payer = field_attrs.iter().find_map(|a| a.realloc_payer.as_ref());
        let realloc_payer = explicit_realloc_payer
            .and_then(|name| find_field_by_name(fields, &name.to_string()))
            .or_else(|| {
                field_attrs
                    .iter()
                    .find_map(|a| a.payer.as_ref())
                    .and_then(|name| find_field_by_name(fields, &name.to_string()))
            })
            .or_else(|| find_field_by_name(fields, "payer"));

        // --- Authority detection by name ---
        let mint_authority = find_field_by_name(fields, "mint_authority")
            .or_else(|| find_field_by_name(fields, "authority"));
        let update_authority = find_field_by_name(fields, "update_authority").or(mint_authority);

        let rent = find_field_by_name(fields, "rent");

        DetectedFields {
            system_program,
            token_program,
            associated_token_program,
            metadata_program,
            metadata_account,
            master_edition_account,
            payer,
            realloc_payer,
            mint_authority,
            update_authority,
            rent,
        }
    }

    fn require(field: Option<&'a Ident>, msg: &str) -> Result<&'a Ident, proc_macro::TokenStream> {
        field.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), msg)
                .to_compile_error()
                .into()
        })
    }
}

pub(super) fn process_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_name_strings: &[String],
) -> Result<ProcessedFields, proc_macro::TokenStream> {
    let field_attrs: Vec<AccountFieldAttrs> = fields
        .iter()
        .map(parse_field_attrs)
        .collect::<syn::Result<Vec<_>>>()
        .map_err(|e| -> proc_macro::TokenStream { e.to_compile_error().into() })?;

    // --- Feature flags ---

    let has_any_init = field_attrs.iter().any(|a| a.is_init || a.init_if_needed);
    let has_any_token_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.token_mint.is_some());
    let has_any_ata_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.associated_token_mint.is_some());
    let has_any_mint_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.mint_decimals.is_some());
    let has_any_realloc = field_attrs.iter().any(|a| a.realloc.is_some());
    let has_any_metadata_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.metadata_name.is_some());
    let has_any_master_edition_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.master_edition_max_supply.is_some());

    // --- Auto-detect fields (single pass over all fields) ---

    let detected = DetectedFields::detect(fields, &field_attrs);

    // --- Validate required fields per feature ---

    let _system_program_field = if has_any_init {
        Some(DetectedFields::require(
            detected.system_program,
            "#[account(init)] requires a `SystemProgram` field in the accounts struct",
        )?)
    } else {
        None
    };

    let payer_field = if has_any_init {
        Some(DetectedFields::require(
            detected.payer,
            "#[account(init)] requires a `payer` field or explicit `payer = field` attribute",
        )?)
    } else {
        None
    };

    let realloc_payer_field = if has_any_realloc {
        Some(DetectedFields::require(
            detected.realloc_payer,
            "#[account(realloc)] requires a `payer` field, `realloc::payer = field`, or `payer = field` attribute",
        )?)
    } else {
        None
    };

    let token_program_field = if has_any_token_init
        || has_any_ata_init
        || has_any_mint_init
        || has_any_master_edition_init
    {
        Some(DetectedFields::require(
                detected.token_program,
                "token/ATA/mint/master_edition init requires a token program field (TokenProgram, Token2022Program, or TokenInterface)",
            )?)
    } else {
        None
    };

    let ata_program_field = if has_any_ata_init {
        Some(DetectedFields::require(
            detected.associated_token_program,
            "#[account(init, associated_token::...)] requires an `AssociatedTokenProgram` field",
        )?)
    } else {
        None
    };

    let metadata_account_field = if has_any_metadata_init {
        let field = detected
            .metadata_account
            .or_else(|| find_field_by_name(fields, "metadata"));
        Some(DetectedFields::require(
            field,
            "`metadata::*` requires a field of type `Account<MetadataAccount>` or a field named `metadata`",
        )?)
    } else {
        None
    };

    let master_edition_account_field = if has_any_master_edition_init {
        let field = detected
            .master_edition_account
            .or_else(|| find_field_by_name(fields, "master_edition"))
            .or_else(|| find_field_by_name(fields, "edition"));
        Some(DetectedFields::require(
            field,
            "`master_edition::*` requires a field of type `Account<MasterEditionAccount>` or a field named `master_edition`/`edition`",
        )?)
    } else {
        None
    };

    let metadata_program_field = if has_any_metadata_init || has_any_master_edition_init {
        Some(DetectedFields::require(
            detected.metadata_program,
            "`metadata::*` / `master_edition::*` requires a `MetadataProgram` field",
        )?)
    } else {
        None
    };

    let mint_authority_field = if has_any_metadata_init || has_any_master_edition_init {
        Some(DetectedFields::require(
            detected.mint_authority,
            "`metadata::*` / `master_edition::*` requires a `mint_authority` or `authority` field",
        )?)
    } else {
        None
    };

    let update_authority_field = if has_any_metadata_init || has_any_master_edition_init {
        Some(detected.update_authority.unwrap())
    } else {
        None
    };

    let rent_field = if has_any_metadata_init || has_any_master_edition_init {
        Some(DetectedFields::require(
            detected.rent,
            "`metadata::*` / `master_edition::*` requires a `rent` field (UncheckedAccount for the Rent sysvar)",
        )?)
    } else {
        None
    };

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
    let mut init_pda_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut init_blocks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut close_fields: Vec<(Ident, Ident)> = Vec::new();

    for (field, attrs) in fields.iter().zip(field_attrs.iter()) {
        let field_name = field.ident.as_ref().unwrap();

        let is_optional = extract_generic_inner_type(&field.ty, "Option").is_some();
        let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
        let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());

        // --- Per-field validation ---

        if (attrs.is_init || attrs.init_if_needed) && !is_ref_mut && !attrs.is_mut {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(init)] requires `&mut` reference or `#[account(mut)]`",
            )
            .to_compile_error()
            .into());
        }

        // Reject Initialize<T> when the derive macro handles init — the macro
        // creates the account, so the user should receive Account<T> (validated,
        // no .init() exposed). Allowing Initialize<T> here risks double-init.
        if attrs.is_init || attrs.init_if_needed {
            let deref_ty = match effective_ty {
                Type::Reference(r) => &*r.elem,
                other => other,
            };
            if extract_generic_inner_type(deref_ty, "Initialize").is_some() {
                return Err(syn::Error::new_spanned(
                    field_name,
                    "#[account(init)] handles account creation — use `Account<T>` instead of `Initialize<T>`. \
                     `Initialize<T>` exposes `.init()` which would double-initialize the account.",
                )
                .to_compile_error()
                .into());
            }
        }

        if attrs.close.is_some() && !is_ref_mut && !attrs.is_mut {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(close)] requires `&mut` reference or `#[account(mut)]`",
            )
            .to_compile_error()
            .into());
        }

        if (attrs.is_init || attrs.init_if_needed) && attrs.close.is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(init)] and #[account(close)] cannot be used on the same field",
            )
            .to_compile_error()
            .into());
        }

        if attrs.is_init && attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(init)] and #[account(init_if_needed)] are mutually exclusive",
            )
            .to_compile_error()
            .into());
        }

        if attrs.payer.is_some() && !attrs.is_init && !attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "`payer` requires `init` or `init_if_needed`",
            )
            .to_compile_error()
            .into());
        }

        if attrs.space.is_some() && !attrs.is_init && !attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "`space` requires `init` or `init_if_needed`",
            )
            .to_compile_error()
            .into());
        }

        if attrs.token_mint.is_some() && !attrs.is_init && !attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "`token::mint` requires `init` or `init_if_needed`",
            )
            .to_compile_error()
            .into());
        }

        if attrs.token_authority.is_some() && !attrs.is_init && !attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "`token::authority` requires `init` or `init_if_needed`",
            )
            .to_compile_error()
            .into());
        }

        // token::mint and token::authority must both be present if either is
        if attrs.token_mint.is_some() != attrs.token_authority.is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "`token::mint` and `token::authority` must both be specified",
            )
            .to_compile_error()
            .into());
        }

        // associated_token::mint and associated_token::authority must both be present
        if attrs.associated_token_mint.is_some() != attrs.associated_token_authority.is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "`associated_token::mint` and `associated_token::authority` must both be specified",
            )
            .to_compile_error()
            .into());
        }

        // token::* and associated_token::* are mutually exclusive
        if attrs.token_mint.is_some() && attrs.associated_token_mint.is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "`token::*` and `associated_token::*` cannot be used on the same field",
            )
            .to_compile_error()
            .into());
        }

        // associated_token::token_program requires associated_token::mint
        if attrs.associated_token_token_program.is_some() && attrs.associated_token_mint.is_none() {
            return Err(syn::Error::new_spanned(
                field_name,
                "`associated_token::token_program` requires `associated_token::mint` and `associated_token::authority`",
            )
            .to_compile_error()
            .into());
        }

        // seeds and associated_token::* are mutually exclusive (ATA address is deterministic)
        if attrs.seeds.is_some() && attrs.associated_token_mint.is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "`seeds` and `associated_token::*` cannot be used on the same field",
            )
            .to_compile_error()
            .into());
        }

        if attrs.realloc.is_some() && !is_ref_mut && !attrs.is_mut {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(realloc)] requires `&mut` reference or `#[account(mut)]`",
            )
            .to_compile_error()
            .into());
        }

        if attrs.realloc.is_some() && (attrs.is_init || attrs.init_if_needed) {
            return Err(syn::Error::new_spanned(
                field_name,
                "#[account(realloc)] and #[account(init)] cannot be used on the same field",
            )
            .to_compile_error()
            .into());
        }

        if attrs.realloc_payer.is_some() && attrs.realloc.is_none() {
            return Err(
                syn::Error::new_spanned(field_name, "`realloc::payer` requires `realloc`")
                    .to_compile_error()
                    .into(),
            );
        }

        // metadata::* requires init
        let has_any_metadata_attr = attrs.metadata_name.is_some()
            || attrs.metadata_symbol.is_some()
            || attrs.metadata_uri.is_some()
            || attrs.metadata_seller_fee_basis_points.is_some()
            || attrs.metadata_is_mutable.is_some();
        if has_any_metadata_attr && !attrs.is_init && !attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "`metadata::*` attributes require `init` or `init_if_needed`",
            )
            .to_compile_error()
            .into());
        }

        // metadata::name, symbol, uri must all be present if any is
        if has_any_metadata_attr
            && (attrs.metadata_name.is_none()
                || attrs.metadata_symbol.is_none()
                || attrs.metadata_uri.is_none())
        {
            return Err(syn::Error::new_spanned(
                field_name,
                "`metadata::name`, `metadata::symbol`, and `metadata::uri` must all be specified",
            )
            .to_compile_error()
            .into());
        }

        // master_edition::max_supply requires init
        if attrs.master_edition_max_supply.is_some() && !attrs.is_init && !attrs.init_if_needed {
            return Err(syn::Error::new_spanned(
                field_name,
                "`master_edition::max_supply` requires `init` or `init_if_needed`",
            )
            .to_compile_error()
            .into());
        }

        // master_edition requires metadata (metadata must be created first)
        if attrs.master_edition_max_supply.is_some() && attrs.metadata_name.is_none() {
            return Err(syn::Error::new_spanned(
                field_name,
                "`master_edition::max_supply` requires `metadata::name`, `metadata::symbol`, and `metadata::uri`",
            )
            .to_compile_error()
            .into());
        }

        // --- Field construction ---

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

        // --- Close field tracking ---

        if let Some(dest) = &attrs.close {
            close_fields.push((field_name.clone(), dest.clone()));
        }

        // --- PDA seeds + init code generation ---

        let is_init_field = attrs.is_init || attrs.init_if_needed;

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

            // Choose target: init_pda_checks for init fields, pda_checks for others
            let target_checks = if is_init_field {
                &mut init_pda_checks
            } else {
                &mut pda_checks
            };

            // Init fields are still raw &AccountView at PDA check time;
            // non-init fields are typed wrappers (rebound via let Self { ref ... } = result)
            let addr_access = if is_init_field {
                quote! { *#field_name.address() }
            } else {
                quote! { *#field_name.to_account_view().address() }
            };

            match &attrs.bump {
                Some(Some(bump_expr)) => {
                    target_checks.push(quote! {
                        {
                            #(#seed_len_checks)*
                            let __bump_val: u8 = #bump_expr;
                            let __bump_ref: &[u8] = &[__bump_val];
                            let __pda_seeds = [#(quasar_core::cpi::Seed::from(#seed_idents),)* quasar_core::cpi::Seed::from(__bump_ref)];
                            let __expected = quasar_core::pda::create_program_address(&__pda_seeds, &crate::ID)?;
                            if #addr_access != __expected {
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump_val;
                        }
                    });
                }
                Some(None) => {
                    target_checks.push(quote! {
                        {
                            #(#seed_len_checks)*
                            let __pda_seeds = [#(quasar_core::cpi::Seed::from(#seed_idents)),*];
                            let (__expected, __bump) = quasar_core::pda::find_program_address(&__pda_seeds, &crate::ID);
                            if #addr_access != __expected {
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

        // --- Init code generation ---

        if is_init_field {
            let is_token_init = attrs.token_mint.is_some();
            let is_ata_init = attrs.associated_token_mint.is_some();
            let has_pda = attrs.seeds.is_some();
            let pay_field = payer_field.unwrap();

            // Build the PDA signing code (if applicable)
            let invoke_expr = if has_pda {
                let bump_var = format_ident!("__bumps_{}", field_name);
                let seed_exprs = attrs.seeds.as_ref().unwrap();
                let seed_slices: Vec<proc_macro2::TokenStream> = seed_exprs
                    .iter()
                    .map(|expr| seed_slice_expr_for_parse(expr, field_name_strings))
                    .collect();
                quote! {
                    let __init_bump_ref: &[u8] = &[#bump_var];
                    let __init_signer_seeds = [#(quasar_core::cpi::Seed::from(#seed_slices),)* quasar_core::cpi::Seed::from(__init_bump_ref)];
                    __init_cpi.invoke_signed(&__init_signer_seeds)?;
                }
            } else {
                quote! { __init_cpi.invoke()?; }
            };

            if is_ata_init {
                let ata_prog = ata_program_field.unwrap();
                let mint_field = attrs.associated_token_mint.as_ref().unwrap();
                let auth_field = attrs.associated_token_authority.as_ref().unwrap();
                let sys_field = _system_program_field.unwrap();

                // Token program for CPI: explicit or auto-detected
                let tok_field = attrs
                    .associated_token_token_program
                    .as_ref()
                    .unwrap_or_else(|| token_program_field.unwrap());

                // Token program address for ATA derivation
                let token_program_addr = if let Some(tp) = &attrs.associated_token_token_program {
                    quote! { #tp.address() }
                } else {
                    quote! { &quasar_spl::SPL_TOKEN_ID }
                };

                if attrs.init_if_needed {
                    init_blocks.push(quote! {
                        {
                            if quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                quasar_core::cpi::CpiCall::new(
                                    #ata_prog.address(),
                                    [
                                        quasar_core::cpi::InstructionAccount::writable_signer(#pay_field.address()),
                                        quasar_core::cpi::InstructionAccount::writable(#field_name.address()),
                                        quasar_core::cpi::InstructionAccount::readonly(#auth_field.address()),
                                        quasar_core::cpi::InstructionAccount::readonly(#mint_field.address()),
                                        quasar_core::cpi::InstructionAccount::readonly(#sys_field.address()),
                                        quasar_core::cpi::InstructionAccount::readonly(#tok_field.address()),
                                    ],
                                    [#pay_field, #field_name, #auth_field, #mint_field, #sys_field, #tok_field],
                                    [1u8],
                                ).invoke()?;
                            } else {
                                quasar_spl::validate_ata(
                                    #field_name, #auth_field.address(), #mint_field.address(), #token_program_addr,
                                )?;
                            }
                        }
                    });
                } else {
                    init_blocks.push(quote! {
                        {
                            if !quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                return Err(ProgramError::AccountAlreadyInitialized);
                            }
                            quasar_core::cpi::CpiCall::new(
                                #ata_prog.address(),
                                [
                                    quasar_core::cpi::InstructionAccount::writable_signer(#pay_field.address()),
                                    quasar_core::cpi::InstructionAccount::writable(#field_name.address()),
                                    quasar_core::cpi::InstructionAccount::readonly(#auth_field.address()),
                                    quasar_core::cpi::InstructionAccount::readonly(#mint_field.address()),
                                    quasar_core::cpi::InstructionAccount::readonly(#sys_field.address()),
                                    quasar_core::cpi::InstructionAccount::readonly(#tok_field.address()),
                                ],
                                [#pay_field, #field_name, #auth_field, #mint_field, #sys_field, #tok_field],
                                [0u8],
                            ).invoke()?;
                        }
                    });
                }
            } else if is_token_init {
                let tok_field = token_program_field.unwrap();
                let mint_field = attrs.token_mint.as_ref().unwrap();
                let auth_field = attrs.token_authority.as_ref().unwrap();

                if attrs.init_if_needed {
                    init_blocks.push(quote! {
                        {
                            if quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                let __init_lamports = __shared_rent.try_minimum_balance(
                                    quasar_spl::TokenAccountState::LEN
                                )?;
                                let __init_cpi = quasar_core::cpi::system::create_account(
                                    #pay_field, #field_name, __init_lamports,
                                    quasar_spl::TokenAccountState::LEN as u64,
                                    #tok_field.address(),
                                );
                                #invoke_expr
                                quasar_spl::initialize_account3(
                                    #tok_field, #field_name, #mint_field, #auth_field.address(),
                                ).invoke()?;
                            } else {
                                quasar_spl::validate_token_account(
                                    #field_name, #mint_field.address(), #auth_field.address(),
                                )?;
                            }
                        }
                    });
                } else {
                    init_blocks.push(quote! {
                        {
                            if !quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                return Err(ProgramError::AccountAlreadyInitialized);
                            }
                            let __init_lamports = __shared_rent.try_minimum_balance(
                                quasar_spl::TokenAccountState::LEN
                            )?;
                            let __init_cpi = quasar_core::cpi::system::create_account(
                                #pay_field, #field_name, __init_lamports,
                                quasar_spl::TokenAccountState::LEN as u64,
                                #tok_field.address(),
                            );
                            #invoke_expr
                            quasar_spl::initialize_account3(
                                #tok_field, #field_name, #mint_field, #auth_field.address(),
                            ).invoke()?;
                        }
                    });
                }
            } else if let Some(decimals_expr) = attrs.mint_decimals.as_ref() {
                // Mint init: create_account (token program owner) + initialize_mint2
                let tok_field = token_program_field.unwrap();
                let auth_field = match attrs.mint_init_authority.as_ref() {
                    Some(f) => f,
                    None => {
                        return Err(syn::Error::new_spanned(
                            field_name,
                            "`mint::decimals` requires `mint::authority = <field>`",
                        )
                        .to_compile_error()
                        .into());
                    }
                };
                let freeze_expr = if let Some(freeze_field) = &attrs.mint_freeze_authority {
                    quote! { Some(#freeze_field.address()) }
                } else {
                    quote! { None }
                };

                if attrs.init_if_needed {
                    init_blocks.push(quote! {
                        {
                            if quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                let __init_lamports = __shared_rent.try_minimum_balance(
                                    quasar_spl::MintAccountState::LEN
                                )?;
                                let __init_cpi = quasar_core::cpi::system::create_account(
                                    #pay_field, #field_name, __init_lamports,
                                    quasar_spl::MintAccountState::LEN as u64,
                                    #tok_field.address(),
                                );
                                #invoke_expr
                                quasar_spl::initialize_mint2(
                                    #tok_field, #field_name,
                                    (#decimals_expr) as u8,
                                    #auth_field.address(),
                                    #freeze_expr,
                                ).invoke()?;
                            } else {
                                quasar_spl::validate_mint(
                                    #field_name, #auth_field.address(),
                                )?;
                            }
                        }
                    });
                } else {
                    init_blocks.push(quote! {
                        {
                            if !quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                return Err(ProgramError::AccountAlreadyInitialized);
                            }
                            let __init_lamports = __shared_rent.try_minimum_balance(
                                quasar_spl::MintAccountState::LEN
                            )?;
                            let __init_cpi = quasar_core::cpi::system::create_account(
                                #pay_field, #field_name, __init_lamports,
                                quasar_spl::MintAccountState::LEN as u64,
                                #tok_field.address(),
                            );
                            #invoke_expr
                            quasar_spl::initialize_mint2(
                                #tok_field, #field_name,
                                (#decimals_expr) as u8,
                                #auth_field.address(),
                                #freeze_expr,
                            ).invoke()?;
                        }
                    });
                }
            } else {
                // Program account init — extract inner type for Space + Discriminator
                let inner_type = extract_account_inner_type(effective_ty);
                if inner_type.is_none() {
                    return Err(syn::Error::new_spanned(
                        field_name,
                        "#[account(init)] on non-Account<T> type requires `token::mint` and `token::authority`, `associated_token::mint` and `associated_token::authority`, or `mint::decimals` and `mint::authority`",
                    )
                    .to_compile_error()
                    .into());
                }
                let inner_type = inner_type.unwrap();

                let space_expr = if let Some(space) = &attrs.space {
                    quote! { (#space) as u64 }
                } else {
                    quote! {
                        <#inner_type as quasar_core::traits::Space>::SPACE as u64
                    }
                };

                if attrs.init_if_needed {
                    init_blocks.push(quote! {
                        {
                            if quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                let __init_space = #space_expr;
                                let __init_lamports = __shared_rent.try_minimum_balance(__init_space as usize)?;
                                let __init_cpi = quasar_core::cpi::system::create_account(
                                    #pay_field, #field_name, __init_lamports, __init_space, &crate::ID,
                                );
                                #invoke_expr
                                let __disc = <#inner_type as quasar_core::traits::Discriminator>::DISCRIMINATOR;
                                unsafe {
                                    core::ptr::copy_nonoverlapping(
                                        __disc.as_ptr(),
                                        #field_name.data_ptr() as *mut u8,
                                        __disc.len(),
                                    );
                                }
                            }
                        }
                    });
                } else {
                    init_blocks.push(quote! {
                        {
                            if !quasar_core::is_system_program(unsafe { #field_name.owner() }) {
                                return Err(ProgramError::AccountAlreadyInitialized);
                            }
                            let __init_space = #space_expr;
                            let __init_lamports = __shared_rent.try_minimum_balance(__init_space as usize)?;
                            let __init_cpi = quasar_core::cpi::system::create_account(
                                #pay_field, #field_name, __init_lamports, __init_space, &crate::ID,
                            );
                            #invoke_expr
                            let __disc = <#inner_type as quasar_core::traits::Discriminator>::DISCRIMINATOR;
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    __disc.as_ptr(),
                                    #field_name.data_ptr() as *mut u8,
                                    __disc.len(),
                                );
                            }
                        }
                    });
                }
            }
        }

        // --- Non-init ATA address validation ---

        if let (false, Some(mint_field), Some(auth_field)) = (
            is_init_field,
            attrs.associated_token_mint.as_ref(),
            attrs.associated_token_authority.as_ref(),
        ) {
            let token_program_addr = if let Some(tp) = &attrs.associated_token_token_program {
                quote! { #tp.to_account_view().address() }
            } else {
                quote! { &quasar_spl::SPL_TOKEN_ID }
            };

            pda_checks.push(quote! {
                quasar_spl::validate_ata(
                    #field_name.to_account_view(),
                    #auth_field.to_account_view().address(),
                    #mint_field.to_account_view().address(),
                    #token_program_addr,
                )?;
            });
        }

        // --- Realloc code generation ---

        if let Some(realloc_expr) = &attrs.realloc {
            let realloc_pay = realloc_payer_field.unwrap();

            init_blocks.push(quote! {
                {
                    let __realloc_space = (#realloc_expr) as usize;
                    quasar_core::accounts::realloc_account(
                        #field_name, __realloc_space, #realloc_pay, Some(&__shared_rent)
                    )?;
                }
            });
        }

        // --- Metadata init CPI generation ---

        if is_init_field {
            if let Some(meta_name) = attrs.metadata_name.as_ref() {
                let meta_symbol = attrs.metadata_symbol.as_ref().unwrap();
                let meta_uri = attrs.metadata_uri.as_ref().unwrap();
                let seller_fee = attrs
                    .metadata_seller_fee_basis_points
                    .as_ref()
                    .map(|e| quote! { (#e) as u16 })
                    .unwrap_or(quote! { 0u16 });
                let is_mutable = attrs
                    .metadata_is_mutable
                    .as_ref()
                    .map(|e| quote! { #e })
                    .unwrap_or(quote! { false });

                let meta_field = metadata_account_field.unwrap();
                let meta_prog = metadata_program_field.unwrap();
                let mint_auth = mint_authority_field.unwrap();
                let update_auth = update_authority_field.unwrap();
                let pay = payer_field.unwrap();
                let sys = _system_program_field.unwrap();
                let rent = rent_field.unwrap();

                init_blocks.push(quote! {
                    {
                        quasar_spl::metadata::MetadataCpi::create_metadata_accounts_v3(
                            #meta_prog, #meta_field, #field_name, #mint_auth,
                            #pay, #update_auth, #sys, #rent,
                            quasar_core::borsh::BorshString::new(#meta_name),
                            quasar_core::borsh::BorshString::new(#meta_symbol),
                            quasar_core::borsh::BorshString::new(#meta_uri),
                            #seller_fee, #is_mutable, true,
                        ).invoke()?;
                    }
                });
            }

            // --- Master Edition init CPI generation ---

            if let Some(max_supply) = attrs.master_edition_max_supply.as_ref() {
                let me_field = master_edition_account_field.unwrap();
                let meta_field = metadata_account_field.unwrap();
                let meta_prog = metadata_program_field.unwrap();
                let mint_auth = mint_authority_field.unwrap();
                let update_auth = update_authority_field.unwrap();
                let pay = payer_field.unwrap();
                let tok = token_program_field.unwrap();
                let sys = _system_program_field.unwrap();
                let rent = rent_field.unwrap();

                init_blocks.push(quote! {
                    {
                        quasar_spl::metadata::MetadataCpi::create_master_edition_v3(
                            #meta_prog, #me_field, #field_name, #update_auth,
                            #mint_auth, #pay, #meta_field, #tok, #sys, #rent,
                            Some(#max_supply as u64),
                        ).invoke()?;
                    }
                });
            }
        }
    }

    let needs_rent = !init_blocks.is_empty();

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
        init_pda_checks,
        init_blocks,
        close_fields,
        needs_rent,
    })
}
