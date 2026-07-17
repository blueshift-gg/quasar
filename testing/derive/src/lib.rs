//! Attribute macros for `quasar-test`.

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, FnArg, ItemFn, Pat},
};

/// Turn a function into a `#[test]` with a loaded on-chain world.
///
/// The function takes the world as its only parameter; the program id
/// defaults to `crate::ID`:
///
/// ```rust,ignore
/// #[quasar_test]
/// fn initialize(q: &mut QuasarTest) {
///     let payer = q.actor();
///     q.send(InitializeInstruction { payer, value: 42 }).succeeds();
/// }
/// ```
///
/// Use `#[quasar_test(program_id = EXPR)]` for an external program, e.g. from
/// an integration test where `crate::ID` is not the program crate.
#[proc_macro_attribute]
pub fn quasar_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    let program_id = if attr.is_empty() {
        quote! { crate::ID }
    } else {
        let args = parse_macro_input!(attr as syn::MetaNameValue);
        if !args.path.is_ident("program_id") {
            return syn::Error::new_spanned(
                &args.path,
                "expected `#[quasar_test]` or `#[quasar_test(program_id = EXPR)]`",
            )
            .to_compile_error()
            .into();
        }
        let expr = &args.value;
        quote! { #expr }
    };

    let signature_error = || {
        syn::Error::new_spanned(
            &func.sig,
            "a #[quasar_test] function takes the world as its only parameter: \
             `fn name(q: &mut QuasarTest)`",
        )
        .to_compile_error()
        .into()
    };
    if func.sig.inputs.len() != 1 {
        return signature_error();
    }
    let Some(FnArg::Typed(param)) = func.sig.inputs.first() else {
        return signature_error();
    };
    let Pat::Ident(world) = &*param.pat else {
        return signature_error();
    };

    let attrs = &func.attrs;
    let name = &func.sig.ident;
    let world_ty = &param.ty;
    let world_name = &world.ident;
    let body = &func.block;

    quote! {
        #(#attrs)*
        #[test]
        fn #name() {
            let mut __quasar_world =
                ::quasar_test::QuasarTest::new_for_crate(#program_id, env!("CARGO_PKG_NAME"));
            let #world_name: #world_ty = &mut __quasar_world;
            #body
        }
    }
    .into()
}
