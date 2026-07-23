//! `#[derive(Accounts)]`: protocol-neutral accounts derive macro.
//!
//! Pipeline:
//!
//! ```text
//! syntax   -> parse raw #[account(...)] directives
//! lower    -> turn parsed directives into FieldSemantics
//! rules    -> validate structural invariants (no protocol knowledge)
//! planner  -> schedule protocol-neutral phase candidates
//! emit     -> generate Rust code from the plan
//! ```
//!
//! Protocol crates own behavior. The derive never knows what `token`, `mint`,
//! `metadata`, etc. mean. Every behavior group is lowered to the same shape:
//! `path::Args::builder()` + `<path::Behavior as AccountBehavior<T>>`.
//!
//! See `quasar_lang::account_behavior::AccountBehavior` for the plugin
//! contract. The compiler boundary is also enforced by the dependency graph:
//! this crate does not depend on protocol crates such as `quasar-spl`.

pub(crate) mod emit;
pub(crate) mod resolve;
mod syntax;

pub(crate) use syntax::{parse_struct_instruction_args, InstructionArg};
use {
    crate::helpers::strip_generics,
    emit::entry::build_accounts_plan,
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_quote, Data, DeriveInput, Fields, GenericParam, Type},
};

pub(crate) fn derive_accounts(input: TokenStream) -> TokenStream {
    derive_accounts_inner(input.into()).into()
}

