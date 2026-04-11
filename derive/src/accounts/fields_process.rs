use {
    super::{
        super::{
            attrs::{parse_field_attrs, AccountFieldAttrs, TypedSeeds},
            composition::validate_composition,
            constraint::verify_all_directives_mapped,
            evidence::{BumpEvidence, FieldEvidence, InitEvidence, OwnerEvidence, PdaEvidence},
            field_kind::{debug_checked, strip_ref, FieldKind},
            init, InstructionArg,
        },
        support::{
            find_field_by_name, resolve_token_program_addr, resolve_token_program_field,
            DetectedFields, TokenProgramResolution,
        },
        CloseFieldInfo, CpiCloseInfo, ProcessedFields, SweepFieldInfo,
    },
    crate::helpers::{
        extract_generic_inner_type, seed_slice_expr_for_parse, strip_generics,
        typed_seed_method_expr, typed_seed_slice_expr,
    },
    quote::{format_ident, quote},
    syn::{Expr, ExprLit, Ident, Lit, Type},
};

/// Per-field context bundling all parameters needed by the extracted helper
/// functions. Built once at the top of each loop iteration and passed by
/// reference to every helper.
struct FieldContext<'a> {
    field: &'a syn::Field,
    field_name: &'a Ident,
    attrs: &'a AccountFieldAttrs,
    kind: FieldKind<'a>,
    effective_ty: &'a Type,
    underlying_ty: &'a Type,
    is_optional: bool,
    is_init_field: bool,
    is_dynamic: bool,
    field_name_strings: &'a [String],
    instruction_args: &'a Option<Vec<InstructionArg>>,
    token_program_for_token: Option<&'a Ident>,
    token_program_for_mint: Option<&'a Ident>,
    token_program_for_ata: Option<&'a Ident>,
}

/// Recursively check if a `syn::Expr` contains an identifier matching `name`.
///
/// Walks method calls (`config.key()`), field access (`config.creator`),
/// references (`&config`), and paths (`config`). Returns `true` if the
/// root identifier of any sub-expression matches.
fn contains_ident(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Path(ep) => {
            ep.path.segments.len() == 1
                && ep.qself.is_none()
                && ep.path.segments[0].ident == name
        }
        Expr::MethodCall(mc) => contains_ident(&mc.receiver, name),
        Expr::Field(ef) => contains_ident(&ef.base, name),
        Expr::Reference(er) => contains_ident(&er.expr, name),
        Expr::Paren(ep) => contains_ident(&ep.expr, name),
        _ => false,
    }
}

/// Check if a syn::Type is `u8`.
fn is_type_u8(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.is_ident("u8"))
}

