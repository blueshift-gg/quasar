//! `#[derive(QuasarSerialize)]` — generates two things for instruction data
//! structs:
//!
//! 1. An `InstructionArg` impl with an alignment-1 ZC companion struct,
//!    enabling zero-copy deserialization inside `#[instruction]` handlers.
//!
//! 2. Wincode `SchemaWrite` and `SchemaRead` impls (gated behind
//!    `cfg(not(solana))`) for off-chain instruction data serialization.

use {
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_macro_input, Data, DeriveInput, Fields},
};

pub(crate) fn derive_quasar_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "QuasarSerialize can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                name,
                "QuasarSerialize can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let zc_name = format_ident!("__{}Zc", name);

    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    // ZC companion fields: <FieldType as InstructionArg>::Zc
    let zc_field_types: Vec<_> = field_types
        .iter()
        .map(|ty| {
            quote! { <#ty as quasar_lang::instruction_arg::InstructionArg>::Zc }
        })
        .collect();

    // from_zc field reconstructions
    let from_zc_fields: Vec<_> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| {
            quote! {
                #name: <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&zc.#name)
            }
        })
        .collect();

    let to_zc_fields: Vec<_> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| {
            quote! {
                #name: <#ty as quasar_lang::instruction_arg::InstructionArg>::to_zc(&self.#name)
            }
        })
        .collect();

    let expanded = quote! {
        // Alignment-1 ZC companion for zero-copy instruction deserialization.
        #[doc(hidden)]
        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #zc_name {
            #(#field_names: #zc_field_types,)*
        }

        impl #impl_generics quasar_lang::instruction_arg::InstructionArg
            for #name #ty_generics #where_clause
        {
            type Zc = #zc_name;
            #[inline(always)]
            fn from_zc(zc: &#zc_name) -> Self {
                Self {
                    #(#from_zc_fields,)*
                }
            }
            #[inline(always)]
            fn to_zc(&self) -> #zc_name {
                #zc_name {
                    #(#to_zc_fields,)*
                }
            }
        }

        // Wincode SchemaWrite + SchemaRead (off-chain only)
        //
        // Serializes each field via its ZC (zero-copy) representation to
        // guarantee the wire format matches the on-chain ZC layout exactly.
        // This is critical for types like Option<T> where wincode's built-in
        // encoding is variable-length but the on-chain ZC companion (OptionZc)
        // is fixed-size.
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl<__C: wincode::config::ConfigCore> wincode::SchemaWrite<__C>
            for #name #ty_generics #where_clause
        {
            type Src = Self;

            fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<#zc_name>())
            }

            fn write(mut __writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                let __zc = <Self as quasar_lang::instruction_arg::InstructionArg>::to_zc(src);
                let __bytes = unsafe {
                    core::slice::from_raw_parts(
                        &__zc as *const #zc_name as *const u8,
                        core::mem::size_of::<#zc_name>(),
                    )
                };
                __writer.write(__bytes)?;
                Ok(())
            }
        }

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl<'__de, __C: wincode::config::ConfigCore> wincode::SchemaRead<'__de, __C>
            for #name #ty_generics #where_clause
        {
            type Dst = Self;

            fn read(
                mut __reader: impl wincode::io::Reader<'__de>,
                __dst: &mut core::mem::MaybeUninit<Self>,
            ) -> wincode::error::ReadResult<()> {
                let __bytes = __reader.take_scoped(core::mem::size_of::<#zc_name>())?;
                let __zc = unsafe { &*(__bytes.as_ptr() as *const #zc_name) };
                __dst.write(<Self as quasar_lang::instruction_arg::InstructionArg>::from_zc(__zc));
                Ok(())
            }
        }
    };

    expanded.into()
}
