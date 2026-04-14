use {
    super::super::{
        attrs::AccountFieldAttrs,
        field_kind::{strip_ref, type_base_name, FieldKind},
    },
    quote::quote,
    syn::{Ident, Type},
};

#[derive(Copy, Clone)]
pub(super) enum TokenProgramResolution {
    ExplicitOnly,
    FallbackRequired,
}

impl TokenProgramResolution {
    pub(super) fn needs_fallback(self) -> bool {
        matches!(self, TokenProgramResolution::FallbackRequired)
    }
}

/// Find a field by type base name or Program<T>/Interface<T> wrapper. Returns
/// the field ident if found.
fn find_field_by_type<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    type_names: &[&str],
) -> Option<&'a Ident> {
    for field in fields.iter() {
        let ty = strip_ref(&field.ty);

        if let Some(base) = type_base_name(ty) {
            if type_names.contains(&base.as_str()) {
                return field.ident.as_ref();
            }
        }

        match FieldKind::classify(ty) {
            FieldKind::Program { inner_ty } | FieldKind::Interface { inner_ty } => {
                if let Some(inner_base) = type_base_name(inner_ty) {
                    if type_names.contains(&inner_base.as_str()) {
                        return field.ident.as_ref();
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_token_program_field(field: &syn::Field) -> bool {
    match FieldKind::classify(strip_ref(&field.ty)) {
        FieldKind::Program { inner_ty } | FieldKind::Interface { inner_ty } => {
            type_base_name(inner_ty)
                .as_deref()
                .is_some_and(|name| matches!(name, "Token" | "Token2022" | "TokenInterface"))
        }
        _ => false,
    }
}

fn resolve_explicit_token_program_field<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    selector: &Ident,
    selector_attr: &str,
) -> Result<&'a Ident, proc_macro::TokenStream> {
    let field = fields
        .iter()
        .find(|f| f.ident.as_ref() == Some(selector))
        .ok_or_else(|| -> proc_macro::TokenStream {
            syn::Error::new_spanned(
                selector,
                format!("`{selector_attr}` references unknown field `{selector}`"),
            )
            .to_compile_error()
            .into()
        })?;

    if !is_token_program_field(field) {
        return Err(syn::Error::new_spanned(
            selector,
            format!(
                "`{selector_attr}` must reference `Program<Token>`, `Program<Token2022>`, or \
                 `Interface<TokenInterface>`"
            ),
        )
        .to_compile_error()
        .into());
    }

    Ok(field
        .ident
        .as_ref()
        .expect("account field must have an identifier"))
}

/// Count how many fields match the given type names (checking Program<T> and
/// Interface<T> wrappers). Used to detect ambiguous token program fields.
fn count_fields_by_type(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    type_names: &[&str],
) -> usize {
    let mut count = 0;
    for field in fields.iter() {
        let ty = strip_ref(&field.ty);
        let matched = match FieldKind::classify(ty) {
            FieldKind::Program { inner_ty } | FieldKind::Interface { inner_ty } => {
                type_base_name(inner_ty).is_some_and(|b| type_names.contains(&b.as_str()))
            }
            _ => false,
        };
        if matched {
            count += 1;
        }
    }
    count
}

/// Find a field by name.
pub(super) fn find_field_by_name<'a>(
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
        let ty = strip_ref(&field.ty);
        if let FieldKind::Account { inner_ty } = FieldKind::classify(ty) {
            if let Some(base) = type_base_name(inner_ty) {
                if inner_type_names.contains(&base.as_str()) {
                    return field.ident.as_ref();
                }
            }
        }
    }
    None
}

pub(super) struct DetectedFields<'a> {
    pub system_program: Option<&'a Ident>,
    pub token_program: Option<&'a Ident>,
    pub token_program_count: usize,
    pub associated_token_program: Option<&'a Ident>,
    pub metadata_program: Option<&'a Ident>,
    pub metadata_account: Option<&'a Ident>,
    pub master_edition_account: Option<&'a Ident>,
    pub payer: Option<&'a Ident>,
    pub realloc_payer: Option<&'a Ident>,
    pub mint_authority: Option<&'a Ident>,
    pub update_authority: Option<&'a Ident>,
    pub rent_sysvar: Option<&'a Ident>,
}

