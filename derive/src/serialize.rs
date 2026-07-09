//! `#[derive(QuasarSerialize)]`: generates instruction-arg type bridges.
//!
//! **Fixed structs** (all fields `Copy`, no lifetimes):
//! 1. A hidden ZeroPod companion struct.
//! 2. `InstructionArg` impl for native<->ZC conversion.
//! 3. Off-chain `SchemaWrite` / `SchemaRead` impls.
//!
//! **Borrowed structs** (has lifetime params):
//! 1. A hidden `#[zeropod(compact)]` schema.
//! 2. A `decode_compact()` method that returns borrowed views from compact Ref.
//!
//! **Enums** (repr-backed, unit variants):
//! 1. `InstructionArg` impl mapping variants to discriminant values.
//! 2. Off-chain `SchemaWrite` / `SchemaRead` impls.

use {
    crate::helpers::{
        canonical_instruction_arg_type, classify_borrowed_as_compact, map_to_pod_type, PodDynField,
    },
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{
        parse_macro_input, parse_quote, spanned::Spanned, Data, DeriveInput, Field, Fields, Type,
    },
};

pub(crate) fn derive_quasar_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let enum_variants = match &input.data {
        Data::Enum(data) => Some(data.variants.iter().cloned().collect::<Vec<_>>()),
        _ => None,
    };
    if let Some(variants) = enum_variants {
        return derive_enum(input, variants);
    }

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields.named.iter().cloned().collect::<Vec<_>>(),
            _ => {
                return syn::Error::new_spanned(
                    &input.ident,
                    "QuasarSerialize can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "QuasarSerialize can only be derived for structs or repr-backed unit enums",
            )
            .to_compile_error()
            .into();
        }
    };

    if input.generics.lifetimes().next().is_some() {
        return derive_borrowed_compact(input, fields);
    }

    derive_fixed(input, fields)
}

