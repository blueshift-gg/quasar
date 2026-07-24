use {
    proc_macro::TokenStream,
    syn::{parse_macro_input, DeriveInput},
};

mod compact;
mod compact_enum;
mod fixed;
mod schema;
mod type_map;

#[proc_macro_derive(ZeroPod, attributes(zeropod))]
pub fn derive_zero_pod(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let output = match &input.data {
        syn::Data::Enum(_) => {
            if has_compact_attr(&input.attrs) {
                compact_enum::generate(&input)
            } else {
                fixed::generate_enum(&input)
            }
        }
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

fn has_compact_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("zeropod") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("compact") {
                found = true;
            }
            Ok(())
        });
        found
    })
}
