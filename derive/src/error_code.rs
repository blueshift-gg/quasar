use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, Data, DeriveInput},
};

pub(crate) fn error_code(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("#[error_code] can only be used on enums"),
    };

    let mut next_discriminant: u32 = 0;
    let match_arms: Vec<_> = variants
        .iter()
        .map(|v| {
            let ident = &v.ident;
            if let Some((_, expr)) = &v.discriminant {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit_int),
                    ..
                }) = expr
                {
                    next_discriminant = lit_int
                        .base10_parse::<u32>()
                        .expect("#[error_code] discriminant must be a valid u32");
                } else {
                    panic!("#[error_code] discriminant must be an integer literal");
                }
            }
            let value = next_discriminant;
            next_discriminant += 1;
            quote! { #value => Ok(#name::#ident) }
        })
        .collect();

    quote! {
        #[repr(u32)]
        #input

        impl From<#name> for ProgramError {
            #[inline(always)]
            fn from(e: #name) -> Self {
                ProgramError::Custom(e as u32)
            }
        }

        impl TryFrom<u32> for #name {
            type Error = ProgramError;

            #[inline(always)]
            fn try_from(error: u32) -> Result<Self, Self::Error> {
                match error {
                    #(#match_arms,)*
                    _ => Err(ProgramError::InvalidArgument),
                }
            }
        }
    }
    .into()
}