fn derive_fixed(input: DeriveInput, fields: Vec<Field>) -> TokenStream {
    let name = &input.ident;
    let schema_generics = extend_fixed_schema_generics(&input.generics, &fields);
    let (schema_impl_generics, schema_ty_generics, schema_where_clause) =
        schema_generics.split_for_impl();

    let schema_name = format_ident!("__{}Schema", name);
    let schema_zc_name = format_ident!("__{}SchemaZc", name);
    let zc_name = format_ident!("{}Zc", name);

    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref()).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();
    let canonical_field_types: Vec<_> = field_types
        .iter()
        .map(|ty| canonical_instruction_arg_type(ty))
        .collect();

    let from_zc_fields: Vec<_> = field_names
        .iter()
        .zip(canonical_field_types.iter())
        .map(|(name, ty)| {
            quote! {
                #name: <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&pod.#name)
            }
        })
        .collect();

    let to_zc_fields: Vec<_> = field_names
        .iter()
        .zip(canonical_field_types.iter())
        .map(|(name, ty)| {
            quote! {
                #name: <#ty as quasar_lang::instruction_arg::InstructionArg>::to_zc(&self.#name)
            }
        })
        .collect();

    let mut schema_write_generics = schema_generics.clone();
    schema_write_generics
        .params
        .push(parse_quote!(__C: wincode::config::ConfigCore));
    let (schema_write_impl_generics, _, _) = schema_write_generics.split_for_impl();

    let mut schema_read_generics = schema_generics.clone();
    schema_read_generics.params.insert(0, parse_quote!('__de));
    schema_read_generics
        .params
        .push(parse_quote!(__C: wincode::config::ConfigCore));
    let (schema_read_impl_generics, _, _) = schema_read_generics.split_for_impl();

    // IDL fragment emission
    let idl_fragment = {
        let name_str = name.to_string();
        let idl_field_defs: Vec<proc_macro2::TokenStream> = fields
            .iter()
            .map(|f| {
                let fname = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                let fty = crate::helpers::type_to_idl_type_tokens(&f.ty);
                let fcodec = crate::helpers::type_to_idl_codec_tokens(&f.ty);
                quote! {
                    quasar_lang::idl_build::__reexport::IdlFieldDef {
                        name: quasar_lang::idl_build::s(#fname),
                        ty: #fty,
                        codec: #fcodec,
                        docs: quasar_lang::idl_build::Vec::new(),
                    }
                }
            })
            .collect();

        quote! {
            #[cfg(feature = "idl-build")]
            quasar_lang::__private_inventory::submit! {
                quasar_lang::idl_build::TypeFragment {
                    build: {
                        fn __build() -> quasar_lang::idl_build::__reexport::IdlTypeDef {
                            quasar_lang::idl_build::__reexport::IdlTypeDef {
                                name: quasar_lang::idl_build::s(#name_str),
                                kind: quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct,
                                docs: quasar_lang::idl_build::Vec::new(),
                                generics: quasar_lang::idl_build::Vec::new(),
                                fields: quasar_lang::idl_build::vec![#(#idl_field_defs),*],
                                variants: quasar_lang::idl_build::Vec::new(),
                                repr: None,
                                alias: None,
                                fallback: None,
                                codec: None,
                                layout: None,
                                space: None,
                                semantics: None,
                            }
                        }
                        __build
                    },
                }
            }
        }
    };

    let expanded = quote! {
        #[doc(hidden)]
        #[derive(quasar_lang::__zeropod::ZeroPod)]
        pub struct #schema_name #schema_generics #schema_where_clause {
            #(pub #field_names: #field_types,)*
        }

        #[doc(hidden)]
        pub type #zc_name #schema_generics = #schema_zc_name #schema_ty_generics;

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_write_impl_generics wincode::SchemaWrite<__C>
            for #schema_zc_name #schema_ty_generics #schema_where_clause
        {
            type Src = Self;

            fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<Self>())
            }

            fn write(mut __writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                // SAFETY: `Self` is the generated ZeroPod companion. Its bytes
                // are initialized and its wire format is exactly its memory layout.
                let __bytes = unsafe {
                    core::slice::from_raw_parts(
                        src as *const Self as *const u8,
                        core::mem::size_of::<Self>(),
                    )
                };
                __writer.write(__bytes)?;
                Ok(())
            }
        }

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_read_impl_generics wincode::SchemaRead<'__de, __C>
            for #schema_zc_name #schema_ty_generics #schema_where_clause
        {
            type Dst = Self;

            fn read(
                mut __reader: impl wincode::io::Reader<'__de>,
                __dst: &mut core::mem::MaybeUninit<Self>,
            ) -> wincode::error::ReadResult<()> {
                let __bytes = __reader.take_scoped(core::mem::size_of::<Self>())?;
                // SAFETY: `take_scoped` returned exactly `size_of::<Self>()`
                // bytes. `read_unaligned` avoids assuming reader-buffer alignment.
                let __zc = unsafe { core::ptr::read_unaligned(__bytes.as_ptr() as *const Self) };
                quasar_lang::__zeropod::ZcValidate::validate_ref(&__zc)
                    .map_err(|_| wincode::error::ReadError::InvalidValue("pod validation failed"))?;
                __dst.write(__zc);
                Ok(())
            }
        }

        impl #schema_impl_generics quasar_lang::instruction_arg::InstructionArg
            for #name #schema_ty_generics #schema_where_clause
        {
            type Zc = #zc_name #schema_ty_generics;

            #[inline(always)]
            fn from_zc(zc: &Self::Zc) -> Self {
                let pod = zc;
                Self {
                    #(#from_zc_fields,)*
                }
            }
            #[inline(always)]
            fn to_zc(&self) -> Self::Zc {
                #zc_name {
                    #(#to_zc_fields,)*
                }
            }
            #[inline(always)]
            fn validate_zc(zc: &Self::Zc) -> Result<(), solana_program_error::ProgramError> {
                <Self::Zc as quasar_lang::__zeropod::ZcValidate>::validate_ref(zc)
                    .map_err(|_| solana_program_error::ProgramError::InvalidInstructionData)
            }
        }

        // From impls for native <-> ZC conversion.
        impl #schema_impl_generics From<#name #schema_ty_generics>
            for #zc_name #schema_ty_generics #schema_where_clause
        {
            #[inline(always)]
            fn from(v: #name #schema_ty_generics) -> Self {
                <#name #schema_ty_generics as quasar_lang::instruction_arg::InstructionArg>::to_zc(&v)
            }
        }

        impl #schema_impl_generics From<#zc_name #schema_ty_generics>
            for #name #schema_ty_generics #schema_where_clause
        {
            #[inline(always)]
            fn from(v: #zc_name #schema_ty_generics) -> Self {
                <#name #schema_ty_generics as quasar_lang::instruction_arg::InstructionArg>::from_zc(&v)
            }
        }

        // ZcField: maps the native schema type to its ZC companion so that
        // zeropod-derive's fallback (`<T as ZcField>::Pod`) resolves correctly
        // when this type appears as a field inside a `#[derive(ZeroPod)]` struct.
        impl #schema_impl_generics quasar_lang::ZcField for #name #schema_ty_generics #schema_where_clause {
            type Pod = #zc_name #schema_ty_generics;
            const POD_SIZE: usize = core::mem::size_of::<#zc_name #schema_ty_generics>();
        }

        // Wincode SchemaWrite + SchemaRead (off-chain only)
        //
        // Serializes each field via its ZC (zero-copy) representation to
        // guarantee the wire format matches the on-chain ZC layout exactly.
        // This is critical for types like Option<T> where wincode's built-in
        // encoding is variable-length but the on-chain ZC companion (OptionZc)
        // is fixed-size.
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_write_impl_generics wincode::SchemaWrite<__C>
            for #name #schema_ty_generics #schema_where_clause
        {
            type Src = Self;

            fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<#zc_name #schema_ty_generics>())
            }

            fn write(mut __writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                let __zc = <Self as quasar_lang::instruction_arg::InstructionArg>::to_zc(src);
                // SAFETY: `__zc` is the generated alignment-1 ZC representation;
                // its bytes are initialized and define the fixed wire format.
                let __bytes = unsafe {
                    core::slice::from_raw_parts(
                        &__zc as *const #zc_name #schema_ty_generics as *const u8,
                        core::mem::size_of::<#zc_name #schema_ty_generics>(),
                    )
                };
                __writer.write(__bytes)?;
                Ok(())
            }
        }

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_read_impl_generics wincode::SchemaRead<'__de, __C>
            for #name #schema_ty_generics #schema_where_clause
        {
            type Dst = Self;

            fn read(
                mut __reader: impl wincode::io::Reader<'__de>,
                __dst: &mut core::mem::MaybeUninit<Self>,
            ) -> wincode::error::ReadResult<()> {
                let __bytes = __reader.take_scoped(core::mem::size_of::<#zc_name #schema_ty_generics>())?;
                // SAFETY: `take_scoped` returned exactly the ZC byte length.
                // `read_unaligned` is required because reader buffers have no
                // alignment contract.
                let __zc = unsafe {
                    core::ptr::read_unaligned(__bytes.as_ptr() as *const #zc_name #schema_ty_generics)
                };
                <#zc_name #schema_ty_generics as quasar_lang::__zeropod::ZcValidate>::validate_ref(&__zc)
                    .map_err(|_| wincode::error::ReadError::InvalidValue("pod validation failed"))?;
                __dst.write(<Self as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__zc));
                Ok(())
            }
        }

        #idl_fragment
    };

    expanded.into()
}

