//! Attribute macros for `quasar-test`.

use {
    proc_macro::TokenStream,
    proc_macro2::Span,
    proc_macro_crate::{crate_name, FoundCrate},
    quote::{format_ident, quote},
    syn::{parse_macro_input, FnArg, ItemFn, Pat},
};

/// Run an ordinary Rust test in an isolated Quasar program world.
///
/// The function takes `&mut Test` as its only parameter and may return any
/// type supported by Rust's test harness, including `Result<(), E>`.
///
/// ```rust,ignore
/// #[quasar_test]
/// fn initialize(test: &mut Test) -> Result<(), Box<dyn std::error::Error>> {
///     test.send(InitializeInstruction { /* ... */ }).succeeds();
///     Ok(())
/// }
/// ```
///
/// `crate::ID` is used by default. An integration test for another program can
/// specify `#[quasar_test(program_id = EXPR)]`.
#[proc_macro_attribute]
pub fn quasar_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);

    let program_id = if attr.is_empty() {
        quote! { crate::ID }
    } else {
        let argument = parse_macro_input!(attr as syn::MetaNameValue);
        if !argument.path.is_ident("program_id") {
            return syn::Error::new_spanned(
                &argument.path,
                "expected `#[quasar_test]` or `#[quasar_test(program_id = EXPR)]`",
            )
            .to_compile_error()
            .into();
        }
        let expression = &argument.value;
        quote! { #expression }
    };

    if let Some(error) = invalid_signature(&function) {
        return error.to_compile_error().into();
    }
    let FnArg::Typed(parameter) = function
        .sig
        .inputs
        .first()
        .expect("signature validation requires one parameter")
    else {
        unreachable!("signature validation rejects receivers")
    };
    let Pat::Ident(world) = &*parameter.pat else {
        unreachable!("signature validation requires an identifier pattern")
    };

    let test_crate = match crate_name("quasar-test") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let name = format_ident!("{name}", span = Span::call_site());
            quote! { ::#name }
        }
        Err(error) => {
            return syn::Error::new(
                Span::call_site(),
                format!("could not resolve the `quasar-test` dependency: {error}"),
            )
            .to_compile_error()
            .into();
        }
    };

    let attributes = &function.attrs;
    let visibility = &function.vis;
    let name = &function.sig.ident;
    let output = &function.sig.output;
    let world_type = &parameter.ty;
    let world_name = &world.ident;
    let body = &function.block;

    quote! {
        #(#attributes)*
        #[test]
        #visibility fn #name() #output {
            let mut __quasar_test = #test_crate::Test::builder(#program_id)
                .crate_name(env!("CARGO_PKG_NAME"))
                .build()
                .unwrap_or_else(|error| ::core::panic!("{error}"));
            let #world_name: #world_type = &mut __quasar_test;
            #body
        }
    }
    .into()
}

fn invalid_signature(function: &ItemFn) -> Option<syn::Error> {
    let signature = &function.sig;
    if signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.inputs.len() != 1
    {
        return Some(signature_error(signature));
    }
    let Some(FnArg::Typed(parameter)) = signature.inputs.first() else {
        return Some(signature_error(signature));
    };
    if !matches!(&*parameter.pat, Pat::Ident(_)) {
        return Some(signature_error(signature));
    }
    None
}

fn signature_error(signature: &syn::Signature) -> syn::Error {
    syn::Error::new_spanned(
        signature,
        "a #[quasar_test] function must be an ordinary function with one test-world parameter: \
         `fn name(test: &mut Test)`",
    )
}
