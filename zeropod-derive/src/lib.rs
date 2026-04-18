use {
    proc_macro::TokenStream,
    syn::{parse_macro_input, DeriveInput},
};

mod compact;
mod fixed;
mod schema;
mod type_map;

#[proc_macro_derive(ZeroPod, attributes(zeropod))]
pub fn derive_zero_pod(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let output = match &input.data {
        syn::Data::Enum(_) => fixed::generate_enum(&input),
        syn::Data::Struct(_) => {
            let schema = match schema::Schema::parse(&input) {
                Ok(s) => s,
                Err(e) => return e.into(),
            };
            if schema.is_compact {
                compact::generate(&schema)
            } else {
                fixed::generate(&schema)
            }
        }
        _ => {
            let msg = "ZeroPod only supports structs and unit enums";
            return quote::quote! { compile_error!(#msg); }.into();
        }
    };

    output.into()
}
