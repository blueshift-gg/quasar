//! `#[derive(InstructionData)]`: the fields in order, each in zeropod's
//! encoding, which is Borsh's, so the data has one size.

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{Data, DeriveInput, Error, Fields, Index, Result, spanned::Spanned};

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(input, "instruction data is a struct"));
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(&input.generics, "instruction data cannot be generic"));
    }
    let name = &input.ident;
    let fields: Vec<_> = data.fields.iter().collect();
    let types: Vec<_> = fields.iter().map(|field| &field.ty).collect();
    let members: Vec<TokenStream> = fields
        .iter()
        .enumerate()
        .map(|(index, field)| match &field.ident {
            Some(ident) => quote!(#ident),
            None => {
                let index = Index::from(index);
                quote!(#index)
            }
        })
        .collect();
    let bytes = quote!(bytes);
    let at = quote!(at);
    let takes = types.iter().map(|ty| quote!(::quasar_lang::data::take::<#ty>(#bytes, &mut #at)?));
    let decoded = match &data.fields {
        Fields::Named(_) => quote!(Self { #(#members: #takes),* }),
        Fields::Unnamed(_) => quote!(Self(#(#takes),*)),
        Fields::Unit => quote!(Self),
    };
    // Each field's size, where the field is written: `String` fails here.
    let sizes = types.iter().map(
        |ty| quote_spanned!(ty.span()=> const _: usize = ::quasar_lang::data::field_size::<#ty>();),
    );
    let idl = fields.iter().zip(&members).map(|(field, member)| {
        let ty = &field.ty;
        let (name, ty) = (member.to_string(), crate::account::type_name(ty));
        quote!(::quasar_lang::client::IdlField { name: #name, ty: #ty })
    });

    // A position only when there is a field to move it past.
    let (position, out) = if fields.is_empty() {
        (quote!(), quote!(_))
    } else {
        (quote!(let mut #at = 0;), quote!(out))
    };

    Ok(quote! {
        #(#sizes)*

        impl ::quasar_lang::data::InstructionData for #name {
            const SIZE: usize = 0 #(+ ::quasar_lang::data::field_size::<#types>())*;

            #[inline(always)]
            fn decode(#bytes: &[u8]) -> ::quasar_lang::Result<Self> {
                if #bytes.len() != Self::SIZE {
                    return ::core::result::Result::Err(::quasar_lang::ProgramError::InvalidInstructionData);
                }
                #position
                ::core::result::Result::Ok(#decoded)
            }

            fn encode(&self, #out: &mut [u8]) {
                #position
                #(::quasar_lang::data::put(&self.#members, out, &mut #at);)*
            }

            ::quasar_lang::host_only! {
                const IDL: &'static [::quasar_lang::client::IdlField] = &[#(#idl),*];
            }
        }
    })
}
