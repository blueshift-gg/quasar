use {
    crate::{schema::Schema, type_map::map_to_pod_type},
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::Type,
};

pub fn generate(schema: &Schema) -> TokenStream {
    let struct_name = &schema.name;
    let generics = &schema.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let zc_name = format_ident!("{}Zc", struct_name);

    // Build ZC struct fields: map each schema field to its pod type.
    let zc_fields: Vec<TokenStream> = schema
        .fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let vis = &f.vis;
            let pod_ty = map_to_pod_type(&f.ty);
            quote! { #vis #name: #pod_ty }
        })
        .collect();

    // Build ZcValidate field delegation.
    let field_names: Vec<&syn::Ident> = schema.fields.iter().map(|f| &f.name).collect();
    let pod_field_types: Vec<TokenStream> = schema
        .fields
        .iter()
        .map(|f| map_to_pod_type(&f.ty))
        .collect();
    let where_clause_with_pod_bounds = {
        let pod_bounds: Vec<_> = pod_field_types
            .iter()
            .map(|pod_ty| quote! { #pod_ty: zeropod::ZcValidate })
            .collect();

        match (where_clause, pod_bounds.is_empty()) {
            (Some(existing), false) => {
                let predicates = existing.predicates.iter();
                quote! { where #(#predicates,)* #(#pod_bounds,)* }
            }
            (Some(existing), true) => quote! { #existing },
            (None, false) => quote! { where #(#pod_bounds,)* },
            (None, true) => quote! {},
        }
    };
    // Generate accessor methods on the Zc companion.
    let accessors = generate_accessors(schema);

    let align_assert = if schema.generics.params.is_empty() {
        quote! {
            const _: () = assert!(core::mem::align_of::<#zc_name #ty_generics>() == 1);
        }
    } else {
        quote! {}
    };

    quote! {
        #[repr(C)]
        pub struct #zc_name #generics #where_clause_with_pod_bounds {
            #( #zc_fields ),*
        }

        impl #impl_generics Copy for #zc_name #ty_generics #where_clause_with_pod_bounds {}

        impl #impl_generics Clone for #zc_name #ty_generics #where_clause_with_pod_bounds {
            fn clone(&self) -> Self {
                *self
            }
        }

        #align_assert

        #accessors

        impl #impl_generics zeropod::ZcValidate for #zc_name #ty_generics #where_clause_with_pod_bounds {
            fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
                #(<#pod_field_types as zeropod::ZcValidate>::validate_ref(&value.#field_names)?;)*
                Ok(())
            }
        }

        impl #impl_generics zeropod::ZeroPodSchema for #struct_name #ty_generics #where_clause_with_pod_bounds {
            const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
        }

        impl #impl_generics zeropod::ZeroPodFixed for #struct_name #ty_generics #where_clause_with_pod_bounds {
            type Zc = #zc_name #ty_generics;
            const SIZE: usize = core::mem::size_of::<#zc_name #ty_generics>();

            fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &*(data.as_ptr() as *const Self::Zc) })
            }

            fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self::Zc) })
            }

            fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
                if data.len() < core::mem::size_of::<#zc_name #ty_generics>() {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __zc = unsafe { &*(data.as_ptr() as *const Self::Zc) };
                <Self::Zc as zeropod::ZcValidate>::validate_ref(__zc)?;
                Ok(())
            }
        }

        impl #impl_generics zeropod::ZcField for #struct_name #ty_generics #where_clause_with_pod_bounds {
            type Pod = #zc_name #ty_generics;
            const POD_SIZE: usize = core::mem::size_of::<#zc_name #ty_generics>();
        }

        // SAFETY: #zc_name is #[repr(C)] with all align-1 fields, verified by const assert above.
        unsafe impl #impl_generics zeropod::ZcElem for #zc_name #ty_generics #where_clause_with_pod_bounds {}
    }
}