fn extend_fixed_schema_generics(generics: &syn::Generics, fields: &[Field]) -> syn::Generics {
    let mut generics = generics.clone();

    for param in generics.type_params_mut() {
        param.bounds.push(parse_quote!(
            quasar_lang::instruction_arg::InstructionArgField
        ));
    }

    let where_clause = generics.make_where_clause();
    for field in fields {
        let pod_ty = map_to_pod_type(&field.ty);
        where_clause
            .predicates
            .push(parse_quote!(#pod_ty: quasar_lang::__zeropod::ZcValidate));
    }

    generics
}

/// Classification of a field in a borrowed compact struct.
enum BorrowedFieldClass {
    /// Fixed (non-reference) field: use native type in schema, extract via
    /// `InstructionArg::from_zc`.
    Fixed,
    /// Dynamic reference field: maps to a PodString or PodVec in the schema.
    Dynamic(PodDynField),
}

fn derive_borrowed_compact(input: DeriveInput, fields: Vec<Field>) -> TokenStream {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let schema_name = format_ident!("__{}CompactSchema", name);
    let ref_name = format_ident!("__{}CompactSchemaRef", name);

    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();

    let mut field_classes: Vec<BorrowedFieldClass> = Vec::with_capacity(fields.len());
    for field in &fields {
        if let Type::Reference(_) = &field.ty {
            match crate::helpers::parse_max_attr(&field.attrs) {
                Some(Ok((max_n, pfx))) => {
                    match classify_borrowed_as_compact(&field.ty, max_n, pfx) {
                        Some(pd) => field_classes.push(BorrowedFieldClass::Dynamic(pd)),
                        None => {
                            return syn::Error::new_spanned(
                                &field.ty,
                                "unsupported borrowed type; use &str or &[T]",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                }
                Some(Err(e)) => return e.to_compile_error().into(),
                None => {
                    return syn::Error::new_spanned(
                        &field.ty,
                        "borrowed fields in QuasarSerialize require #[max(N)] annotation",
                    )
                    .to_compile_error()
                    .into();
                }
            }
        } else {
            field_classes.push(BorrowedFieldClass::Fixed);
        }
    }

    let schema_field_types: Vec<proc_macro2::TokenStream> = field_classes
        .iter()
        .zip(fields.iter())
        .map(|(cls, field)| match cls {
            BorrowedFieldClass::Fixed => {
                let ty = &field.ty;
                quote!(#ty)
            }
            BorrowedFieldClass::Dynamic(PodDynField::Str { max, prefix_bytes }) => {
                quote!(zeropod::pod::PodString<#max, #prefix_bytes>)
            }
            BorrowedFieldClass::Dynamic(PodDynField::Vec {
                elem,
                max,
                prefix_bytes,
            }) => {
                quote!(zeropod::pod::PodVec<#elem, #max, #prefix_bytes>)
            }
        })
        .collect();

    let extract_fields: Vec<proc_macro2::TokenStream> = field_classes
        .iter()
        .zip(fields.iter())
        .map(|(cls, field)| {
            let fname = field.ident.as_ref().unwrap();
            match cls {
                BorrowedFieldClass::Fixed => {
                    let ty = &field.ty;
                    quote! {
                        let #fname = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ref.#fname);
                    }
                }
                BorrowedFieldClass::Dynamic(_) => {
                    quote! {
                        let #fname = __ref.#fname();
                    }
                }
            }
        })
        .collect();

    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #[doc(hidden)]
            #[inline(always)]
            pub fn decode_compact(data: &'a [u8]) -> Result<Self, quasar_lang::prelude::ProgramError> {
                use quasar_lang::__zeropod as zeropod;

                // Re-derive the schema inside the method so the Ref type is in scope.
                #[derive(zeropod::ZeroPod)]
                #[zeropod(compact)]
                struct #schema_name {
                    #(#field_names: #schema_field_types,)*
                }

                <#schema_name as quasar_lang::ZeroPodCompact>::validate(data)
                    .map_err(|_| quasar_lang::prelude::ProgramError::InvalidInstructionData)?;
                let __ref = unsafe { #ref_name::new_unchecked(data) };
                #(#extract_fields)*
                Ok(Self { #(#field_names,)* })
            }
        }
    };

    expanded.into()
}

fn parse_repr_type(input: &DeriveInput) -> Result<Type, syn::Error> {
    for attr in &input.attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let mut repr_ty: Option<Type> = None;
        attr.parse_nested_meta(|meta| {
            let ident = meta
                .path
                .get_ident()
                .ok_or_else(|| syn::Error::new(meta.path.span(), "unsupported #[repr(...)]"))?;
            let supported = matches!(
                ident.to_string().as_str(),
                "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64"
            );
            if supported {
                repr_ty = Some(Type::Path(syn::TypePath {
                    qself: None,
                    path: ident.clone().into(),
                }));
            }
            Ok(())
        })?;
        if let Some(repr_ty) = repr_ty {
            return Ok(repr_ty);
        }
    }

    Err(syn::Error::new_spanned(
        &input.ident,
        "QuasarSerialize enums require #[repr(u8|u16|u32|u64|i8|i16|i32|i64)]",
    ))
}

fn derive_enum(input: DeriveInput, variants: Vec<syn::Variant>) -> TokenStream {
    if input.generics.lifetimes().next().is_some() {
        return syn::Error::new_spanned(
            &input.ident,
            "QuasarSerialize enums cannot have lifetime parameters",
        )
        .to_compile_error()
        .into();
    }

    let repr_ty = match parse_repr_type(&input) {
        Ok(repr_ty) => repr_ty,
        Err(err) => return err.to_compile_error().into(),
    };

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut match_from_zc = Vec::with_capacity(variants.len());
    let mut match_to_zc = Vec::with_capacity(variants.len());
    let mut validate_arms = Vec::with_capacity(variants.len());

    for variant in &variants {
        if !matches!(variant.fields, Fields::Unit) {
            return syn::Error::new_spanned(
                &variant.ident,
                "QuasarSerialize enums must contain only unit variants",
            )
            .to_compile_error()
            .into();
        }

        let discriminant = match &variant.discriminant {
            Some((_, expr)) => expr,
            None => {
                return syn::Error::new_spanned(
                    &variant.ident,
                    "QuasarSerialize enums require explicit discriminants on every variant",
                )
                .to_compile_error()
                .into();
            }
        };

        let ident = &variant.ident;
        match_from_zc.push(quote! { #discriminant => Self::#ident });
        match_to_zc.push(quote! { Self::#ident => #discriminant });
        validate_arms.push(quote! { #discriminant => Ok(()) });
    }

    // IDL data for enum fragment
    let name_str = name.to_string();
    let repr_name_str = quote!(#repr_ty).to_string();
    let idl_variant_defs: Vec<proc_macro2::TokenStream> = variants
        .iter()
        .map(|v| {
            let vname = v.ident.to_string();
            // Use the explicit discriminant expression: guaranteed present by
            // the earlier validation loop.
            let disc_expr = &v.discriminant.as_ref().unwrap().1;
            quote! {
                quasar_lang::idl_build::__reexport::IdlEnumVariant {
                    name: quasar_lang::idl_build::s(#vname),
                    value: #disc_expr as u64,
                    fields: quasar_lang::idl_build::Vec::new(),
                    layout: None,
                }
            }
        })
        .collect();

    let mut schema_write_generics = input.generics.clone();
    schema_write_generics
        .params
        .push(parse_quote!(__C: wincode::config::ConfigCore));
    let (schema_write_impl_generics, _, _) = schema_write_generics.split_for_impl();

    let mut schema_read_generics = input.generics.clone();
    schema_read_generics.params.insert(0, parse_quote!('__de));
    schema_read_generics
        .params
        .push(parse_quote!(__C: wincode::config::ConfigCore));
    let (schema_read_impl_generics, _, _) = schema_read_generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics quasar_lang::instruction_arg::InstructionArg
            for #name #ty_generics #where_clause
        {
            type Zc = <#repr_ty as quasar_lang::instruction_arg::InstructionArg>::Zc;

            #[inline(always)]
            fn from_zc(zc: &Self::Zc) -> Self {
                match <#repr_ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(zc) {
                    #(#match_from_zc,)*
                    // SAFETY: validate_zc rejects invalid discriminants
                    // before from_zc is called. This branch is unreachable.
                    _ => unsafe { core::hint::unreachable_unchecked() },
                }
            }

            #[inline(always)]
            fn to_zc(&self) -> Self::Zc {
                let raw: #repr_ty = match self {
                    #(#match_to_zc,)*
                };
                <#repr_ty as quasar_lang::instruction_arg::InstructionArg>::to_zc(&raw)
            }

            #[inline(always)]
            fn validate_zc(
                zc: &Self::Zc,
            ) -> Result<(), quasar_lang::prelude::ProgramError> {
                <#repr_ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(zc)?;
                match <#repr_ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(zc) {
                    #(#validate_arms,)*
                    _ => Err(quasar_lang::prelude::ProgramError::InvalidInstructionData),
                }
            }
        }

        // ZcField: maps the enum to its repr-type's pod type so that zeropod
        // schema derivation works for structs containing this enum as a field.
        impl #impl_generics quasar_lang::ZcField for #name #ty_generics #where_clause {
            type Pod = <#repr_ty as quasar_lang::ZcField>::Pod;
            const POD_SIZE: usize = <#repr_ty as quasar_lang::ZcField>::POD_SIZE;
        }

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_write_impl_generics wincode::SchemaWrite<__C>
            for #name #ty_generics #where_clause
        {
            type Src = Self;

            fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<<Self as quasar_lang::instruction_arg::InstructionArg>::Zc>())
            }

            fn write(mut __writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                let __zc = <Self as quasar_lang::instruction_arg::InstructionArg>::to_zc(src);
                // SAFETY: `__zc` is the repr-backed enum's alignment-1 ZC
                // representation and is fully initialized.
                let __bytes = unsafe {
                    core::slice::from_raw_parts(
                        &__zc as *const <Self as quasar_lang::instruction_arg::InstructionArg>::Zc as *const u8,
                        core::mem::size_of::<<Self as quasar_lang::instruction_arg::InstructionArg>::Zc>(),
                    )
                };
                __writer.write(__bytes)?;
                Ok(())
            }
        }

        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_read_impl_generics wincode::SchemaRead<'__de, __C>
            for #name #ty_generics #where_clause
        {
            type Dst = Self;

            fn read(
                mut __reader: impl wincode::io::Reader<'__de>,
                __dst: &mut core::mem::MaybeUninit<Self>,
            ) -> wincode::error::ReadResult<()> {
                let __bytes = __reader.take_scoped(core::mem::size_of::<<Self as quasar_lang::instruction_arg::InstructionArg>::Zc>())?;
                // SAFETY: `take_scoped` returned exactly the enum ZC byte length.
                // `read_unaligned` avoids assuming the reader buffer is aligned.
                let __zc = unsafe {
                    core::ptr::read_unaligned(
                        __bytes.as_ptr() as *const <Self as quasar_lang::instruction_arg::InstructionArg>::Zc,
                    )
                };
                <Self as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__zc)
                    .map_err(|_| wincode::error::ReadError::InvalidValue("invalid enum discriminant"))?;
                __dst.write(<Self as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__zc));
                Ok(())
            }
        }

        #[cfg(feature = "idl-build")]
        quasar_lang::__private_inventory::submit! {
            quasar_lang::idl_build::TypeFragment {
                build: {
                    fn __build() -> quasar_lang::idl_build::__reexport::IdlTypeDef {
                        quasar_lang::idl_build::__reexport::IdlTypeDef {
                            name: quasar_lang::idl_build::s(#name_str),
                            kind: quasar_lang::idl_build::__reexport::IdlTypeDefKind::Enum,
                            docs: quasar_lang::idl_build::Vec::new(),
                            generics: quasar_lang::idl_build::Vec::new(),
                            fields: quasar_lang::idl_build::Vec::new(),
                            variants: quasar_lang::idl_build::vec![#(#idl_variant_defs),*],
                            repr: Some(quasar_lang::idl_build::s(#repr_name_str)),
                            alias: None,
                            fallback: None,
                            codec: None,
                            layout: None,
                            space: None,
                            semantics: None,
                        }
                    }
                    __build
                },
            }
        }
    };

    expanded.into()
}