/// Generate the PDA bump verification code shared by both raw and typed seed
/// paths. Returns the token stream to push into target_checks.
///
/// Both seed codegen paths share identical bump handling logic:
/// - `bump = expr`: verify with explicit bump value
/// - bare `bump`: try ix arg match, then BUMP_OFFSET fast path, then
///   find_program_address
/// - missing bump: compile error
#[allow(clippy::too_many_arguments)]
fn gen_bump_check(
    field_name: &Ident,
    bump: &Option<Option<Expr>>,
    bump_var: &Ident,
    seed_idents: &[Ident],
    seed_len_checks: &[proc_macro2::TokenStream],
    addr_access: &proc_macro2::TokenStream,
    is_init_field: bool,
    kind: &FieldKind<'_>,
    instruction_args: &Option<Vec<InstructionArg>>,
    bare_bump_pda_count: usize,
    seeds_syntax_label: &str,
    raw_seed_exprs: Option<&[Expr]>,
) -> Result<proc_macro2::TokenStream, proc_macro::TokenStream> {
    // --- Compile-time PDA precomputation ---
    // When all seeds are byte literals and we can discover the program ID,
    // emit const bump + const address to skip runtime derivation entirely.
    if let Some(seed_exprs) = raw_seed_exprs {
        if let Some(seed_bytes) = crate::pda_precompute::seeds_as_byte_literals(seed_exprs) {
            if let Some(program_id) = crate::pda_precompute::discover_program_id() {
                let seed_refs: Vec<&[u8]> = seed_bytes.iter().map(|v| v.as_slice()).collect();
                {
                    let (precomputed_bump, precomputed_addr) =
                        crate::pda_precompute::precompute_pda(&seed_refs, &program_id);
                    let bump_lit = precomputed_bump;
                    let addr_bytes = precomputed_addr;
                    let addr_array: Vec<proc_macro2::TokenStream> =
                        addr_bytes.iter().map(|b| quote! { #b }).collect();

                    if is_init_field {
                        // For init: emit const bump, skip find entirely.
                        return Ok(quote! {
                            {
                                #bump_var = #bump_lit;
                            }
                        });
                    } else {
                        // For non-init: emit const address, use keys_eq instead
                        // of verify_program_address.
                        return Ok(quote! {
                            {
                                const __PRECOMPUTED_PDA: quasar_lang::prelude::Address =
                                    quasar_lang::prelude::Address::new_from_array([#(#addr_array),*]);
                                if !quasar_lang::keys_eq(&#addr_access, &__PRECOMPUTED_PDA) {
                                    #[cfg(feature = "debug")]
                                    quasar_lang::prelude::log(concat!(
                                        "Account '", stringify!(#field_name),
                                        "': PDA address mismatch (compile-time precomputed)"
                                    ));
                                    return Err(QuasarError::InvalidPda.into());
                                }
                                #bump_var = #bump_lit;
                            }
                        });
                    }
                }
            }
        }
    }
    // --- End precomputation; fall through to runtime paths ---

    match bump {
        Some(Some(bump_expr)) => Ok(quote! {
            {
                #(#seed_len_checks)*
                let __bump_val: u8 = #bump_expr;
                let __bump_ref: &[u8] = &[__bump_val];
                let __pda_seeds = [#(#seed_idents,)* __bump_ref];
                quasar_lang::pda::verify_program_address(&__pda_seeds, __program_id, &#addr_access)
                    .map_err(|__e| {
                        #[cfg(feature = "debug")]
                        quasar_lang::prelude::log(concat!(
                            "Account '", stringify!(#field_name),
                            "': PDA verification failed"
                        ));
                        __e
                    })?;
                #bump_var = __bump_val;
            }
        }),
        Some(None) => {
            let field_bump_name = format!("{}_bump", field_name);

            let ix_arg_match = instruction_args.as_ref().and_then(|args| {
                args.iter().find(|a| {
                    if !is_type_u8(&a.ty) {
                        return false;
                    }
                    let name = a.name.to_string();
                    if name == field_bump_name {
                        return true;
                    }
                    if name == "bump" && bare_bump_pda_count == 1 {
                        return true;
                    }
                    false
                })
            });

            let check = if let Some(arg) = ix_arg_match {
                let arg_ident = &arg.name;
                quote! {
                    {
                        #(#seed_len_checks)*
                        let __bump_val: u8 = #arg_ident;
                        let __bump_ref: &[u8] = &[__bump_val];
                        let __pda_seeds = [#(#seed_idents,)* __bump_ref];
                        quasar_lang::pda::verify_program_address(&__pda_seeds, __program_id, &#addr_access)
                            .map_err(|__e| {
                                #[cfg(feature = "debug")]
                                quasar_lang::prelude::log(concat!(
                                    "Account '", stringify!(#field_name),
                                    "': PDA verification failed"
                                ));
                                __e
                            })?;
                        #bump_var = __bump_val;
                    }
                }
            } else if !is_init_field {
                if let FieldKind::Account { inner_ty } = kind {
                    let view_access = quote! { #field_name.to_account_view() };
                    quote! {
                        {
                            #(#seed_len_checks)*
                            if let Some(__offset) = <#inner_ty as Discriminator>::BUMP_OFFSET {
                                let __bump_val: u8 = quasar_lang::pda::read_bump_from_account(
                                    #view_access, __offset,
                                ).map_err(|__e| {
                                    #[cfg(feature = "debug")]
                                    quasar_lang::prelude::log(concat!(
                                        "BUMP_OFFSET out of bounds for account '",
                                        stringify!(#field_name), "'"
                                    ));
                                    __e
                                })?;
                                let __bump_ref: &[u8] = &[__bump_val];
                                let __pda_seeds = [#(#seed_idents,)* __bump_ref];
                                quasar_lang::pda::verify_program_address(&__pda_seeds, __program_id, &#addr_access)
                                    .map_err(|__e| {
                                        #[cfg(feature = "debug")]
                                        quasar_lang::prelude::log(concat!(
                                            "Account '", stringify!(#field_name),
                                            "': PDA verification failed"
                                        ));
                                        __e
                                    })?;
                                #bump_var = __bump_val;
                            } else {
                                let __pda_seeds = [#(#seed_idents),*];
                                #bump_var = quasar_lang::pda::find_bump_for_address(&__pda_seeds, __program_id, &#addr_access)
                                    .map_err(|__e| {
                                        #[cfg(feature = "debug")]
                                        quasar_lang::prelude::log(concat!(
                                            "Account '", stringify!(#field_name),
                                            "': PDA verification failed"
                                        ));
                                        QuasarError::InvalidPda
                                    })?;
                            }
                        }
                    }
                } else {
                    // Non-Account type (UncheckedAccount, Mint, TokenAccount, etc.):
                    // must use based_try_find_program_address with on-curve check.
                    // We can't skip the on-curve check because we didn't create these
                    // accounts — only our own program's PDAs are guaranteed off-curve.
                    quote! {
                        {
                            #(#seed_len_checks)*
                            let __pda_seeds = [#(#seed_idents),*];
                            let (__expected, __bump) = quasar_lang::pda::based_try_find_program_address(&__pda_seeds, __program_id)?;
                            if #addr_access != __expected {
                                #[cfg(feature = "debug")]
                                quasar_lang::prelude::log(concat!(
                                    "Account '", stringify!(#field_name),
                                    "': PDA verification failed"
                                ));
                                return Err(QuasarError::InvalidPda.into());
                            }
                            #bump_var = __bump;
                        }
                    }
                }
            } else {
                quote! {
                    {
                        #(#seed_len_checks)*
                        let __pda_seeds = [#(#seed_idents),*];
                        let (__expected, __bump) = quasar_lang::pda::based_try_find_program_address(&__pda_seeds, __program_id)?;
                        if #addr_access != __expected {
                            #[cfg(feature = "debug")]
                            quasar_lang::prelude::log(concat!(
                                "Account '", stringify!(#field_name),
                                "': PDA verification failed"
                            ));
                            return Err(QuasarError::InvalidPda.into());
                        }
                        #bump_var = __bump;
                    }
                }
            };

            Ok(check)
        }
        None => Err(syn::Error::new_spanned(
            field_name,
            format!(
                "#[account({})] requires a `bump` or `bump = expr` directive",
                seeds_syntax_label,
            ),
        )
        .to_compile_error()
        .into()),
    }
}

/// Generate owner/discriminator/sysvar/program/interface/system-account type
/// checks for a single field. Returns the checks to push into
/// `this_field_checks`.
fn gen_type_checks(
    ctx: &FieldContext<'_>,
    skip_mut_checks: bool,
) -> Vec<proc_macro2::TokenStream> {
    let field_name = ctx.field_name;
    let underlying_ty = ctx.underlying_ty;
    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    let field_name_str = field_name.to_string();
    match &ctx.kind {
        FieldKind::Account { inner_ty } => {
            if !skip_mut_checks {
                checks.push(quote! {
                    quasar_lang::validation::check_account::<#inner_ty>(#field_name.to_account_view(), #field_name_str)?;
                });
            }
        }
        FieldKind::InterfaceAccount { inner_ty } => {
            if !skip_mut_checks {
                let owner = debug_checked(
                    &field_name_str,
                    quote! {
                        quasar_lang::accounts::interface_account::InterfaceAccount::<#inner_ty>::from_account_view(#field_name.to_account_view()).map(|_| ())
                    },
                    "Owner/data check failed for interface account '{}'",
                );
                checks.push(owner);
            }
        }
        FieldKind::Sysvar { inner_ty } => {
            checks.push(quote! {
                quasar_lang::validation::check_sysvar::<#inner_ty>(#field_name.to_account_view(), #field_name_str)?;
            });
        }
        FieldKind::Program { inner_ty } => {
            checks.push(quote! {
                quasar_lang::validation::check_program::<#inner_ty>(#field_name.to_account_view(), #field_name_str)?;
            });
        }
        FieldKind::Interface { inner_ty } => {
            checks.push(quote! {
                quasar_lang::validation::check_interface::<#inner_ty>(#field_name.to_account_view(), #field_name_str)?;
            });
        }
        FieldKind::SystemAccount => {
            let base_type = strip_generics(underlying_ty);
            checks.push(quote! {
                <#base_type as quasar_lang::checks::Owner>::check(#field_name.to_account_view())?;
            });
        }
        FieldKind::Signer | FieldKind::Other => {}
    }

    checks
}

/// Generate the field construction expression (dynamic / standard path).
fn gen_field_construct(
    ctx: &FieldContext<'_>,
) -> Result<proc_macro2::TokenStream, proc_macro::TokenStream> {
    let field_name = ctx.field_name;
    let effective_ty = ctx.effective_ty;

    let construct = |expr: proc_macro2::TokenStream| {
        if ctx.is_optional {
            quote! { #field_name: if quasar_lang::keys_eq(#field_name.address(), __program_id) { None } else { Some(#expr) } }
        } else {
            quote! { #field_name: #expr }
        }
    };

    if ctx.is_dynamic {
        if let FieldKind::Account { inner_ty } = &ctx.kind {
            let inner_base = strip_generics(inner_ty);
            Ok(construct(
                quote! { #inner_base::from_account_view(#field_name)? },
            ))
        } else {
            let base_type = strip_generics(effective_ty);
            Ok(quote! { #field_name: #base_type::from_account_view(#field_name)? })
        }
    } else if let Type::Reference(_) = effective_ty {
        Err(
            syn::Error::new_spanned(ctx.field, "Reference types are not supported")
                .to_compile_error()
                .into(),
        )
    } else {
        let base_type = strip_generics(effective_ty);
        Ok(construct(
            quote! { unsafe { core::ptr::read(#base_type::from_account_view_unchecked(#field_name)) } },
        ))
    }
}

/// Generate has_one, constraint, address, and has_one/PDA overlap warning
/// checks for a single field.
fn gen_validation_checks(ctx: &FieldContext<'_>) -> Vec<proc_macro2::TokenStream> {
    let field_name = ctx.field_name;
    let attrs = ctx.attrs;
    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    for (target, custom_error) in &attrs.has_ones {
        let error = match custom_error {
            Some(err) => quote! { #err.into() },
            None => quote! { QuasarError::HasOneMismatch.into() },
        };
        checks.push(quote! {
            quasar_lang::validation::check_has_one(
                &#field_name.#target,
                #target.to_account_view().address(),
                #error,
            )?;
        });
    }

    // --- has_one + PDA seed overlap warning ---
    // Warn when a has_one target also appears as a seed reference, since
    // the PDA derivation already validates the relationship.
    if let Some(ref seed_exprs) = attrs.seeds {
        for (target, _) in &attrs.has_ones {
            let target_str = target.to_string();
            let is_seed_ref = seed_exprs.iter().any(|expr| {
                contains_ident(expr, &target_str)
            });
            if is_seed_ref {
                // Emit compile-time deprecation warning. The const decl
                // triggers the warning; the usage is allow(deprecated) so
                // #![deny(deprecated)] won't hard-error.
                let warn_msg = format!(
                    "has_one = {} may be redundant: '{}' is already a PDA seed for '{}', \
                     so the derivation validates this relationship. Consider removing \
                     has_one for ~10 CU savings, or keep for defense-in-depth.",
                    target, target, field_name,
                );
                let warn_const = format_ident!(
                    "__WARN_REDUNDANT_HAS_ONE_{}_{}",
                    field_name.to_string().to_uppercase(),
                    target_str.to_uppercase(),
                );
                checks.push(quote! {
                    {
                        #[deprecated(note = #warn_msg)]
                        const #warn_const: () = ();
                        #[allow(deprecated)]
                        let _ = #warn_const;
                    }
                });
            }
        }
    }

    for (expr, custom_error) in &attrs.constraints {
        let error = match custom_error {
            Some(err) => quote! { #err.into() },
            None => quote! { QuasarError::ConstraintViolation.into() },
        };
        checks.push(quote! {
            quasar_lang::validation::check_constraint(#expr, #error)?;
        });
    }

    if let Some((addr_expr, custom_error)) = &attrs.address {
        let error = match custom_error {
            Some(err) => quote! { #err.into() },
            None => quote! { QuasarError::AddressMismatch.into() },
        };
        checks.push(quote! {
            quasar_lang::validation::check_address_match(
                #field_name.to_account_view().address(),
                &#addr_expr,
                #error,
            )?;
        });
    }

    checks
}

/// Handle close and sweep directives for a single field.
#[allow(clippy::too_many_arguments)]
fn gen_close_sweep(
    ctx: &FieldContext<'_>,
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_attrs: &[AccountFieldAttrs],
    close_fields: &mut Vec<CloseFieldInfo>,
    sweep_fields: &mut Vec<SweepFieldInfo>,
) -> Result<(), proc_macro::TokenStream> {
    let field_name = ctx.field_name;
    let attrs = ctx.attrs;

    if let Some(dest) = &attrs.close {
        let cpi_close = if ctx.kind.is_token_account() {
            let authority = attrs
                .token_authority
                .clone()
                .or_else(|| attrs.associated_token_authority.clone())
                .ok_or_else(|| -> proc_macro::TokenStream {
                    syn::Error::new_spanned(
                        field_name,
                        "#[account(close)] on token account types requires `token::authority`",
                    )
                    .to_compile_error()
                    .into()
                })?;
            let tp_field: Ident = ctx.token_program_for_token
                .cloned()
                .expect("token close requires a resolved token program field");
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

    if let Some(receiver) = &attrs.sweep {
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
        if !FieldKind::classify(strip_ref(&receiver_field.ty)).is_token_account() {
            return Err(syn::Error::new_spanned(
                receiver,
                "sweep target must be a token account (Account<Token>, Account<Token2022>, or \
                 InterfaceAccount<Token>)",
            )
            .to_compile_error()
            .into());
        }
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
        if let Some(auth_name) = &attrs.token_authority {
            let auth_field = fields.iter().find(|f| f.ident.as_ref() == Some(auth_name));
            if let Some(af) = auth_field {
                if !matches!(FieldKind::classify(strip_ref(&af.ty)), FieldKind::Signer) {
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

        let mint = attrs
            .token_mint
            .clone()
            .expect("token_mint must be set when sweep is configured");
        let authority = attrs
            .token_authority
            .clone()
            .expect("token_authority must be set when sweep is configured");
        let tp_field = ctx.token_program_for_token
            .cloned()
            .expect("token_program field must be present when sweep is configured");

        sweep_fields.push(SweepFieldInfo {
            field: field_name.clone(),
            receiver: receiver.clone(),
            mint,
            authority,
            token_program: tp_field,
        });
    }

    Ok(())
}

/// Generate raw `seeds = [...]` PDA codegen for a single field.
#[allow(clippy::too_many_arguments)]
fn gen_raw_pda_seeds(
    ctx: &FieldContext<'_>,
    seed_exprs: &[Expr],
    bumps_name: &Ident,
    bare_bump_pda_count: usize,
    bump_init_vars: &mut Vec<proc_macro2::TokenStream>,
    bump_struct_fields: &mut Vec<proc_macro2::TokenStream>,
    bump_struct_inits: &mut Vec<proc_macro2::TokenStream>,
    target_checks: &mut Vec<proc_macro2::TokenStream>,
    seeds_methods: &mut Vec<proc_macro2::TokenStream>,
) -> Result<(), proc_macro::TokenStream> {
    let field_name = ctx.field_name;
    let field_name_strings = ctx.field_name_strings;

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
        .map(|((ident, seed), expr)| match expr {
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
            _ => quote! {
                let #ident: &[u8] = #seed;
                if #ident.len() > 32 {
                    return Err(QuasarError::InvalidSeeds.into());
                }
            },
        })
        .collect();

    let addr_access = if ctx.is_init_field {
        quote! { *#field_name.address() }
    } else {
        quote! { *#field_name.to_account_view().address() }
    };

    let check = gen_bump_check(
        field_name,
        &ctx.attrs.bump,
        &bump_var,
        &seed_idents,
        &seed_len_checks,
        &addr_access,
        ctx.is_init_field,
        &ctx.kind,
        ctx.instruction_args,
        bare_bump_pda_count,
        "seeds = [...]",
        Some(seed_exprs),
    )?;
    target_checks.push(check);

    let method_name = format_ident!("{}_seeds", field_name);
    let seed_count = seed_exprs.len() + 1;
    let mut seed_elements: Vec<proc_macro2::TokenStream> = Vec::new();

    for expr in seed_exprs {
        if let Expr::Path(ep) = expr {
            if ep.qself.is_none() && ep.path.segments.len() == 1 {
                let ident = &ep.path.segments[0].ident;
                if field_name_strings.contains(&ident.to_string()) {
                    // Account key — live reference via the Accounts struct
                    seed_elements.push(
                        quote! { quasar_lang::cpi::Seed::from(self.#ident.to_account_view().address().as_ref()) },
                    );
                    continue;
                }
            }
        }
        seed_elements.push(quote! { quasar_lang::cpi::Seed::from((#expr) as &[u8]) });
    }

    seed_elements
        .push(quote! { quasar_lang::cpi::Seed::from(&bumps.#bump_arr_field as &[u8]) });

    seeds_methods.push(quote! {
        #[inline(always)]
        pub fn #method_name<'a>(&'a self, bumps: &'a #bumps_name) -> [quasar_lang::cpi::Seed<'a>; #seed_count] {
            [#(#seed_elements),*]
        }
    });

    Ok(())
}

/// Generate typed `seeds = Type::seeds(...)` PDA codegen for a single field.
#[allow(clippy::too_many_arguments)]
fn gen_typed_pda_seeds(
    ctx: &FieldContext<'_>,
    typed: &TypedSeeds,
    bumps_name: &Ident,
    bare_bump_pda_count: usize,
    bump_init_vars: &mut Vec<proc_macro2::TokenStream>,
    bump_struct_fields: &mut Vec<proc_macro2::TokenStream>,
    bump_struct_inits: &mut Vec<proc_macro2::TokenStream>,
    target_checks: &mut Vec<proc_macro2::TokenStream>,
    seeds_methods: &mut Vec<proc_macro2::TokenStream>,
    field_checks: &mut Vec<proc_macro2::TokenStream>,
    seed_addr_captures: &mut Vec<proc_macro2::TokenStream>,
) -> Result<(), proc_macro::TokenStream> {
    let field_name = ctx.field_name;
    let field_name_strings = ctx.field_name_strings;
    let instruction_args = ctx.instruction_args;
    let type_path = &typed.type_path;

    let bump_var = format_ident!("__bumps_{}", field_name);

    bump_init_vars.push(quote! { let mut #bump_var: u8 = 0; });
    bump_struct_fields.push(quote! { pub #field_name: u8 });
    bump_struct_inits.push(quote! { #field_name: #bump_var });

    let bump_arr_field = format_ident!("__{}_bump", field_name);
    bump_struct_fields.push(quote! { #bump_arr_field: [u8; 1] });
    bump_struct_inits.push(quote! { #bump_arr_field: [#bump_var] });

    // Build seed slices: prefix from type const + dynamic args
    let mut all_seed_slices: Vec<proc_macro2::TokenStream> =
        vec![quote! { <#type_path as quasar_lang::traits::HasSeeds>::SEED_PREFIX }];
    for arg in &typed.args {
        all_seed_slices.push(typed_seed_slice_expr(
            arg,
            field_name_strings,
            instruction_args,
        ));
    }

    // Arity check: number of args must match SEED_DYNAMIC_COUNT.
    let arg_count = typed.args.len();
    let type_name_str = quote!(#type_path).to_string();
    let expected_msg = format!(
        "{}::seeds() argument count mismatch (check #[seeds] definition on {})",
        type_name_str, type_name_str,
    );
    field_checks.push(quote! {
        const _: () = assert!(
            <#type_path as quasar_lang::traits::HasSeeds>::SEED_DYNAMIC_COUNT == #arg_count,
            #expected_msg,
        );
    });

    if all_seed_slices.len() > 15 {
        return Err(syn::Error::new_spanned(
            field_name,
            format!(
                "`{}` exceeds Solana's PDA seed limit: {} seeds provided, max is 16 \
                 including bump",
                field_name,
                all_seed_slices.len()
            ),
        )
        .to_compile_error()
        .into());
    }

    let seed_idents: Vec<Ident> = all_seed_slices
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("__seed_{}_{}", field_name, idx))
        .collect();

    let seed_len_checks: Vec<proc_macro2::TokenStream> = seed_idents
        .iter()
        .zip(all_seed_slices.iter())
        .map(|(ident, seed)| {
            quote! {
                let #ident: &[u8] = #seed;
                if #ident.len() > 32 {
                    return Err(QuasarError::InvalidSeeds.into());
                }
            }
        })
        .collect();

    let addr_access = if ctx.is_init_field {
        quote! { *#field_name.address() }
    } else {
        quote! { *#field_name.to_account_view().address() }
    };

    let check = gen_bump_check(
        field_name,
        &ctx.attrs.bump,
        &bump_var,
        &seed_idents,
        &seed_len_checks,
        &addr_access,
        ctx.is_init_field,
        &ctx.kind,
        instruction_args,
        bare_bump_pda_count,
        "seeds = Type::seeds(...)",
        None, // typed seeds can't be precomputed at macro time
    )?;
    target_checks.push(check);

    // CPI seed method — generated on the Accounts struct with a
    // bumps parameter. Account keys are referenced live via self,
    // field access seeds via self, and ix arg captures via bumps.
    let method_name = format_ident!("{}_seeds", field_name);
    // prefix + dynamic args + bump
    let total_seed_count = typed.args.len() + 2;
    let mut seed_elements: Vec<proc_macro2::TokenStream> = Vec::new();

    // Prefix seed
    seed_elements.push(
        quote! { quasar_lang::cpi::Seed::from(<#type_path as quasar_lang::traits::HasSeeds>::SEED_PREFIX) },
    );

    // Dynamic seed elements — live references for CPI
    for arg in &typed.args {
        if let Expr::Path(ep) = arg {
            if ep.qself.is_none() && ep.path.segments.len() == 1 {
                let ident = &ep.path.segments[0].ident;
                if field_name_strings.contains(&ident.to_string()) {
                    // Account key — live reference via Accounts struct
                    seed_elements.push(
                        quote! { quasar_lang::cpi::Seed::from(self.#ident.to_account_view().address().as_ref()) },
                    );
                    continue;
                }
            }
        }
        // Check if this is an instruction arg captured in Bumps.
        let mut captured = false;
        if let Expr::Path(ep) = arg {
            if ep.qself.is_none() && ep.path.segments.len() == 1 {
                let ident = &ep.path.segments[0].ident;
                if let Some(args) = instruction_args {
                    if let Some(ix_arg) = args.iter().find(|a| a.name == *ident) {
                        // Capture instruction arg as byte array in Bumps struct
                        let ix_bytes_field =
                            format_ident!("__seed_{}_{}", field_name, ident);
                        let capture_var =
                            format_ident!("__seed_ix_{}_{}", field_name, ident);
                        let ty = &ix_arg.ty;
                        let type_str = quote!(#ty).to_string().replace(' ', "");
                        match type_str.as_str() {
                            "u8" => {
                                seed_addr_captures
                                    .push(quote! { let #capture_var: [u8; 1] = [#ident]; });
                                bump_struct_fields
                                    .push(quote! { #ix_bytes_field: [u8; 1] });
                            }
                            "bool" => {
                                seed_addr_captures.push(
                                    quote! { let #capture_var: [u8; 1] = [#ident as u8]; },
                                );
                                bump_struct_fields
                                    .push(quote! { #ix_bytes_field: [u8; 1] });
                            }
                            "Address" | "Pubkey" => {
                                seed_addr_captures
                                    .push(quote! { let #capture_var = #ident; });
                                bump_struct_fields
                                    .push(quote! { #ix_bytes_field: Address });
                            }
                            _ => {
                                // Numeric types — store as le bytes array
                                seed_addr_captures.push(
                                    quote! { let #capture_var = #ident.to_le_bytes(); },
                                );
                                bump_struct_fields
                                    .push(quote! { #ix_bytes_field: [u8; core::mem::size_of::<#ty>()] });
                            }
                        }
                        bump_struct_inits.push(quote! { #ix_bytes_field: #capture_var });
                        // Reference ix arg bytes via bumps parameter
                        seed_elements.push(
                            quote! { quasar_lang::cpi::Seed::from(&bumps.#ix_bytes_field as &[u8]) },
                        );
                        captured = true;
                    }
                }
            }
        }
        if !captured {
            // Field access expressions (e.g. config.namespace) and other
            // expressions — reference live via self on the Accounts struct.
            // On sBPF (little-endian), in-memory representation is LE bytes,
            // so this is a zero-cost reference.
            let seed_expr =
                typed_seed_method_expr(arg, field_name_strings, instruction_args);
            seed_elements.push(quote! { quasar_lang::cpi::Seed::from(#seed_expr) });
        }
    }

    // Bump seed element — reference via bumps parameter
    seed_elements
        .push(quote! { quasar_lang::cpi::Seed::from(&bumps.#bump_arr_field as &[u8]) });

    seeds_methods.push(quote! {
        #[inline(always)]
        pub fn #method_name<'a>(&'a self, bumps: &'a #bumps_name) -> [quasar_lang::cpi::Seed<'a>; #total_seed_count] {
            [#(#seed_elements),*]
        }
    });

    Ok(())
}

/// Generate non-init ATA / token-account / mint validation checks.
fn gen_token_validation(ctx: &FieldContext<'_>) -> Vec<proc_macro2::TokenStream> {
    let field_name = ctx.field_name;
    let attrs = ctx.attrs;
    let effective_ty = ctx.effective_ty;
    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    if let (false, Some(mint_field), Some(auth_field)) = (
        ctx.is_init_field,
        attrs.associated_token_mint.as_ref(),
        attrs.associated_token_authority.as_ref(),
    ) {
        let token_program_addr = if let Some(tp) = &attrs.associated_token_token_program {
            quote! { #tp.to_account_view().address() }
        } else {
            resolve_token_program_addr(effective_ty, ctx.token_program_for_ata)
        };

        checks.push(quote! {
            quasar_spl::validate_ata(
                #field_name.to_account_view(),
                #auth_field.to_account_view().address(),
                #mint_field.to_account_view().address(),
                #token_program_addr,
            )?;
        });
    }

    if let (false, Some(mint_field), Some(auth_field)) = (
        ctx.is_init_field,
        attrs.token_mint.as_ref(),
        attrs.token_authority.as_ref(),
    ) {
        let token_program_addr =
            resolve_token_program_addr(effective_ty, ctx.token_program_for_token);
        checks.push(quote! {
            quasar_spl::validate_token_account(
                #field_name.to_account_view(),
                #mint_field.to_account_view().address(),
                #auth_field.to_account_view().address(),
                #token_program_addr,
            )?;
        });
    }

    if let (false, Some(decimals_expr), Some(auth_field)) = (
        ctx.is_init_field,
        attrs.mint_decimals.as_ref(),
        attrs.mint_init_authority.as_ref(),
    ) {
        let token_program_addr =
            resolve_token_program_addr(effective_ty, ctx.token_program_for_mint);
        let freeze_expr = if let Some(freeze_field) = &attrs.mint_freeze_authority {
            quote! { Some(#freeze_field.to_account_view().address()) }
        } else {
            quote! { None }
        };
        checks.push(quote! {
            quasar_spl::validate_mint(
                #field_name.to_account_view(),
                #auth_field.to_account_view().address(),
                (#decimals_expr) as u8,
                #freeze_expr,
                #token_program_addr,
            )?;
        });
    }

    checks
}

pub(crate) fn process_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_name_strings: &[String],
    instruction_args: &Option<Vec<InstructionArg>>,
    bumps_name: &Ident,
) -> Result<ProcessedFields, proc_macro::TokenStream> {
    let parsed: Vec<super::super::attrs::ParsedAttrs> = fields
        .iter()
        .map(parse_field_attrs)
        .collect::<syn::Result<Vec<_>>>()
        .map_err(|e| -> proc_macro::TokenStream { e.to_compile_error().into() })?;

    // Verify every directive variant has a known handler. This exhaustive
    // match is the compiler-enforced completeness guarantee — adding a new
    // AccountDirective variant without handling it here causes a compile error.
    for p in &parsed {
        verify_all_directives_mapped(&p.directives);
    }

    let field_attrs: Vec<AccountFieldAttrs> = parsed.into_iter().map(|p| p.attrs).collect();

    let bare_bump_pda_count = field_attrs
        .iter()
        .filter(|a| (a.seeds.is_some() || a.typed_seeds.is_some()) && matches!(a.bump, Some(None)))
        .count();

    let has_any_init = field_attrs.iter().any(|a| a.is_init || a.init_if_needed);
    let has_any_ata_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.associated_token_mint.is_some());
    let has_any_realloc = field_attrs.iter().any(|a| a.realloc.is_some());
    let has_any_metadata_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.metadata_name.is_some());
    let has_any_master_edition_init = field_attrs
        .iter()
        .any(|a| (a.is_init || a.init_if_needed) && a.master_edition_max_supply.is_some());

    let detected = DetectedFields::detect(fields, &field_attrs);

    let system_program_field = if has_any_init {
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
        Some(
            detected
                .update_authority
                .expect("update_authority field must be present for metadata/master_edition init"),
        )
    } else {
        None
    };

    let rent_field = if has_any_metadata_init || has_any_master_edition_init {
        Some(DetectedFields::require(
            detected.rent_sysvar,
            "`metadata::*` / `master_edition::*` requires a `Sysvar<Rent>` field",
        )?)
    } else {
        None
    };

    let mut field_constructs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut field_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_init_vars: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bump_struct_inits: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seeds_methods: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seed_addr_captures: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut init_pda_checks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut init_blocks: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut close_fields: Vec<CloseFieldInfo> = Vec::new();
    let mut sweep_fields: Vec<SweepFieldInfo> = Vec::new();
    let mut needs_rent = false;

    for (field, attrs) in fields.iter().zip(field_attrs.iter()) {
        let field_name = field
            .ident
            .as_ref()
            .expect("account field must have an identifier");

        let is_optional = extract_generic_inner_type(&field.ty, "Option").is_some();
        let effective_ty = extract_generic_inner_type(&field.ty, "Option").unwrap_or(&field.ty);
        let underlying_ty = strip_ref(effective_ty);
        let kind = FieldKind::classify(underlying_ty);
        let is_dynamic = kind.is_dynamic();

        validate_composition(field, field_name, attrs, &kind)?;

        let token_program_for_token = if attrs.token_mint.is_some()
            || attrs.sweep.is_some()
            || (attrs.close.is_some() && kind.is_token_account())
        {
            let requires_runtime_field = attrs.is_init
                || attrs.init_if_needed
                || attrs.sweep.is_some()
                || (attrs.close.is_some() && kind.is_token_account())
                || matches!(kind, FieldKind::InterfaceAccount { .. });
            resolve_token_program_field(
                fields,
                &detected,
                field_name,
                attrs.token_token_program.as_ref(),
                "token::token_program",
                "`token::*` on this field",
                if requires_runtime_field {
                    TokenProgramResolution::FallbackRequired
                } else {
                    TokenProgramResolution::ExplicitOnly
                },
            )?
        } else {
            None
        };

        let token_program_for_mint =
            if attrs.mint_decimals.is_some() || attrs.master_edition_max_supply.is_some() {
                let requires_runtime_field = attrs.is_init
                    || attrs.init_if_needed
                    || attrs.master_edition_max_supply.is_some()
                    || matches!(kind, FieldKind::InterfaceAccount { .. });
                resolve_token_program_field(
                    fields,
                    &detected,
                    field_name,
                    attrs.mint_token_program.as_ref(),
                    "mint::token_program",
                    "`mint::*` / `master_edition::*` on this field",
                    if requires_runtime_field {
                        TokenProgramResolution::FallbackRequired
                    } else {
                        TokenProgramResolution::ExplicitOnly
                    },
                )?
            } else {
                None
            };

        let token_program_for_ata = if attrs.associated_token_mint.is_some() {
            let requires_runtime_field = attrs.is_init
                || attrs.init_if_needed
                || matches!(kind, FieldKind::InterfaceAccount { .. });
            resolve_token_program_field(
                fields,
                &detected,
                field_name,
                attrs.associated_token_token_program.as_ref(),
                "associated_token::token_program",
                "`associated_token::*` on this field",
                if requires_runtime_field {
                    TokenProgramResolution::FallbackRequired
                } else {
                    TokenProgramResolution::ExplicitOnly
                },
            )?
        } else {
            None
        };

        let has_validate_call = attrs.token_mint.is_some()
            || attrs.associated_token_mint.is_some()
            || attrs.mint_decimals.is_some();
        let skip_mut_checks = attrs.is_init
            || (attrs.init_if_needed
                && matches!(
                    kind,
                    FieldKind::Account { .. } | FieldKind::InterfaceAccount { .. }
                ))
            || (has_validate_call
                && matches!(
                    kind,
                    FieldKind::Account { .. } | FieldKind::InterfaceAccount { .. }
                ))
            || (is_dynamic && matches!(kind, FieldKind::Account { .. }));

        let is_init_field = attrs.is_init || attrs.init_if_needed;

        let ctx = FieldContext {
            field,
            field_name,
            attrs,
            kind,
            effective_ty,
            underlying_ty,
            is_optional,
            is_init_field,
            is_dynamic,
            field_name_strings,
            instruction_args,
            token_program_for_token,
            token_program_for_mint,
            token_program_for_ata,
        };

        let mut evidence = FieldEvidence::default();

        let mut this_field_checks = gen_type_checks(&ctx, skip_mut_checks);
        if !skip_mut_checks && matches!(ctx.kind, FieldKind::Account { .. }) {
            evidence.owner = Some(OwnerEvidence::produced());
        }

        field_constructs.push(gen_field_construct(&ctx)?);

        this_field_checks.extend(gen_validation_checks(&ctx));

        gen_close_sweep(&ctx, fields, &field_attrs, &mut close_fields, &mut sweep_fields)?;

        // Reject using both seed syntaxes on the same field.
        if attrs.seeds.is_some() && attrs.typed_seeds.is_some() {
            return Err(syn::Error::new_spanned(
                field_name,
                "cannot use both `seeds = [...]` and `seeds = Type::seeds(...)` on the same field",
            )
            .to_compile_error()
            .into());
        }

        // Enforce: Account<T> with raw seeds = [...] must use typed seeds instead.
        // External account types (Mint, Token, etc.) with token:: or mint::
        // attributes are exempt since they are defined in external crates.
        if let FieldKind::Account { inner_ty } = &ctx.kind {
            if attrs.seeds.is_some()
                && attrs.typed_seeds.is_none()
                && attrs.token_mint.is_none()
                && attrs.mint_decimals.is_none()
                && attrs.associated_token_mint.is_none()
            {
                return Err(syn::Error::new_spanned(
                    field_name,
                    format!(
                        "raw `seeds = [...]` is not allowed on program accounts. Add \
                         `#[seeds(...)]` to `{}` and use `{}::seeds(...)` instead.",
                        quote::quote!(#inner_ty),
                        quote::quote!(#inner_ty),
                    ),
                )
                .to_compile_error()
                .into());
            }
        }

        if let Some(seed_exprs) = &attrs.seeds {
            let pda_target_checks = if is_init_field {
                &mut init_pda_checks
            } else {
                &mut this_field_checks
            };
            gen_raw_pda_seeds(
                &ctx,
                seed_exprs,
                bumps_name,
                bare_bump_pda_count,
                &mut bump_init_vars,
                &mut bump_struct_fields,
                &mut bump_struct_inits,
                pda_target_checks,
                &mut seeds_methods,
            )?;
            evidence.pda = Some(PdaEvidence::produced());
            evidence.bump = Some(BumpEvidence::produced());
        }

        // --- Typed seeds: seeds = Type::seeds(arg1, arg2, ...) ---
        if let Some(typed) = &attrs.typed_seeds {
            let pda_target_checks = if is_init_field {
                &mut init_pda_checks
            } else {
                &mut this_field_checks
            };
            gen_typed_pda_seeds(
                &ctx,
                typed,
                bumps_name,
                bare_bump_pda_count,
                &mut bump_init_vars,
                &mut bump_struct_fields,
                &mut bump_struct_inits,
                pda_target_checks,
                &mut seeds_methods,
                &mut field_checks,
                &mut seed_addr_captures,
            )?;
            evidence.pda = Some(PdaEvidence::produced());
            evidence.bump = Some(BumpEvidence::produced());
        }

        if is_init_field {
            let init_ctx = init::InitContext {
                payer: payer_field.expect("payer field must be present for init"),
                system_program: system_program_field
                    .expect("system_program field must be present for init"),
                token_program: if attrs.token_mint.is_some() || attrs.sweep.is_some() {
                    token_program_for_token
                } else if attrs.associated_token_mint.is_some() {
                    token_program_for_ata
                } else if attrs.mint_decimals.is_some() || attrs.master_edition_max_supply.is_some()
                {
                    token_program_for_mint
                } else {
                    None
                },
                ata_program: ata_program_field,
                metadata_account: metadata_account_field,
                master_edition_account: master_edition_account_field,
                metadata_program: metadata_program_field,
                mint_authority: mint_authority_field,
                update_authority: update_authority_field,
                rent: rent_field,
                field_name_strings,
                instruction_args,
            };

            if let Some(result) = init::gen_init_block(field_name, attrs, effective_ty, &init_ctx)?
            {
                init_blocks.push(result.tokens);
                needs_rent |= result.uses_rent;
                evidence.init = Some(InitEvidence::produced());
            }

            if let Some(block) = init::gen_metadata_init(field_name, attrs, &init_ctx) {
                init_blocks.push(block);
            }

            if let Some(block) = init::gen_master_edition_init(field_name, attrs, &init_ctx) {
                init_blocks.push(block);
            }
        }

        this_field_checks.extend(gen_token_validation(&ctx));

        // Validate that every declared constraint produced its evidence.
        let has_seeds = attrs.seeds.is_some() || attrs.typed_seeds.is_some();
        evidence.validate(&field_name.to_string(), has_seeds, is_init_field);

        if let Some(realloc_expr) = &attrs.realloc {
            let realloc_pay = realloc_payer_field.expect("payer field must be present for realloc");
            needs_rent = true;

            init_blocks.push(quote! {
                {
                    let __realloc_space = (#realloc_expr) as usize;
                    quasar_lang::accounts::realloc_account_raw(
                        #field_name, __realloc_space, #realloc_pay, __rent_lpb, __rent_threshold
                    )?;
                }
            });
        }

        if !this_field_checks.is_empty() {
            if is_optional {
                field_checks.push(quote! {
                    if let Some(ref #field_name) = #field_name {
                        #(#this_field_checks)*
                    }
                });
            } else {
                field_checks.extend(this_field_checks);
            }
        }
    }

    Ok(ProcessedFields {
        field_constructs,
        field_checks,
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
        rent_sysvar_field: detected.rent_sysvar.cloned(),
    })
}