/// Classify a schema field type to determine what kind of accessor to generate.
enum AccessorKind {
    /// `u8`, `i8` — copy the field directly.
    CopyDirect,
    /// `u16`–`u128`, `bool` — pod type has `From` to native; return native via `.into()`.
    NativeViaFrom(TokenStream),
    /// `Address`, `[u8; N]` — borrow; return `&T`.
    Borrow,
    /// `PodOption<T, 1>` — return `Option<T>` via `.get()`.
    PodOptionGet,
    /// `PodOption<T, PFX>` with an explicit non-default prefix — borrow via
    /// `.get_ref()` to avoid copying wider COption-style payloads.
    PodOptionRef,
    /// `#[zeropod(skip_accessor)]` — skip.
    Skip,
}

fn classify_accessor(ty: &Type, skip: bool) -> AccessorKind {
    if skip {
        return AccessorKind::Skip;
    }

    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            let name = seg.ident.to_string();
            match name.as_str() {
                "u8" | "i8" => return AccessorKind::CopyDirect,
                "u16" => return AccessorKind::NativeViaFrom(quote! { u16 }),
                "u32" => return AccessorKind::NativeViaFrom(quote! { u32 }),
                "u64" => return AccessorKind::NativeViaFrom(quote! { u64 }),
                "u128" => return AccessorKind::NativeViaFrom(quote! { u128 }),
                "i16" => return AccessorKind::NativeViaFrom(quote! { i16 }),
                "i32" => return AccessorKind::NativeViaFrom(quote! { i32 }),
                "i64" => return AccessorKind::NativeViaFrom(quote! { i64 }),
                "i128" => return AccessorKind::NativeViaFrom(quote! { i128 }),
                "bool" => return AccessorKind::NativeViaFrom(quote! { bool }),
                "PodOption" => {
                    if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                        let mut iter = ab.args.iter();
                        let _inner = iter.next(); // T
                        match iter.next() {
                            None => return AccessorKind::PodOptionGet, // default PFX=1
                            Some(syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(lit),
                                ..
                            }))) => {
                                if lit.base10_parse::<usize>().ok() == Some(1) {
                                    return AccessorKind::PodOptionGet;
                                }
                                return AccessorKind::PodOptionRef;
                            }
                            _ => return AccessorKind::PodOptionRef,
                        }
                    }
                    return AccessorKind::PodOptionGet;
                }
                _ => {}
            }
        }
    }

    // Array types [u8; N] and Address (which is [u8; 32] under the hood but appears
    // as a named type) — borrow.
    if matches!(ty, Type::Array(_)) {
        return AccessorKind::Borrow;
    }

    // Named types that we know are borrow-friendly (align 1, fixed).
    // For anything else (custom ZcField types), borrow by default.
    AccessorKind::Borrow
}

/// Extract the inner type T from `PodOption<T>` or `PodOption<T, PFX>`.
/// Maps the inner type through `map_to_pod_type` so native types become pod types.
fn extract_pod_option_inner(ty: &Type) -> TokenStream {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                    return map_to_pod_type(inner);
                }
            }
        }
    }
    // Fallback — shouldn't happen since we only call this for PodOption fields.
    quote! { () }
}

fn generate_accessors(schema: &Schema) -> TokenStream {
    // Only generate for non-generic fixed structs.
    if !schema.generics.params.is_empty() || schema.is_compact {
        return quote! {};
    }

    let zc_name = format_ident!("{}Zc", schema.name);

    let methods: Vec<TokenStream> = schema
        .fields
        .iter()
        .filter_map(|f| {
            let name = &f.name;
            let pod_ty = map_to_pod_type(&f.ty);

            match classify_accessor(&f.ty, f.skip_accessor) {
                AccessorKind::CopyDirect => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> #pod_ty {
                        self.#name
                    }
                }),
                AccessorKind::NativeViaFrom(native_ty) => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> #native_ty {
                        #native_ty::from(self.#name)
                    }
                }),
                AccessorKind::Borrow => Some(quote! {
                    #[inline(always)]
                    pub fn #name(&self) -> &#pod_ty {
                        &self.#name
                    }
                }),
                AccessorKind::PodOptionGet => {
                    // Extract the inner type T from PodOption<T> / PodOption<T, 1>.
                    let inner_ty = extract_pod_option_inner(&f.ty);
                    Some(quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> Option<#inner_ty> {
                            self.#name.get()
                        }
                    })
                }
                AccessorKind::PodOptionRef => {
                    let inner_ty = extract_pod_option_inner(&f.ty);
                    Some(quote! {
                        #[inline(always)]
                        pub fn #name(&self) -> Option<&#inner_ty> {
                            self.#name.get_ref()
                        }
                    })
                }
                AccessorKind::Skip => None,
            }
        })
        .collect();

    if methods.is_empty() {
        return quote! {};
    }

    quote! {
        impl #zc_name {
            #( #methods )*
        }
    }
}

