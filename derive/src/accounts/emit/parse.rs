use {
    super::super::{
        resolve::{BumpSyntax, FieldSemantics, FieldShape, PdaConstraint, UserCheckKind},
        syntax::{
            render_seed_expr, seeds_to_emit_nodes, AccountWrapperKind, SeedEmitNode,
            SeedRenderContext,
        },
    },
    crate::helpers::strip_generics,
    quote::{format_ident, quote},
    std::collections::BTreeMap,
};

pub(crate) fn emit_parse_body(
    semantics: &[FieldSemantics],
    cx: &super::EmitCx,
) -> syn::Result<proc_macro2::TokenStream> {
    let rent_fetch = emit_rent_fetch(semantics);
    let init_stmts = super::init::emit_init_stmts(semantics)?;
    let realloc_stmts = super::init::emit_realloc_steps(semantics)?;
    let construct_stmts = emit_construct_steps(semantics);
    let check_stmts = emit_check_blocks(semantics);
    let bump_vars = emit_bump_vars(semantics);
    let bump_init = emit_bump_init(semantics, &cx.bumps_name);
    let field_names = field_idents(semantics);

    if !check_stmts.is_empty() {
        Ok(quote! {
            #bump_vars
            #rent_fetch
            #(#init_stmts)*
            #(#realloc_stmts)*
            let result = Self { #(#construct_stmts,)* };
            {
                let Self { #(ref #field_names,)* } = result;
                #(#check_stmts)*
            }
            Ok((result, #bump_init))
        })
    } else {
        Ok(quote! {
            #bump_vars
            #rent_fetch
            #(#init_stmts)*
            #(#realloc_stmts)*
            Ok((Self { #(#construct_stmts,)* }, #bump_init))
        })
    }
}

pub(crate) fn emit_bump_struct_def(
    semantics: &[FieldSemantics],
    cx: &super::EmitCx,
) -> proc_macro2::TokenStream {
    emit_bump_struct(semantics, &cx.bumps_name)
}

pub(crate) fn emit_seed_methods(
    semantics: &[FieldSemantics],
    cx: &super::EmitCx,
) -> proc_macro2::TokenStream {
    emit_seed_methods_impl(semantics, &cx.bumps_name)
}

fn emit_rent_fetch(semantics: &[FieldSemantics]) -> proc_macro2::TokenStream {
    if !semantics.iter().any(FieldSemantics::needs_rent) {
        return quote! {};
    }

    match semantics
        .iter()
        .filter(|sem| sem.needs_rent())
        .find_map(|sem| sem.support.rent_sysvar.as_ref())
    {
        Some(rent_field) => quote! {
            let __shared_rent = unsafe {
                core::clone::Clone::clone(
                    <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::from_bytes_unchecked(
                        #rent_field.borrow_unchecked()
                    )
                )
            };
        },
        None => quote! {
            let __shared_rent = <quasar_lang::sysvars::rent::Rent as quasar_lang::sysvars::Sysvar>::get()?;
        },
    }
}

fn field_idents(semantics: &[FieldSemantics]) -> Vec<&syn::Ident> {
    semantics.iter().map(|sem| &sem.core.ident).collect()
}

fn emit_construct_steps(semantics: &[FieldSemantics]) -> Vec<proc_macro2::TokenStream> {
    semantics.iter().map(emit_one_construct).collect()
}

fn emit_one_construct(sem: &FieldSemantics) -> proc_macro2::TokenStream {
    let ident = &sem.core.ident;
    let expr = emit_inner_expr(sem);
    if sem.core.optional {
        quote! {
            #ident: if quasar_lang::keys_eq(#ident.address(), __program_id) { None } else { Some(#expr) }
        }
    } else {
        quote! { #ident: #expr }
    }
}

fn emit_inner_expr(sem: &FieldSemantics) -> proc_macro2::TokenStream {
    let ident = &sem.core.ident;
    let ty = &sem.core.effective_ty;
    let skip_checks =
        sem.has_init() && (sem.token.is_some() || sem.ata.is_some() || sem.mint.is_some());

    if matches!(sem.core.shape, FieldShape::Composite) {
        quote! { #ident }
    } else if sem.core.dynamic {
        let inner_ty = match &sem.core.shape {
            FieldShape::Account { inner_ty } => inner_ty,
            _ => &sem.core.effective_ty,
        };
        let base = strip_generics(inner_ty);
        quote! { #base::from_account_view(#ident)? }
    } else if skip_checks {
        quote! {
            unsafe {
                core::ptr::read(
                    <#ty as quasar_lang::account_load::AccountLoad>::from_view_unchecked(#ident)
                )
            }
        }
    } else {
        let field_name_str = ident.to_string();
        quote! {
            <#ty as quasar_lang::account_load::AccountLoad>::load(#ident, #field_name_str)?
        }
    }
}

fn emit_check_blocks(semantics: &[FieldSemantics]) -> Vec<proc_macro2::TokenStream> {
    semantics
        .iter()
        .map(|sem| emit_one_check_block(sem, semantics))
        .filter(|ts| !ts.is_empty())
        .collect()
}

fn emit_one_check_block(
    sem: &FieldSemantics,
    all_semantics: &[FieldSemantics],
) -> proc_macro2::TokenStream {
    let field_ident = &sem.core.ident;
    let mut stmts = Vec::new();

    for uc in &sem.user_checks {
        match &uc.kind {
            UserCheckKind::HasOne { target } => {
                let err = match &uc.error {
                    Some(e) => quote! { #e.into() },
                    None => quote! { QuasarError::HasOneMismatch.into() },
                };
                let field_name_str = field_ident.to_string();
                let target_str = target.to_string();
                stmts.push(quote! {
                    #[cfg(feature = "debug")]
                    if !quasar_lang::keys_eq(&#field_ident.#target, #target.to_account_view().address()) {
                        quasar_lang::prelude::log(concat!(
                            "has_one mismatch: ", #field_name_str, ".", #target_str,
                            " != ", #target_str, ".address()"
                        ));
                    }
                    quasar_lang::validation::check_address_match(
                        &#field_ident.#target,
                        #target.to_account_view().address(),
                        #err,
                    )?;
                });
            }
            UserCheckKind::Constraint { expr } => {
                let err = match &uc.error {
                    Some(e) => quote! { #e.into() },
                    None => quote! { QuasarError::ConstraintViolation.into() },
                };
                stmts.push(quote! {
                    quasar_lang::validation::check_constraint(#expr, #err)?;
                });
            }
            UserCheckKind::Address { expr } => {
                let err = match &uc.error {
                    Some(e) => quote! { #e.into() },
                    None => quote! { QuasarError::AddressMismatch.into() },
                };
                stmts.push(quote! {
                    quasar_lang::validation::check_address_match(
                        #field_ident.to_account_view().address(),
                        &#expr,
                        #err,
                    )?;
                });
            }
        }
    }

    if !sem.has_init() {
        if let Some(pda) = &sem.pda {
            stmts.push(emit_pda_check(sem, field_ident, pda, all_semantics));
        }
        if let Some(token_check) = super::init::emit_non_init_check(sem) {
            stmts.push(token_check);
        }
    }

    if stmts.is_empty() {
        quote! {}
    } else if sem.core.optional {
        quote! {
            if let Some(ref #field_ident) = #field_ident {
                #(#stmts)*
            }
        }
    } else {
        quote! { #(#stmts)* }
    }
}

fn emit_pda_check(
    sem: &FieldSemantics,
    field: &syn::Ident,
    pda: &PdaConstraint,
    all_semantics: &[FieldSemantics],
) -> proc_macro2::TokenStream {
    let bump_var = format_ident!("__bumps_{}", field);
    let bindings = emit_seed_bindings(field, pda, all_semantics, SeedRenderContext::Parse, "seed");
    let seed_lets = bindings.seed_lets;
    let seed_array_name = format_ident!("__pda_seeds_{}", field);
    let explicit_bump_name = format_ident!("__bump_val_{}", field);
    let addr_access = quote! { #field.to_account_view().address() };
    let literal_seeds = detect_literal_seeds(pda, all_semantics);
    let bump_assign = emit_pda_bump_assignment(
        field,
        pda,
        &bindings.seed_idents,
        PdaBumpAssignment {
            bump_var: &bump_var,
            addr_expr: &addr_access,
            seed_array_name: &seed_array_name,
            explicit_bump_name: &explicit_bump_name,
            bare_mode: if sem.core.shape.supports_existing_pda_fast_path() {
                PdaBareMode::KnownAddress
            } else {
                PdaBareMode::DeriveExpected
            },
            log_failure: true,
            literal_seeds,
        },
    );

    quote! {
        {
            #(#seed_lets)*
            #bump_assign
        }
    }
}

fn emit_bump_vars(semantics: &[FieldSemantics]) -> proc_macro2::TokenStream {
    let vars: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.pda.is_some())
        .map(|sem| {
            let var = format_ident!("__bumps_{}", sem.core.ident);
            quote! { let mut #var: u8 = 0; }
        })
        .collect();

    if vars.is_empty() {
        quote! {}
    } else {
        quote! { #(#vars)* }
    }
}

fn emit_bump_struct(
    semantics: &[FieldSemantics],
    bumps_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let fields: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.pda.is_some())
        .flat_map(|sem| {
            let name = &sem.core.ident;
            let arr_name = format_ident!("__{}_bump", name);
            vec![quote! { pub #name: u8 }, quote! { pub #arr_name: [u8; 1] }]
        })
        .collect();

    if fields.is_empty() {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name; }
    } else {
        quote! { #[derive(Copy, Clone)] pub struct #bumps_name { #(#fields,)* } }
    }
}

fn emit_bump_init(
    semantics: &[FieldSemantics],
    bumps_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let inits: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter(|sem| sem.pda.is_some())
        .flat_map(|sem| {
            let name = &sem.core.ident;
            let var = format_ident!("__bumps_{}", name);
            let arr_name = format_ident!("__{}_bump", name);
            vec![quote! { #name: #var }, quote! { #arr_name: [#var] }]
        })
        .collect();

    if inits.is_empty() {
        quote! { #bumps_name }
    } else {
        quote! { #bumps_name { #(#inits,)* } }
    }
}

fn emit_seed_methods_impl(
    semantics: &[FieldSemantics],
    bumps_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let methods: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter_map(|sem| {
            let pda = sem.pda.as_ref()?;
            let seeds = seeds_to_emit_nodes(&pda.source, semantics);
            if seeds.iter().any(|s| matches!(s, SeedEmitNode::InstructionArg { .. })) {
                return None;
            }

            let field_ident = &sem.core.ident;
            let method_name = format_ident!("{}_seeds", field_ident);
            let bump_arr_field = format_ident!("__{}_bump", field_ident);

            let mut seed_elements: Vec<proc_macro2::TokenStream> = seeds
                .iter()
                .map(|node| {
                    let bytes = render_seed_expr(node, SeedRenderContext::Method);
                    quote! { quasar_lang::cpi::Seed::from(#bytes) }
                })
                .collect();

            seed_elements
                .push(quote! { quasar_lang::cpi::Seed::from(&bumps.#bump_arr_field as &[u8]) });

            let seed_count = seed_elements.len();
            Some(quote! {
                #[inline(always)]
                pub fn #method_name<'a>(&'a self, bumps: &'a #bumps_name) -> [quasar_lang::cpi::Seed<'a>; #seed_count] {
                    [#(#seed_elements),*]
                }
            })
        })
        .collect();

    quote! { #(#methods)* }
}

pub(super) struct SeedBindingParts {
    pub(super) seed_idents: Vec<syn::Ident>,
    pub(super) seed_lets: Vec<proc_macro2::TokenStream>,
}

#[derive(Clone, Copy)]
pub(super) enum PdaBareMode {
    KnownAddress,
    DeriveExpected,
}

pub(super) struct PdaBumpAssignment<'a> {
    pub(super) bump_var: &'a syn::Ident,
    pub(super) addr_expr: &'a proc_macro2::TokenStream,
    pub(super) seed_array_name: &'a syn::Ident,
    pub(super) explicit_bump_name: &'a syn::Ident,
    pub(super) bare_mode: PdaBareMode,
    pub(super) log_failure: bool,
    /// Raw bytes when every seed is a byte literal — enables the const-PDA
    /// fast path in `emit_pda_bump_assignment`.
    pub(super) literal_seeds: Option<Vec<Vec<u8>>>,
}

/// Returns the raw seed bytes when every seed is a compile-time byte
/// literal; `None` if any seed references a field, an instruction arg, or
/// any other non-literal expression.
pub(super) fn detect_literal_seeds(
    pda: &PdaConstraint,
    all_semantics: &[FieldSemantics],
) -> Option<Vec<Vec<u8>>> {
    seeds_to_emit_nodes(&pda.source, all_semantics)
        .iter()
        .map(|n| match n {
            SeedEmitNode::Literal(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn emit_seed_bindings(
    field: &syn::Ident,
    pda: &PdaConstraint,
    all_semantics: &[FieldSemantics],
    ctx: SeedRenderContext,
    name_prefix: &str,
) -> SeedBindingParts {
    let seeds = seeds_to_emit_nodes(&pda.source, all_semantics);
    emit_seed_bindings_from_nodes(field, &seeds, ctx, name_prefix)
}

fn emit_seed_bindings_from_nodes(
    field: &syn::Ident,
    seeds: &[SeedEmitNode],
    ctx: SeedRenderContext,
    name_prefix: &str,
) -> SeedBindingParts {
    let mut rooted_init_bindings = BTreeMap::<String, syn::Ident>::new();
    let mut root_lets = Vec::new();

    if matches!(ctx, SeedRenderContext::Init) {
        for node in seeds {
            let SeedEmitNode::FieldRootedExpr {
                root_ident,
                inner_ty: Some(inner_ty),
                wrapper_kind,
                ..
            } = node
            else {
                continue;
            };

            let key = root_ident.to_string();
            if rooted_init_bindings.contains_key(&key) {
                continue;
            }

            let binding_ident = format_ident!("__{}_{}_root_{}", name_prefix, field, root_ident);
            let typed_cast = emit_root_wrapper_cast(root_ident, inner_ty, wrapper_kind.as_ref());
            root_lets.push(quote! { let #binding_ident = unsafe { #typed_cast }; });
            rooted_init_bindings.insert(key, binding_ident);
        }
    }

    let seed_idents: Vec<syn::Ident> = seeds
        .iter()
        .enumerate()
        .map(|(i, _)| format_ident!("__{}_{}_{}", name_prefix, field, i))
        .collect();

    let seed_lets: Vec<proc_macro2::TokenStream> = seed_idents
        .iter()
        .zip(seeds.iter())
        .map(|(ident, node)| {
            let expr = match (ctx, node) {
                (
                    SeedRenderContext::Init,
                    SeedEmitNode::FieldRootedExpr {
                        expr,
                        root_ident,
                        inner_ty: Some(_),
                        ..
                    },
                ) => {
                    let binding_ident = &rooted_init_bindings[&root_ident.to_string()];
                    render_expr_with_bound_root(expr, root_ident, binding_ident)
                }
                _ => render_seed_expr(node, ctx),
            };
            quote! { let #ident: &[u8] = #expr; }
        })
        .collect();

    SeedBindingParts {
        seed_idents,
        seed_lets: root_lets.into_iter().chain(seed_lets).collect(),
    }
}

fn emit_root_wrapper_cast(
    root_ident: &syn::Ident,
    inner_ty: &syn::Type,
    wrapper_kind: Option<&AccountWrapperKind>,
) -> proc_macro2::TokenStream {
    let base_ty = strip_generics(inner_ty);
    match wrapper_kind {
        Some(AccountWrapperKind::InterfaceAccount) => {
            quote! {
                quasar_lang::accounts::interface_account::InterfaceAccount::<#base_ty>::from_account_view_unchecked(#root_ident)
            }
        }
        _ => {
            quote! {
                quasar_lang::accounts::account::Account::<#base_ty>::from_account_view_unchecked(#root_ident)
            }
        }
    }
}

fn render_expr_with_bound_root(
    expr: &syn::Expr,
    root_ident: &syn::Ident,
    binding_ident: &syn::Ident,
) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
            let ident = &ep.path.segments[0].ident;
            if ident == root_ident {
                quote! { #binding_ident }
            } else {
                quote! { #expr }
            }
        }
        syn::Expr::Field(field_expr) => {
            let base = render_expr_with_bound_root(&field_expr.base, root_ident, binding_ident);
            let member = &field_expr.member;
            quote! { (#base).#member }
        }
        syn::Expr::Paren(paren_expr) => {
            let inner = render_expr_with_bound_root(&paren_expr.expr, root_ident, binding_ident);
            quote! { (#inner) }
        }
        syn::Expr::MethodCall(method_call) => {
            let receiver =
                render_expr_with_bound_root(&method_call.receiver, root_ident, binding_ident);
            let method = &method_call.method;
            let turbofish = &method_call.turbofish;
            let args: Vec<_> = method_call.args.iter().collect();
            quote! { (#receiver).#method #turbofish ( #(#args),* ) }
        }
        syn::Expr::Reference(reference_expr) => {
            let inner =
                render_expr_with_bound_root(&reference_expr.expr, root_ident, binding_ident);
            let mutability = &reference_expr.mutability;
            quote! { &#mutability (#inner) }
        }
        _ => quote! { #expr },
    }
}

pub(super) fn emit_pda_bump_assignment(
    field: &syn::Ident,
    pda: &PdaConstraint,
    seed_idents: &[syn::Ident],
    assignment: PdaBumpAssignment<'_>,
) -> proc_macro2::TokenStream {
    let PdaBumpAssignment {
        bump_var,
        addr_expr,
        seed_array_name,
        explicit_bump_name,
        bare_mode,
        log_failure,
        literal_seeds,
    } = assignment;

    // Const-PDA fast path: when every seed is a compile-time byte literal
    // and the bump is bare, evaluate the PDA at compile time via
    // `find_program_address_const` and emit a single `keys_eq` at runtime —
    // skipping ~300 CU of `find_bump_for_address` / `based_try_find_program_address`.
    if let (Some(bytes), Some(BumpSyntax::Bare) | None) = (literal_seeds.as_ref(), &pda.bump) {
        let invalid_pda_error = emit_invalid_pda_error_expr(field, log_failure);
        let seed_bytes = bytes.iter().map(|seed| {
            let b = seed.iter().map(|b| quote! { #b });
            quote! { &[#(#b),*] as &[u8] }
        });
        return quote! {
            const __PDA_CONST: (quasar_lang::prelude::Address, u8) =
                quasar_lang::pda::find_program_address_const(
                    &[#(#seed_bytes),*],
                    &crate::ID,
                );
            if !quasar_lang::keys_eq(#addr_expr, &__PDA_CONST.0) {
                return Err({ #invalid_pda_error });
            }
            #bump_var = __PDA_CONST.1;
        };
    }

    match &pda.bump {
        Some(BumpSyntax::Explicit(expr)) => match bare_mode {
            PdaBareMode::KnownAddress => {
                let failure = emit_failure_log(field, log_failure);
                quote! {
                    let #explicit_bump_name: u8 = #expr;
                    let __bump_ref: &[u8] = &[#explicit_bump_name];
                    let #seed_array_name = [#(#seed_idents,)* __bump_ref];
                    quasar_lang::pda::verify_program_address(&#seed_array_name, __program_id, #addr_expr)
                        .map_err(|__e| {
                            #failure
                            __e
                        })?;
                    #bump_var = #explicit_bump_name;
                }
            }
            PdaBareMode::DeriveExpected => {
                let invalid_pda_error = emit_invalid_pda_error_expr(field, log_failure);
                quote! {
                    let #explicit_bump_name: u8 = #expr;
                    let #seed_array_name = [#(#seed_idents),*];
                    let (__expected, __derived_bump) =
                        quasar_lang::pda::based_try_find_program_address(&#seed_array_name, __program_id)?;
                    if !quasar_lang::keys_eq(#addr_expr, &__expected) || __derived_bump != #explicit_bump_name {
                        return Err({ #invalid_pda_error });
                    }
                    #bump_var = #explicit_bump_name;
                }
            }
        },
        Some(BumpSyntax::Bare) | None => {
            let invalid_pda_error = emit_invalid_pda_error_expr(field, log_failure);
            match bare_mode {
                PdaBareMode::KnownAddress => quote! {
                    let #seed_array_name = [#(#seed_idents),*];
                    #bump_var = quasar_lang::pda::find_bump_for_address(
                        &#seed_array_name,
                        __program_id,
                        #addr_expr,
                    ).map_err(|_| { #invalid_pda_error })?;
                },
                PdaBareMode::DeriveExpected => quote! {
                    let #seed_array_name = [#(#seed_idents),*];
                    let (__expected, __derived_bump) =
                        quasar_lang::pda::based_try_find_program_address(&#seed_array_name, __program_id)?;
                    if !quasar_lang::keys_eq(#addr_expr, &__expected) {
                        return Err({ #invalid_pda_error });
                    }
                    #bump_var = __derived_bump;
                },
            }
        }
    }
}

fn emit_failure_log(field: &syn::Ident, enabled: bool) -> proc_macro2::TokenStream {
    if enabled {
        quote! {
            #[cfg(feature = "debug")]
            quasar_lang::prelude::log(concat!(
                "Account '", stringify!(#field),
                "': PDA verification failed"
            ));
        }
    } else {
        quote! {}
    }
}

fn emit_invalid_pda_error_expr(field: &syn::Ident, log_failure: bool) -> proc_macro2::TokenStream {
    let log = emit_failure_log(field, log_failure);
    quote! {
        #log
        quasar_lang::prelude::ProgramError::from(QuasarError::InvalidPda)
    }
}
