use {
    crate::{
        schema::Schema,
        type_map::{map_to_pod_type, FieldKind, TailField, TailPayload, TailPresence},
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

pub fn generate(schema: &Schema) -> TokenStream {
    let struct_name = &schema.name;
    let header_name = format_ident!("{}Header", struct_name);
    let ref_name = format_ident!("{}Ref", struct_name);
    let mut_name = format_ident!("{}Mut", struct_name);
    let (_, ty_generics, _) = schema.generics.split_for_impl();
    let header_ty = quote! { #header_name #ty_generics };

    let header_ts = generate_header(schema, &header_name);
    let trait_impl_ts = generate_trait_impl(schema, &header_ty);
    let ref_ts = generate_ref(schema, &header_ty, &ref_name);
    let mut_ts = generate_mut(schema, &header_ty, &mut_name);

    quote! {
        #header_ts
        #trait_impl_ts
        #ref_ts
        #mut_ts
    }
}

// ---------------------------------------------------------------------------
// Header generation
// ---------------------------------------------------------------------------

fn generate_header(schema: &Schema, header_name: &syn::Ident) -> TokenStream {
    let generics = &schema.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut fields = Vec::new();

    let inline_field_names: Vec<&syn::Ident> = schema.inline_fields().map(|f| &f.name).collect();
    let inline_pod_types: Vec<TokenStream> = schema
        .inline_fields()
        .map(|f| map_to_pod_type(&f.ty))
        .collect();

    for f in schema.inline_fields() {
        let name = &f.name;
        let vis = &f.vis;
        let pod_ty = map_to_pod_type(&f.ty);
        fields.push(quote! { #vis #name: #pod_ty });
    }

    for f in schema.tail_fields() {
        match &f.kind {
            FieldKind::Tail(tail) => match tail.presence() {
                TailPresence::Always => {
                    let len_name = format_ident!("__{}_len", f.name);
                    let pfx = tail.payload().pfx();
                    fields.push(quote! { #len_name: [u8; #pfx] });
                }
                TailPresence::OptionTag => {
                    let tag_name = format_ident!("__{}_tag", f.name);
                    fields.push(quote! { #tag_name: [u8; 1] });
                }
            },
            _ => unreachable!(),
        }
    }

    let pod_bounds: Vec<_> = inline_pod_types
        .iter()
        .map(|pod_ty| quote! { #pod_ty: zeropod::ZcValidate })
        .collect();
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, pod_bounds.iter());

    let align_assert = if schema.generics.params.is_empty() {
        quote! {
            const _: () = assert!(core::mem::align_of::<#header_name>() == 1);
        }
    } else {
        quote! {}
    };

    quote! {
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct #header_name #generics #where_clause_with_bounds {
            #( #fields ),*
        }

        #align_assert

        impl #impl_generics zeropod::ZcValidate for #header_name #ty_generics #where_clause_with_bounds {
            fn validate_ref(value: &Self) -> Result<(), zeropod::ZeroPodError> {
                #(<#inline_pod_types as zeropod::ZcValidate>::validate_ref(&value.#inline_field_names)?;)*
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ZeroPodCompact trait impl
// ---------------------------------------------------------------------------

fn generate_trait_impl(schema: &Schema, header_ty: &TokenStream) -> TokenStream {
    let struct_name = &schema.name;
    let (impl_generics, ty_generics, where_clause) = schema.generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let mut tail_validations = Vec::new();
    for f in schema.tail_fields() {
        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { max, pfx },
            }) => {
                let len_name = format_ident!("__{}_len", f.name);
                let read_len = read_len_expr(&len_name, *pfx);
                tail_validations.push(quote! {
                    let #len_name = #read_len;
                    if #len_name > #max {
                        return Err(zeropod::ZeroPodError::InvalidLength);
                    }
                    if __tail_offset + #len_name > data.len() {
                        return Err(zeropod::ZeroPodError::BufferTooSmall);
                    }
                    if core::str::from_utf8(&data[__tail_offset..__tail_offset + #len_name]).is_err() {
                        return Err(zeropod::ZeroPodError::InvalidUtf8);
                    }
                    __tail_offset += #len_name;
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let len_name = format_ident!("__{}_len", f.name);
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                tail_validations.push(quote! {
                    let #len_name = #read_len;
                    if #len_name > #max {
                        return Err(zeropod::ZeroPodError::InvalidLength);
                    }
                    let __byte_len = #len_name * core::mem::size_of::<#mapped_elem>();
                    if __tail_offset + __byte_len > data.len() {
                        return Err(zeropod::ZeroPodError::BufferTooSmall);
                    }
                    let __elem_size = core::mem::size_of::<#mapped_elem>();
                    for __i in 0..#len_name {
                        let __elem_ptr = unsafe {
                            &*(data.as_ptr().add(__tail_offset + __i * __elem_size) as *const #mapped_elem)
                        };
                        <#mapped_elem as zeropod::ZcValidate>::validate_ref(__elem_ptr)?;
                    }
                    __tail_offset += __byte_len;
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { max, pfx },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let read_payload_len = read_data_len_expr(quote! { __tail_offset }, *pfx);
                tail_validations.push(quote! {
                    match __hdr.#tag_name[0] {
                        0 => {}
                        1 => {
                            if __tail_offset + #pfx > data.len() {
                                return Err(zeropod::ZeroPodError::BufferTooSmall);
                            }
                            let __byte_len = #read_payload_len;
                            if __byte_len > #max {
                                return Err(zeropod::ZeroPodError::InvalidLength);
                            }
                            let __payload_offset = __tail_offset + #pfx;
                            if __payload_offset + __byte_len > data.len() {
                                return Err(zeropod::ZeroPodError::BufferTooSmall);
                            }
                            if core::str::from_utf8(&data[__payload_offset..__payload_offset + __byte_len]).is_err() {
                                return Err(zeropod::ZeroPodError::InvalidUtf8);
                            }
                            __tail_offset = __payload_offset + __byte_len;
                        }
                        _ => return Err(zeropod::ZeroPodError::InvalidTag),
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, max, pfx },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let mapped_elem = map_to_pod_type(elem);
                let read_payload_len = read_data_len_expr(quote! { __tail_offset }, *pfx);
                tail_validations.push(quote! {
                    match __hdr.#tag_name[0] {
                        0 => {}
                        1 => {
                            if __tail_offset + #pfx > data.len() {
                                return Err(zeropod::ZeroPodError::BufferTooSmall);
                            }
                            let __count = #read_payload_len;
                            if __count > #max {
                                return Err(zeropod::ZeroPodError::InvalidLength);
                            }
                            let __payload_offset = __tail_offset + #pfx;
                            let __elem_size = core::mem::size_of::<#mapped_elem>();
                            let __byte_len = __count * __elem_size;
                            if __payload_offset + __byte_len > data.len() {
                                return Err(zeropod::ZeroPodError::BufferTooSmall);
                            }
                            for __i in 0..__count {
                                let __elem_ptr = unsafe {
                                    &*(data.as_ptr().add(__payload_offset + __i * __elem_size) as *const #mapped_elem)
                                };
                                <#mapped_elem as zeropod::ZcValidate>::validate_ref(__elem_ptr)?;
                            }
                            __tail_offset = __payload_offset + __byte_len;
                        }
                        _ => return Err(zeropod::ZeroPodError::InvalidTag),
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    quote! {
        impl #impl_generics zeropod::ZeroPodSchema for #struct_name #ty_generics #where_clause_with_bounds {
            const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Compact;
        }

        impl #impl_generics zeropod::ZeroPodCompact for #struct_name #ty_generics #where_clause_with_bounds {
            type Header = #header_ty;
            const HEADER_SIZE: usize = core::mem::size_of::<#header_ty>();

            fn header(data: &[u8]) -> Result<&Self::Header, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &*(data.as_ptr() as *const #header_ty) })
            }

            fn header_mut(data: &mut [u8]) -> Result<&mut Self::Header, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &mut *(data.as_mut_ptr() as *mut #header_ty) })
            }

            fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
                if data.len() < core::mem::size_of::<#header_ty>() {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __hdr = unsafe { &*(data.as_ptr() as *const #header_ty) };
                <#header_ty as zeropod::ZcValidate>::validate_ref(__hdr)?;
                let mut __tail_offset = core::mem::size_of::<#header_ty>();
                #( #tail_validations )*
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ref generation
// ---------------------------------------------------------------------------

fn generate_ref(schema: &Schema, header_ty: &TokenStream, ref_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let (_, struct_ty_generics, where_clause) = schema.generics.split_for_impl();
    let ref_generics = generics_with_lifetime(&schema.generics);
    let (ref_impl_generics, ref_ty_generics, _) = ref_generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let tail_fields: Vec<_> = schema.tail_fields().collect();
    let mut accessors = Vec::new();

    for (i, f) in tail_fields.iter().enumerate() {
        let fname = &f.name;
        let offset_computation = compute_offset_tokens(header_ty, &tail_fields, i);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let len_name = format_ident!("__{}_len", fname);
                let read_len = read_len_expr(&len_name, *pfx);
                accessors.push(quote! {
                    pub fn #fname(&self) -> &'a str {
                        let __hdr = self.header();
                        let __byte_len = #read_len;
                        #offset_computation
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__offset);
                            let __slice = core::slice::from_raw_parts(__ptr, __byte_len);
                            core::str::from_utf8_unchecked(__slice)
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let len_name = format_ident!("__{}_len", fname);
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                accessors.push(quote! {
                    pub fn #fname(&self) -> &'a [#mapped_elem] {
                        let __hdr = self.header();
                        let __count = #read_len;
                        #offset_computation
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__offset) as *const #mapped_elem;
                            core::slice::from_raw_parts(__ptr, __count)
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                accessors.push(quote! {
                    pub fn #fname(&self) -> Option<&'a str> {
                        let __hdr = self.header();
                        if __hdr.#tag_name[0] == 0 {
                            return None;
                        }
                        #offset_computation
                        let __byte_len = #read_len;
                        let __payload_offset = __offset + #pfx;
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__payload_offset);
                            let __slice = core::slice::from_raw_parts(__ptr, __byte_len);
                            Some(core::str::from_utf8_unchecked(__slice))
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                accessors.push(quote! {
                    pub fn #fname(&self) -> Option<&'a [#mapped_elem]> {
                        let __hdr = self.header();
                        if __hdr.#tag_name[0] == 0 {
                            return None;
                        }
                        #offset_computation
                        let __count = #read_len;
                        let __payload_offset = __offset + #pfx;
                        unsafe {
                            let __ptr = self.data.as_ptr().add(__payload_offset) as *const #mapped_elem;
                            Some(core::slice::from_raw_parts(__ptr, __count))
                        }
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    quote! {
        pub struct #ref_name #ref_generics #where_clause_with_bounds {
            data: &'a [u8],
        }

        impl #ref_impl_generics core::ops::Deref for #ref_name #ref_ty_generics #where_clause_with_bounds {
            type Target = #header_ty;
            fn deref(&self) -> &#header_ty {
                self.header()
            }
        }

        impl #ref_impl_generics #ref_name #ref_ty_generics #where_clause_with_bounds {
            pub fn new(data: &'a [u8]) -> Result<Self, zeropod::ZeroPodError> {
                <#struct_name #struct_ty_generics as zeropod::ZeroPodCompact>::validate(data)?;
                Ok(Self { data })
            }

            pub unsafe fn new_unchecked(data: &'a [u8]) -> Self {
                Self { data }
            }

            fn header(&self) -> &'a #header_ty {
                unsafe { &*(self.data.as_ptr() as *const #header_ty) }
            }

            #( #accessors )*
        }
    }
}

// ---------------------------------------------------------------------------
// Mut generation
// ---------------------------------------------------------------------------

fn generate_mut(schema: &Schema, header_ty: &TokenStream, mut_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let (_, struct_ty_generics, where_clause) = schema.generics.split_for_impl();
    let mut_generics = generics_with_lifetime(&schema.generics);
    let (mut_impl_generics, mut_ty_generics, _) = mut_generics.split_for_impl();
    let bounds = compact_bounds(schema);
    let where_clause_with_bounds = where_clause_with_bounds(where_clause, bounds.iter());
    let tail_fields: Vec<_> = schema.tail_fields().collect();

    // Edit descriptor fields.
    let mut edit_fields = Vec::new();
    for f in &tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                payload: TailPayload::String { .. },
                ..
            }) => {
                edit_fields.push(quote! { #edit_name: Option<(*const u8, usize)> });
            }
            FieldKind::Tail(TailField::Segment {
                payload: TailPayload::Vec { .. },
                ..
            }) => {
                edit_fields.push(quote! { #edit_name: Option<(*const u8, usize, usize)> });
            }
            _ => unreachable!(),
        }
    }

    let edit_inits: Vec<_> = tail_fields
        .iter()
        .map(|f| {
            let edit_name = format_ident!("__{}_edit", f.name);
            quote! { #edit_name: None }
        })
        .collect();

    // Setter methods.
    let mut setters = Vec::new();
    for f in &tail_fields {
        let fname = &f.name;
        let setter_name = format_ident!("set_{}", fname);
        let edit_name = format_ident!("__{}_edit", fname);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { max, .. },
            }) => {
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a str) -> Result<(), zeropod::ZeroPodError> {
                        if value.len() > #max {
                            return Err(zeropod::ZeroPodError::Overflow);
                        }
                        self.#edit_name = Some((value.as_ptr(), value.len()));
                        Ok(())
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, max, .. },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a [#mapped_elem]) -> Result<(), zeropod::ZeroPodError> {
                        if value.len() > #max {
                            return Err(zeropod::ZeroPodError::Overflow);
                        }
                        self.#edit_name = Some((
                            value.as_ptr() as *const u8,
                            value.len(),
                            core::mem::size_of::<#mapped_elem>(),
                        ));
                        Ok(())
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { max, .. },
            }) => {
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: Option<&'a str>) -> Result<(), zeropod::ZeroPodError> {
                        if let Some(value) = value {
                            if value.len() > #max {
                                return Err(zeropod::ZeroPodError::Overflow);
                            }
                            self.#edit_name = Some((value.as_ptr(), value.len()));
                        } else {
                            self.#edit_name = Some((core::ptr::null(), 0));
                        }
                        Ok(())
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, max, .. },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: Option<&'a [#mapped_elem]>) -> Result<(), zeropod::ZeroPodError> {
                        if let Some(value) = value {
                            if value.len() > #max {
                                return Err(zeropod::ZeroPodError::Overflow);
                            }
                            self.#edit_name = Some((
                                value.as_ptr() as *const u8,
                                value.len(),
                                core::mem::size_of::<#mapped_elem>(),
                            ));
                        } else {
                            self.#edit_name = Some((core::ptr::null(), 0, core::mem::size_of::<#mapped_elem>()));
                        }
                        Ok(())
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // projected_size()
    let mut proj_parts = Vec::new();
    for (i, f) in tail_fields.iter().enumerate() {
        let edit_name = format_ident!("__{}_edit", f.name);
        let len_name = format_ident!("__{}_len", f.name);
        let offset_computation = compute_offset_tokens(header_ty, &tail_fields, i);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                proj_parts.push(quote! {
                    + if let Some((_, byte_len)) = self.#edit_name {
                        byte_len
                    } else {
                        let __hdr = self.header();
                        #read_len
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                proj_parts.push(quote! {
                    + if let Some((_, count, elem_size)) = self.#edit_name {
                        count * elem_size
                    } else {
                        let __hdr = self.header();
                        let __count = #read_len;
                        __count * core::mem::size_of::<#mapped_elem>()
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let old_size =
                    old_option_string_size_unchecked_expr(&tag_name, *pfx, quote! { __offset });
                proj_parts.push(quote! {
                    + if let Some((ptr, byte_len)) = self.#edit_name {
                        if ptr.is_null() { 0 } else { #pfx + byte_len }
                    } else {
                        let __hdr = self.header();
                        #offset_computation
                        #old_size
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let mapped_elem = map_to_pod_type(elem);
                let old_size = old_option_vec_size_unchecked_expr(
                    &tag_name,
                    *pfx,
                    quote! { __offset },
                    &mapped_elem,
                );
                proj_parts.push(quote! {
                    + if let Some((ptr, count, elem_size)) = self.#edit_name {
                        if ptr.is_null() { 0 } else { #pfx + count * elem_size }
                    } else {
                        let __hdr = self.header();
                        #offset_computation
                        #old_size
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // commit()
    let commit_body = generate_commit_body(header_ty, &tail_fields);

    quote! {
        pub struct #mut_name #mut_generics #where_clause_with_bounds {
            data: &'a mut [u8],
            total_len: usize,
            #( #edit_fields ),*
        }

        impl #mut_impl_generics core::ops::Deref for #mut_name #mut_ty_generics #where_clause_with_bounds {
            type Target = #header_ty;
            fn deref(&self) -> &#header_ty {
                self.header()
            }
        }

        impl #mut_impl_generics core::ops::DerefMut for #mut_name #mut_ty_generics #where_clause_with_bounds {
            fn deref_mut(&mut self) -> &mut #header_ty {
                self.header_mut()
            }
        }

        impl #mut_impl_generics #mut_name #mut_ty_generics #where_clause_with_bounds {
            pub fn new(data: &'a mut [u8]) -> Result<Self, zeropod::ZeroPodError> {
                <#struct_name #struct_ty_generics as zeropod::ZeroPodCompact>::validate(data)?;
                let total_len = data.len();
                Ok(Self {
                    data,
                    total_len,
                    #( #edit_inits ),*
                })
            }

            /// # Safety
            /// Caller must ensure `data` is at least `HEADER_SIZE` bytes and
            /// contains a valid compact header. The tail region must be
            /// consistent with the header length prefixes.
            pub unsafe fn new_unchecked(data: &'a mut [u8]) -> Self {
                let total_len = data.len();
                Self {
                    data,
                    total_len,
                    #( #edit_inits ),*
                }
            }

            fn header(&self) -> &#header_ty {
                unsafe { &*(self.data.as_ptr() as *const #header_ty) }
            }

            fn header_mut(&mut self) -> &mut #header_ty {
                unsafe { &mut *(self.data.as_mut_ptr() as *mut #header_ty) }
            }

            #( #setters )*

            pub fn projected_size(&self) -> usize {
                core::mem::size_of::<#header_ty>()
                #( #proj_parts )*
            }

            #commit_body
        }
    }
}

fn generate_commit_body(
    header_ty: &TokenStream,
    tail_fields: &[&crate::schema::SchemaField],
) -> TokenStream {
    if tail_fields.is_empty() {
        return quote! {
            pub fn commit(&mut self) -> Result<usize, zeropod::ZeroPodError> {
                Ok(core::mem::size_of::<#header_ty>())
            }
        };
    }

    let header_size = quote! { core::mem::size_of::<#header_ty>() };

    // Step 1: compute per-field old/new offsets and lengths in field order.
    let mut setup_positions = Vec::new();
    for (i, f) in tail_fields.iter().enumerate() {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let len_name = format_ident!("__{}_len", fname);
        let old_off_var = format_ident!("__old_off_{}", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        let old_len_var = format_ident!("__old_len_{}", fname);
        let new_len_var = format_ident!("__new_len_{}", fname);

        let offsets = if i == 0 {
            quote! {
                let #old_off_var: usize = #header_size;
                let #new_off_var: usize = #header_size;
            }
        } else {
            let prev_f = &tail_fields[i - 1];
            let prev_old_off = format_ident!("__old_off_{}", prev_f.name);
            let prev_new_off = format_ident!("__new_off_{}", prev_f.name);
            let prev_old_len = format_ident!("__old_len_{}", prev_f.name);
            let prev_new_len = format_ident!("__new_len_{}", prev_f.name);
            quote! {
                let #old_off_var: usize = #prev_old_off + #prev_old_len;
                let #new_off_var: usize = #prev_new_off + #prev_new_len;
            }
        };

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        #read_len
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((_, __bl)) => __bl,
                        None => #old_len_var,
                    };
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let read_len = read_len_expr(&len_name, *pfx);
                let mapped_elem = map_to_pod_type(elem);
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        let __count = #read_len;
                        __count * core::mem::size_of::<#mapped_elem>()
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((_, __count, __sz)) => __count * __sz,
                        None => #old_len_var,
                    };
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let old_size =
                    old_option_string_size_expr(&tag_name, *pfx, quote! { #old_off_var });
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        #old_size
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((ptr, __bl)) => {
                            if ptr.is_null() { 0 } else { #pfx + __bl }
                        }
                        None => #old_len_var,
                    };
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", fname);
                let mapped_elem = map_to_pod_type(elem);
                let old_size = old_option_vec_size_expr(
                    &tag_name,
                    *pfx,
                    quote! { #old_off_var },
                    &mapped_elem,
                );
                setup_positions.push(quote! {
                    #offsets
                    let #old_len_var: usize = {
                        let __hdr = self.header();
                        #old_size
                    };
                    let #new_len_var: usize = match self.#edit_name {
                        Some((ptr, __count, __sz)) => {
                            if ptr.is_null() { 0 } else { #pfx + __count * __sz }
                        }
                        None => #old_len_var,
                    };
                });
            }
            _ => unreachable!(),
        }
    }

    let last_f = tail_fields.last().unwrap();
    let last_new_off = format_ident!("__new_off_{}", last_f.name);
    let last_new_len = format_ident!("__new_len_{}", last_f.name);

    // Phase 1a: unedited fields that shift backward, in forward iteration order.
    // Phase 1b: unedited fields that shift forward, in reverse iteration order.
    // This two-pass ordering ensures source bytes are never read after being
    // overwritten by an earlier step, regardless of mixed grow/shrink edits.
    let mut phase_1a = Vec::new();
    for f in tail_fields.iter() {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let old_off_var = format_ident!("__old_off_{}", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        let old_len_var = format_ident!("__old_len_{}", fname);
        phase_1a.push(quote! {
            if self.#edit_name.is_none()
                && #new_off_var < #old_off_var
                && #old_len_var > 0
            {
                unsafe {
                    core::ptr::copy(
                        __buf_ptr.add(#old_off_var) as *const u8,
                        __buf_ptr.add(#new_off_var),
                        #old_len_var,
                    );
                }
            }
        });
    }

    let mut phase_1b = Vec::new();
    for f in tail_fields.iter().rev() {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let old_off_var = format_ident!("__old_off_{}", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        let old_len_var = format_ident!("__old_len_{}", fname);
        phase_1b.push(quote! {
            if self.#edit_name.is_none()
                && #new_off_var > #old_off_var
                && #old_len_var > 0
            {
                unsafe {
                    core::ptr::copy(
                        __buf_ptr.add(#old_off_var) as *const u8,
                        __buf_ptr.add(#new_off_var),
                        #old_len_var,
                    );
                }
            }
        });
    }

    // Phase 2: write edited fields to their final positions.
    let mut phase_2 = Vec::new();
    for f in tail_fields {
        let fname = &f.name;
        let edit_name = format_ident!("__{}_edit", fname);
        let new_off_var = format_ident!("__new_off_{}", fname);
        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __new_byte_len)) = self.#edit_name {
                        if __new_byte_len > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    __src_ptr,
                                    __buf_ptr.add(#new_off_var),
                                    __new_byte_len,
                                );
                            }
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __count, __elem_size)) = self.#edit_name {
                        let __new_byte_len = __count * __elem_size;
                        if __new_byte_len > 0 {
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    __src_ptr,
                                    __buf_ptr.add(#new_off_var),
                                    __new_byte_len,
                                );
                            }
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __new_byte_len)) = self.#edit_name {
                        if !__src_ptr.is_null() {
                            let __bytes = (__new_byte_len as u64).to_le_bytes();
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    __bytes.as_ptr(),
                                    __buf_ptr.add(#new_off_var),
                                    #pfx,
                                );
                                if __new_byte_len > 0 {
                                    core::ptr::copy_nonoverlapping(
                                        __src_ptr,
                                        __buf_ptr.add(#new_off_var + #pfx),
                                        __new_byte_len,
                                    );
                                }
                            }
                        }
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { pfx, .. },
            }) => {
                phase_2.push(quote! {
                    if let Some((__src_ptr, __count, __elem_size)) = self.#edit_name {
                        if !__src_ptr.is_null() {
                            let __bytes = (__count as u64).to_le_bytes();
                            let __new_byte_len = __count * __elem_size;
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    __bytes.as_ptr(),
                                    __buf_ptr.add(#new_off_var),
                                    #pfx,
                                );
                                if __new_byte_len > 0 {
                                    core::ptr::copy_nonoverlapping(
                                        __src_ptr,
                                        __buf_ptr.add(#new_off_var + #pfx),
                                        __new_byte_len,
                                    );
                                }
                            }
                        }
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // Update header length prefixes for edited fields.
    let mut update_lens = Vec::new();
    for f in tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        let len_name = format_ident!("__{}_len", f.name);
        let pfx_lit = tail_pfx(&f.kind);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { .. },
            }) => {
                update_lens.push(quote! {
                    if let Some((_, __new_byte_len)) = self.#edit_name {
                        let __bytes = (__new_byte_len as u64).to_le_bytes();
                        self.header_mut().#len_name[..#pfx_lit].copy_from_slice(&__bytes[..#pfx_lit]);
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { .. },
            }) => {
                update_lens.push(quote! {
                    if let Some((_, __count, _)) = self.#edit_name {
                        let __bytes = (__count as u64).to_le_bytes();
                        self.header_mut().#len_name[..#pfx_lit].copy_from_slice(&__bytes[..#pfx_lit]);
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                update_lens.push(quote! {
                    if let Some((ptr, _)) = self.#edit_name {
                        self.header_mut().#tag_name[0] = if ptr.is_null() { 0 } else { 1 };
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                update_lens.push(quote! {
                    if let Some((ptr, _, _)) = self.#edit_name {
                        self.header_mut().#tag_name[0] = if ptr.is_null() { 0 } else { 1 };
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    let clear_edits: Vec<_> = tail_fields
        .iter()
        .map(|f| {
            let edit_name = format_ident!("__{}_edit", f.name);
            quote! { self.#edit_name = None; }
        })
        .collect();

    quote! {
        pub fn commit(&mut self) -> Result<usize, zeropod::ZeroPodError> {
            #( #setup_positions )*

            let __final_total: usize = #last_new_off + #last_new_len;
            if __final_total > self.data.len() {
                return Err(zeropod::ZeroPodError::BufferTooSmall);
            }

            let __buf_ptr = self.data.as_mut_ptr();

            // Move unedited fields that shift to a lower offset. Forward
            // iteration is safe here because writing to a lower address never
            // clobbers the source bytes of a later unedited field.
            #( #phase_1a )*

            // Move unedited fields that shift to a higher offset. Reverse
            // iteration is required so earlier fields still read untouched
            // source bytes even after later fields have been moved forward.
            #( #phase_1b )*

            // All unedited tail bytes are now at their final positions, so we
            // can safely copy caller-provided data into the edited slots
            // without risking aliasing with remaining unedited data.
            #( #phase_2 )*

            #( #update_lens )*

            self.total_len = __final_total;
            #( #clear_edits )*

            Ok(__final_total)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compact_bounds(schema: &Schema) -> Vec<TokenStream> {
    let mut bounds: Vec<TokenStream> = schema
        .inline_fields()
        .map(|f| {
            let pod_ty = map_to_pod_type(&f.ty);
            quote! { #pod_ty: zeropod::ZcValidate }
        })
        .collect();

    bounds.extend(schema.tail_fields().filter_map(|f| match &f.kind {
        FieldKind::Tail(TailField::Segment {
            payload: TailPayload::Vec { elem, .. },
            ..
        }) => {
            let mapped_elem = map_to_pod_type(elem);
            Some(quote! { #mapped_elem: zeropod::ZcElem })
        }
        _ => None,
    }));

    bounds
}

fn where_clause_with_bounds<'a>(
    where_clause: Option<&syn::WhereClause>,
    bounds: impl IntoIterator<Item = &'a TokenStream>,
) -> TokenStream {
    let bounds: Vec<&TokenStream> = bounds.into_iter().collect();
    match (where_clause, bounds.is_empty()) {
        (Some(existing), false) => {
            let predicates = existing.predicates.iter();
            quote! { where #(#predicates,)* #(#bounds,)* }
        }
        (Some(existing), true) => quote! { #existing },
        (None, false) => quote! { where #(#bounds,)* },
        (None, true) => quote! {},
    }
}

fn generics_with_lifetime(generics: &syn::Generics) -> syn::Generics {
    let mut generics = generics.clone();
    generics.params.insert(0, syn::parse_quote!('a));
    generics
}

fn read_len_expr(len_name: &syn::Ident, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { __hdr.#len_name[0] as usize },
        2 => quote! { u16::from_le_bytes(__hdr.#len_name) as usize },
        4 => quote! { u32::from_le_bytes(__hdr.#len_name) as usize },
        8 => quote! { u64::from_le_bytes(__hdr.#len_name) as usize },
        _ => unreachable!("invalid PFX: {}", pfx),
    }
}

fn read_data_len_expr(offset: TokenStream, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { data[#offset] as usize },
        2 => quote! { u16::from_le_bytes([data[#offset], data[#offset + 1]]) as usize },
        4 => quote! {
            u32::from_le_bytes([
                data[#offset],
                data[#offset + 1],
                data[#offset + 2],
                data[#offset + 3],
            ]) as usize
        },
        8 => quote! {
            u64::from_le_bytes([
                data[#offset],
                data[#offset + 1],
                data[#offset + 2],
                data[#offset + 3],
                data[#offset + 4],
                data[#offset + 5],
                data[#offset + 6],
                data[#offset + 7],
            ]) as usize
        },
        _ => unreachable!("invalid PFX: {}", pfx),
    }
}

fn read_self_data_len_expr(offset: TokenStream, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { self.data[#offset] as usize },
        2 => quote! { u16::from_le_bytes([self.data[#offset], self.data[#offset + 1]]) as usize },
        4 => quote! {
            u32::from_le_bytes([
                self.data[#offset],
                self.data[#offset + 1],
                self.data[#offset + 2],
                self.data[#offset + 3],
            ]) as usize
        },
        8 => quote! {
            u64::from_le_bytes([
                self.data[#offset],
                self.data[#offset + 1],
                self.data[#offset + 2],
                self.data[#offset + 3],
                self.data[#offset + 4],
                self.data[#offset + 5],
                self.data[#offset + 6],
                self.data[#offset + 7],
            ]) as usize
        },
        _ => unreachable!("invalid PFX: {}", pfx),
    }
}

fn old_option_string_size_expr(
    tag_name: &syn::Ident,
    pfx: usize,
    offset: TokenStream,
) -> TokenStream {
    let read_len = read_self_data_len_expr(offset.clone(), pfx);
    quote! {
        match __hdr.#tag_name[0] {
            0 => 0,
            1 => {
                if #offset + #pfx > self.total_len {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __byte_len = #read_len;
                if #offset + #pfx + __byte_len > self.total_len {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                #pfx + __byte_len
            }
            _ => return Err(zeropod::ZeroPodError::InvalidTag),
        }
    }
}

fn old_option_vec_size_expr(
    tag_name: &syn::Ident,
    pfx: usize,
    offset: TokenStream,
    mapped_elem: &TokenStream,
) -> TokenStream {
    let read_len = read_self_data_len_expr(offset.clone(), pfx);
    quote! {
        match __hdr.#tag_name[0] {
            0 => 0,
            1 => {
                if #offset + #pfx > self.total_len {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __count = #read_len;
                let __byte_len = __count * core::mem::size_of::<#mapped_elem>();
                if #offset + #pfx + __byte_len > self.total_len {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                #pfx + __byte_len
            }
            _ => return Err(zeropod::ZeroPodError::InvalidTag),
        }
    }
}

fn old_option_string_size_unchecked_expr(
    tag_name: &syn::Ident,
    pfx: usize,
    offset: TokenStream,
) -> TokenStream {
    let read_len = read_self_data_len_expr(offset.clone(), pfx);
    quote! {
        if __hdr.#tag_name[0] == 0 {
            0
        } else {
            #pfx + #read_len
        }
    }
}

fn old_option_vec_size_unchecked_expr(
    tag_name: &syn::Ident,
    pfx: usize,
    offset: TokenStream,
    mapped_elem: &TokenStream,
) -> TokenStream {
    let read_len = read_self_data_len_expr(offset.clone(), pfx);
    quote! {
        if __hdr.#tag_name[0] == 0 {
            0
        } else {
            #pfx + #read_len * core::mem::size_of::<#mapped_elem>()
        }
    }
}

fn tail_pfx(kind: &FieldKind) -> usize {
    match kind {
        FieldKind::Tail(tail) => tail.payload().pfx(),
        _ => unreachable!(),
    }
}

fn compute_offset_tokens(
    header_ty: &TokenStream,
    tail_fields: &[&crate::schema::SchemaField],
    target_index: usize,
) -> TokenStream {
    let header_size = quote! { core::mem::size_of::<#header_ty>() };
    let mut steps = Vec::new();
    for f in &tail_fields[..target_index] {
        let len_name = format_ident!("__{}_len", f.name);
        let pfx = tail_pfx(&f.kind);
        let read_len = read_len_expr(&len_name, pfx);

        match &f.kind {
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::String { .. },
            }) => {
                steps.push(quote! {
                    __offset += #read_len;
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::Always,
                payload: TailPayload::Vec { elem, .. },
            }) => {
                let mapped_elem = map_to_pod_type(elem);
                steps.push(quote! {
                    let __count = #read_len;
                    __offset += __count * core::mem::size_of::<#mapped_elem>();
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::String { pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                steps.push(quote! {
                    if __hdr.#tag_name[0] != 0 {
                        let __byte_len = #read_len;
                        __offset += #pfx + __byte_len;
                    }
                });
            }
            FieldKind::Tail(TailField::Segment {
                presence: TailPresence::OptionTag,
                payload: TailPayload::Vec { elem, pfx, .. },
            }) => {
                let tag_name = format_ident!("__{}_tag", f.name);
                let mapped_elem = map_to_pod_type(elem);
                let read_len = read_self_data_len_expr(quote! { __offset }, *pfx);
                steps.push(quote! {
                    if __hdr.#tag_name[0] != 0 {
                        let __count = #read_len;
                        __offset += #pfx + __count * core::mem::size_of::<#mapped_elem>();
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    quote! {
        let mut __offset = #header_size;
        #( #steps )*
    }
}