pub fn generate_enum(input: &syn::DeriveInput) -> TokenStream {
    let enum_name = &input.ident;
    let zc_name = format_ident!("{}Zc", enum_name);

    // 1. Parse #[repr(uN)] attribute.
    let repr = match parse_enum_repr(input) {
        Some(r) => r,
        None => {
            return quote! {
                compile_error!("ZeroPod enums require #[repr(u8)], #[repr(u16)], #[repr(u32)], or #[repr(u64)]");
            };
        }
    };

    // 2. Extract variants — all must be unit variants with explicit discriminants.
    let variants = match &input.data {
        syn::Data::Enum(data) => &data.variants,
        _ => unreachable!("generate_enum called on non-enum"),
    };

    let mut variant_names = Vec::new();
    let mut discriminant_values = Vec::new();

    for v in variants {
        if !v.fields.is_empty() {
            let msg = format!(
                "ZeroPod enum variant `{}` must be a unit variant (no data fields)",
                v.ident
            );
            return quote! { compile_error!(#msg); };
        }
        let disc = match &v.discriminant {
            Some((_, expr)) => expr.clone(),
            None => {
                let msg = format!(
                    "ZeroPod enum variant `{}` must have an explicit discriminant (e.g. `= 0`)",
                    v.ident
                );
                return quote! { compile_error!(#msg); };
            }
        };
        variant_names.push(&v.ident);
        discriminant_values.push(disc);
    }

    // 3. Map repr to types and sizes.
    let (native_ty, pod_ty, repr_size): (TokenStream, TokenStream, usize) = match repr.as_str() {
        "u8" => (quote! { u8 }, quote! { u8 }, 1),
        "u16" => (quote! { u16 }, quote! { zeropod::pod::PodU16 }, 2),
        "u32" => (quote! { u32 }, quote! { zeropod::pod::PodU32 }, 4),
        "u64" => (quote! { u64 }, quote! { zeropod::pod::PodU64 }, 8),
        _ => unreachable!(),
    };

    // 4. Build the valid discriminant set for validation.
    let valid_arms: Vec<TokenStream> = discriminant_values.iter().map(|d| quote! { #d }).collect();

    // 5. Build the From<Enum> -> PodType match arms.
    let from_arms: Vec<TokenStream> = variant_names
        .iter()
        .zip(discriminant_values.iter())
        .map(|(name, disc)| {
            quote! { #enum_name::#name => (#disc as #native_ty).into() }
        })
        .collect();

    // For repr(u8), the Zc is a newtype wrapping u8 with .get().
    // For repr(u16+), the Zc is the Pod type which already has .get().
    // To unify the interface, we generate a Zc newtype for all cases.
    let read_value = match repr.as_str() {
        "u8" => quote! { self.0[0] },
        _ => quote! { <#native_ty>::from_le_bytes(self.0) },
    };

    quote! {
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub struct #zc_name([u8; #repr_size]);

        impl #zc_name {
            #[inline(always)]
            pub fn get(&self) -> #native_ty {
                #read_value
            }
        }

        impl zeropod::ZcValidate for #zc_name {
            #[allow(clippy::manual_range_patterns)]
            fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
                let v = value.get();
                match v {
                    #( #valid_arms )|* => Ok(()),
                    _ => Err(zeropod::ZeroPodError::InvalidDiscriminant),
                }
            }
        }

        impl zeropod::ZeroPodSchema for #enum_name {
            const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Fixed;
        }

        impl zeropod::ZeroPodFixed for #enum_name {
            type Zc = #zc_name;
            const SIZE: usize = #repr_size;

            fn from_bytes(data: &[u8]) -> Result<&Self::Zc, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &*(data.as_ptr() as *const #zc_name) })
            }

            fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self::Zc, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &mut *(data.as_mut_ptr() as *mut #zc_name) })
            }

            fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
                if data.len() < #repr_size {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __zc = unsafe { &*(data.as_ptr() as *const #zc_name) };
                <#zc_name as zeropod::ZcValidate>::validate_ref(__zc)?;
                Ok(())
            }
        }

        impl From<#enum_name> for #pod_ty {
            fn from(v: #enum_name) -> Self {
                match v {
                    #( #from_arms ),*
                }
            }
        }

        impl zeropod::ZcField for #enum_name {
            type Pod = #zc_name;
            const POD_SIZE: usize = #repr_size;
        }

        // --- Enum ergonomics ---

        impl From<#enum_name> for #zc_name {
            fn from(v: #enum_name) -> Self {
                let raw: #native_ty = match v {
                    #( #enum_name::#variant_names => #discriminant_values as #native_ty ),*
                };
                Self(raw.to_le_bytes())
            }
        }

        impl PartialEq<#enum_name> for #zc_name {
            fn eq(&self, other: &#enum_name) -> bool {
                let other_raw: #native_ty = match other {
                    #( #enum_name::#variant_names => #discriminant_values as #native_ty ),*
                };
                self.get() == other_raw
            }
        }

        impl #zc_name {
            /// Try to convert the raw ZC value back to the enum.
            #[allow(clippy::manual_range_patterns)]
            pub fn try_to_enum(&self) -> Result<#enum_name, zeropod::ZeroPodError> {
                let val = self.get();
                match val {
                    #( #valid_arms => Ok(#enum_name::#variant_names), )*
                    _ => Err(zeropod::ZeroPodError::InvalidDiscriminant),
                }
            }

            pub fn is(&self, variant: #enum_name) -> bool {
                let other: #zc_name = variant.into();
                self.get() == other.get()
            }
        }

        impl core::fmt::Display for #zc_name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.get() {
                    #( #discriminant_values => write!(f, stringify!(#variant_names)), )*
                    other => write!(f, "{}(invalid: {})", stringify!(#enum_name), other),
                }
            }
        }

        impl core::fmt::Debug for #zc_name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.get() {
                    #( #discriminant_values => write!(f, "{}Zc({})", stringify!(#enum_name), stringify!(#variant_names)), )*
                    other => write!(f, "{}Zc(invalid: {})", stringify!(#enum_name), other),
                }
            }
        }

        impl PartialEq for #zc_name {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl PartialEq<#native_ty> for #zc_name {
            fn eq(&self, other: &#native_ty) -> bool {
                self.get() == *other
            }
        }

        // SAFETY: #zc_name is #[repr(transparent)] over [u8; #repr_size], alignment is 1.
        unsafe impl zeropod::ZcElem for #zc_name {}
    }
}

fn parse_enum_repr(input: &syn::DeriveInput) -> Option<String> {
    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            let mut repr_name = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("u8") {
                    repr_name = Some("u8".to_string());
                } else if meta.path.is_ident("u16") {
                    repr_name = Some("u16".to_string());
                } else if meta.path.is_ident("u32") {
                    repr_name = Some("u32".to_string());
                } else if meta.path.is_ident("u64") {
                    repr_name = Some("u64".to_string());
                }
                Ok(())
            });
            if repr_name.is_some() {
                return repr_name;
            }
        }
    }
    None
}
