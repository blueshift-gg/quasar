//! `#[derive(Accounts)]` — protocol-neutral accounts derive macro.
//!
//! Pipeline:
//!
//! ```text
//! syntax   → parse raw #[account(...)] directives
//! lower    → turn parsed directives into FieldSemantics
//! rules    → validate structural invariants (no protocol knowledge)
//! planner  → schedule protocol-neutral phase candidates
//! emit     → generate Rust code from the plan
//! ```
//!
//! Protocol crates own behavior. The derive never knows what `token`, `mint`,
//! `metadata`, etc. mean. Every behavior group is lowered to the same shape:
//! `path::Args::builder()` + `<path::Behavior as AccountBehavior<T>>`.
//!
//! See `quasar_lang::account_behavior::AccountBehavior` for the plugin
//! contract.

pub(crate) mod emit;
mod plan;
pub(crate) mod resolve;
mod syntax;

pub(crate) use syntax::InstructionArg;
use {
    crate::helpers::strip_generics,
    plan::build_accounts_plan,
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{
        parse_macro_input, parse_quote, Data, DeriveInput, Expr, ExprCall, Fields, GenericParam,
        Member, Type,
    },
    syntax::{generate_instruction_arg_extraction, parse_struct_instruction_args},
};

pub(crate) fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
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
        return syn::Error::new_spanned(param, message)
            .to_compile_error()
            .into();
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
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "Accounts can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let instruction_args = match parse_struct_instruction_args(&input) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error().into(),
    };

    // --- Pipeline: syntax → resolve → plan → emit ---

    let semantics = match resolve::lower_semantics(fields, &instruction_args) {
        Ok(semantics) => semantics,
        Err(e) => return e.to_compile_error().into(),
    };

    let typed_plan = match resolve::planner::build_plan(&semantics) {
        Ok(plan) => plan,
        Err(e) => return e.to_compile_error().into(),
    };

    let emit_cx = emit::EmitCx {
        bumps_name: bumps_name.clone(),
    };

    let accounts_plan = match build_accounts_plan(&semantics, &typed_plan, &emit_cx) {
        Ok(parts) => parts,
        Err(e) => return e.to_compile_error().into(),
    };
    let plan::AccountsPlan {
        parse_steps,
        count_expr,
        typed_seed_asserts,
        parse_body,
        direct_parse_body,
    } = accounts_plan;

    let bumps_struct = emit::emit_bump_struct_def(&semantics, &emit_cx);
    let epilogue_method = match emit::emit_epilogue(&semantics, &typed_plan) {
        Ok(ts) => ts,
        Err(e) => return e.to_compile_error().into(),
    };
    let has_epilogue_expr = emit::emit_has_epilogue(&typed_plan, &semantics);

    let seeds_methods = quote::quote! {};

    let client_macro = crate::client_macro::generate_accounts_macro(name, &semantics);

    // Instruction arg extraction
    let ix_arg_extraction = if let Some(ref ix_args) = instruction_args {
        generate_instruction_arg_extraction(ix_args)
    } else {
        quote! {}
    };

    // IDL accounts meta fragment (feature-gated behind `idl-build`)
    let idl_accounts_meta = emit_idl_accounts_meta(name, &semantics, &instruction_args);

    let main_output = emit::emit_accounts_output(emit::AccountsOutput {
        name,
        bumps_name: &bumps_name,
        impl_generics: impl_generics_ts,
        ty_generics: ty_generics_ts,
        where_clause: where_clause_ts,
        parse_impl_generics: parse_impl_generics_ts,
        parse_where_clause: parse_where_clause_ts,
        count_expr,
        needs_event_cpi_expr: emit_needs_event_cpi_expr(&semantics),
        parse_steps,
        typed_seed_asserts,
        parse_body,
        direct_parse_body,
        bumps_struct,
        epilogue_method,
        has_epilogue_expr,
        seeds_methods,
        client_macro,
        ix_arg_extraction,
    });

    TokenStream::from(quote::quote! {
        #main_output
        #idl_accounts_meta
    })
}

