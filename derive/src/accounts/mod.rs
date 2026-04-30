//! `#[derive(Accounts)]` — op-dispatch derive macro.
//!
//! Clean-room implementation. No imports from `derive/src/accounts/`.

pub(crate) mod emit;
mod plan;
pub(crate) mod resolve;
mod syntax;

pub(crate) use syntax::InstructionArg;
use {
    plan::build_accounts_plan,
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam},
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

    // --- Pipeline: syntax → resolve → emit ---

    let semantics = match resolve::lower_semantics(fields, &instruction_args) {
        Ok(semantics) => semantics,
        Err(e) => return e.to_compile_error().into(),
    };

    let emit_cx = emit::EmitCx {
        bumps_name: bumps_name.clone(),
    };

    let accounts_plan = match build_accounts_plan(&semantics, &emit_cx) {
        Ok(parts) => parts,
        Err(e) => return e.to_compile_error().into(),
    };
    let plan::AccountsPlan {
        parse_steps,
        count_expr,
        typed_seed_asserts,
        parse_body,
    } = accounts_plan;

    let bumps_struct = emit::emit_bump_struct_def(&semantics, &emit_cx);
    let epilogue_method = match emit::emit_epilogue(&semantics) {
        Ok(ts) => ts,
        Err(e) => return e.to_compile_error().into(),
    };
    let has_epilogue_expr = emit::emit_has_epilogue(&semantics);

    let seeds_methods = quote::quote! {};

    let client_macro = crate::client_macro::generate_accounts_macro(name, &semantics);

    // Instruction arg extraction
    let ix_arg_extraction = if let Some(ref ix_args) = instruction_args {
        generate_instruction_arg_extraction(ix_args)
    } else {
        quote! {}
    };

    TokenStream::from(emit::emit_accounts_output(emit::AccountsOutput {
        name,
        bumps_name: &bumps_name,
        impl_generics: impl_generics_ts,
        ty_generics: ty_generics_ts,
        where_clause: where_clause_ts,
        parse_impl_generics: parse_impl_generics_ts,
        parse_where_clause: parse_where_clause_ts,
        count_expr,
        parse_steps,
        typed_seed_asserts,
        parse_body,
        bumps_struct,
        epilogue_method,
        has_epilogue_expr,
        seeds_methods,
        client_macro,
        ix_arg_extraction,
    }))
}
