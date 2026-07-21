//! `#[derive(QuasarSerialize)]`: generates instruction-arg type bridges.
//!
//! **Fixed structs** (all fields `Copy`, no lifetimes):
//! 1. A hidden ZeroPod companion struct.
//! 2. `InstructionArg` impl for native<->ZC conversion.
//! 3. Off-chain `SchemaWrite` / `SchemaRead` impls.
//!
//! **Borrowed structs** (has lifetime params):
//! 1. A hidden compact zeropod schema.
//! 2. A `decode_compact()` method that returns borrowed views from compact Ref.
//!
//! **Enums** (repr-backed, unit variants):
//! 1. `InstructionArg` impl mapping variants to discriminant values.
//! 2. Off-chain `SchemaWrite` / `SchemaRead` impls.

use {
    crate::helpers::{
        canonical_instruction_arg_type, check_fixed_before_dynamic, classify_instruction_arg,
        instruction_schema_type, ArgClass, ArgSite, PodDynField,
    },
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quasar_schema::pascal_to_snake,
    quote::{format_ident, quote},
    syn::{parse_quote, spanned::Spanned, Data, DeriveInput, Field, Fields, Type},
};

pub(crate) fn derive_quasar_serialize(input: TokenStream) -> TokenStream {
    derive_quasar_serialize_inner(input.into()).into()
}

/// Build the wincode codec generics from a base: the `SchemaWrite` side adds
/// `__C: ConfigCore`; the `SchemaRead` side additionally prepends the `'__de`
/// deserialization lifetime.
fn wincode_codec_generics(base: &syn::Generics) -> (syn::Generics, syn::Generics) {
    let mut write = base.clone();
    write
        .params
        .push(parse_quote!(__C: wincode::config::ConfigCore));

    let mut read = base.clone();
    read.params.insert(0, parse_quote!('__de));
    read.params
        .push(parse_quote!(__C: wincode::config::ConfigCore));

    (write, read)
}

pub(crate) fn derive_quasar_serialize_inner(input: TokenStream2) -> TokenStream2 {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };

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
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "QuasarSerialize can only be derived for structs or repr-backed unit enums",
            )
            .to_compile_error();
        }
    };

    if input.generics.lifetimes().next().is_some() {
        return derive_borrowed_compact(input, fields);
    }

    derive_fixed(input, fields)
}