impl<'a> DetectedFields<'a> {
    pub(super) fn detect(
        fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
        field_attrs: &[AccountFieldAttrs],
    ) -> Self {
        let system_program = find_field_by_type(fields, &["System"]);
        let token_program = find_field_by_type(fields, &["Token", "Token2022", "TokenInterface"]);
        let token_program_count =
            count_fields_by_type(fields, &["Token", "Token2022", "TokenInterface"]);
        let associated_token_program = find_field_by_type(fields, &["AssociatedTokenProgram"]);
        let metadata_program = find_field_by_type(fields, &["MetadataProgram"]);

        let metadata_account = find_field_by_account_inner_type(fields, &["MetadataAccount"]);
        let master_edition_account =
            find_field_by_account_inner_type(fields, &["MasterEditionAccount"]);

        let explicit_payer = field_attrs.iter().find_map(|a| a.payer.as_ref());
        let payer = explicit_payer
            .and_then(|name| find_field_by_name(fields, &name.to_string()))
            .or_else(|| find_field_by_name(fields, "payer"));

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

        let mint_authority = find_field_by_name(fields, "mint_authority")
            .or_else(|| find_field_by_name(fields, "authority"));
        let update_authority = find_field_by_name(fields, "update_authority").or(mint_authority);

        let rent_sysvar = fields.iter().find_map(|field| {
            let ty = strip_ref(&field.ty);
            if let FieldKind::Sysvar { inner_ty } = FieldKind::classify(ty) {
                if type_base_name(inner_ty).as_deref() == Some("Rent") {
                    return field.ident.as_ref();
                }
            }
            None
        });

        Self {
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
            rent_sysvar,
        }
    }

    pub(super) fn require(
        field: Option<&'a Ident>,
        msg: &str,
    ) -> Result<&'a Ident, proc_macro::TokenStream> {
        field.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), msg)
                .to_compile_error()
                .into()
        })
    }
}

pub(super) fn resolve_token_program_field<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    detected: &DetectedFields<'a>,
    field_name: &Ident,
    selector: Option<&Ident>,
    selector_attr: &str,
    consumer_desc: &str,
    resolution: TokenProgramResolution,
) -> Result<Option<&'a Ident>, proc_macro::TokenStream> {
    if let Some(selector) = selector {
        return Ok(Some(resolve_explicit_token_program_field(
            fields,
            selector,
            selector_attr,
        )?));
    }

    if detected.token_program_count > 1 {
        return Err(syn::Error::new_spanned(
            field_name,
            format!(
                "Multiple token program fields detected. `{selector_attr} = <field>` is required \
                 for {consumer_desc} when more than one token program field is present."
            ),
        )
        .to_compile_error()
        .into());
    }

    if resolution.needs_fallback() {
        return Ok(Some(DetectedFields::require(
            detected.token_program,
            &format!(
                "{consumer_desc} requires a token program field (Program<Token>, \
                 Program<Token2022>, or Interface<TokenInterface>)"
            ),
        )?));
    }

    Ok(None)
}

/// Resolve the token program address expression for a non-init field based on
/// its wrapper type.
pub(super) fn resolve_token_program_addr(
    effective_ty: &Type,
    token_program_field: Option<&Ident>,
) -> proc_macro2::TokenStream {
    if let Some(tp) = token_program_field {
        return quote! { #tp.to_account_view().address() };
    }

    let underlying = strip_ref(effective_ty);
    if let FieldKind::Account { inner_ty } = FieldKind::classify(underlying) {
        if let Some(name) = type_base_name(inner_ty) {
            match name.as_str() {
                "Token" | "Mint" => return quote! { &quasar_spl::SPL_TOKEN_ID },
                "Token2022" | "Mint2022" => return quote! { &quasar_spl::TOKEN_2022_ID },
                _ => {}
            }
        }
    }

    // Composition rules and validate_composition guarantee that every
    // InterfaceAccount with token/ata attrs has a resolved token program field.
    // Reaching here means a handler bypassed that check — this is a framework bug.
    unreachable!("BUG: InterfaceAccount with token/ata attrs requires a token program field")
}
