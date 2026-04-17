use {
    crate::{
        schema::Schema,
        type_map::{map_to_pod_type, FieldKind, TailField},
    },
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

pub fn generate(schema: &Schema) -> TokenStream {
    let struct_name = &schema.name;
    let header_name = format_ident!("{}Header", struct_name);
    let ref_name = format_ident!("{}Ref", struct_name);
    let mut_name = format_ident!("{}Mut", struct_name);

    let header_ts = generate_header(schema, &header_name);
    let trait_impl_ts = generate_trait_impl(schema, &header_name);
    let ref_ts = generate_ref(schema, &header_name, &ref_name);
    let mut_ts = generate_mut(schema, &header_name, &mut_name);

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
        let len_name = format_ident!("__{}_len", f.name);
        let pfx_lit = tail_pfx(&f.kind);
        fields.push(quote! { #len_name: [u8; #pfx_lit] });
    }

    quote! {
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct #header_name {
            #( #fields ),*
        }

        const _: () = assert!(core::mem::align_of::<#header_name>() == 1);

        impl zeropod::ZcValidate for #header_name {
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

fn generate_trait_impl(schema: &Schema, header_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let mut validations = Vec::new();

    // Inline field validation via ZcValidate on the header.
    validations.push(quote! {
        <#header_name as zeropod::ZcValidate>::validate_ref(__hdr)?;
    });

    // Tail field length checks + UTF-8 validation for strings.
    let mut tail_size_exprs = Vec::new();
    let mut tail_offset_parts = Vec::new();
    for f in schema.tail_fields() {
        let len_name = format_ident!("__{}_len", f.name);
        let pfx = tail_pfx(&f.kind);
        let read_len = read_len_expr(&len_name, pfx);

        match &f.kind {
            FieldKind::Tail(TailField::String { max, .. }) => {
                let max_lit = *max;
                validations.push(quote! {
                    let #len_name = #read_len;
                    if #len_name > #max_lit {
                        return Err(zeropod::ZeroPodError::InvalidLength);
                    }
                });
                tail_size_exprs.push(quote! { #len_name });
                // We'll add UTF-8 check after total_check when we know the buffer is big
                // enough.
                tail_offset_parts.push(Some(len_name.clone()));
            }
            FieldKind::Tail(TailField::Vec { elem, max, .. }) => {
                let max_lit = *max;
                let mapped_elem = map_to_pod_type(elem);
                validations.push(quote! {
                    let #len_name = #read_len;
                    if #len_name > #max_lit {
                        return Err(zeropod::ZeroPodError::InvalidLength);
                    }
                });
                tail_size_exprs.push(quote! {
                    #len_name * core::mem::size_of::<#mapped_elem>()
                });
                tail_offset_parts.push(None);
            }
            _ => unreachable!(),
        }
    }

    let total_check = if tail_size_exprs.is_empty() {
        TokenStream::new()
    } else {
        quote! {
            let __total_tail: usize = 0 #( + #tail_size_exprs )*;
            if core::mem::size_of::<#header_name>() + __total_tail > data.len() {
                return Err(zeropod::ZeroPodError::BufferTooSmall);
            }
        }
    };

    // Build UTF-8 validation for String tail fields (after we know the buffer is
    // big enough).
    let mut utf8_checks = Vec::new();
    let tail_fields: Vec<_> = schema.tail_fields().collect();
    for (i, f) in tail_fields.iter().enumerate() {
        if let FieldKind::Tail(TailField::String { .. }) = &f.kind {
            let len_name = format_ident!("__{}_len", f.name);
            // Build offset: header_size + sum of preceding tail byte-sizes.
            let preceding_exprs: Vec<TokenStream> = tail_size_exprs[..i].to_vec();
            utf8_checks.push(quote! {
                {
                    let __str_offset = core::mem::size_of::<#header_name>() #( + #preceding_exprs )*;
                    if core::str::from_utf8(&data[__str_offset..__str_offset + #len_name]).is_err() {
                        return Err(zeropod::ZeroPodError::InvalidUtf8);
                    }
                }
            });
        }
    }

    // Build element validation for Vec tail fields (after buffer size check).
    let mut vec_elem_checks = Vec::new();
    for (i, f) in tail_fields.iter().enumerate() {
        if let FieldKind::Tail(TailField::Vec { elem, .. }) = &f.kind {
            let len_name = format_ident!("__{}_len", f.name);
            let mapped_elem = map_to_pod_type(elem);
            let preceding_exprs: Vec<TokenStream> = tail_size_exprs[..i].to_vec();
            vec_elem_checks.push(quote! {
                {
                    let __vec_offset = core::mem::size_of::<#header_name>() #( + #preceding_exprs )*;
                    let __elem_size = core::mem::size_of::<#mapped_elem>();
                    for __i in 0..#len_name {
                        let __elem_ptr = unsafe {
                            &*(data.as_ptr().add(__vec_offset + __i * __elem_size) as *const #mapped_elem)
                        };
                        <#mapped_elem as zeropod::ZcValidate>::validate_ref(__elem_ptr)?;
                    }
                }
            });
        }
    }

    quote! {
        impl zeropod::ZeroPodSchema for #struct_name {
            const LAYOUT: zeropod::LayoutKind = zeropod::LayoutKind::Compact;
        }

        impl zeropod::ZeroPodCompact for #struct_name {
            type Header = #header_name;
            const HEADER_SIZE: usize = core::mem::size_of::<#header_name>();

            fn header(data: &[u8]) -> Result<&Self::Header, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &*(data.as_ptr() as *const #header_name) })
            }

            fn header_mut(data: &mut [u8]) -> Result<&mut Self::Header, zeropod::ZeroPodError> {
                Self::validate(data)?;
                Ok(unsafe { &mut *(data.as_mut_ptr() as *mut #header_name) })
            }

            fn validate(data: &[u8]) -> Result<(), zeropod::ZeroPodError> {
                if data.len() < core::mem::size_of::<#header_name>() {
                    return Err(zeropod::ZeroPodError::BufferTooSmall);
                }
                let __hdr = unsafe { &*(data.as_ptr() as *const #header_name) };
                #( #validations )*
                #total_check
                #( #utf8_checks )*
                #( #vec_elem_checks )*
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ref generation
// ---------------------------------------------------------------------------

fn generate_ref(schema: &Schema, header_name: &syn::Ident, ref_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let tail_fields: Vec<_> = schema.tail_fields().collect();
    let mut accessors = Vec::new();

    for (i, f) in tail_fields.iter().enumerate() {
        let fname = &f.name;
        let offset_computation = compute_offset_tokens(header_name, &tail_fields, i);

        match &f.kind {
            FieldKind::Tail(TailField::String { pfx, .. }) => {
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
            FieldKind::Tail(TailField::Vec { elem, pfx, .. }) => {
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
            _ => unreachable!(),
        }
    }

    // Compile-time assertion: Vec tail element types must implement ZcElem.
    // ZcElem guarantees align-1, valid validation, and safe packed-byte access.
    let mut align_assertions = Vec::new();
    for f in &tail_fields {
        if let FieldKind::Tail(TailField::Vec { elem, .. }) = &f.kind {
            let mapped_elem = map_to_pod_type(elem);
            align_assertions.push(quote! {
                const _: fn() = || {
                    fn assert_zc_elem<T: zeropod::ZcElem>() {}
                    assert_zc_elem::<#mapped_elem>();
                };
            });
        }
    }

    quote! {
        #( #align_assertions )*

        pub struct #ref_name<'a> {
            data: &'a [u8],
        }

        impl<'a> core::ops::Deref for #ref_name<'a> {
            type Target = #header_name;
            fn deref(&self) -> &#header_name {
                self.header()
            }
        }

        impl<'a> #ref_name<'a> {
            pub fn new(data: &'a [u8]) -> Result<Self, zeropod::ZeroPodError> {
                <#struct_name as zeropod::ZeroPodCompact>::validate(data)?;
                Ok(Self { data })
            }

            pub unsafe fn new_unchecked(data: &'a [u8]) -> Self {
                Self { data }
            }

            fn header(&self) -> &'a #header_name {
                unsafe { &*(self.data.as_ptr() as *const #header_name) }
            }

            #( #accessors )*
        }
    }
}

// ---------------------------------------------------------------------------
// Mut generation
// ---------------------------------------------------------------------------

fn generate_mut(schema: &Schema, header_name: &syn::Ident, mut_name: &syn::Ident) -> TokenStream {
    let struct_name = &schema.name;
    let tail_fields: Vec<_> = schema.tail_fields().collect();

    // Edit descriptor fields.
    let mut edit_fields = Vec::new();
    for f in &tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        match &f.kind {
            FieldKind::Tail(TailField::String { .. }) => {
                edit_fields.push(quote! { #edit_name: Option<(*const u8, usize)> });
            }
            FieldKind::Tail(TailField::Vec { .. }) => {
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
            FieldKind::Tail(TailField::String { max, .. }) => {
                let max_lit = *max;
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a str) -> Result<(), zeropod::ZeroPodError> {
                        if value.len() > #max_lit {
                            return Err(zeropod::ZeroPodError::Overflow);
                        }
                        self.#edit_name = Some((value.as_ptr(), value.len()));
                        Ok(())
                    }
                });
            }
            FieldKind::Tail(TailField::Vec { elem, max, .. }) => {
                let max_lit = *max;
                let mapped_elem = map_to_pod_type(elem);
                setters.push(quote! {
                    pub fn #setter_name(&mut self, value: &'a [#mapped_elem]) -> Result<(), zeropod::ZeroPodError> {
                        if value.len() > #max_lit {
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
            _ => unreachable!(),
        }
    }

    // projected_size()
    let mut proj_parts = Vec::new();
    for f in &tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        let len_name = format_ident!("__{}_len", f.name);

        match &f.kind {
            FieldKind::Tail(TailField::String { pfx, .. }) => {
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
            FieldKind::Tail(TailField::Vec { elem, pfx, .. }) => {
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
            _ => unreachable!(),
        }
    }

    // commit()
    let commit_body = generate_commit_body(header_name, &tail_fields);

    quote! {
        pub struct #mut_name<'a> {
            data: &'a mut [u8],
            total_len: usize,
            #( #edit_fields ),*
        }

        impl<'a> core::ops::Deref for #mut_name<'a> {
            type Target = #header_name;
            fn deref(&self) -> &#header_name {
                self.header()
            }
        }

        impl<'a> core::ops::DerefMut for #mut_name<'a> {
            fn deref_mut(&mut self) -> &mut #header_name {
                self.header_mut()
            }
        }

        impl<'a> #mut_name<'a> {
            pub fn new(data: &'a mut [u8]) -> Result<Self, zeropod::ZeroPodError> {
                <#struct_name as zeropod::ZeroPodCompact>::validate(data)?;
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

            fn header(&self) -> &#header_name {
                unsafe { &*(self.data.as_ptr() as *const #header_name) }
            }

            fn header_mut(&mut self) -> &mut #header_name {
                unsafe { &mut *(self.data.as_mut_ptr() as *mut #header_name) }
            }

            #( #setters )*

            pub fn projected_size(&self) -> usize {
                core::mem::size_of::<#header_name>()
                #( #proj_parts )*
            }

            #commit_body
        }
    }
}

fn generate_commit_body(
    header_name: &syn::Ident,
    tail_fields: &[&crate::schema::SchemaField],
) -> TokenStream {
    if tail_fields.is_empty() {
        return quote! {
            pub fn commit(&mut self) -> Result<usize, zeropod::ZeroPodError> {
                Ok(core::mem::size_of::<#header_name>())
            }
        };
    }

    let header_size = quote! { core::mem::size_of::<#header_name>() };

    // Step 1: Snapshot old byte-lengths from header.
    let mut snapshot_old = Vec::new();
    for f in tail_fields {
        let len_name = format_ident!("__{}_len", f.name);
        let old_len_name = format_ident!("__old_{}_bytelen", f.name);
        let pfx = tail_pfx(&f.kind);
        let read_len = read_len_expr(&len_name, pfx);

        match &f.kind {
            FieldKind::Tail(TailField::String { .. }) => {
                snapshot_old.push(quote! {
                    let #old_len_name = {
                        let __hdr = self.header();
                        #read_len
                    };
                });
            }
            FieldKind::Tail(TailField::Vec { elem, .. }) => {
                let mapped_elem = map_to_pod_type(elem);
                snapshot_old.push(quote! {
                    let #old_len_name = {
                        let __hdr = self.header();
                        let __count = #read_len;
                        __count * core::mem::size_of::<#mapped_elem>()
                    };
                });
            }
            _ => unreachable!(),
        }
    }

    // Step 2-4: Process each tail field — write new or memmove old.
    let mut process_fields = Vec::new();
    for f in tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        let old_len_name = format_ident!("__old_{}_bytelen", f.name);

        match &f.kind {
            FieldKind::Tail(TailField::String { .. }) => {
                process_fields.push(quote! {
                    if let Some((__src_ptr, __new_byte_len)) = self.#edit_name {
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                __src_ptr,
                                __buf_ptr.add(__new_offset),
                                __new_byte_len,
                            );
                        }
                        __new_offset += __new_byte_len;
                    } else {
                        if __old_offset != __new_offset && #old_len_name > 0 {
                            unsafe {
                                core::ptr::copy(
                                    __buf_ptr.add(__old_offset) as *const u8,
                                    __buf_ptr.add(__new_offset),
                                    #old_len_name,
                                );
                            }
                        }
                        __new_offset += #old_len_name;
                    }
                    __old_offset += #old_len_name;
                });
            }
            FieldKind::Tail(TailField::Vec { .. }) => {
                process_fields.push(quote! {
                    if let Some((__src_ptr, __count, __elem_size)) = self.#edit_name {
                        let __new_byte_len = __count * __elem_size;
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                __src_ptr,
                                __buf_ptr.add(__new_offset),
                                __new_byte_len,
                            );
                        }
                        __new_offset += __new_byte_len;
                    } else {
                        if __old_offset != __new_offset && #old_len_name > 0 {
                            unsafe {
                                core::ptr::copy(
                                    __buf_ptr.add(__old_offset) as *const u8,
                                    __buf_ptr.add(__new_offset),
                                    #old_len_name,
                                );
                            }
                        }
                        __new_offset += #old_len_name;
                    }
                    __old_offset += #old_len_name;
                });
            }
            _ => unreachable!(),
        }
    }

    // Step 5: Update header length fields for edited fields.
    let mut update_lens = Vec::new();
    for f in tail_fields {
        let edit_name = format_ident!("__{}_edit", f.name);
        let len_name = format_ident!("__{}_len", f.name);
        let pfx_lit = tail_pfx(&f.kind);

        match &f.kind {
            FieldKind::Tail(TailField::String { .. }) => {
                update_lens.push(quote! {
                    if let Some((_, __new_byte_len)) = self.#edit_name {
                        let __bytes = (__new_byte_len as u64).to_le_bytes();
                        self.header_mut().#len_name[..#pfx_lit].copy_from_slice(&__bytes[..#pfx_lit]);
                    }
                });
            }
            FieldKind::Tail(TailField::Vec { .. }) => {
                update_lens.push(quote! {
                    if let Some((_, __count, _)) = self.#edit_name {
                        let __bytes = (__count as u64).to_le_bytes();
                        self.header_mut().#len_name[..#pfx_lit].copy_from_slice(&__bytes[..#pfx_lit]);
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    // Step 6: Clear edit descriptors.
    let clear_edits: Vec<_> = tail_fields
        .iter()
        .map(|f| {
            let edit_name = format_ident!("__{}_edit", f.name);
            quote! { self.#edit_name = None; }
        })
        .collect();

    quote! {
        pub fn commit(&mut self) -> Result<usize, zeropod::ZeroPodError> {
            // Guard: ensure buffer can hold the projected size.
            let __required = self.projected_size();
            if __required > self.data.len() {
                return Err(zeropod::ZeroPodError::BufferTooSmall);
            }

            // Snapshot old byte-lengths before any writes.
            #( #snapshot_old )*

            // Obtain a single mutable pointer for all buffer operations.
            let __buf_ptr = self.data.as_mut_ptr();

            // Process tail fields: write new data or memmove old data.
            let mut __new_offset = #header_size;
            let mut __old_offset = #header_size;

            #( #process_fields )*

            // Update header length prefixes for edited fields.
            #( #update_lens )*

            // Finalize.
            self.total_len = __new_offset;
            #( #clear_edits )*

            Ok(__new_offset)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_len_expr(len_name: &syn::Ident, pfx: usize) -> TokenStream {
    match pfx {
        1 => quote! { __hdr.#len_name[0] as usize },
        2 => quote! { u16::from_le_bytes(__hdr.#len_name) as usize },
        4 => quote! { u32::from_le_bytes(__hdr.#len_name) as usize },
        8 => quote! { u64::from_le_bytes(__hdr.#len_name) as usize },
        _ => unreachable!("invalid PFX: {}", pfx),
    }
}

fn tail_pfx(kind: &FieldKind) -> usize {
    match kind {
        FieldKind::Tail(TailField::String { pfx, .. }) => *pfx,
        FieldKind::Tail(TailField::Vec { pfx, .. }) => *pfx,
        _ => unreachable!(),
    }
}

fn compute_offset_tokens(
    header_name: &syn::Ident,
    tail_fields: &[&crate::schema::SchemaField],
    target_index: usize,
) -> TokenStream {
    let header_size = quote! { core::mem::size_of::<#header_name>() };

    if target_index == 0 {
        return quote! { let __offset = #header_size; };
    }

    let mut addends = Vec::new();
    for f in &tail_fields[..target_index] {
        let len_name = format_ident!("__{}_len", f.name);
        let pfx = tail_pfx(&f.kind);
        let read_len = read_len_expr(&len_name, pfx);

        match &f.kind {
            FieldKind::Tail(TailField::String { .. }) => {
                addends.push(quote! { #read_len });
            }
            FieldKind::Tail(TailField::Vec { elem, .. }) => {
                let mapped_elem = map_to_pod_type(elem);
                addends.push(quote! {
                    {
                        let __count = #read_len;
                        __count * core::mem::size_of::<#mapped_elem>()
                    }
                });
            }
            _ => unreachable!(),
        }
    }

    quote! {
        let __offset = #header_size #( + #addends )*;
    }
}
