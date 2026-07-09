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
//! contract.

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
    syntax::generate_instruction_arg_extraction,
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

    // Instruction arg extraction
    let ix_arg_extraction = if let Some(ref ix_args) = instruction_args {
        generate_instruction_arg_extraction(ix_args)
    } else {
        quote! {}
    };

    let bumps_struct = emit::parse::emit_bump_struct_def(&typed_plan.fields, &emit_cx);
    let signer_helpers_impl = emit_signer_helpers_impl(SignerHelpersCtx {
        name,
        bumps_name: &bumps_name,
        plan: &typed_plan,
        impl_generics: &impl_generics_ts,
        ty_generics: &ty_generics_ts,
        where_clause: &where_clause_ts,
        ix_arg_extraction: &ix_arg_extraction,
        has_instruction_args: typed_plan.has_instruction_args,
    });
    let epilogue_method = emit::parse::emit_epilogue(&typed_plan);
    let has_epilogue_expr = emit::parse::emit_has_epilogue_typed(&typed_plan);

    let client_macro = crate::client_macro::generate_accounts_macro(name, &typed_plan);

    // IDL accounts meta fragment (feature-gated behind `idl-build`)
    let idl_accounts_meta = emit_idl_accounts_meta(name, &typed_plan);

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
        ix_arg_extraction,
    });

    quote::quote! {
        #main_output
        #idl_accounts_meta
    }
}

/// Emit an `AccountsMetaFragment` inventory submission for this accounts
/// struct.
fn emit_idl_accounts_meta(
    name: &syn::Ident,
    plan: &resolve::specs::AccountsPlanTyped,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let struct_name_str = name.to_string();

    let account_nodes: Vec<proc_macro2::TokenStream> = plan
        .fields
        .iter()
        .map(|fp| {
            let field_name = crate::helpers::snake_to_camel(&fp.ident.to_string());
            let optional = fp.optional;
            let writable = fp.writable;
            let signer = fp.signer;

            let resolver_tokens = fp.idl_resolver.as_ref().map(emit_idl_resolver).unwrap_or_else(
                || quote! { quasar_lang::idl_build::__reexport::IdlResolver::Input {} },
            );
            let node_docs = crate::helpers::docs_tokens_from_lines(&fp.docs);

            quote! {
                quasar_lang::idl_build::__reexport::IdlAccountNode {
                    name: quasar_lang::idl_build::s(#field_name),
                    optional: #optional,
                    writable: quasar_lang::idl_build::__reexport::AccountFlag::Fixed(#writable),
                    signer: quasar_lang::idl_build::__reexport::AccountFlag::Fixed(#signer),
                    resolver: #resolver_tokens,
                    docs: #node_docs,
                }
            }
        })
        .collect();

    quote! {
        #[cfg(feature = "idl-build")]
        quasar_lang::__private_inventory::submit! {
            quasar_lang::idl_build::AccountsMetaFragment(|| {
                (
                    quasar_lang::idl_build::s(#struct_name_str),
                    quasar_lang::idl_build::vec![#(#account_nodes),*],
                )
            })
        }
    }
}

/// Format an already-resolved typed-seeds PDA resolver into IDL tokens. All
/// seed resolution happened once in the planner; this is a pure formatter.
fn emit_idl_resolver(resolver: &resolve::specs::IdlResolverPlan) -> proc_macro2::TokenStream {
    let account_ty = &resolver.account_ty;

    let mut seed_tokens = Vec::with_capacity(resolver.seeds.len() + 1);
    seed_tokens.push(quote! {
        quasar_lang::idl_build::__reexport::IdlPdaSeed::Const {
            value: quasar_lang::idl_build::Vec::from(
                <#account_ty as quasar_lang::traits::HasSeeds>::SEED_PREFIX
            ),
        }
    });
    for seed in &resolver.seeds {
        seed_tokens.push(emit_idl_pda_seed(seed));
    }

    quote! {
        quasar_lang::idl_build::__reexport::IdlResolver::Pda {
            program: quasar_lang::idl_build::__reexport::IdlPdaProgram::ProgramId {},
            seeds: quasar_lang::idl_build::vec![#(#seed_tokens),*],
        }
    }
}

/// Format one resolved `IdlSeedPlan` into IDL tokens.
fn emit_idl_pda_seed(seed: &resolve::specs::IdlSeedPlan) -> proc_macro2::TokenStream {
    use resolve::specs::IdlSeedPlan;
    match seed {
        IdlSeedPlan::AccountAddr { base } => {
            let path = crate::helpers::snake_to_camel(&base.to_string());
            quote! {
                quasar_lang::idl_build::__reexport::IdlPdaSeed::Account {
                    path: quasar_lang::idl_build::s(#path),
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
                quasar_lang::idl_build::__reexport::IdlPdaSeed::AccountField {
                    path: quasar_lang::idl_build::s(#path),
                    account: quasar_lang::idl_build::s(#account),
                    field: quasar_lang::idl_build::s(#field),
                }
            }
        }
        IdlSeedPlan::IxArg { name, ty } => {
            let path = name.to_string();
            let idl_type = crate::helpers::type_to_idl_type_tokens(ty);
            quote! {
                quasar_lang::idl_build::__reexport::IdlPdaSeed::Arg {
                    path: quasar_lang::idl_build::s(#path),
                    ty: #idl_type,
                }
            }
        }
        IdlSeedPlan::Const { expr } => quote! {
            quasar_lang::idl_build::__reexport::IdlPdaSeed::Const {
                value: quasar_lang::idl_build::Vec::from(
                    quasar_lang::pda::seed_bytes(&(#expr))
                ),
            }
        },
    }
}

fn emit_needs_event_cpi_expr(plan: &resolve::specs::AccountsPlanTyped) -> proc_macro2::TokenStream {
    use resolve::specs::EventCpiTerm;
    let terms: Vec<proc_macro2::TokenStream> = plan
        .event_cpi
        .iter()
        .map(|term| match term {
            EventCpiTerm::Composite(ty) => {
                let inner_ty = composite_event_ty(ty);
                quote! { <#inner_ty as AccountCount>::NEEDS_EVENT_CPI }
            }
            EventCpiTerm::EventAuthority => quote! { true },
            EventCpiTerm::Never => quote! { false },
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
                        impl quasar_lang::cpi::CpiSignerSeeds + '__quasar_seed,
                        quasar_lang::prelude::ProgramError,
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
                    ) -> impl quasar_lang::cpi::CpiSignerSeeds + '__quasar_seed {
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

        impl #impl_generics quasar_lang::traits::AccountBumps for #name #ty_generics #where_clause {
            type Bumps = #bumps_name;
        }

        impl #impl_generics quasar_lang::traits::AccountGroup for #name #ty_generics #where_clause {}
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