pub(crate) fn derive_accounts_inner(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };
    let name = &input.ident;
    let bumps_name = format_ident!("{}Bumps", name);

    // Only lifetime generics supported.
    if let Some(param) = input
        .generics
        .params
        .iter()
        .find(|param| !matches!(param, GenericParam::Lifetime(_)))
    {
        let message = match param {
            GenericParam::Type(_) => {
                "#[derive(Accounts)] only supports lifetime parameters; type parameters are not \
                 supported"
            }
            GenericParam::Const(_) => {
                "#[derive(Accounts)] only supports lifetime parameters; const parameters are not \
                 supported"
            }
            GenericParam::Lifetime(_) => "",
        };
        return syn::Error::new_spanned(param, message).to_compile_error();
    }
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let impl_generics_ts = quote! { #impl_generics };
    let ty_generics_ts = quote! { #ty_generics };
    let where_clause_ts = quote! { #where_clause };

    let mut parse_generics = input.generics.clone();
    parse_generics.params.push(parse_quote!('input));
    {
        let parse_where = parse_generics.make_where_clause();
        for lifetime in input.generics.lifetimes() {
            let lifetime = &lifetime.lifetime;
            parse_where
                .predicates
                .push(syn::parse_quote!('input: #lifetime));
        }
    }
    let (parse_impl_generics, _, parse_where_clause) = parse_generics.split_for_impl();
    let parse_impl_generics_ts = quote! { #parse_impl_generics };
    let parse_where_clause_ts = quote! { #parse_where_clause };

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "Accounts can only be derived for structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Accounts can only be derived for structs")
                .to_compile_error();
        }
    };

    let instruction_args = match parse_struct_instruction_args(&input) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    // --- Pipeline: syntax -> resolve -> plan -> emit ---

    let semantics =
        match resolve::lower_semantics(fields, instruction_args.as_deref().unwrap_or(&[])) {
            Ok(semantics) => semantics,
            Err(e) => return e.to_compile_error(),
        };

    let typed_plan = match resolve::planner::build_plan(
        &semantics,
        instruction_args.as_deref().unwrap_or(&[]),
        instruction_args.is_some(),
    ) {
        Ok(plan) => plan,
        Err(e) => return e.to_compile_error(),
    };

    let emit_cx = emit::EmitCx {
        bumps_name: bumps_name.clone(),
    };

    let accounts_plan = build_accounts_plan(&typed_plan, &emit_cx);
    let emit::entry::AccountsPlan {
        parse_steps,
        count_expr,
        parse_body,
        direct_parse_body,
    } = accounts_plan;

    // Instruction arg extraction: emitted ONCE as `Self::__extract_ix_args` and
    // called (destructured) from each splice site.
    let ix_args_slice = instruction_args.as_deref().unwrap_or(&[]);
    let ix_arg_extraction_fn = emit::ix_args::emit_extract_ix_args_fn(ix_args_slice);
    let ix_arg_extraction_call = emit::ix_args::emit_extract_ix_args_call(ix_args_slice);

    let bumps_struct = emit::parse::emit_bump_struct_def(&typed_plan.fields, &emit_cx);
    let signer_helpers_impl = emit_signer_helpers_impl(SignerHelpersCtx {
        name,
        bumps_name: &bumps_name,
        plan: &typed_plan,
        impl_generics: &impl_generics_ts,
        ty_generics: &ty_generics_ts,
        where_clause: &where_clause_ts,
        ix_arg_extraction: &ix_arg_extraction_call,
        has_instruction_args: typed_plan.has_instruction_args,
    });
    let epilogue_method = emit::parse::emit_epilogue(&typed_plan, &ix_arg_extraction_call);
    let has_epilogue_expr = emit::parse::emit_has_epilogue_typed(&typed_plan);

    let client_macro =
        crate::client_macro::generate_accounts_macro(name, &input.generics, &typed_plan);
    let signer_meta_impl = emit_account_signer_constants(
        name,
        &typed_plan,
        &impl_generics_ts,
        &ty_generics_ts,
        &where_clause_ts,
    );
    let fixed_address_impl = emit_fixed_address_constants(
        name,
        &typed_plan,
        &impl_generics_ts,
        &ty_generics_ts,
        &where_clause_ts,
    );
    let pda_address_impl = emit_pda_address_fns(
        name,
        &typed_plan,
        &impl_generics_ts,
        &ty_generics_ts,
        &where_clause_ts,
    );

    // IDL accounts meta fragment (feature-gated behind `idl-build`)
    let idl_accounts_meta = emit_idl_accounts_meta(name, &typed_plan);
    let idl_validation_meta = emit_idl_validation_meta(name, &typed_plan);

    // Typed `EventCpi` impl (only for structs with an event-authority field).
    // Computed before `emit_accounts_output` moves the generics. A missing
    // program field yields a `compile_error!` that is appended to (not
    // substituted for) the account impls, so the single spanned message
    // surfaces without an E0277 cascade from a struct left without impls.
    let event_cpi_impl = emit_event_cpi_impl(
        name,
        &typed_plan,
        &impl_generics_ts,
        &ty_generics_ts,
        &where_clause_ts,
    );

    let main_output = emit::emit_accounts_output(emit::AccountsOutput {
        name,
        bumps_name: &bumps_name,
        impl_generics: impl_generics_ts,
        ty_generics: ty_generics_ts,
        where_clause: where_clause_ts,
        parse_impl_generics: parse_impl_generics_ts,
        parse_where_clause: parse_where_clause_ts,
        count_expr,
        needs_event_cpi_expr: emit_needs_event_cpi_expr(&typed_plan),
        parse_steps,
        parse_body,
        direct_parse_body,
        bumps_struct,
        signer_helpers_impl,
        epilogue_method,
        has_epilogue_expr,
        client_macro,
        ix_arg_extraction: ix_arg_extraction_call,
        extract_ix_args_fn: ix_arg_extraction_fn,
        assert_builder_fn: emit::typed_emit::emit_assert_builder_fn(
            typed_plan.fields.iter().any(|fp| !fp.behaviors.is_empty()),
        ),
    });

    quote::quote! {
        #main_output
        #signer_meta_impl
        #fixed_address_impl
        #pda_address_impl
        #idl_accounts_meta
        #idl_validation_meta
        #event_cpi_impl
    }
}

/// Resolve behavior-defined signer requirements where the behavior paths and
/// account types are in scope. The exported client macro can then read the
/// values through the accounts type without leaking those local paths into the
/// program module where that macro is invoked.
fn emit_account_signer_constants(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
    impl_generics: &proc_macro2::TokenStream,
    ty_generics: &proc_macro2::TokenStream,
    where_clause: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if !plan.fields.iter().any(|field| field.behavior_init_signer) {
        return quote! {};
    }

    let signers = plan.fields.iter().map(emit_account_signer);
    let count = plan.fields.len();
    quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #[doc(hidden)]
            pub const __QUASAR_ACCOUNT_SIGNERS: [bool; #count] = [#(#signers),*];
        }
    }
}

/// Resolve `Program<T>`/`Sysvar<T>` canonical addresses where those types are
/// in scope. The exported client macro reads them through the accounts type
/// (same mechanism as `__QUASAR_ACCOUNT_SIGNERS`), so its instruction struct
/// can drop the fields entirely.
fn emit_fixed_address_constants(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
    impl_generics: &proc_macro2::TokenStream,
    ty_generics: &proc_macro2::TokenStream,
    where_clause: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let consts: Vec<proc_macro2::TokenStream> = plan
        .fields
        .iter()
        .filter_map(|field| {
            let expr = crate::client_macro::fixed_address_expr(field)?;
            let ident = crate::client_macro::fixed_address_const(&field.ident);
            Some(quote! {
                #[doc(hidden)]
                pub const #ident: #krate::prelude::Address = #expr;
            })
        })
        .collect();
    if consts.is_empty() {
        return quote! {};
    }
    quote! {
        #[allow(non_upper_case_globals)]
        impl #impl_generics #name #ty_generics #where_clause {
            #(#consts)*
        }
    }
}

/// Resolve typed-seeds PDA addresses where the seed types are in scope. The
/// exported client macro calls these through the accounts type, so its
/// instruction struct can drop every client-derivable PDA field.
fn emit_pda_address_fns(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
    impl_generics: &proc_macro2::TokenStream,
    ty_generics: &proc_macro2::TokenStream,
    where_clause: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    use crate::client_macro::{
        build_seed_naming, derivation_roots, direct_derived_deps, field_derivation, DeriveRoot,
        FieldDerivation, SeedSource,
    };
    let krate = crate::krate::lang_path();
    let naming = build_seed_naming(plan);
    let source_arg = |source: &SeedSource| match source {
        // Plain accounts and Address args are `&Address` parameters; derived
        // accounts are `let` locals holding an owned `Address`.
        SeedSource::PlainAccount(i) | SeedSource::ArgRef(i) => quote! { #i },
        SeedSource::DerivedAccount(i) => quote! { &#i },
        SeedSource::ArgValue(i, _) => quote! { #i },
        SeedSource::Const(expr) => quote! { #expr },
        SeedSource::FieldValue { input, .. } => quote! { #input },
    };
    // `find_address` takes every seed by value.
    let owned_source_arg = |source: &SeedSource| match source {
        SeedSource::PlainAccount(i) | SeedSource::ArgRef(i) => quote! { *#i },
        SeedSource::DerivedAccount(i) => quote! { #i },
        SeedSource::ArgValue(i, _) => quote! { #i },
        SeedSource::FieldValue { input, .. } => quote! { #input },
        SeedSource::Const(expr) => quote! { #expr },
    };
    let fns: Vec<proc_macro2::TokenStream> = plan
        .fields
        .iter()
        .filter_map(|field| {
            let derivation = field_derivation(plan, field, &mut Vec::new(), &naming)?;
            let fn_ident = crate::client_macro::pda_address_fn(&field.ident);
            let roots = derivation_roots(plan, &derivation, &naming);
            let params = roots.iter().map(|root| match root {
                DeriveRoot::Account(i) | DeriveRoot::ArgRef(i) => {
                    quote! { #i: &#krate::prelude::Address }
                }
                DeriveRoot::ArgValue(i, ty) => quote! { #i: #ty },
                DeriveRoot::SeedInput { input, alias } => quote! { #input: #alias },
            });
            // Chained derivations: materialize each directly-read derived
            // field by calling its own fn with the shared root parameters.
            let deps = direct_derived_deps(&derivation);
            let lets = deps.iter().map(|dep| {
                let dep_field = plan
                    .fields
                    .iter()
                    .find(|other| other.ident == **dep)
                    .expect("derived dep exists");
                let dep_derivation = field_derivation(plan, dep_field, &mut Vec::new(), &naming)
                    .expect("derived dep re-resolves");
                let dep_fn = crate::client_macro::pda_address_fn(dep);
                let dep_args = derivation_roots(plan, &dep_derivation, &naming)
                    .into_iter()
                    .map(|root| {
                        let ident = root.ident();
                        quote! { #ident }
                    });
                quote! { let #dep = Self::#dep_fn(#(#dep_args,)* program_id); }
            });
            let tail = match &derivation {
                FieldDerivation::Pda { account_ty, seeds } => {
                    if seeds
                        .iter()
                        .any(|seed| matches!(seed, SeedSource::FieldValue { .. }))
                    {
                        let args = seeds.iter().map(owned_source_arg);
                        quote! { <#account_ty>::find_address(#(#args,)* program_id) }
                    } else {
                        let args = seeds.iter().map(source_arg);
                        quote! {
                            let seeds = <#account_ty>::seeds(#(#args),*);
                            #krate::pda::find_program_address_const(&seeds.as_slices(), program_id).0
                        }
                    }
                }
                FieldDerivation::Ata {
                    behavior_path,
                    authority,
                    mint,
                    token_program,
                } => {
                    let authority = source_arg(authority);
                    let mint = source_arg(mint);
                    let token_program = match token_program {
                        crate::client_macro::BehaviorProgramArg::Fixed(field) => {
                            let const_ident = crate::client_macro::fixed_address_const(field);
                            quote! { Some(&Self::#const_ident) }
                        }
                        crate::client_macro::BehaviorProgramArg::Field(field) => {
                            quote! { Some(#field) }
                        }
                        crate::client_macro::BehaviorProgramArg::Default => quote! { None },
                    };
                    quote! { #behavior_path::client_address(#authority, #mint, #token_program) }
                }
            };
            // A leaf ATA never reads `program_id`; chained and PDA forms do.
            let program_param =
                if deps.is_empty() && matches!(derivation, FieldDerivation::Ata { .. }) {
                    quote! { _program_id: &#krate::prelude::Address }
                } else {
                    quote! { program_id: &#krate::prelude::Address }
                };
            Some(quote! {
                #[doc(hidden)]
                #[inline]
                pub fn #fn_ident(
                    #(#params,)*
                    #program_param,
                ) -> #krate::prelude::Address {
                    #(#lets)*
                    #tail
                }
            })
        })
        .collect();
    if fns.is_empty() {
        return quote! {};
    }
    quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#fns)*
        }
    }
}

/// Emit the account-meta signer expression from the typed plan. Core account
/// semantics produce a fixed requirement; delegated init can additionally ask
/// the one behavior that supplies init params whether the account must sign.
pub(crate) fn emit_account_signer(field: &resolve::specs::FieldPlan) -> proc_macro2::TokenStream {
    let fixed = field.signer;
    if !field.behavior_init_signer {
        return quote! { #fixed };
    }

    let krate = crate::krate::lang_path();
    let ty = &field.effective_ty;
    let behavior_requirements = field.behaviors.iter().map(|behavior| {
        let path = &behavior.path;
        quote! {
            (<#path::Behavior as #krate::account_behavior::AccountBehavior<#ty>>::SETS_INIT_PARAMS
                && <#path::Behavior as #krate::account_behavior::AccountBehavior<#ty>>::INIT_REQUIRES_SIGNER)
        }
    });

    quote! { #fixed #(|| #behavior_requirements)* }
}

/// Emit the compiler's resolved account-validation plan into the opaque IDL
/// extension channel. The on-chain program never links this host-only data.
fn emit_idl_validation_meta(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
) -> proc_macro2::TokenStream {
    use resolve::describe::{epilogue, load, post_load, pre_load, rent, tokens};

    let krate = crate::krate::lang_path();
    let struct_name = name.to_string();
    let rent = rent(&plan.rent);
    let accounts = plan.fields.iter().map(|field| {
        let name = crate::helpers::snake_to_camel(&field.ident.to_string());
        let account_type = tokens(&field.effective_ty);
        let wrapper = format!("{:?}", field.wrapper);
        let writable = field.writable;
        let signer = emit_account_signer(field);
        let optional = field.optional;
        let allow_duplicate = field.dup;
        let load = load(&field.load);
        let pre_load: Vec<String> = field.pre_load.iter().map(pre_load).collect();
        let post_load: Vec<String> = field.post_load.iter().map(post_load).collect();
        let epilogue: Vec<String> = field.epilogue.iter().map(epilogue).collect();

        quote! {
            #krate::idl_build::__reexport::IdlAccountValidation {
                name: #krate::idl_build::s(#name),
                account_type: #krate::idl_build::s(#account_type),
                wrapper: #krate::idl_build::s(#wrapper),
                writable: #writable,
                signer: #signer,
                optional: #optional,
                allow_duplicate: #allow_duplicate,
                load: #krate::idl_build::s(#load),
                pre_load: #krate::idl_build::vec![#(#krate::idl_build::s(#pre_load)),*],
                post_load: #krate::idl_build::vec![#(#krate::idl_build::s(#post_load)),*],
                epilogue: #krate::idl_build::vec![#(#krate::idl_build::s(#epilogue)),*],
            }
        }
    });

    quote! {
        #[cfg(feature = "idl-build")]
        #krate::__private_inventory::submit! {
            #krate::idl_build::AccountsValidationFragment(|| {
                (
                    #krate::idl_build::s(#struct_name),
                    #krate::idl_build::__reexport::IdlAccountsValidation {
                        rent: #krate::idl_build::s(#rent),
                        accounts: #krate::idl_build::vec![#(#accounts),*],
                    },
                )
            })
        }
    }
}

/// Emit the typed `EventCpi` impl for a struct that carries both an
/// event-authority field and a program field, wiring `emit_cpi!` to the
/// program's self-CPI. Reads the plan's per-field event-CPI terms and wrapper
/// kinds (never `FieldSemantics`):
///
/// - authority field = the field the plan marked `EventCpiTerm::EventAuthority`
///   (named `event_authority` or typed `EventAuthority`);
/// - program field   = the first `Program<T>` field, detected by type so it
///   need not be named `program`.
///
/// A struct with an event-authority field but no program field is a spanned
/// error (previously this silently generated nothing).
fn emit_event_cpi_impl(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
    impl_generics: &proc_macro2::TokenStream,
    ty_generics: &proc_macro2::TokenStream,
    where_clause: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    use resolve::{specs::EventCpiTerm, wrapper::WrapperKind};

    let Some(authority_field) = plan
        .fields
        .iter()
        .zip(plan.event_cpi.iter())
        .find(|(_, term)| matches!(term, EventCpiTerm::EventAuthority))
        .map(|(fp, _)| &fp.ident)
    else {
        // No event authority: this struct does not participate in event CPI.
        return quote! {};
    };

    let Some(program_field) = plan
        .fields
        .iter()
        .find(|fp| fp.wrapper == WrapperKind::Program)
        .map(|fp| &fp.ident)
    else {
        return syn::Error::new_spanned(
            authority_field,
            "event CPI requires a `program: Program<...>` field alongside `event_authority`",
        )
        .to_compile_error();
    };

    quote! {
        impl #impl_generics #krate::event::EventCpi for #name #ty_generics #where_clause {
            const EVENT_AUTHORITY_BUMP: u8 = crate::EventAuthority::BUMP;
            #[inline(always)]
            fn event_program(&self) -> &#krate::__internal::AccountView {
                self.#program_field.to_account_view()
            }
            #[inline(always)]
            fn event_authority(&self) -> &#krate::__internal::AccountView {
                self.#authority_field.to_account_view()
            }
        }
    }
}

/// Emit an `AccountsMetaFragment` inventory submission for this accounts
/// struct.
fn emit_idl_accounts_meta(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    use quote::quote;

    let struct_name_str = name.to_string();

    let account_nodes: Vec<proc_macro2::TokenStream> = plan
        .fields
        .iter()
        .map(|fp| {
            let field_name = crate::helpers::snake_to_camel(&fp.ident.to_string());
            let optional = fp.optional;
            let writable = fp.writable;
            let signer = emit_account_signer(fp);

            let resolver_tokens = if let Some(resolver) = fp.idl_resolver.as_ref() {
                emit_idl_resolver(resolver)
            } else if fp.behaviors.is_empty() {
                quote! { #krate::idl_build::__reexport::IdlResolver::Input {} }
            } else {
                let field_ty = &fp.effective_ty;
                let field_names: Vec<String> = plan
                    .fields
                    .iter()
                    .map(|other| crate::helpers::snake_to_camel(&other.ident.to_string()))
                    .collect();
                let candidates = fp.behaviors.iter().map(|behavior| {
                    let path = &behavior.path;
                    let args = behavior.idl_account_args.iter().map(|arg| {
                        let key = &arg.key;
                        let field = &arg.field;
                        quote! { (#key, #field) }
                    });
                    quote! {
                        #krate::idl_build::behavior_resolver(
                            <#path::Behavior as #krate::account_behavior::AccountBehavior<#field_ty>>::IDL_RESOLVER,
                            &[#(#args),*],
                            &[#(#field_names),*],
                        )
                    }
                });
                quote! {
                    #krate::idl_build::one_behavior_resolver(
                        #field_name,
                        [#(#candidates),*],
                    ).unwrap_or_else(|| {
                        #krate::idl_build::__reexport::IdlResolver::Input {}
                    })
                }
            };
            let node_docs = crate::helpers::docs_tokens_from_lines(&fp.docs);

            quote! {
                #krate::idl_build::__reexport::IdlAccountNode {
                    name: #krate::idl_build::s(#field_name),
                    optional: #optional,
                    writable: #krate::idl_build::__reexport::AccountFlag::Fixed(#writable),
                    signer: #krate::idl_build::__reexport::AccountFlag::Fixed(#signer),
                    resolver: #resolver_tokens,
                    docs: #node_docs,
                }
            }
        })
        .collect();

    quote! {
        #[cfg(feature = "idl-build")]
        #krate::__private_inventory::submit! {
            #krate::idl_build::AccountsMetaFragment(|| {
                (
                    #krate::idl_build::s(#struct_name_str),
                    #krate::idl_build::vec![#(#account_nodes),*],
                )
            })
        }
    }
}

/// Format an already-planned IDL resolver without reclassifying field syntax.
fn emit_idl_resolver(resolver: &resolve::specs::IdlResolverPlan) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    use resolve::specs::{FixedAddressSource, IdlResolverPlan};

    match resolver {
        IdlResolverPlan::FixedAddress { inner_ty, source } => {
            let address = match source {
                FixedAddressSource::Program => {
                    quote! { <#inner_ty as #krate::traits::Id>::ID }
                }
                FixedAddressSource::Sysvar => {
                    quote! { <#inner_ty as #krate::sysvars::Sysvar>::ID }
                }
            };
            quote! {
                #krate::idl_build::__reexport::IdlResolver::Const {
                    address: #krate::idl_build::address_to_base58(&#address),
                }
            }
        }
        IdlResolverPlan::Pda { account_ty, seeds } => {
            let mut seed_tokens = Vec::with_capacity(seeds.len());
            for seed in seeds {
                seed_tokens.push(emit_idl_pda_seed(seed));
            }

            quote! {
                #krate::idl_build::__reexport::IdlResolver::Pda {
                    program: #krate::idl_build::__reexport::IdlPdaProgram::ProgramId {},
                    seeds: {
                        let mut seeds = #krate::idl_build::Vec::new();
                        if <#account_ty as #krate::traits::HasSeeds>::HAS_SEED_PREFIX {
                            seeds.push(#krate::idl_build::__reexport::IdlPdaSeed::Const {
                                value: #krate::idl_build::Vec::from(
                                    <#account_ty as #krate::traits::HasSeeds>::SEED_PREFIX
                                ),
                            });
                        }
                        #(seeds.push(#seed_tokens);)*
                        seeds
                    },
                }
            }
        }
    }
}

/// Format one resolved `IdlSeedPlan` into IDL tokens.
fn emit_idl_pda_seed(seed: &resolve::specs::IdlSeedPlan) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    use resolve::specs::IdlSeedPlan;
    match seed {
        IdlSeedPlan::AccountAddr { base } => {
            let path = crate::helpers::snake_to_camel(&base.to_string());
            quote! {
                #krate::idl_build::__reexport::IdlPdaSeed::Account {
                    path: #krate::idl_build::s(#path),
                }
            }
        }
        IdlSeedPlan::AccountField {
            base,
            account,
            field,
        } => {
            let path = crate::helpers::snake_to_camel(&base.to_string());
            quote! {
                #krate::idl_build::__reexport::IdlPdaSeed::AccountField {
                    path: #krate::idl_build::s(#path),
                    account: #krate::idl_build::s(#account),
                    field: #krate::idl_build::s(#field),
                }
            }
        }
        IdlSeedPlan::IxArg { name, ty } => {
            let path = name.to_string();
            let idl_type = crate::idl::type_to_idl_type_tokens(ty);
            quote! {
                #krate::idl_build::__reexport::IdlPdaSeed::Arg {
                    path: #krate::idl_build::s(#path),
                    ty: #idl_type,
                }
            }
        }
        IdlSeedPlan::Const { expr } => quote! {
            #krate::idl_build::__reexport::IdlPdaSeed::Const {
                value: #krate::idl_build::Vec::from(
                    #krate::pda::seed_bytes(&(#expr))
                ),
            }
        },
    }
}

fn emit_needs_event_cpi_expr(plan: &resolve::specs::AccountsPlanTyped) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    use resolve::specs::EventCpiTerm;
    // Only fields that can contribute `true` are ORed in; plain single fields
    // (`Never`) would only add redundant `|| false`, so they are dropped.
    let terms: Vec<proc_macro2::TokenStream> = plan
        .event_cpi
        .iter()
        .filter_map(|term| match term {
            EventCpiTerm::Composite(ty) => {
                let inner_ty = composite_event_ty(ty);
                Some(quote! { <#inner_ty as #krate::traits::AccountCount>::NEEDS_EVENT_CPI })
            }
            EventCpiTerm::EventAuthority => Some(quote! { true }),
            EventCpiTerm::Never => None,
        })
        .collect();

    quote! { false #(|| #terms)* }
}

struct SignerHelpersCtx<'a> {
    name: &'a syn::Ident,
    bumps_name: &'a syn::Ident,
    plan: &'a resolve::specs::AccountsPlanTyped,
    impl_generics: &'a proc_macro2::TokenStream,
    ty_generics: &'a proc_macro2::TokenStream,
    where_clause: &'a proc_macro2::TokenStream,
    ix_arg_extraction: &'a proc_macro2::TokenStream,
    has_instruction_args: bool,
}

fn emit_signer_helpers_impl(ctx: SignerHelpersCtx<'_>) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let SignerHelpersCtx {
        name,
        bumps_name,
        plan,
        impl_generics,
        ty_generics,
        where_clause,
        ix_arg_extraction,
        has_instruction_args,
    } = ctx;

    let field_refs: Vec<proc_macro2::TokenStream> = plan
        .fields
        .iter()
        .map(|fp| {
            let field_name = &fp.ident;
            quote! { let #field_name = &self.#field_name; }
        })
        .collect();

    let signer_methods: Vec<proc_macro2::TokenStream> = plan
        .fields
        .iter()
        .filter_map(|fp| {
            let field_name = &fp.ident;
            let signer_helper = fp.signer_helper.as_ref()?;
            let addr_expr = &signer_helper.addr_expr;
            let set_ty = &signer_helper.set_ty;
            let method_name = format_ident!("{}_signer", field_name);
            if has_instruction_args {
                Some(quote! {
                    #[inline(always)]
                    #[allow(unused_variables)]
                    pub fn #method_name<'__quasar_seed>(
                        &'__quasar_seed self,
                        bumps: &'__quasar_seed #bumps_name,
                        data: &'__quasar_seed [u8],
                    ) -> Result<
                        <#set_ty as #krate::traits::HasSeeds>::WithBump<'__quasar_seed>,
                        #krate::prelude::ProgramError,
                    > {
                        let __ix_data = data;
                        #ix_arg_extraction
                        #(#field_refs)*
                        Ok(#addr_expr.with_bump(bumps.#field_name))
                    }
                })
            } else {
                Some(quote! {
                    #[inline(always)]
                    #[allow(unused_variables)]
                    pub fn #method_name<'__quasar_seed>(
                        &'__quasar_seed self,
                        bumps: &'__quasar_seed #bumps_name,
                    ) -> <#set_ty as #krate::traits::HasSeeds>::WithBump<'__quasar_seed> {
                        #(#field_refs)*
                        #addr_expr.with_bump(bumps.#field_name)
                    }
                })
            }
        })
        .collect();

    quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #(#signer_methods)*
        }

        impl #impl_generics #krate::traits::AccountBumps for #name #ty_generics #where_clause {
            type Bumps = #bumps_name;
        }

        impl #impl_generics #krate::traits::AccountGroup for #name #ty_generics #where_clause {}
    }
}

fn composite_event_ty(ty: &Type) -> proc_macro2::TokenStream {
    if resolve::wrapper::classify_wrapper(ty) == resolve::wrapper::WrapperKind::AccountsArray {
        return quote! { #ty };
    }
    // Composite field types are always path types; the fallback keeps a valid
    // type token (no cascade) if that invariant is ever broken -- the invalid
    // type then fails with a localized trait-bound error.
    strip_generics(ty).unwrap_or_else(|_| quote! { #ty })
}