fn derive_fixed(input: DeriveInput, fields: Vec<Field>) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    let name = &input.ident;
    let schema_generics = extend_fixed_schema_generics(&input.generics);
    let (schema_impl_generics, schema_ty_generics, schema_where_clause) =
        schema_generics.split_for_impl();

    let schema_name = format_ident!("__{}Schema", name);
    let schema_zc_name = format_ident!("__{}SchemaZc", name);
    let zc_name = format_ident!("{}Zc", name);
    let snake_name = pascal_to_snake(&name.to_string());
    let zc_offchain_mod = format_ident!("__{}_zc_offchain", snake_name);
    let offchain_mod = format_ident!("__{}_offchain", snake_name);

    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref()).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();
    let schema_field_types: Vec<_> = field_types
        .iter()
        .map(|ty| instruction_schema_type(ty))
        .collect();
    let canonical_field_types: Vec<_> = field_types
        .iter()
        .map(|ty| canonical_instruction_arg_type(ty))
        .collect();

    let from_zc_fields: Vec<_> = field_names
        .iter()
        .zip(canonical_field_types.iter())
        .map(|(name, ty)| {
            quote! {
                #name: <#ty as #krate::instruction_arg::InstructionArg>::from_zc(&pod.#name)
            }
        })
        .collect();

    let to_zc_fields: Vec<_> = field_names
        .iter()
        .zip(canonical_field_types.iter())
        .map(|(name, ty)| {
            quote! {
                #name: <#ty as #krate::instruction_arg::InstructionArg>::to_zc(&self.#name)
            }
        })
        .collect();

    let (schema_write_generics, schema_read_generics) = wincode_codec_generics(&schema_generics);
    let (schema_write_impl_generics, _, _) = schema_write_generics.split_for_impl();
    let (schema_read_impl_generics, _, _) = schema_read_generics.split_for_impl();

    // IDL fragment emission
    let idl_fragment = {
        let name_str = name.to_string();
        let type_docs = crate::helpers::docs_tokens(&input.attrs);
        let idl_field_defs: Vec<proc_macro2::TokenStream> = fields
            .iter()
            .map(|f| {
                let fname = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                let fty = crate::idl::type_to_idl_type_tokens(&f.ty);
                let fcodec = crate::idl::type_to_idl_codec_tokens(&f.ty);
                let fdocs = crate::helpers::docs_tokens(&f.attrs);
                quote! {
                    #krate::idl_build::__reexport::IdlFieldDef {
                        name: #krate::idl_build::s(#fname),
                        ty: #fty,
                        codec: #fcodec,
                        docs: #fdocs,
                    }
                }
            })
            .collect();

        quote! {
            #[cfg(feature = "idl-build")]
            #krate::__private_inventory::submit! {
                #krate::idl_build::TypeFragment {
                    build: {
                        fn __build() -> #krate::idl_build::__reexport::IdlTypeDef {
                            #krate::idl_build::__reexport::IdlTypeDef {
                                name: #krate::idl_build::s(#name_str),
                                kind: #krate::idl_build::__reexport::IdlTypeDefKind::Struct,
                                docs: #type_docs,
                                fields: #krate::idl_build::vec![#(#idl_field_defs),*],
                                variants: #krate::idl_build::Vec::new(),
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
        #[derive(#krate::__zeropod::ZeroPod)]
        pub struct #schema_name #schema_generics #schema_where_clause {
            #(pub #field_names: #schema_field_types,)*
        }

        #[doc(hidden)]
        pub type #zc_name #schema_generics = #schema_zc_name #schema_ty_generics;

        #[doc(hidden)]
        #[allow(unexpected_cfgs)]
        mod #zc_offchain_mod {
        use super::*;
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
                #krate::__zeropod::ZcValidate::validate_ref(&__zc)
                    .map_err(|_| wincode::error::ReadError::InvalidValue("pod validation failed"))?;
                __dst.write(__zc);
                Ok(())
            }
        }
        }

        impl #schema_impl_generics #krate::instruction_arg::InstructionArg
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
                <Self::Zc as #krate::__zeropod::ZcValidate>::validate_ref(zc)
                    .map_err(|_| solana_program_error::ProgramError::InvalidInstructionData)
            }
        }

        // From impls for native <-> ZC conversion.
        impl #schema_impl_generics From<#name #schema_ty_generics>
            for #zc_name #schema_ty_generics #schema_where_clause
        {
            #[inline(always)]
            fn from(v: #name #schema_ty_generics) -> Self {
                <#name #schema_ty_generics as #krate::instruction_arg::InstructionArg>::to_zc(&v)
            }
        }

        impl #schema_impl_generics From<#zc_name #schema_ty_generics>
            for #name #schema_ty_generics #schema_where_clause
        {
            #[inline(always)]
            fn from(v: #zc_name #schema_ty_generics) -> Self {
                <#name #schema_ty_generics as #krate::instruction_arg::InstructionArg>::from_zc(&v)
            }
        }

        // ZcField: maps the native schema type to its ZC companion so that
        // zeropod-derive's fallback (`<T as ZcField>::Pod`) resolves correctly
        // when this type appears as a field inside a `#[derive(ZeroPod)]` struct.
        impl #schema_impl_generics #krate::ZcField for #name #schema_ty_generics #schema_where_clause {
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
        #[doc(hidden)]
        #[allow(unexpected_cfgs)]
        mod #offchain_mod {
        use super::*;
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_write_impl_generics wincode::SchemaWrite<__C>
            for #name #schema_ty_generics #schema_where_clause
        {
            type Src = Self;

            fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<#zc_name #schema_ty_generics>())
            }

            fn write(mut __writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                let __zc = <Self as #krate::instruction_arg::InstructionArg>::to_zc(src);
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
                <#zc_name #schema_ty_generics as #krate::__zeropod::ZcValidate>::validate_ref(&__zc)
                    .map_err(|_| wincode::error::ReadError::InvalidValue("pod validation failed"))?;
                __dst.write(<Self as #krate::instruction_arg::InstructionArg>::from_zc(&__zc));
                Ok(())
            }
        }
        }

        #idl_fragment
    };

    expanded
}

