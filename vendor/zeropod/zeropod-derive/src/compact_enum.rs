use {
    crate::type_map::{classify_field, map_to_pod_type, FieldKind, TailField, TailPayload},
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{Data, DeriveInput, Expr, Fields, Type, Variant},
};

enum VariantPayload {
    Unit,
    String {
        max: Expr,
        pfx: usize,
    },
    Vec {
        elem: Box<Type>,
        max: Expr,
        pfx: usize,
    },
    Fixed {
        ty: Type,
    },
    Compact {
        ty: Type,
        ref_ty: syn::Ident,
    },
}

struct CompactVariant<'a> {
    name: &'a syn::Ident,
    disc: Expr,
    payload: VariantPayload,
}

pub fn generate(input: &DeriveInput) -> TokenStream {
    let enum_name = &input.ident;
    let ref_name = format_ident!("{}Ref", enum_name);
    let mut_name = format_ident!("{}Mut", enum_name);
    let header_name = format_ident!("{}Header", enum_name);

    let repr = match parse_enum_repr(input) {
        Some(r) => r,
        None => {
            return quote! {
                compile_error!("compact ZeroPod enums require #[repr(u8)], #[repr(u16)], #[repr(u32)], or #[repr(u64)]");
            };
        }
    };

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => unreachable!("compact enum generation called on non-enum"),
    };

    let (native_ty, tag_size): (TokenStream, usize) = match repr.as_str() {
        "u8" => (quote! { u8 }, 1),
        "u16" => (quote! { u16 }, 2),
        "u32" => (quote! { u32 }, 4),
        "u64" => (quote! { u64 }, 8),
        _ => unreachable!(),
    };

    let mut parsed = Vec::new();
    for variant in variants {
        let disc = match &variant.discriminant {
            Some((_, expr)) => expr.clone(),
            None => {
                let msg = format!(
                    "compact ZeroPod enum variant `{}` must have an explicit discriminant",
                    variant.ident
                );
                return quote! { compile_error!(#msg); };
            }
        };

        let payload = match parse_payload(variant) {
            Ok(payload) => payload,
            Err(tokens) => return tokens,
        };

        parsed.push(CompactVariant {
            name: &variant.ident,
            disc,
            payload,
        });
    }

    let ref_variants: Vec<_> = parsed
        .iter()
        .map(|variant| {
            let name = variant.name;
            match &variant.payload {
                VariantPayload::Unit => quote! { #name },
                VariantPayload::String { .. } => quote! { #name(&'a str) },
                VariantPayload::Vec { elem, .. } => {
                    let mapped_elem = map_to_pod_type(elem);
                    quote! { #name(&'a [#mapped_elem]) }
                }
                VariantPayload::Fixed { ty } => {
                    quote! { #name(&'a <#ty as zeropod::ZeroPodFixed>::Zc) }
                }
                VariantPayload::Compact { ref_ty, .. } => quote! { #name(#ref_ty<'a>) },
            }
        })
        .collect();

    let validate_arms: Vec<_> = parsed
        .iter()
        .map(|variant| {
            let disc = &variant.disc;
            let validate = validate_payload_tokens(&variant.payload, tag_size);
            quote! { x if x == (#disc as #native_ty) => { #validate } }
        })
        .collect();

    let ref_arms: Vec<_> = parsed
        .iter()
        .map(|variant| {
            let name = variant.name;
            let disc = &variant.disc;
            let construct = construct_ref_tokens(&variant.payload, name, tag_size);
            quote! { x if x == (#disc as #native_ty) => { #construct } }
        })
        .collect();

    let read_tag = read_tag_expr(tag_size, quote! { data });
    let mut_impl = generate_mut_impl(enum_name, &mut_name, &parsed, tag_size, &native_ty);

    quote! {
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct #header_name {
            __tag: [u8; #tag_size],
        }

        pub enum #ref_name<'a> {
            #( #ref_variants ),*
        }

        impl zeropod::ZeroPodSchema for #enum_name {
            const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Compact;
        }

        impl zeropod::ZeroPodCompact for #enum_name {
            type Header = #header_name;
            const HEADER_SIZE: usize = #tag_size;

            fn header(data: &[u8]) -> Result<&Self::Header, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &*(data.as_ptr() as *const #header_name) })
            }

            fn header_mut(data: &mut [u8]) -> Result<&mut Self::Header, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &mut *(data.as_mut_ptr() as *mut #header_name) })
            }

            fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
                if data.len() < #tag_size {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __tag: #native_ty = #read_tag;
                match __tag {
                    #( #validate_arms, )*
                    _ => Err(zeropod::ZeroPodError::InvalidDiscriminant),
                }
            }
        }

        impl<'a> #ref_name<'a> {
            pub fn new(data: &'a [u8]) -> Result<Self, zeropod::ZeroPodError> {
                <#enum_name as zeropod::ZeroPodCompact>::validate(data)?;
                let __tag: #native_ty = #read_tag;
                match __tag {
                    #( #ref_arms, )*
                    _ => Err(zeropod::ZeroPodError::InvalidDiscriminant),
                }
            }
        }

        #mut_impl
    }
}

fn generate_mut_impl(
    enum_name: &syn::Ident,
    mut_name: &syn::Ident,
    variants: &[CompactVariant<'_>],
    tag_size: usize,
    native_ty: &TokenStream,
) -> TokenStream {
    let mut edit_variants = Vec::new();
    let mut setters = Vec::new();
    let mut projected_arms = Vec::new();
    let mut commit_arms = Vec::new();
    let edit_enum = format_ident!("__{}Edit", enum_name);

    for variant in variants {
        let name = variant.name;
        let edit_name = format_ident!("{}", name);
        let setter_name = format_ident!("set_{}", to_snake_case(&name.to_string()));
        let disc = &variant.disc;

        match &variant.payload {
            VariantPayload::Unit => {
                edit_variants.push(quote! { #edit_name });
                setters.push(quote! {
                    pub fn #setter_name(&mut self) -> Result<(), zeropod::ZeroPodError> {
                        self.edit = Some(#edit_enum::#edit_name);
                        Ok(())
                    }
                });
                projected_arms.push(quote! {
                    #edit_enum::#edit_name => #tag_size
                });
                commit_arms.push(quote! {
                    #edit_enum::#edit_name => {
                        write_tag(self.data, (#disc as #native_ty));
                    }
                });
            }
            VariantPayload::String { max, pfx } => {
                edit_variants.push(quote! { #edit_name { ptr: *const u8, len: usize } });
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a str) -> Result<(), zeropod::ZeroPodError> {
                        if value.len() > #max {
                            return Err(zeropod::ZeroPodError::Overflow);
                        }
                        self.edit = Some(#edit_enum::#edit_name {
                            ptr: value.as_ptr(),
                            len: value.len(),
                        });
                        Ok(())
                    }
                });
                projected_arms.push(quote! {
                    #edit_enum::#edit_name { len, .. } => #tag_size + #pfx + len
                });
                commit_arms.push(quote! {
                    #edit_enum::#edit_name { ptr, len } => {
                        write_tag(self.data, (#disc as #native_ty));
                        write_len(self.data, #tag_size, #pfx, len);
                        if len > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    ptr,
                                    self.data.as_mut_ptr().add(#tag_size + #pfx),
                                    len,
                                );
                            }
                        }
                    }
                });
            }
            VariantPayload::Vec { elem, max, pfx } => {
                let mapped_elem = map_to_pod_type(elem);
                edit_variants
                    .push(quote! { #edit_name { ptr: *const u8, count: usize, elem_size: usize } });
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a [#mapped_elem]) -> Result<(), zeropod::ZeroPodError> {
                        if value.len() > #max {
                            return Err(zeropod::ZeroPodError::Overflow);
                        }
                        self.edit = Some(#edit_enum::#edit_name {
                            ptr: value.as_ptr() as *const u8,
                            count: value.len(),
                            elem_size: core::mem::size_of::<#mapped_elem>(),
                        });
                        Ok(())
                    }
                });
                projected_arms.push(quote! {
                    #edit_enum::#edit_name { count, elem_size, .. } => {
                        #tag_size + #pfx + count * elem_size
                    }
                });
                commit_arms.push(quote! {
                    #edit_enum::#edit_name { ptr, count, elem_size } => {
                        let __byte_len = count * elem_size;
                        write_tag(self.data, (#disc as #native_ty));
                        write_len(self.data, #tag_size, #pfx, count);
                        if __byte_len > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    ptr,
                                    self.data.as_mut_ptr().add(#tag_size + #pfx),
                                    __byte_len,
                                );
                            }
                        }
                    }
                });
            }
            VariantPayload::Fixed { ty } => {
                edit_variants.push(quote! { #edit_name { ptr: *const u8 } });
                setters.push(quote! {
                    pub fn #setter_name(
                        &mut self,
                        value: &'a <#ty as zeropod::ZeroPodFixed>::Zc,
                    ) -> Result<(), zeropod::ZeroPodError> {
                        self.edit = Some(#edit_enum::#edit_name {
                            ptr: value as *const <#ty as zeropod::ZeroPodFixed>::Zc as *const u8,
                        });
                        Ok(())
                    }
                });
                projected_arms.push(quote! {
                    #edit_enum::#edit_name { .. } => {
                        #tag_size + <#ty as zeropod::ZeroPodFixed>::SIZE
                    }
                });
                commit_arms.push(quote! {
                    #edit_enum::#edit_name { ptr } => {
                        let __byte_len = <#ty as zeropod::ZeroPodFixed>::SIZE;
                        write_tag(self.data, (#disc as #native_ty));
                        if __byte_len > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    ptr,
                                    self.data.as_mut_ptr().add(#tag_size),
                                    __byte_len,
                                );
                            }
                        }
                    }
                });
            }
            VariantPayload::Compact { ty, .. } => {
                edit_variants.push(quote! { #edit_name { ptr: *const u8, len: usize } });
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a [u8]) -> Result<(), zeropod::ZeroPodError> {
                        <#ty as zeropod::ZeroPodCompact>::validate(value)?;
                        self.edit = Some(#edit_enum::#edit_name {
                            ptr: value.as_ptr(),
                            len: value.len(),
                        });
                        Ok(())
                    }
                });
                projected_arms.push(quote! {
                    #edit_enum::#edit_name { len, .. } => #tag_size + len
                });
                commit_arms.push(quote! {
                    #edit_enum::#edit_name { ptr, len } => {
                        write_tag(self.data, (#disc as #native_ty));
                        if len > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    ptr,
                                    self.data.as_mut_ptr().add(#tag_size),
                                    len,
                                );
                            }
                        }
                    }
                });
            }
        }
    }

    let write_tag = write_tag_fn(tag_size, native_ty);
    let write_len = write_len_fn();

    quote! {
        enum #edit_enum<'a> {
            #( #edit_variants ),*,
            #[allow(dead_code)]
            __Lifetime(core::marker::PhantomData<&'a ()>),
        }

        pub struct #mut_name<'a> {
            data: &'a mut [u8],
            edit: Option<#edit_enum<'a>>,
        }

        impl<'a> #mut_name<'a> {
            pub fn new(data: &'a mut [u8]) -> Result<Self, zeropod::ZeroPodError> {
                <#enum_name as zeropod::ZeroPodCompact>::validate(data)?;
                Ok(Self { data, edit: None })
            }

            /// # Safety
            /// Caller must ensure `data` contains a valid compact enum value.
            pub unsafe fn new_unchecked(data: &'a mut [u8]) -> Self {
                Self { data, edit: None }
            }

            #( #setters )*

            pub fn projected_size(&self) -> usize {
                match self.edit.as_ref() {
                    Some(edit) => match edit {
                        #( #projected_arms, )*
                        #edit_enum::__Lifetime(_) => unreachable!(),
                    },
                    None => self.data.len(),
                }
            }

            pub fn commit(&mut self) -> Result<usize, zeropod::ZeroPodError> {
                let __new_size = self.projected_size();
                if __new_size > self.data.len() {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }

                if let Some(edit) = self.edit.take() {
                    #write_tag
                    #write_len
                    match edit {
                        #( #commit_arms, )*
                        #edit_enum::__Lifetime(_) => unreachable!(),
                    }
                }

                Ok(__new_size)
            }
        }
    }
}

