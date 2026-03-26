//! Per-field codegen for `#[derive(Accounts)]`.
//!
//! Each field in the accounts struct produces: parsing code (account extraction
//! from the accounts slice), validation code (constraint checks), and PDA seed
//! reconstruction. This module is the largest in the derive crate because each
//! field attribute combination produces distinct codegen paths.

use {
    super::attrs::{parse_field_attrs, AccountFieldAttrs},
    crate::helpers::{extract_generic_inner_type, seed_slice_expr_for_parse, strip_generics},
    quote::{format_ident, quote},
    syn::{Expr, ExprLit, Ident, Lit, Type},
};

/// Info for a single `#[account(close = dest)]` field.
pub(super) struct CloseFieldInfo {
    pub field: Ident,
    pub destination: Ident,
    /// For token/mint types, CPI close requires the token program and
    /// authority.
    pub cpi_close: Option<CpiCloseInfo>,
}

pub(super) struct CpiCloseInfo {
    pub token_program: Ident,
    pub authority: Ident,
}

/// Info for a single `#[account(sweep = receiver)]` field.
pub(super) struct SweepFieldInfo {
    pub field: Ident,
    pub receiver: Ident,
    pub mint: Ident,
    pub authority: Ident,
    pub token_program: Ident,
}

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
    pub close_fields: Vec<CloseFieldInfo>,
    pub sweep_fields: Vec<SweepFieldInfo>,
    pub needs_rent: bool,
}

/// Extract the base name (last segment) of a type, stripping references and
/// generics.
fn type_base_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(type_path) => type_path.path.segments.last().map(|s| s.ident.to_string()),
        Type::Reference(type_ref) => type_base_name(&type_ref.elem),
        _ => None,
    }
}

/// Find a field by type base name or Program<T>/Interface<T> wrapper. Returns
/// the field ident if found.
fn find_field_by_type<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    type_names: &[&str],
) -> Option<&'a Ident> {
    for field in fields.iter() {
        // Strip reference if present
        let ty = match &field.ty {
            syn::Type::Reference(type_ref) => &*type_ref.elem,
            other => other,
        };

        // Check direct type name match
        if let Some(base) = type_base_name(&field.ty) {
            if type_names.contains(&base.as_str()) {
                return field.ident.as_ref();
            }
        }

        // Check Program<T> wrapper
        if let Some(inner_ty) = extract_generic_inner_type(ty, "Program") {
            if let Some(inner_base) = type_base_name(inner_ty) {
                if type_names.contains(&inner_base.as_str()) {
                    return field.ident.as_ref();
                }
            }
        }

        // Check Interface<T> wrapper
        if let Some(inner_ty) = extract_generic_inner_type(ty, "Interface") {
            if let Some(inner_base) = type_base_name(inner_ty) {
                if type_names.contains(&inner_base.as_str()) {
                    return field.ident.as_ref();
                }
            }
        }
    }
    None
}

/// Check if a field's type is `InterfaceAccount<T>` (or
/// `&InterfaceAccount<T>`).
fn is_interface_account_field(field: &syn::Field) -> bool {
    let ty = match &field.ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    extract_generic_inner_type(ty, "InterfaceAccount").is_some()
}

