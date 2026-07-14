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
//! contract, and `ARCHITECTURE.md` (section 2) for how this pipeline fits the
//! rest of the compiler.

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
    let epilogue_method = emit::parse::emit_epilogue(&typed_plan);
    let has_epilogue_expr = emit::parse::emit_has_epilogue_typed(&typed_plan);

    let client_macro = crate::client_macro::generate_accounts_macro(name, &typed_plan);

    // IDL accounts meta fragment (feature-gated behind `idl-build`)
    let idl_accounts_meta = emit_idl_accounts_meta(name, &typed_plan);

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
        #idl_accounts_meta
        #event_cpi_impl
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
            let signer = fp.signer;

            let resolver_tokens = fp
                .idl_resolver
                .as_ref()
                .map(emit_idl_resolver)
                .unwrap_or_else(|| quote! { #krate::idl_build::__reexport::IdlResolver::Input {} });
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

/// Format an already-resolved typed-seeds PDA resolver into IDL tokens. All
/// seed resolution happened once in the planner; this is a pure formatter.
fn emit_idl_resolver(resolver: &resolve::specs::IdlResolverPlan) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let account_ty = &resolver.account_ty;

    let mut seed_tokens = Vec::with_capacity(resolver.seeds.len() + 1);
    seed_tokens.push(quote! {
        #krate::idl_build::__reexport::IdlPdaSeed::Const {
            value: #krate::idl_build::Vec::from(
                <#account_ty as #krate::traits::HasSeeds>::SEED_PREFIX
            ),
        }
    });
    for seed in &resolver.seeds {
        seed_tokens.push(emit_idl_pda_seed(seed));
    }

    quote! {
        #krate::idl_build::__reexport::IdlResolver::Pda {
            program: #krate::idl_build::__reexport::IdlPdaProgram::ProgramId {},
            seeds: #krate::idl_build::vec![#(#seed_tokens),*],
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
                        impl #krate::cpi::CpiSignerSeeds + '__quasar_seed,
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
                    ) -> impl #krate::cpi::CpiSignerSeeds + '__quasar_seed {
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