fn parse_payload(variant: &Variant) -> Result<VariantPayload, TokenStream> {
    match &variant.fields {
        Fields::Unit => Ok(VariantPayload::Unit),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = fields.unnamed[0].ty.clone();
            if has_compact_attr(&variant.attrs) {
                let ref_ty = compact_ref_ident(&ty).ok_or_else(|| {
                    let msg = format!(
                        "compact ZeroPod enum variant `{}` uses #[zeropod(compact)] with an unsupported payload type",
                        variant.ident
                    );
                    quote! { compile_error!(#msg); }
                })?;
                return Ok(VariantPayload::Compact { ty, ref_ty });
            }

            match classify_field(&ty) {
                FieldKind::Tail(TailField::Segment {
                    payload: TailPayload::String { max, pfx },
                    ..
                }) => Ok(VariantPayload::String { max, pfx }),
                FieldKind::Tail(TailField::Segment {
                    payload: TailPayload::Vec { elem, max, pfx },
                    ..
                }) => Ok(VariantPayload::Vec { elem, max, pfx }),
                _ => Ok(VariantPayload::Fixed { ty }),
            }
        }
        _ => {
            let msg = format!(
                "compact ZeroPod enum variant `{}` must be unit-like or contain exactly one unnamed payload field",
                variant.ident
            );
            Err(quote! { compile_error!(#msg); })
        }
    }
}

fn validate_payload_tokens(payload: &VariantPayload, tag_size: usize) -> TokenStream {
    match payload {
        VariantPayload::Unit => quote! { Ok(()) },
        VariantPayload::String { max, pfx } => {
            let read_len = read_len_at_expr(quote! { data }, quote! { #tag_size }, *pfx);
            quote! {
                if data.len() < #tag_size + #pfx {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __byte_len = #read_len;
                if __byte_len > #max {
                    return Err(zeropod::ZeroPodError::InvalidLength);
                }
                let __payload_offset = #tag_size + #pfx;
                if data.len() < __payload_offset + __byte_len {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                if core::str::from_utf8(&data[__payload_offset..__payload_offset + __byte_len]).is_err() {
                    return Err(zeropod::ZeroPodError::InvalidUtf8);
                }
                Ok(())
            }
        }
        VariantPayload::Vec { elem, max, pfx } => {
            let mapped_elem = map_to_pod_type(elem);
            let read_len = read_len_at_expr(quote! { data }, quote! { #tag_size }, *pfx);
            quote! {
                if data.len() < #tag_size + #pfx {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __count = #read_len;
                if __count > #max {
                    return Err(zeropod::ZeroPodError::InvalidLength);
                }
                let __payload_offset = #tag_size + #pfx;
                let __elem_size = core::mem::size_of::<#mapped_elem>();
                let __byte_len = __count * __elem_size;
                if data.len() < __payload_offset + __byte_len {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                for __i in 0..__count {
                    let __elem_ptr = unsafe {
                        &*(data.as_ptr().add(__payload_offset + __i * __elem_size) as *const #mapped_elem)
                    };
                    <#mapped_elem as zeropod::ZcValidate>::validate_ref(__elem_ptr)?;
                }
                Ok(())
            }
        }
        VariantPayload::Fixed { ty } => quote! {
            if data.len() < #tag_size + <#ty as zeropod::ZeroPodFixed>::SIZE {
                return Err(zeropod::ZeroPodError::BufferTooSmall);
            }
            <#ty as zeropod::ZeroPodFixed>::validate(&data[#tag_size..])?;
            Ok(())
        },
        VariantPayload::Compact { ty, .. } => quote! {
            <#ty as zeropod::ZeroPodCompact>::validate(&data[#tag_size..])?;
            Ok(())
        },
    }
}

fn construct_ref_tokens(
    payload: &VariantPayload,
    name: &syn::Ident,
    tag_size: usize,
) -> TokenStream {
    match payload {
        VariantPayload::Unit => quote! { Ok(Self::#name) },
        VariantPayload::String { pfx, .. } => {
            let read_len = read_len_at_expr(quote! { data }, quote! { #tag_size }, *pfx);
            quote! {
                let __byte_len = #read_len;
                let __payload_offset = #tag_size + #pfx;
                let __bytes = &data[__payload_offset..__payload_offset + __byte_len];
                Ok(Self::#name(unsafe { core::str::from_utf8_unchecked(__bytes) }))
            }
        }
        VariantPayload::Vec { elem, pfx, .. } => {
            let mapped_elem = map_to_pod_type(elem);
            let read_len = read_len_at_expr(quote! { data }, quote! { #tag_size }, *pfx);
            quote! {
                let __count = #read_len;
                let __payload_offset = #tag_size + #pfx;
                let __ptr = unsafe { data.as_ptr().add(__payload_offset) as *const #mapped_elem };
                Ok(Self::#name(unsafe { core::slice::from_raw_parts(__ptr, __count) }))
            }
        }
        VariantPayload::Fixed { ty } => quote! {
            Ok(Self::#name(<#ty as zeropod::ZeroPodFixed>::from_bytes(&data[#tag_size..])?))
        },
        VariantPayload::Compact { ref_ty, .. } => quote! {
            Ok(Self::#name(#ref_ty::new(&data[#tag_size..])?))
        },
    }
}

fn read_tag_expr(tag_size: usize, data: TokenStream) -> TokenStream {
    match tag_size {
        1 => quote! { #data[0] },
        2 => quote! { u16::from_le_bytes([#data[0], #data[1]]) },
        4 => quote! { u32::from_le_bytes([#data[0], #data[1], #data[2], #data[3]]) },
        8 => quote! {
            u64::from_le_bytes([
                #data[0], #data[1], #data[2], #data[3],
                #data[4], #data[5], #data[6], #data[7],
            ])
        },
        _ => unreachable!("invalid repr size"),
    }
}

fn read_len_at_expr(data: TokenStream, offset: TokenStream, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { #data[#offset] as usize },
        2 => quote! { u16::from_le_bytes([#data[#offset], #data[#offset + 1]]) as usize },
        4 => quote! {
            u32::from_le_bytes([
                #data[#offset],
                #data[#offset + 1],
                #data[#offset + 2],
                #data[#offset + 3],
            ]) as usize
        },
        8 => quote! {
            u64::from_le_bytes([
                #data[#offset],
                #data[#offset + 1],
                #data[#offset + 2],
                #data[#offset + 3],
                #data[#offset + 4],
                #data[#offset + 5],
                #data[#offset + 6],
                #data[#offset + 7],
            ]) as usize
        },
        _ => unreachable!("invalid prefix size"),
    }
}

fn write_tag_fn(tag_size: usize, native_ty: &TokenStream) -> TokenStream {
    match tag_size {
        1 => quote! {
            fn write_tag(data: &mut [u8], value: #native_ty) {
                data[0] = value as u8;
            }
        },
        2 | 4 | 8 => quote! {
            fn write_tag(data: &mut [u8], value: #native_ty) {
                let bytes = value.to_le_bytes();
                data[..#tag_size].copy_from_slice(&bytes[..#tag_size]);
            }
        },
        _ => unreachable!("invalid repr size"),
    }
}

fn write_len_fn() -> TokenStream {
    quote! {
        fn write_len(data: &mut [u8], offset: usize, pfx: usize, value: usize) {
            let bytes = (value as u64).to_le_bytes();
            data[offset..offset + pfx].copy_from_slice(&bytes[..pfx]);
        }
    }
}

fn to_snake_case(value: &str) -> String {
    let mut out = String::new();
    for (i, ch) in value.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn compact_ref_ident(ty: &Type) -> Option<syn::Ident> {
    let path = match ty {
        Type::Path(path) => &path.path,
        _ => return None,
    };
    let ident = &path.segments.last()?.ident;
    Some(format_ident!("{}Ref", ident))
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

fn parse_enum_repr(input: &DeriveInput) -> Option<String> {
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