/// Check if a field's type wraps a token or mint inner type
/// (`Account<Token>`, `Account<Token2022>`, `Account<Mint>`,
/// `Account<Mint2022>`, `InterfaceAccount<Token>`, `InterfaceAccount<Mint>`).
fn is_token_or_mint_field(field: &syn::Field) -> bool {
    const TOKEN_MINT_TYPES: &[&str] = &["Token", "Token2022", "Mint", "Mint2022"];
    let ty = match &field.ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    for wrapper in &["Account", "InterfaceAccount"] {
        if let Some(inner) = extract_generic_inner_type(ty, wrapper) {
            if let Some(base) = type_base_name(inner) {
                if TOKEN_MINT_TYPES.contains(&base.as_str()) {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a field's type wraps a token account inner type (not mint).
/// (`Account<Token>`, `Account<Token2022>`, `InterfaceAccount<Token>`).
fn is_token_account_field(field: &syn::Field) -> bool {
    const TOKEN_TYPES: &[&str] = &["Token", "Token2022"];
    let ty = match &field.ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    for wrapper in &["Account", "InterfaceAccount"] {
        if let Some(inner) = extract_generic_inner_type(ty, wrapper) {
            if let Some(base) = type_base_name(inner) {
                if TOKEN_TYPES.contains(&base.as_str()) {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if a field's type is `Signer` (or `&Signer` / `&mut Signer`).
fn is_signer_field(field: &syn::Field) -> bool {
    let ty = match &field.ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    type_base_name(ty).as_deref() == Some("Signer")
}

/// Resolve the token program address expression for a non-init field based on
/// its wrapper type.
///
/// - `Account<Token>` → compile-time `&quasar_spl::SPL_TOKEN_ID`
/// - `Account<Token2022>` → compile-time `&quasar_spl::TOKEN_2022_ID`
/// - `InterfaceAccount<Token>` → runtime `token_program_field.address()`
fn resolve_token_program_addr(
    effective_ty: &Type,
    token_program_field: Option<&Ident>,
) -> proc_macro2::TokenStream {
    let underlying = match effective_ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    if let Some(inner) = extract_generic_inner_type(underlying, "Account") {
        if let Some(name) = type_base_name(inner) {
            match name.as_str() {
                "Token" | "Mint" => return quote! { &quasar_spl::SPL_TOKEN_ID },
                "Token2022" | "Mint2022" => return quote! { &quasar_spl::TOKEN_2022_ID },
                _ => {}
            }
        }
    }
    // InterfaceAccount or other — use runtime token_program_field
    let tp = token_program_field
        .expect("InterfaceAccount with token/ata attrs requires a token program field");
    quote! { #tp.to_account_view().address() }
}

/// Count how many fields match the given type names (checking Program<T> and
/// Interface<T> wrappers). Used to detect ambiguous token program fields.
fn count_fields_by_type(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    type_names: &[&str],
) -> usize {
    let mut count = 0;
    for field in fields.iter() {
        let ty = match &field.ty {
            syn::Type::Reference(type_ref) => &*type_ref.elem,
            other => other,
        };
        let matched = if let Some(inner_ty) = extract_generic_inner_type(ty, "Program") {
            type_base_name(inner_ty).is_some_and(|b| type_names.contains(&b.as_str()))
        } else if let Some(inner_ty) = extract_generic_inner_type(ty, "Interface") {
            type_base_name(inner_ty).is_some_and(|b| type_names.contains(&b.as_str()))
        } else {
            false
        };
        if matched {
            count += 1;
        }
    }
    count
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

/// Find a field whose type is `Account<T>` (or `&Account<T>` / `&mut
/// Account<T>`) where `T` matches one of the given inner type names.
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

/// Extract the inner type T from Account<T> or similar wrapper, handling
/// references.
fn extract_account_inner_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    let deref_ty = match ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    extract_generic_inner_type(deref_ty, "Account").map(|inner| quote!(#inner))
}

/// Check if the inner type T of Account<T> has a lifetime parameter,
/// indicating a dynamic account type (e.g., Account<Profile<'info>>).
fn is_dynamic_account_type(ty: &Type) -> bool {
    let deref_ty = match ty {
        Type::Reference(r) => &*r.elem,
        other => other,
    };
    if let Some(Type::Path(type_path)) = extract_generic_inner_type(deref_ty, "Account") {
        if let Some(last) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                return args
                    .args
                    .iter()
                    .any(|arg| matches!(arg, syn::GenericArgument::Lifetime(_)));
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Auto-detected fields — populated once, consumed by init/CPI codegen
// ---------------------------------------------------------------------------

struct DetectedFields<'a> {
    // Programs
    system_program: Option<&'a Ident>,
    token_program: Option<&'a Ident>,
    token_program_count: usize,
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
        let system_program = find_field_by_type(fields, &["System"]);
        let token_program = find_field_by_type(fields, &["Token", "Token2022", "TokenInterface"]);
        let token_program_count =
            count_fields_by_type(fields, &["Token", "Token2022", "TokenInterface"]);
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
            token_program_count,
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

/// Wrap a CPI body with the init/init_if_needed guard pattern.
///
/// - `init`: reject if already initialized, then run CPI body.
/// - `init_if_needed`: if uninitialized run CPI body, else run validation (if
///   provided).
fn wrap_init_block(
    field_name: &Ident,
    init_if_needed: bool,
    cpi_body: proc_macro2::TokenStream,
    validate_existing: Option<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    if init_if_needed {
        let validate = validate_existing.unwrap_or_default();
        quote! {
            {
                if quasar_lang::is_system_program(#field_name.owner()) {
                    #cpi_body
                } else {
                    #validate
                }
            }
        }
    } else {
        quote! {
            {
                if !quasar_lang::is_system_program(#field_name.owner()) {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }
                #cpi_body
            }
        }
    }
}

/// Validate attribute combinations on a single field. Returns a compile error
/// on conflict.
fn validate_field_attrs(
    field: &syn::Field,
    field_name: &Ident,
    attrs: &AccountFieldAttrs,
    is_ref_mut: bool,
) -> Result<(), proc_macro::TokenStream> {
    // Helper: emit a compile error attached to the field name.
    macro_rules! reject {
        ($cond:expr, $msg:expr) => {
            if $cond {
                return Err(syn::Error::new_spanned(field_name, $msg)
                    .to_compile_error()
                    .into());
            }
        };
    }

    let is_init = attrs.is_init || attrs.init_if_needed;
    let is_writable = is_ref_mut || attrs.is_mut;

    // Mutability requirements
    reject!(
        is_init && !is_writable,
        "#[account(init)] requires `&mut` reference or `#[account(mut)]`"
    );
    reject!(
        attrs.close.is_some() && !is_writable,
        "#[account(close)] requires `&mut` reference or `#[account(mut)]`"
    );
    reject!(
        attrs.realloc.is_some() && !is_writable,
        "#[account(realloc)] requires `&mut` reference or `#[account(mut)]`"
    );

    // Mutual exclusions
    reject!(
        is_init && attrs.close.is_some(),
        "#[account(init)] and #[account(close)] cannot be used on the same field"
    );
    reject!(
        attrs.is_init && attrs.init_if_needed,
        "#[account(init)] and #[account(init_if_needed)] are mutually exclusive"
    );
    reject!(
        attrs.realloc.is_some() && is_init,
        "#[account(realloc)] and #[account(init)] cannot be used on the same field"
    );
    reject!(
        attrs.realloc.is_some() && is_token_or_mint_field(field),
        "#[account(realloc)] cannot be used on token or mint accounts — their size is fixed by \
         the token program"
    );

    // Sweep validations
    reject!(
        attrs.sweep.is_some() && !is_writable,
        "#[account(sweep)] requires `&mut` reference or `#[account(mut)]`"
    );
    reject!(
        attrs.sweep.is_some() && !is_token_account_field(field),
        "#[account(sweep)] is only valid on token accounts, not mint accounts"
    );
    reject!(
        attrs.sweep.is_some() && attrs.token_mint.is_none(),
        "#[account(sweep)] requires `token::mint` (needed for transfer_checked decimals)"
    );
    reject!(
        attrs.sweep.is_some() && attrs.token_authority.is_none(),
        "#[account(sweep)] requires `token::authority` (needed for transfer signer)"
    );

    reject!(
        attrs.token_mint.is_some() && attrs.associated_token_mint.is_some(),
        "`token::*` and `associated_token::*` cannot be used on the same field"
    );
    reject!(
        attrs.seeds.is_some() && attrs.associated_token_mint.is_some(),
        "`seeds` and `associated_token::*` cannot be used on the same field"
    );

    // Attributes that require init
    reject!(
        attrs.payer.is_some() && !is_init,
        "`payer` requires `init` or `init_if_needed`"
    );
    reject!(
        attrs.space.is_some() && !is_init,
        "`space` requires `init` or `init_if_needed`"
    );

    // Paired attributes
    reject!(
        attrs.token_mint.is_some() != attrs.token_authority.is_some(),
        "`token::mint` and `token::authority` must both be specified"
    );
    reject!(
        attrs.associated_token_mint.is_some() != attrs.associated_token_authority.is_some(),
        "`associated_token::mint` and `associated_token::authority` must both be specified"
    );
    reject!(
        attrs.associated_token_token_program.is_some() && attrs.associated_token_mint.is_none(),
        "`associated_token::token_program` requires `associated_token::mint` and \
         `associated_token::authority`"
    );
    reject!(
        attrs.realloc_payer.is_some() && attrs.realloc.is_none(),
        "`realloc::payer` requires `realloc`"
    );

    // Metadata attributes
    let has_metadata = attrs.metadata_name.is_some()
        || attrs.metadata_symbol.is_some()
        || attrs.metadata_uri.is_some()
        || attrs.metadata_seller_fee_basis_points.is_some()
        || attrs.metadata_is_mutable.is_some();
    reject!(
        has_metadata && !is_init,
        "`metadata::*` attributes require `init` or `init_if_needed`"
    );
    reject!(
        has_metadata
            && (attrs.metadata_name.is_none()
                || attrs.metadata_symbol.is_none()
                || attrs.metadata_uri.is_none()),
        "`metadata::name`, `metadata::symbol`, and `metadata::uri` must all be specified"
    );

    // Master edition attributes
    reject!(
        attrs.master_edition_max_supply.is_some() && !is_init,
        "`master_edition::max_supply` requires `init` or `init_if_needed`"
    );
    reject!(
        attrs.master_edition_max_supply.is_some() && attrs.metadata_name.is_none(),
        "`master_edition::max_supply` requires `metadata::name`, `metadata::symbol`, and \
         `metadata::uri`"
    );

    // #[account(dup)] requires a safety doc comment
    if attrs.dup {
        let has_doc = field.attrs.iter().any(|a| a.path().is_ident("doc"));
        reject!(
            !has_doc,
            "#[account(dup)] requires a /// CHECK: <reason> doc comment explaining why this \
             account is safe to use as a duplicate. Duplicate accounts allow aliased mutable \
             access to the same underlying data."
        );
    }

    Ok(())
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
            "#[account(init)] requires a `Program<System>` field in the accounts struct",
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
            "#[account(realloc)] requires a `payer` field, `realloc::payer = field`, or `payer = \
             field` attribute",
        )?)
    } else {
        None
    };

    // Validate payer writability: init and realloc payers must be mutable.
    for (label, payer) in [("init", &payer_field), ("realloc", &realloc_payer_field)] {
        if let Some(payer_ident) = payer {
            let writable = fields
                .iter()
                .zip(field_attrs.iter())
                .find(|(f, _)| f.ident.as_ref() == Some(payer_ident))
                .map(|(f, attrs)| {
                    let eff = extract_generic_inner_type(&f.ty, "Option").unwrap_or(&f.ty);
                    attrs.is_mut || matches!(eff, Type::Reference(r) if r.mutability.is_some())
                })
                .unwrap_or(false);
            if !writable {
                return Err(syn::Error::new_spanned(
                    payer_ident,
                    format!(
                        "`{}` payer `{}` must be `&mut` or `#[account(mut)]`",
                        label, payer_ident
                    ),
                )
                .to_compile_error()
                .into());
            }
        }
    }

    // Close on token/mint types requires a token program field for CPI close.
    let has_any_token_close = field_attrs
        .iter()
        .zip(fields.iter())
        .any(|(a, f)| a.close.is_some() && is_token_or_mint_field(f));

    // Sweep requires a token program field for transfer_checked CPI.
    let has_any_sweep = field_attrs.iter().any(|a| a.sweep.is_some());

    // Non-init InterfaceAccount fields with token/ata attrs need a runtime
    // token_program_field. Account<Token>/Account<Token2022> resolve at compile
    // time and do NOT require one.
    let has_any_non_init_interface_needing_program =
        field_attrs.iter().zip(fields.iter()).any(|(a, f)| {
            let is_init = a.is_init || a.init_if_needed;
            let needs_program = a.token_mint.is_some()
                || (a.associated_token_mint.is_some()
                    && a.associated_token_token_program.is_none())
                || a.mint_decimals.is_some();
            !is_init && needs_program && is_interface_account_field(f)
        });

    let token_program_field = if has_any_token_init
        || has_any_ata_init
        || has_any_mint_init
        || has_any_master_edition_init
        || has_any_non_init_interface_needing_program
        || has_any_token_close
        || has_any_sweep
    {
        // Check for multiple token program fields when InterfaceAccount ATA attrs
        // don't specify an explicit program — ambiguity must be resolved by the user.
        if has_any_non_init_interface_needing_program && detected.token_program_count > 1 {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Multiple token program fields detected. Use `associated_token::token_program = \
                 <field>` to specify which one.",
            )
            .to_compile_error()
            .into());
        }
        Some(DetectedFields::require(
            detected.token_program,
            "token/ATA/mint/master_edition init or InterfaceAccount validation requires a token \
             program field (Program<Token>, Program<Token2022>, or Interface<TokenInterface>)",
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
            "`metadata::*` requires a field of type `Account<MetadataAccount>` or a field named \
             `metadata`",
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
            "`master_edition::*` requires a field of type `Account<MasterEditionAccount>` or a \
             field named `master_edition`/`edition`",
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
            "`metadata::*` / `master_edition::*` requires a `rent` field (UncheckedAccount for \
             the Rent sysvar)",
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
    let mut close_fields: Vec<CloseFieldInfo> = Vec::new();
    let mut sweep_fields: Vec<SweepFieldInfo> = Vec::new();

    for (field, attrs) in fields.iter().zip(field_attrs.iter()) {
        let field_name = field.ident.as_ref().unwrap();

        let is_optional = extract_generic_inner_type(&field.ty, "Option").is_some();
        let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
        let is_ref_mut = matches!(effective_ty, Type::Reference(r) if r.mutability.is_some());

        validate_field_attrs(field, field_name, attrs, is_ref_mut)?;

        let is_dynamic = is_dynamic_account_type(effective_ty);
        let underlying_ty = match effective_ty {
            Type::Reference(type_ref) => &*type_ref.elem,
            other => other,
        };

        // Generate type-specific validation (owner, discriminator, address).
        // Flags are already validated via u32 header in parse_accounts.
        //
        // For init/init_if_needed fields with token/mint/ATA attrs, skip
        // Account<T> and InterfaceAccount<T> checks here — the init block's
        // inline validation handles everything (owner, data_len,
        // is_initialized, field-specific). This avoids redundant key
        // comparisons and saves CUs. For generic Account<T> init_if_needed
        // (no token/mint attrs), keep mut_checks since the init block has
        // no validate_existing and these are the only defense.
        let has_inline_validation = attrs.token_mint.is_some()
            || attrs.associated_token_mint.is_some()
            || attrs.mint_decimals.is_some();
        let skip_mut_checks = (attrs.is_init || attrs.init_if_needed) && has_inline_validation;
        let mut push_check = |check: proc_macro2::TokenStream| {
            if is_optional {
                mut_checks.push(quote! { if let Some(ref #field_name) = #field_name { #check } });
            } else {
                mut_checks.push(check);
            }
        };

        if let Some(inner_ty) = extract_generic_inner_type(underlying_ty, "Account") {
            if !skip_mut_checks {
                let field_name_str = field_name.to_string();
                push_check(quote! {
                    #[cfg(feature = "debug")]
                    if let Err(e) = <#inner_ty as quasar_lang::traits::CheckOwner>::check_owner(#field_name.to_account_view()) {
                        quasar_lang::prelude::log(&::alloc::format!("Owner check failed for account '{}'", #field_name_str));
                        return Err(e);
                    }
                    #[cfg(not(feature = "debug"))]
                    <#inner_ty as quasar_lang::traits::CheckOwner>::check_owner(#field_name.to_account_view())?;

                    #[cfg(feature = "debug")]
                    if let Err(e) = <#inner_ty as quasar_lang::traits::AccountCheck>::check(#field_name.to_account_view()) {
                        quasar_lang::prelude::log(&::alloc::format!("Discriminator check failed for account '{}': data may be uninitialized or corrupted", #field_name_str));
                        return Err(e);
                    }
                    #[cfg(not(feature = "debug"))]
                    <#inner_ty as quasar_lang::traits::AccountCheck>::check(#field_name.to_account_view())?;
                });
            }
        } else if let Some(inner_ty) = extract_generic_inner_type(underlying_ty, "InterfaceAccount")
        {
            if !skip_mut_checks {
                let field_name_str = field_name.to_string();
                push_check(quote! {
                    {
                        let __owner = #field_name.to_account_view().owner();
                        if quasar_lang::utils::hint::unlikely(
                            !quasar_lang::keys_eq(__owner, &quasar_spl::SPL_TOKEN_ID)
                                && !quasar_lang::keys_eq(__owner, &quasar_spl::TOKEN_2022_ID)
                        ) {
                            #[cfg(feature = "debug")]
                            quasar_lang::prelude::log(&::alloc::format!(
                                "Owner check failed for interface account '{}': not owned by SPL Token or Token-2022",
                                #field_name_str
                            ));
                            return Err(ProgramError::IllegalOwner);
                        }
                    }

                    #[cfg(feature = "debug")]
                    if let Err(e) = <#inner_ty as quasar_lang::traits::AccountCheck>::check(#field_name.to_account_view()) {
                        quasar_lang::prelude::log(&::alloc::format!("Account check failed for interface account '{}': data may be uninitialized or corrupted", #field_name_str));
                        return Err(e);
                    }
                    #[cfg(not(feature = "debug"))]
                    <#inner_ty as quasar_lang::traits::AccountCheck>::check(#field_name.to_account_view())?;
                });
            }
        } else if let Some(inner_ty) = extract_generic_inner_type(underlying_ty, "Sysvar") {
            let field_name_str = field_name.to_string();
            push_check(quote! {
                if #field_name.to_account_view().address() != &<#inner_ty as quasar_lang::sysvars::Sysvar>::ID {
                    #[cfg(feature = "debug")]
                    quasar_lang::prelude::log(&::alloc::format!(
                        "Incorrect sysvar address for account '{}': expected {}, got {}",
                        #field_name_str,
                        <#inner_ty as quasar_lang::sysvars::Sysvar>::ID,
                        #field_name.to_account_view().address()
                    ));
                    return Err(ProgramError::IncorrectProgramId);
                }
            });
        } else if let Some(inner_ty) = extract_generic_inner_type(underlying_ty, "Program") {
            let field_name_str = field_name.to_string();
            push_check(quote! {
                if *#field_name.to_account_view().address() != <#inner_ty as quasar_lang::traits::Id>::ID {
                    #[cfg(feature = "debug")]
                    quasar_lang::prelude::log(&::alloc::format!(
                        "Incorrect program ID for account '{}': expected {}, got {}",
                        #field_name_str,
                        <#inner_ty as quasar_lang::traits::Id>::ID,
                        #field_name.to_account_view().address()
                    ));
                    return Err(ProgramError::IncorrectProgramId);
                }
            });
        } else if let Some(inner_ty) = extract_generic_inner_type(underlying_ty, "Interface") {
            let field_name_str = field_name.to_string();
            push_check(quote! {
                if !<#inner_ty as quasar_lang::traits::ProgramInterface>::matches(#field_name.to_account_view().address()) {
                    #[cfg(feature = "debug")]
                    quasar_lang::prelude::log(&::alloc::format!(
                        "Program interface mismatch for account '{}': address {} does not match any allowed programs",
                        #field_name_str,
                        #field_name.to_account_view().address()
                    ));
                    return Err(ProgramError::IncorrectProgramId);
                }
            });
        } else if type_base_name(&field.ty).as_deref() == Some("SystemAccount") {
            let field_name_str = field_name.to_string();
            let base_type = strip_generics(underlying_ty);
            push_check(quote! {
                #[cfg(feature = "debug")]
                if let Err(e) = <#base_type as quasar_lang::checks::Owner>::check(#field_name.to_account_view()) {
                    quasar_lang::prelude::log(&::alloc::format!("Owner check failed for account '{}': not owned by system program", #field_name_str));
                    return Err(e);
                }
                #[cfg(not(feature = "debug"))]
                <#base_type as quasar_lang::checks::Owner>::check(#field_name.to_account_view())?;
            });
        }

        // Field construction — flags already validated via header check
        let construct = |expr: proc_macro2::TokenStream| {
            if is_optional {
                quote! { #field_name: if quasar_lang::keys_eq(#field_name.address(), __program_id) { None } else { Some(#expr) } }
            } else {
                quote! { #field_name: #expr }
            }
        };

        match effective_ty {
            Type::Reference(type_ref) if !is_dynamic => {
                let base_type = strip_generics(&type_ref.elem);
                let expr = if type_ref.mutability.is_some() {
                    quote! { unsafe { #base_type::from_account_view_unchecked_mut(#field_name) } }
                } else {
                    quote! { unsafe { #base_type::from_account_view_unchecked(#field_name) } }
                };
                field_constructs.push(construct(expr));
            }
            _ if is_dynamic => {
                if let Some(inner) = extract_generic_inner_type(underlying_ty, "Account") {
                    let inner_base = strip_generics(inner);
                    field_constructs.push(construct(
                        quote! { #inner_base::from_account_view(#field_name)? },
                    ));
                } else {
                    let base_type = strip_generics(effective_ty);
                    field_constructs
                        .push(quote! { #field_name: #base_type::from_account_view(#field_name)? });
                }
            }
            _ => {
                let base_type = strip_generics(effective_ty);
                field_constructs.push(construct(
                    quote! { unsafe { #base_type::from_account_view_unchecked(#field_name) } },
                ));
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
            let cpi_close = if is_token_or_mint_field(field) {
                // Token/mint accounts require CPI close via the token program.
                // Resolve the authority from token::authority (or associated_token::authority).
                let authority = attrs
                    .token_authority
                    .clone()
                    .or_else(|| attrs.associated_token_authority.clone())
                    .ok_or_else(|| -> proc_macro::TokenStream {
                        syn::Error::new_spanned(
                            field_name,
                            "#[account(close)] on token/mint types requires `token::authority`",
                        )
                        .to_compile_error()
                        .into()
                    })?;
                // Find the token program field.
                let tp_field: Ident =
                    token_program_field
                        .cloned()
                        .ok_or_else(|| -> proc_macro::TokenStream {
                            syn::Error::new_spanned(
                                field_name,
                                "#[account(close)] on token/mint types requires a token program \
                                 field",
                            )
                            .to_compile_error()
                            .into()
                        })?;
                Some(CpiCloseInfo {
                    token_program: tp_field,
                    authority,
                })
            } else {
                None
            };
            close_fields.push(CloseFieldInfo {
                field: field_name.clone(),
                destination: dest.clone(),
                cpi_close,
            });
        }

        // --- Sweep field tracking ---

        if let Some(receiver) = &attrs.sweep {
            // Validate the sweep target field exists and is a token account type.
            let receiver_field = fields
                .iter()
                .find(|f| f.ident.as_ref() == Some(receiver))
                .ok_or_else(|| -> proc_macro::TokenStream {
                    syn::Error::new_spanned(
                        receiver,
                        format!("sweep target `{}` not found in accounts struct", receiver),
                    )
                    .to_compile_error()
                    .into()
                })?;
            if !is_token_account_field(receiver_field) {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "sweep target must be a token account (Account<Token>, Account<Token2022>, or \
                     InterfaceAccount<Token>)",
                )
                .to_compile_error()
                .into());
            }
            // Validate target is mutable.
            let target_is_mut = matches!(&receiver_field.ty, Type::Reference(r) if r.mutability.is_some())
                || field_attrs
                    .iter()
                    .zip(fields.iter())
                    .any(|(a, f)| f.ident.as_ref() == Some(receiver) && a.is_mut);
            if !target_is_mut {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "sweep target must be mutable (`&mut` or `#[account(mut)]`)",
                )
                .to_compile_error()
                .into());
            }
            // Validate token::authority is a Signer.
            if let Some(auth_name) = &attrs.token_authority {
                let auth_field = fields.iter().find(|f| f.ident.as_ref() == Some(auth_name));
                if let Some(af) = auth_field {
                    if !is_signer_field(af) {
                        return Err(syn::Error::new_spanned(
                            auth_name,
                            "sweep requires `token::authority` to be a Signer (it must sign the \
                             transfer_checked CPI)",
                        )
                        .to_compile_error()
                        .into());
                    }
                }
            }

            let mint = attrs.token_mint.clone().unwrap();
            let authority = attrs.token_authority.clone().unwrap();
            let tp_field = token_program_field.cloned().unwrap();

            sweep_fields.push(SweepFieldInfo {
                field: field_name.clone(),
                receiver: receiver.clone(),
                mint,
                authority,
                token_program: tp_field,
            });
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

            // Solana enforces MAX_SEEDS = 16 at runtime; we catch it here at compile time
            // since seed count is always statically known from the attribute.
            // (15 explicit seeds + 1 bump = 16 total)
            if seed_slices.len() > 15 {
                return Err(syn::Error::new_spanned(
                    field_name,
                    format!(
                        "`{}` exceeds Solana's PDA seed limit: {} seeds provided, max is 16 \
                         including bump",
                        field_name,
                        seed_slices.len()
                    ),
                )
                .to_compile_error()
                .into());
            }

            let seed_idents: Vec<Ident> = seed_slices
                .iter()
                .enumerate()
                .map(|(idx, _)| format_ident!("__seed_{}_{}", field_name, idx))
                .collect();

            let seed_len_checks: Vec<proc_macro2::TokenStream> = seed_idents
                .iter()
                .zip(seed_slices.iter())
                .zip(seed_exprs.iter())
                .map(|((ident, seed), expr)| {
                    match expr {
                        // static byte string (b"quasar") — compile time check
                        Expr::Lit(ExprLit {
                            lit: Lit::ByteStr(b),
                            ..
                        }) => {
                            let len = b.value().len();
                            if len > 32 {
                                return syn::Error::new_spanned(
                                    expr,
                                    format!(
                                        "seed b\"{}\" is {} bytes, exceeds MAX_SEED_LEN of 32",
                                        String::from_utf8_lossy(&b.value()),
                                        len
                                    ),
                                )
                                .to_compile_error();
                            }
                            quote! { let #ident: &[u8] = #seed; }
                        }

                        // dynamic — runtime check
                        _ => quote! {
                            let #ident: &[u8] = #seed;
                            if #ident.len() > 32 {
                                return Err(QuasarError::InvalidSeeds.into());
                            }
                        },
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
            // non-init fields are typed wrappers (rebound via let Self { ref ... } =
            // result)
            let addr_access = if is_init_field {
                quote! { *#field_name.address() }
            } else {
                quote! { *#field_name.to_account_view().address() }
            };

            match &attrs.bump {
                Some(Some(bump_expr)) => {
                    let check = quote! {
                        {
                            #(#seed_len_checks)*
                            let __bump_val: u8 = #bump_expr;
                            let __bump_ref: &[u8] = &[__bump_val];
                            let __pda_seeds = [#(#seed_idents,)* __bump_ref];
                            quasar_lang::pda::verify_program_address(&__pda_seeds, __program_id, &#addr_access)?;
                            #bump_var = __bump_val;
                        }
                    };
                    if is_optional && !is_init_field {
                        target_checks
                            .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
                    } else {
                        target_checks.push(check);
                    }
                }
                Some(None) => {
                    let check = quote! {
                        {
                            #(#seed_len_checks)*
                            let __pda_seeds = [#(#seed_idents),*];
                            let (__expected, __bump) = quasar_lang::pda::based_try_find_program_address(&__pda_seeds, __program_id)?;
                            if #addr_access != __expected {
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump;
                        }
                    };
                    if is_optional && !is_init_field {
                        target_checks
                            .push(quote! { if let Some(ref #field_name) = #field_name { #check } });
                    } else {
                        target_checks.push(check);
                    }
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
                                quote! { quasar_lang::cpi::Seed::from(self.#addr_field.as_ref()) },
                            );
                            continue;
                        }
                    }
                }
                seed_elements.push(quote! { quasar_lang::cpi::Seed::from((#expr) as &[u8]) });
            }

            seed_elements
                .push(quote! { quasar_lang::cpi::Seed::from(&self.#bump_arr_field as &[u8]) });

            seeds_methods.push(quote! {
                #[inline(always)]
                pub fn #method_name(&self) -> [quasar_lang::cpi::Seed<'_>; #seed_count] {
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
                    let __init_signer_seeds = [#(quasar_lang::cpi::Seed::from(#seed_slices),)* quasar_lang::cpi::Seed::from(__init_bump_ref)];
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
                let tok_field = attrs
                    .associated_token_token_program
                    .as_ref()
                    .unwrap_or_else(|| token_program_field.unwrap());
                let token_program_addr = if let Some(tp) = &attrs.associated_token_token_program {
                    quote! { #tp.address() }
                } else {
                    let tp = token_program_field.unwrap();
                    quote! { #tp.address() }
                };

                let ata_cpi = |instruction_byte: u8| {
                    quote! {
                        quasar_lang::cpi::CpiCall::new(
                            #ata_prog.address(),
                            [
                                quasar_lang::cpi::InstructionAccount::writable_signer(#pay_field.address()),
                                quasar_lang::cpi::InstructionAccount::writable(#field_name.address()),
                                quasar_lang::cpi::InstructionAccount::readonly(#auth_field.address()),
                                quasar_lang::cpi::InstructionAccount::readonly(#mint_field.address()),
                                quasar_lang::cpi::InstructionAccount::readonly(#sys_field.address()),
                                quasar_lang::cpi::InstructionAccount::readonly(#tok_field.address()),
                            ],
                            [#pay_field, #field_name, #auth_field, #mint_field, #sys_field, #tok_field],
                            [#instruction_byte],
                        ).invoke()?;
                    }
                };

                let validate = quote! {
                    quasar_spl::validate_ata(
                        #field_name.to_account_view(),
                        #auth_field.to_account_view().address(),
                        #mint_field.to_account_view().address(),
                        #token_program_addr,
                    )?;
                };
                // ATA: create_idempotent (1) for init_if_needed, create (0) for init
                init_blocks.push(wrap_init_block(
                    field_name,
                    attrs.init_if_needed,
                    ata_cpi(if attrs.init_if_needed { 1 } else { 0 }),
                    Some(validate),
                ));
            } else if is_token_init {
                let tok_field = token_program_field.unwrap();
                let mint_field = attrs.token_mint.as_ref().unwrap();
                let auth_field = attrs.token_authority.as_ref().unwrap();

                let cpi_body = quote! {
                    let __init_lamports = __shared_rent.try_minimum_balance(
                        quasar_spl::TokenAccountState::LEN
                    )?;
                    let __init_cpi = quasar_lang::cpi::system::create_account(
                        #pay_field, #field_name, __init_lamports,
                        quasar_spl::TokenAccountState::LEN as u64,
                        #tok_field.address(),
                    );
                    #invoke_expr
                    quasar_spl::initialize_account3(
                        #tok_field, #field_name, #mint_field, #auth_field.address(),
                    ).invoke()?;
                };
                let tok_addr = quote! { #tok_field.address() };
                let validate = quote! {
                    quasar_spl::validate_token_account(
                        #field_name.to_account_view(),
                        #mint_field.to_account_view().address(),
                        #auth_field.to_account_view().address(),
                        #tok_addr,
                    )?;
                };
                init_blocks.push(wrap_init_block(
                    field_name,
                    attrs.init_if_needed,
                    cpi_body,
                    Some(validate),
                ));
            } else if let Some(decimals_expr) = attrs.mint_decimals.as_ref() {
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

                let cpi_body = quote! {
                    let __init_lamports = __shared_rent.try_minimum_balance(
                        quasar_spl::MintAccountState::LEN
                    )?;
                    let __init_cpi = quasar_lang::cpi::system::create_account(
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
                };
                let tok_addr = quote! { #tok_field.address() };
                let freeze_expr_ref = if let Some(freeze_field) = &attrs.mint_freeze_authority {
                    quote! { Some(#freeze_field.address()) }
                } else {
                    quote! { None }
                };
                let validate = quote! {
                    quasar_spl::validate_mint(
                        #field_name.to_account_view(),
                        #auth_field.to_account_view().address(),
                        (#decimals_expr) as u8,
                        #freeze_expr_ref,
                        #tok_addr,
                    )?;
                };
                init_blocks.push(wrap_init_block(
                    field_name,
                    attrs.init_if_needed,
                    cpi_body,
                    Some(validate),
                ));
            } else {
                let inner_type = extract_account_inner_type(effective_ty);
                if inner_type.is_none() {
                    return Err(syn::Error::new_spanned(
                        field_name,
                        "#[account(init)] on non-Account<T> type requires `token::mint` and \
                         `token::authority`, `associated_token::mint` and \
                         `associated_token::authority`, or `mint::decimals` and `mint::authority`",
                    )
                    .to_compile_error()
                    .into());
                }
                let inner_type = inner_type.unwrap();

                let space_expr = if let Some(space) = &attrs.space {
                    quote! { (#space) as u64 }
                } else {
                    quote! { <#inner_type as quasar_lang::traits::Space>::SPACE as u64 }
                };

                let cpi_body = quote! {
                    let __init_space = #space_expr;
                    let __init_lamports = __shared_rent.try_minimum_balance(__init_space as usize)?;
                    let __init_cpi = quasar_lang::cpi::system::create_account(
                        #pay_field, #field_name, __init_lamports, __init_space, &crate::ID,
                    );
                    #invoke_expr
                    let __disc = <#inner_type as quasar_lang::traits::Discriminator>::DISCRIMINATOR;
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            __disc.as_ptr(), #field_name.data_mut_ptr(), __disc.len(),
                        );
                    }
                };
                init_blocks.push(wrap_init_block(
                    field_name,
                    attrs.init_if_needed,
                    cpi_body,
                    None,
                ));
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
                resolve_token_program_addr(effective_ty, token_program_field)
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

        // --- Non-init token account validation ---

        if let (false, Some(mint_field), Some(auth_field)) = (
            is_init_field,
            attrs.token_mint.as_ref(),
            attrs.token_authority.as_ref(),
        ) {
            let token_program_addr = resolve_token_program_addr(effective_ty, token_program_field);
            pda_checks.push(quote! {
                quasar_spl::validate_token_account(
                    #field_name.to_account_view(),
                    #mint_field.to_account_view().address(),
                    #auth_field.to_account_view().address(),
                    #token_program_addr,
                )?;
            });
        }

        // --- Non-init mint validation ---

        if let (false, Some(decimals_expr), Some(auth_field)) = (
            is_init_field,
            attrs.mint_decimals.as_ref(),
            attrs.mint_init_authority.as_ref(),
        ) {
            let token_program_addr = resolve_token_program_addr(effective_ty, token_program_field);
            let freeze_expr = if let Some(freeze_field) = &attrs.mint_freeze_authority {
                quote! { Some(#freeze_field.to_account_view().address()) }
            } else {
                quote! { None }
            };
            pda_checks.push(quote! {
                quasar_spl::validate_mint(
                    #field_name.to_account_view(),
                    #auth_field.to_account_view().address(),
                    (#decimals_expr) as u8,
                    #freeze_expr,
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
                    quasar_lang::accounts::realloc_account(
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
                            quasar_lang::borsh::BorshString::new(#meta_name),
                            quasar_lang::borsh::BorshString::new(#meta_symbol),
                            quasar_lang::borsh::BorshString::new(#meta_uri),
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
        sweep_fields,
        needs_rent,
    })
}

/// Determine which NODUP constant to use for a field.
/// Returns the constant name as a string for code generation.
pub(super) fn determine_nodup_constant(
    field: &syn::Field,
    attrs: &super::attrs::AccountFieldAttrs,
    is_ref_mut: bool,
) -> &'static str {
    let type_name = type_base_name(&field.ty).unwrap_or_default();

    // Program<T> and Interface<T> are always executable
    let underlying_ty = match field.ty {
        Type::Reference(ref type_ref) => &*type_ref.elem,
        ref other => other,
    };
    if extract_generic_inner_type(underlying_ty, "Program").is_some()
        || extract_generic_inner_type(underlying_ty, "Interface").is_some()
    {
        return "NODUP_EXECUTABLE";
    }

    // Determine signer requirement
    let is_signer = type_name == "Signer"
        || (attrs.is_init && attrs.seeds.is_none() && attrs.associated_token_mint.is_none());

    // Determine writable requirement
    let is_writable = is_ref_mut || attrs.is_mut;

    match (is_signer, is_writable) {
        (true, true) => "NODUP_MUT_SIGNER",
        (true, false) => "NODUP_SIGNER",
        (false, true) => "NODUP_MUT",
        (false, false) => "NODUP",
    }
}

/// Compute the expected u32 header value for a field based on its attributes
/// and type.
///
/// Returns a u32 in little-endian byte order:
/// - Byte 0: borrow_state (always 0xFF for no-dup)
/// - Byte 1: is_signer (1 if required, 0 otherwise)
/// - Byte 2: is_writable (1 if required, 0 otherwise)
/// - Byte 3: executable (1 if required, 0 otherwise)
pub(super) fn compute_header_expected(
    field: &syn::Field,
    attrs: &super::attrs::AccountFieldAttrs,
    is_ref_mut: bool,
) -> u32 {
    let mut header: u32 = 0xFF; // byte 0: always no-dup (0xFF)

    // Detect signer requirement from type name or init without seeds/PDA
    let type_name = type_base_name(&field.ty).unwrap_or_default();
    let is_signer = type_name == "Signer";
    // Only init (not init_if_needed) without seeds/PDA requires signer (for
    // pre-signing) init_if_needed accounts might already exist, so they won't
    // necessarily be signers associated_token accounts are PDAs, so they're not
    // signers
    let is_init_without_seeds =
        attrs.is_init && attrs.seeds.is_none() && attrs.associated_token_mint.is_none();
    if is_signer || is_init_without_seeds {
        header |= 0x01 << 8; // byte 1: is_signer = 1
    }

    // Detect writable requirement from &mut or #[account(mut)]
    let is_writable = is_ref_mut || attrs.is_mut;
    if is_writable {
        header |= 0x01 << 16; // byte 2: is_writable = 1
    }

    // Detect executable requirement from Program<T> or Interface<T>
    let underlying_ty = match field.ty {
        Type::Reference(ref type_ref) => &*type_ref.elem,
        ref other => other,
    };
    let is_program_generic = extract_generic_inner_type(underlying_ty, "Program").is_some();
    let is_interface_generic = extract_generic_inner_type(underlying_ty, "Interface").is_some();
    if is_program_generic || is_interface_generic {
        header |= 0x01 << 24; // byte 3: executable = 1
    }

    header
}