/// Emit an `AccountsMetaFragment` inventory submission for this accounts
/// struct.
fn emit_idl_accounts_meta(
    name: &syn::Ident,
    semantics: &[resolve::FieldSemantics],
    instruction_args: &Option<Vec<InstructionArg>>,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let struct_name_str = name.to_string();
    let ix_args = instruction_args.as_deref().unwrap_or(&[]);

    let account_nodes: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .map(|sem| {
            let field_name = crate::helpers::snake_to_camel(&sem.core.ident.to_string());
            let writable = sem.is_writable();
            let signer = is_signer_type(&sem.core.effective_ty);

            let resolver_tokens = emit_idl_resolver(sem, semantics, ix_args).unwrap_or_else(
                || quote! { quasar_lang::idl_build::__reexport::IdlResolver::Input {} },
            );

            quote! {
                quasar_lang::idl_build::__reexport::IdlAccountNode {
                    name: quasar_lang::idl_build::s(#field_name),
                    client_type: None,
                    writable: quasar_lang::idl_build::__reexport::AccountFlag::Fixed(#writable),
                    signer: quasar_lang::idl_build::__reexport::AccountFlag::Fixed(#signer),
                    resolver: #resolver_tokens,
                    docs: quasar_lang::idl_build::Vec::new(),
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

fn emit_idl_resolver(
    sem: &resolve::FieldSemantics,
    semantics: &[resolve::FieldSemantics],
    instruction_args: &[InstructionArg],
) -> Option<proc_macro2::TokenStream> {
    let Expr::Call(call) = sem.address.as_ref()? else {
        return None;
    };
    emit_typed_seeds_resolver(call, semantics, instruction_args)
}

fn emit_typed_seeds_resolver(
    call: &ExprCall,
    semantics: &[resolve::FieldSemantics],
    instruction_args: &[InstructionArg],
) -> Option<proc_macro2::TokenStream> {
    let Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let mut segments: Vec<_> = path.path.segments.iter().collect();
    let last = segments.pop()?;
    if last.ident != "seeds" || segments.is_empty() {
        return None;
    }

    let account_ty_segments = segments.iter().map(|segment| &segment.ident);
    let account_ty = quote! { #(#account_ty_segments)::* };
    let mut seeds = Vec::with_capacity(call.args.len() + 1);
    seeds.push(quote! {
        quasar_lang::idl_build::__reexport::IdlPdaSeed::Const {
            value: quasar_lang::idl_build::Vec::from(
                <#account_ty as quasar_lang::traits::HasSeeds>::SEED_PREFIX
            ),
        }
    });

    for arg in &call.args {
        let seed = emit_idl_pda_seed(arg, semantics, instruction_args)?;
        seeds.push(seed);
    }

    Some(quote! {
        quasar_lang::idl_build::__reexport::IdlResolver::Pda {
            program: quasar_lang::idl_build::__reexport::IdlPdaProgram::ProgramId {},
            seeds: quasar_lang::idl_build::vec![#(#seeds),*],
            bump: None,
        }
    })
}

fn emit_idl_pda_seed(
    expr: &Expr,
    semantics: &[resolve::FieldSemantics],
    instruction_args: &[InstructionArg],
) -> Option<proc_macro2::TokenStream> {
    let expr = strip_seed_into(expr);

    if let Some(path) = account_address_seed_path(expr, semantics) {
        return Some(quote! {
            quasar_lang::idl_build::__reexport::IdlPdaSeed::Account {
                path: quasar_lang::idl_build::s(#path),
            }
        });
    }

    if let Some((path, account, field)) = account_field_seed_path(expr, semantics) {
        return Some(quote! {
            quasar_lang::idl_build::__reexport::IdlPdaSeed::AccountField {
                path: quasar_lang::idl_build::s(#path),
                account: quasar_lang::idl_build::s(#account),
                field: quasar_lang::idl_build::s(#field),
            }
        });
    }

    if let Some(arg) = instruction_arg_seed(expr, instruction_args) {
        let path = arg.name.to_string();
        let idl_type = crate::helpers::type_to_idl_type_tokens(&arg.ty);
        return Some(quote! {
            quasar_lang::idl_build::__reexport::IdlPdaSeed::Arg {
                path: quasar_lang::idl_build::s(#path),
                ty: #idl_type,
                encoding: None,
            }
        });
    }

    Some(quote! {
        quasar_lang::idl_build::__reexport::IdlPdaSeed::Const {
            value: quasar_lang::idl_build::Vec::from(
                quasar_lang::pda::seed_bytes(&(#expr))
            ),
        }
    })
}

fn strip_seed_into(expr: &Expr) -> &Expr {
    if let Expr::MethodCall(call) = expr {
        if call.method == "into" && call.args.is_empty() {
            return strip_seed_into(&call.receiver);
        }
    }
    expr
}

fn account_address_seed_path(expr: &Expr, semantics: &[resolve::FieldSemantics]) -> Option<String> {
    let Expr::MethodCall(call) = expr else {
        return None;
    };
    if call.method != "address" || !call.args.is_empty() {
        return None;
    }
    let Expr::Path(path) = call.receiver.as_ref() else {
        return None;
    };
    if path.path.segments.len() != 1 {
        return None;
    }
    let ident = &path.path.segments.first()?.ident;
    has_account_field(ident, semantics).then(|| crate::helpers::snake_to_camel(&ident.to_string()))
}

fn account_field_seed_path(
    expr: &Expr,
    semantics: &[resolve::FieldSemantics],
) -> Option<(String, String, String)> {
    let mut fields = Vec::new();
    let mut cur = expr;

    loop {
        match cur {
            Expr::Field(field) => {
                let name = match &field.member {
                    Member::Named(ident) => ident.to_string(),
                    Member::Unnamed(_) => return None,
                };
                fields.push(name);
                cur = &field.base;
            }
            Expr::Path(path) if path.path.segments.len() == 1 => {
                let base = &path.path.segments.first()?.ident;
                let sem = semantics.iter().find(|sem| sem.core.ident == *base)?;
                if fields.is_empty() {
                    return None;
                }
                fields.reverse();
                let path = crate::helpers::snake_to_camel(&base.to_string());
                let account = account_type_name(sem.core.inner_ty.as_ref()?)?;
                return Some((path, account, fields.join(".")));
            }
            _ => return None,
        }
    }
}

fn instruction_arg_seed<'a>(
    expr: &Expr,
    instruction_args: &'a [InstructionArg],
) -> Option<&'a InstructionArg> {
    let Expr::Path(path) = expr else {
        return None;
    };
    if path.path.segments.len() != 1 {
        return None;
    }
    let ident = &path.path.segments.first()?.ident;
    instruction_args.iter().find(|arg| arg.name == *ident)
}

fn has_account_field(ident: &syn::Ident, semantics: &[resolve::FieldSemantics]) -> bool {
    semantics.iter().any(|sem| sem.core.ident == *ident)
}

fn account_type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn emit_needs_event_cpi_expr(semantics: &[resolve::FieldSemantics]) -> proc_macro2::TokenStream {
    let terms: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .map(|sem| match sem.core.kind {
            resolve::FieldKind::Composite => {
                let inner_ty = strip_generics(&sem.core.effective_ty);
                quote! { <#inner_ty as AccountCount>::NEEDS_EVENT_CPI }
            }
            resolve::FieldKind::Single if is_event_cpi_field(sem) => {
                quote! { true }
            }
            resolve::FieldKind::Single => quote! { false },
        })
        .collect();

    quote! { false #(|| #terms)* }
}

fn is_event_cpi_field(sem: &resolve::FieldSemantics) -> bool {
    if sem.core.ident == "event_authority" {
        return true;
    }

    if let syn::Type::Path(type_path) = &sem.core.effective_ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "EventAuthority")
    } else {
        false
    }
}

/// Check if the effective type is `Signer` (by last path segment name).
fn is_signer_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            return last.ident == "Signer";
        }
    }
    false
}