fn extend_fixed_schema_generics(generics: &syn::Generics) -> syn::Generics {
    let krate = crate::krate::lang_path();
    let mut generics = generics.clone();

    for param in generics.type_params_mut() {
        param.bounds.push(parse_quote!(
            #krate::instruction_arg::InstructionArgField
        ));
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

fn derive_borrowed_compact(input: DeriveInput, fields: Vec<Field>) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let schema_name = format_ident!("__{}CompactSchema", name);
    let ref_name = format_ident!("__{}CompactSchemaRef", name);

    let field_names: Vec<_> = fields
        .iter()
        .map(|f| {
            f.ident
                .as_ref()
                .unwrap_or_else(|| ice!("named struct field has no identifier"))
        })
        .collect();

    let mut field_classes: Vec<BorrowedFieldClass> = Vec::with_capacity(fields.len());
    for field in &fields {
        if let Type::Reference(_) = &field.ty {
            // Borrowed field: route the `&str`/`&[T]` + `#[max(N)]` desugaring
            // through the shared classifier (same missing-max/unsupported
            // diagnostics as the `#[instruction]` handler decode).
            match classify_instruction_arg(&field.ty, &field.attrs, ArgSite::SerializeField) {
                Ok(ArgClass::Borrowed(pd)) => field_classes.push(BorrowedFieldClass::Dynamic(pd)),
                Ok(_) => ice!("a reference field classifies as Borrowed or errors"),
                Err(e) => return e.to_compile_error(),
            }
        } else {
            field_classes.push(BorrowedFieldClass::Fixed);
        }
    }

    // Fixed fields must precede all borrowed (compact tail) fields.
    let is_dynamic: Vec<bool> = field_classes
        .iter()
        .map(|c| matches!(c, BorrowedFieldClass::Dynamic(_)))
        .collect();
    if let Err(e) = check_fixed_before_dynamic(
        &fields,
        &is_dynamic,
        "fixed fields must precede all borrowed fields",
    ) {
        return e.to_compile_error();
    }

    // Build the compact schema IR (raw `PodVec` element — matches the `&[elem]`
    // view the borrowed decode yields), then emit the schema struct + decode via
    // the shared single-source emitters.
    let schema_fields: Vec<crate::schema_ir::SchemaField> = field_classes
        .iter()
        .zip(fields.iter())
        .map(|(cls, field)| {
            let ident = field
                .ident
                .clone()
                .unwrap_or_else(|| ice!("named struct field has no identifier"));
            let class = match cls {
                BorrowedFieldClass::Fixed => {
                    let ty = &field.ty;
                    crate::schema_ir::LayoutClass::Fixed { ty: quote!(#ty) }
                }
                BorrowedFieldClass::Dynamic(pd) => {
                    crate::schema_ir::LayoutClass::from_pod_dyn(pd, |e| quote!(#e))
                }
            };
            crate::schema_ir::SchemaField::private(ident, class)
        })
        .collect();
    let ir = match crate::schema_ir::SchemaIR::new(schema_fields) {
        Ok(ir) => ir,
        Err(e) => return e.to_compile_error(),
    };
    let schema_struct =
        crate::schema_ir::emit_compact_schema(&schema_name, &ir, &syn::Visibility::Inherited);
    let decode = crate::schema_ir::emit_compact_decode(
        &ir,
        &crate::schema_ir::DecodeOpts {
            schema_name: schema_name.clone(),
            ref_name: ref_name.clone(),
            data: quote!(data),
            err: quote!(#krate::prelude::ProgramError::InvalidInstructionData),
            validate_fixed: false,
        },
    );

    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            #[doc(hidden)]
            #[inline(always)]
            pub fn decode_compact(data: &'a [u8]) -> Result<Self, #krate::prelude::ProgramError> {
                use #krate::__zeropod as zeropod;

                // Re-derive the schema inside the method so the Ref type is in scope.
                #schema_struct

                #(#decode)*
                Ok(Self { #(#field_names,)* })
            }
        }
    };

    expanded
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

fn derive_enum(input: DeriveInput, variants: Vec<syn::Variant>) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    if input.generics.lifetimes().next().is_some() {
        return syn::Error::new_spanned(
            &input.ident,
            "QuasarSerialize enums cannot have lifetime parameters",
        )
        .to_compile_error();
    }

    let repr_ty = match parse_repr_type(&input) {
        Ok(repr_ty) => repr_ty,
        Err(err) => return err.to_compile_error(),
    };

    let name = &input.ident;
    let offchain_mod = format_ident!("__{}_offchain", pascal_to_snake(&name.to_string()));
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
            .to_compile_error();
        }

        let discriminant = match &variant.discriminant {
            Some((_, expr)) => expr,
            None => {
                return syn::Error::new_spanned(
                    &variant.ident,
                    "QuasarSerialize enums require explicit discriminants on every variant",
                )
                .to_compile_error();
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
            let disc_expr = &v
                .discriminant
                .as_ref()
                .unwrap_or_else(|| ice!("enum variant discriminant validated present above"))
                .1;
            quote! {
                #krate::idl_build::__reexport::IdlEnumVariant {
                    name: #krate::idl_build::s(#vname),
                    value: #disc_expr as u64,
                    fields: #krate::idl_build::Vec::new(),
                    layout: None,
                }
            }
        })
        .collect();

    let (schema_write_generics, schema_read_generics) = wincode_codec_generics(&input.generics);
    let (schema_write_impl_generics, _, _) = schema_write_generics.split_for_impl();
    let (schema_read_impl_generics, _, _) = schema_read_generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics #krate::instruction_arg::InstructionArg
            for #name #ty_generics #where_clause
        {
            type Zc = <#repr_ty as #krate::instruction_arg::InstructionArg>::Zc;

            #[inline(always)]
            fn from_zc(zc: &Self::Zc) -> Self {
                match <#repr_ty as #krate::instruction_arg::InstructionArg>::from_zc(zc) {
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
                <#repr_ty as #krate::instruction_arg::InstructionArg>::to_zc(&raw)
            }

            #[inline(always)]
            fn validate_zc(
                zc: &Self::Zc,
            ) -> Result<(), #krate::prelude::ProgramError> {
                <#repr_ty as #krate::instruction_arg::InstructionArg>::validate_zc(zc)?;
                match <#repr_ty as #krate::instruction_arg::InstructionArg>::from_zc(zc) {
                    #(#validate_arms,)*
                    _ => Err(#krate::prelude::ProgramError::InvalidInstructionData),
                }
            }
        }

        // ZcField: maps the enum to its repr-type's pod type so that zeropod
        // schema derivation works for structs containing this enum as a field.
        impl #impl_generics #krate::ZcField for #name #ty_generics #where_clause {
            type Pod = <#repr_ty as #krate::ZcField>::Pod;
            const POD_SIZE: usize = <#repr_ty as #krate::ZcField>::POD_SIZE;
        }

        #[doc(hidden)]
        #[allow(unexpected_cfgs)]
        mod #offchain_mod {
        use super::*;
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        unsafe impl #schema_write_impl_generics wincode::SchemaWrite<__C>
            for #name #ty_generics #where_clause
        {
            type Src = Self;

            fn size_of(_src: &Self) -> wincode::error::WriteResult<usize> {
                Ok(core::mem::size_of::<<Self as #krate::instruction_arg::InstructionArg>::Zc>())
            }

            fn write(mut __writer: impl wincode::io::Writer, src: &Self) -> wincode::error::WriteResult<()> {
                let __zc = <Self as #krate::instruction_arg::InstructionArg>::to_zc(src);
                // SAFETY: `__zc` is the repr-backed enum's alignment-1 ZC
                // representation and is fully initialized.
                let __bytes = unsafe {
                    core::slice::from_raw_parts(
                        &__zc as *const <Self as #krate::instruction_arg::InstructionArg>::Zc as *const u8,
                        core::mem::size_of::<<Self as #krate::instruction_arg::InstructionArg>::Zc>(),
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
                let __bytes = __reader.take_scoped(core::mem::size_of::<<Self as #krate::instruction_arg::InstructionArg>::Zc>())?;
                // SAFETY: `take_scoped` returned exactly the enum ZC byte length.
                // `read_unaligned` avoids assuming the reader buffer is aligned.
                let __zc = unsafe {
                    core::ptr::read_unaligned(
                        __bytes.as_ptr() as *const <Self as #krate::instruction_arg::InstructionArg>::Zc,
                    )
                };
                <Self as #krate::instruction_arg::InstructionArg>::validate_zc(&__zc)
                    .map_err(|_| wincode::error::ReadError::InvalidValue("invalid enum discriminant"))?;
                __dst.write(<Self as #krate::instruction_arg::InstructionArg>::from_zc(&__zc));
                Ok(())
            }
        }
        }

        #[cfg(feature = "idl-build")]
        #krate::__private_inventory::submit! {
            #krate::idl_build::TypeFragment {
                build: {
                    fn __build() -> #krate::idl_build::__reexport::IdlTypeDef {
                        #krate::idl_build::__reexport::IdlTypeDef {
                            name: #krate::idl_build::s(#name_str),
                            kind: #krate::idl_build::__reexport::IdlTypeDefKind::Enum,
                            docs: #krate::idl_build::Vec::new(),
                            fields: #krate::idl_build::Vec::new(),
                            variants: #krate::idl_build::vec![#(#idl_variant_defs),*],
                            repr: Some(#krate::idl_build::s(#repr_name_str)),
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

    expanded
}
