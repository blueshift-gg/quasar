use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Type};

use crate::helpers::{
    is_dynamic_string, is_dynamic_vec, map_to_pod_type, zc_deserialize_field, zc_serialize_field,
    InstructionArgs,
};

enum DynKind {
    Fixed,
    Str { max: usize },
    Vec { elem: Box<Type>, max: usize },
}

pub(crate) fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

    let disc_values: Vec<u8> = disc_bytes
        .iter()
        .map(|lit| {
            lit.base10_parse::<u8>()
                .expect("discriminator byte must be 0-255")
        })
        .collect();
    if disc_values.iter().all(|&b| b == 0) {
        return syn::Error::new_spanned(
            &args.discriminator[0],
            "discriminator must contain at least one non-zero byte; all-zero discriminators are indistinguishable from uninitialized account data",
        ).to_compile_error().into();
    }

    let disc_indices: Vec<usize> = (0..disc_len).collect();

    let fields_data = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("#[account] can only be used on structs with named fields"),
        },
        _ => panic!("#[account] can only be used on structs"),
    };

    let field_kinds: Vec<DynKind> = fields_data
        .iter()
        .map(|f| {
            if let Some(max) = is_dynamic_string(&f.ty) {
                DynKind::Str { max }
            } else if let Some((elem, max)) = is_dynamic_vec(&f.ty) {
                DynKind::Vec { elem: Box::new(elem), max }
            } else {
                DynKind::Fixed
            }
        })
        .collect();

    let has_dynamic = field_kinds.iter().any(|k| !matches!(k, DynKind::Fixed));

    if !has_dynamic {
        return generate_fixed_account(name, disc_bytes, disc_len, &disc_indices, fields_data, &input);
    }

    // Validate: fixed fields must precede all dynamic fields
    let mut seen_dynamic = false;
    for (f, kind) in fields_data.iter().zip(field_kinds.iter()) {
        match kind {
            DynKind::Fixed => {
                if seen_dynamic {
                    return syn::Error::new_spanned(
                        f,
                        "fixed fields must precede all dynamic fields (String/Vec)",
                    )
                    .to_compile_error()
                    .into();
                }
            }
            _ => seen_dynamic = true,
        }
    }

    // Validate: Vec element types must not be dynamic (no nested String/Vec).
    // Future: nested Vec<Vec<T, N>, M> could use a count-table layout —
    //   ZC header: [outer_count: PodU16][inner_counts: [PodU16; M]]
    //   Tail: all elements packed contiguously.
    //   Access inner vec i by summing counts[0..i]. For now, use the flatten
    //   pattern: separate Vec<PodU16, M> for lengths + Vec<T, N*M> for data.
    for (f, kind) in fields_data.iter().zip(field_kinds.iter()) {
        if let DynKind::Vec { elem, .. } = kind {
            if is_dynamic_string(elem).is_some() || is_dynamic_vec(elem).is_some() {
                return syn::Error::new_spanned(
                    f,
                    "Vec element type must be a fixed-size type; nested dynamic types (String/Vec) are not supported",
                )
                .to_compile_error()
                .into();
            }
        }
    }

    // Validate: struct must have a lifetime parameter
    if input.generics.lifetimes().next().is_none() {
        return syn::Error::new_spanned(
            name,
            "structs with dynamic fields (String/Vec) must have a lifetime parameter, e.g. Profile<'a>",
        )
        .to_compile_error()
        .into();
    }

    generate_dynamic_account(
        name,
        disc_bytes,
        disc_len,
        &disc_indices,
        fields_data,
        &field_kinds,
        &input,
    )
}

/// Generate code for accounts with only fixed-size fields.
/// This path is identical to the pre-dynamic codegen — zero changes.
fn generate_fixed_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    fields_data: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    input: &DeriveInput,
) -> TokenStream {
    let field_types: Vec<_> = fields_data.iter().map(|f| &f.ty).collect();
    let zc_name = format_ident!("{}Zc", name);

    let zc_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .map(|f| {
            let fname = &f.ident;
            let vis = &f.vis;
            let zc_ty = map_to_pod_type(&f.ty);
            quote! { #vis #fname: #zc_ty }
        })
        .collect();

    let serialize_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .map(|f| zc_serialize_field(f.ident.as_ref().unwrap(), &f.ty))
        .collect();

    let deserialize_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .map(|f| zc_deserialize_field(f.ident.as_ref().unwrap(), &f.ty))
        .collect();

    quote! {
        #[repr(C)]
        #input

        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #zc_name {
            #(#zc_fields,)*
        }

        const _: () = assert!(
            core::mem::align_of::<#zc_name>() == 1,
            "ZC companion struct must have alignment 1; all fields must use Pod types or alignment-1 types"
        );

        impl Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Space for #name {
            const SPACE: usize = #disc_len #(+ core::mem::size_of::<#field_types>())*;
        }

        impl Owner for #name {
            const OWNER: Address = crate::ID;
        }

        impl AccountCheck for #name {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                if __data.len() < #disc_len + core::mem::size_of::<#zc_name>() {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                #(
                    if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                        return Err(ProgramError::InvalidAccountData);
                    }
                )*
                Ok(())
            }
        }

        impl ZeroCopyDeref for #name {
            type Target = #zc_name;

            #[inline(always)]
            fn deref_from(view: &AccountView) -> &Self::Target {
                unsafe { &*(view.data_ptr().add(Self::DISCRIMINATOR.len()) as *const #zc_name) }
            }

            #[inline(always)]
            fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
                unsafe { &mut *(view.data_ptr().add(Self::DISCRIMINATOR.len()) as *mut #zc_name) }
            }
        }

        impl QuasarAccount for #name {
            #[inline(always)]
            fn deserialize(data: &[u8]) -> Result<Self, ProgramError> {
                let __zc = unsafe { &*(data.as_ptr() as *const #zc_name) };
                Ok(Self {
                    #(#deserialize_fields,)*
                })
            }

            #[inline(always)]
            fn serialize(&self, data: &mut [u8]) -> Result<(), ProgramError> {
                let __zc = unsafe { &mut *(data.as_mut_ptr() as *mut #zc_name) };
                #(#serialize_stmts)*
                Ok(())
            }
        }

        impl #name {
            #[inline(always)]
            pub fn init(self, account: &mut Initialize<Self>, payer: &AccountView, rent: Option<&Rent>) -> Result<(), ProgramError> {
                self.init_signed(account, payer, rent, &[])
            }

            #[inline(always)]
            pub fn init_signed(self, account: &mut Initialize<Self>, payer: &AccountView, rent: Option<&Rent>, signers: &[quasar_core::cpi::Signer]) -> Result<(), ProgramError> {
                let view = account.to_account_view();

                {
                    let __existing = unsafe { view.borrow_unchecked() };
                    if __existing.len() >= #disc_len {
                        #(
                            if unsafe { *__existing.get_unchecked(#disc_indices) } != 0 {
                                return Err(QuasarError::AccountAlreadyInitialized.into());
                            }
                        )*
                    }
                }

                let lamports = match rent {
                    Some(rent_account) => unsafe { rent_account.get_unchecked() }.minimum_balance_unchecked(Self::SPACE),
                    None => {
                        use quasar_core::sysvars::Sysvar;
                        quasar_core::sysvars::rent::Rent::get()?.minimum_balance_unchecked(Self::SPACE)
                    }
                };

                if view.lamports() == 0 {
                    quasar_core::cpi::system::create_account(payer, view, lamports, Self::SPACE as u64, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                } else {
                    let required = lamports.saturating_sub(view.lamports());
                    if required > 0 {
                        quasar_core::cpi::system::transfer(payer, view, required)
                            .invoke_with_signers(signers)?;
                    }
                    quasar_core::cpi::system::assign(view, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                    unsafe { view.resize_unchecked(Self::SPACE) }?;
                }

                let data = unsafe { view.borrow_unchecked_mut() };
                data[..Self::DISCRIMINATOR.len()].copy_from_slice(Self::DISCRIMINATOR);
                self.serialize(&mut data[Self::DISCRIMINATOR.len()..])?;
                Ok(())
            }
        }
    }
    .into()
}

/// Generate code for accounts with dynamic fields (String/Vec).
fn generate_dynamic_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    fields_data: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    field_kinds: &[DynKind],
    input: &DeriveInput,
) -> TokenStream {
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;
    let lt = &input.generics.lifetimes().next().unwrap().lifetime;
    let zc_name = format_ident!("{}Zc", name);
    let view_name = format_ident!("{}View", name);

    // --- 1. Transformed struct fields ---
    let transformed_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| {
            let fname = &f.ident;
            let fvis = &f.vis;
            match kind {
                DynKind::Fixed => {
                    let fty = &f.ty;
                    quote! { #fvis #fname: #fty }
                }
                DynKind::Str { .. } => {
                    quote! { #fvis #fname: &#lt str }
                }
                DynKind::Vec { elem, .. } => {
                    quote! { #fvis #fname: &#lt [#elem] }
                }
            }
        })
        .collect();

    // --- 2. ZC companion fields ---
    let zc_fields: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| {
            let fvis = &f.vis;
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Fixed => {
                    let zc_ty = map_to_pod_type(&f.ty);
                    quote! { #fvis #fname: #zc_ty }
                }
                DynKind::Str { .. } => {
                    let len_name = format_ident!("{}_len", fname);
                    quote! { #fvis #len_name: quasar_core::pod::PodU16 }
                }
                DynKind::Vec { .. } => {
                    let count_name = format_ident!("{}_count", fname);
                    quote! { #fvis #count_name: quasar_core::pod::PodU16 }
                }
            }
        })
        .collect();

    // --- 3. ZC header serialize (fixed fields + length descriptors) ---
    let zc_header_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Fixed => zc_serialize_field(fname, &f.ty),
                DynKind::Str { .. } => {
                    let len_name = format_ident!("{}_len", fname);
                    quote! { __zc.#len_name = quasar_core::pod::PodU16::from(self.#fname.len() as u16); }
                }
                DynKind::Vec { .. } => {
                    let count_name = format_ident!("{}_count", fname);
                    quote! { __zc.#count_name = quasar_core::pod::PodU16::from(self.#fname.len() as u16); }
                }
            }
        })
        .collect();

    // --- 4. Variable tail serialize ---
    let var_serialize_stmts: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => quote! {
                    {
                        let __len = self.#fname.len();
                        __data[__var_offset..__var_offset + __len].copy_from_slice(self.#fname.as_bytes());
                        __var_offset += __len;
                    }
                },
                DynKind::Vec { elem, .. } => quote! {
                    {
                        let __bytes = self.#fname.len() * core::mem::size_of::<#elem>();
                        if __bytes > 0 {
                            // SAFETY: Source and destination do not overlap. Alignment 1 guaranteed.
                            unsafe {
                                core::ptr::copy_nonoverlapping(
                                    self.#fname.as_ptr() as *const u8,
                                    __data[__var_offset..].as_mut_ptr(),
                                    __bytes,
                                );
                            }
                        }
                        __var_offset += __bytes;
                    }
                },
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 5. Max length checks for init ---
    let max_checks: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { max } => quote! {
                    if self.#fname.len() > #max {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                },
                DynKind::Vec { max, .. } => quote! {
                    if self.#fname.len() > #max {
                        return Err(QuasarError::DynamicFieldTooLong.into());
                    }
                },
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 6. Dynamic space terms ---
    let space_terms: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => quote! { + self.#fname.len() },
                DynKind::Vec { elem, .. } => {
                    quote! { + self.#fname.len() * core::mem::size_of::<#elem>() }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 7. MAX_SPACE terms ---
    let max_space_terms: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(_, kind)| match kind {
            DynKind::Str { max } => quote! { + #max },
            DynKind::Vec { elem, max } => {
                quote! { + #max * core::mem::size_of::<#elem>() }
            }
            _ => unreachable!(),
        })
        .collect();

    // --- 8. AccountCheck variable region terms ---
    let var_check_terms: Vec<proc_macro2::TokenStream> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => {
                    let len_name = format_ident!("{}_len", fname);
                    quote! { + __zc.#len_name.get() as usize }
                }
                DynKind::Vec { elem, .. } => {
                    let count_name = format_ident!("{}_count", fname);
                    quote! { + __zc.#count_name.get() as usize * core::mem::size_of::<#elem>() }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 9. Read accessor methods ---
    let dyn_fields: Vec<(&syn::Field, &DynKind)> = fields_data
        .iter()
        .zip(field_kinds.iter())
        .filter(|(_, k)| !matches!(k, DynKind::Fixed))
        .collect();

    let accessor_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();

            // Offset = disc_len + sizeof(ZC) + sum of preceding dynamic field lengths
            let preceding: Vec<proc_macro2::TokenStream> = dyn_fields[..i]
                .iter()
                .map(|(pf, pk)| {
                    let pname = pf.ident.as_ref().unwrap();
                    match pk {
                        DynKind::Str { .. } => {
                            let plen = format_ident!("{}_len", pname);
                            quote! { + __zc.#plen.get() as usize }
                        }
                        DynKind::Vec { elem, .. } => {
                            let pcount = format_ident!("{}_count", pname);
                            quote! { + __zc.#pcount.get() as usize * core::mem::size_of::<#elem>() }
                        }
                        _ => unreachable!(),
                    }
                })
                .collect();

            match kind {
                DynKind::Str { .. } => {
                    let len_name = format_ident!("{}_len", fname);
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &str {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                            let __offset = #disc_len + core::mem::size_of::<#zc_name>() #(#preceding)*;
                            let __len = __zc.#len_name.get() as usize;
                            // SAFETY: Bounds validated by AccountCheck::check. UTF-8 validated on init.
                            unsafe { core::str::from_utf8_unchecked(&__data[__offset..__offset + __len]) }
                        }
                    }
                }
                DynKind::Vec { elem, .. } => {
                    let count_name = format_ident!("{}_count", fname);
                    quote! {
                        #[inline(always)]
                        pub fn #fname(&self) -> &[#elem] {
                            let __data = unsafe { self.to_account_view().borrow_unchecked() };
                            let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                            let __offset = #disc_len + core::mem::size_of::<#zc_name>() #(#preceding)*;
                            let __count = __zc.#count_name.get() as usize;
                            // SAFETY: Bounds validated by AccountCheck::check. Alignment 1 guaranteed.
                            unsafe { core::slice::from_raw_parts(__data[__offset..].as_ptr() as *const #elem, __count) }
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 10. Write setter methods ---
    let write_methods: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .enumerate()
        .map(|(i, (f, kind))| {
            let fname = f.ident.as_ref().unwrap();
            let setter_name = format_ident!("set_{}", fname);

            let preceding: Vec<proc_macro2::TokenStream> = dyn_fields[..i]
                .iter()
                .map(|(pf, pk)| {
                    let pname = pf.ident.as_ref().unwrap();
                    match pk {
                        DynKind::Str { .. } => {
                            let plen = format_ident!("{}_len", pname);
                            quote! { + __zc.#plen.get() as usize }
                        }
                        DynKind::Vec { elem, .. } => {
                            let pcount = format_ident!("{}_count", pname);
                            quote! { + __zc.#pcount.get() as usize * core::mem::size_of::<#elem>() }
                        }
                        _ => unreachable!(),
                    }
                })
                .collect();

            match kind {
                DynKind::Str { max } => {
                    let len_name = format_ident!("{}_len", fname);
                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&self, __payer: &impl AsAccountView, __value: &str) -> Result<(), ProgramError> {
                            if __value.len() > #max {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __view = self.to_account_view();
                            let __old_len;
                            let __old_total;
                            let __field_offset;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                                __old_len = __zc.#len_name.get() as usize;
                                __old_total = __data.len();
                                __field_offset = #disc_len + core::mem::size_of::<#zc_name>() #(#preceding)*;
                            }
                            let __new_len = __value.len();
                            if __old_len != __new_len {
                                let __new_total = __old_total + __new_len - __old_len;
                                let __tail_start = __field_offset + __old_len;
                                let __tail_len = __old_total - __tail_start;
                                if __new_len > __old_len {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                                if __tail_len > 0 {
                                    let __new_tail = __field_offset + __new_len;
                                    let __data = unsafe { __view.borrow_unchecked_mut() };
                                    // SAFETY: copy handles overlapping source/dest.
                                    unsafe {
                                        core::ptr::copy(
                                            __data.as_ptr().add(__tail_start),
                                            __data.as_mut_ptr().add(__new_tail),
                                            __tail_len,
                                        );
                                    }
                                }
                                if __new_len < __old_len {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                            }
                            let __data = unsafe { __view.borrow_unchecked_mut() };
                            __data[__field_offset..__field_offset + __new_len].copy_from_slice(__value.as_bytes());
                            let __zc = unsafe { &mut *(__data[#disc_len..].as_mut_ptr() as *mut #zc_name) };
                            __zc.#len_name = quasar_core::pod::PodU16::from(__new_len as u16);
                            Ok(())
                        }
                    }
                }
                DynKind::Vec { elem, max } => {
                    let count_name = format_ident!("{}_count", fname);
                    let mut_name = format_ident!("{}_mut", fname);
                    quote! {
                        #[inline(always)]
                        pub fn #setter_name(&self, __payer: &impl AsAccountView, __value: &[#elem]) -> Result<(), ProgramError> {
                            if __value.len() > #max {
                                return Err(QuasarError::DynamicFieldTooLong.into());
                            }
                            let __elem_size = core::mem::size_of::<#elem>();
                            let __view = self.to_account_view();
                            let __old_count;
                            let __old_total;
                            let __field_offset;
                            {
                                let __data = unsafe { __view.borrow_unchecked() };
                                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                                __old_count = __zc.#count_name.get() as usize;
                                __old_total = __data.len();
                                __field_offset = #disc_len + core::mem::size_of::<#zc_name>() #(#preceding)*;
                            }
                            let __old_bytes = __old_count * __elem_size;
                            let __new_bytes = __value.len() * __elem_size;
                            if __old_bytes != __new_bytes {
                                let __new_total = __old_total + __new_bytes - __old_bytes;
                                let __tail_start = __field_offset + __old_bytes;
                                let __tail_len = __old_total - __tail_start;
                                if __new_bytes > __old_bytes {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                                if __tail_len > 0 {
                                    let __new_tail = __field_offset + __new_bytes;
                                    let __data = unsafe { __view.borrow_unchecked_mut() };
                                    unsafe {
                                        core::ptr::copy(
                                            __data.as_ptr().add(__tail_start),
                                            __data.as_mut_ptr().add(__new_tail),
                                            __tail_len,
                                        );
                                    }
                                }
                                if __new_bytes < __old_bytes {
                                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                                }
                            }
                            let __data = unsafe { __view.borrow_unchecked_mut() };
                            if !__value.is_empty() {
                                // SAFETY: Source and dest do not overlap. Alignment 1 guaranteed.
                                unsafe {
                                    core::ptr::copy_nonoverlapping(
                                        __value.as_ptr() as *const u8,
                                        __data[__field_offset..].as_mut_ptr(),
                                        __new_bytes,
                                    );
                                }
                            }
                            let __zc = unsafe { &mut *(__data[#disc_len..].as_mut_ptr() as *mut #zc_name) };
                            __zc.#count_name = quasar_core::pod::PodU16::from(__value.len() as u16);
                            Ok(())
                        }

                        #[inline(always)]
                        #[allow(clippy::mut_from_ref)]
                        pub fn #mut_name(&self) -> &mut [#elem] {
                            let __data = unsafe { self.to_account_view().borrow_unchecked_mut() };
                            let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                            let __offset = #disc_len + core::mem::size_of::<#zc_name>() #(#preceding)*;
                            let __count = __zc.#count_name.get() as usize;
                            // SAFETY: Bounds validated by AccountCheck::check. Alignment 1 guaranteed.
                            unsafe { core::slice::from_raw_parts_mut(__data[__offset..].as_mut_ptr() as *mut #elem, __count) }
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- 11. Batch fields struct for single-pass access ---
    let fields_name = format_ident!("{}DynamicFields", name);

    let fields_struct_fields: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = &f.ident;
            let fvis = &f.vis;
            match kind {
                DynKind::Str { .. } => quote! { #fvis #fname: &#lt str },
                DynKind::Vec { elem, .. } => quote! { #fvis #fname: &#lt [#elem] },
                _ => unreachable!(),
            }
        })
        .collect();

    let fields_extract_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => {
                    let len_name = format_ident!("{}_len", fname);
                    quote! {
                        let #fname = {
                            let __len = __zc.#len_name.get() as usize;
                            let __s = unsafe { core::str::from_utf8_unchecked(&__data[__offset..__offset + __len]) };
                            __offset += __len;
                            __s
                        };
                    }
                }
                DynKind::Vec { elem, .. } => {
                    let count_name = format_ident!("{}_count", fname);
                    quote! {
                        let #fname = {
                            let __count = __zc.#count_name.get() as usize;
                            let __slice = unsafe {
                                core::slice::from_raw_parts(
                                    __data[__offset..].as_ptr() as *const #elem,
                                    __count,
                                )
                            };
                            __offset += __count * core::mem::size_of::<#elem>();
                            __slice
                        };
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    let fields_field_names: Vec<&syn::Ident> = dyn_fields
        .iter()
        .map(|(f, _)| f.ident.as_ref().unwrap())
        .collect();

    // --- 12. Batch set_dynamic_fields method (Option params, stack buffer) ---
    let set_dyn_params: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => quote! { #fname: Option<&str> },
                DynKind::Vec { elem, .. } => quote! { #fname: Option<&[#elem]> },
                _ => unreachable!(),
            }
        })
        .collect();

    let set_dyn_buf_stmts: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { max } => {
                    let len_name = format_ident!("{}_len", fname);
                    let new_len_var = format_ident!("__{}_new_len", fname);
                    quote! {
                        let #new_len_var: usize;
                        {
                            let __old_len = __zc.#len_name.get() as usize;
                            match #fname {
                                Some(__val) => {
                                    if __val.len() > #max {
                                        return Err(QuasarError::DynamicFieldTooLong.into());
                                    }
                                    #new_len_var = __val.len();
                                    __buf[__buf_offset..__buf_offset + #new_len_var]
                                        .copy_from_slice(__val.as_bytes());
                                }
                                None => {
                                    #new_len_var = __old_len;
                                    __buf[__buf_offset..__buf_offset + __old_len]
                                        .copy_from_slice(&__data[__old_offset..__old_offset + __old_len]);
                                }
                            }
                            __buf_offset += #new_len_var;
                            __old_offset += __old_len;
                        }
                    }
                }
                DynKind::Vec { elem, max } => {
                    let count_name = format_ident!("{}_count", fname);
                    let new_count_var = format_ident!("__{}_new_count", fname);
                    quote! {
                        let #new_count_var: usize;
                        {
                            let __old_count = __zc.#count_name.get() as usize;
                            let __elem_size = core::mem::size_of::<#elem>();
                            match #fname {
                                Some(__val) => {
                                    if __val.len() > #max {
                                        return Err(QuasarError::DynamicFieldTooLong.into());
                                    }
                                    #new_count_var = __val.len();
                                    let __bytes = #new_count_var * __elem_size;
                                    if __bytes > 0 {
                                        unsafe {
                                            core::ptr::copy_nonoverlapping(
                                                __val.as_ptr() as *const u8,
                                                __buf[__buf_offset..].as_mut_ptr(),
                                                __bytes,
                                            );
                                        }
                                    }
                                    __buf_offset += __bytes;
                                }
                                None => {
                                    #new_count_var = __old_count;
                                    let __bytes = __old_count * __elem_size;
                                    __buf[__buf_offset..__buf_offset + __bytes]
                                        .copy_from_slice(&__data[__old_offset..__old_offset + __bytes]);
                                    __buf_offset += __bytes;
                                }
                            }
                            __old_offset += __old_count * __elem_size;
                        }
                    }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    let set_dyn_zc_updates: Vec<proc_macro2::TokenStream> = dyn_fields
        .iter()
        .map(|(f, kind)| {
            let fname = f.ident.as_ref().unwrap();
            match kind {
                DynKind::Str { .. } => {
                    let len_name = format_ident!("{}_len", fname);
                    let new_len_var = format_ident!("__{}_new_len", fname);
                    quote! { __zc.#len_name = quasar_core::pod::PodU16::from(#new_len_var as u16); }
                }
                DynKind::Vec { .. } => {
                    let count_name = format_ident!("{}_count", fname);
                    let new_count_var = format_ident!("__{}_new_count", fname);
                    quote! { __zc.#count_name = quasar_core::pod::PodU16::from(#new_count_var as u16); }
                }
                _ => unreachable!(),
            }
        })
        .collect();

    // --- Combine ---
    quote! {
        #(#attrs)*
        #vis struct #name #generics {
            #(#transformed_fields,)*
        }

        #[repr(C)]
        #[derive(Copy, Clone)]
        pub struct #zc_name {
            #(#zc_fields,)*
        }

        const _: () = assert!(
            core::mem::align_of::<#zc_name>() == 1,
            "ZC companion struct must have alignment 1; all fields must use Pod types or alignment-1 types"
        );

        #vis struct #fields_name<#lt> {
            #(#fields_struct_fields,)*
        }

        impl Discriminator for #name<'_> {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
        }

        impl Space for #name<'_> {
            const SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>();
        }

        impl Owner for #name<'_> {
            const OWNER: Address = crate::ID;
        }

        impl AccountCheck for #name<'_> {
            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
                let __data = unsafe { view.borrow_unchecked() };
                let __min = #disc_len + core::mem::size_of::<#zc_name>();
                if __data.len() < __min {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                #(
                    if unsafe { *__data.get_unchecked(#disc_indices) } != #disc_bytes {
                        return Err(ProgramError::InvalidAccountData);
                    }
                )*
                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                let __total = __min #(#var_check_terms)*;
                if __total > __data.len() {
                    return Err(ProgramError::AccountDataTooSmall);
                }
                Ok(())
            }
        }

        /// View type for dynamic account — sits in the deref chain between
        /// `Account<#name<'_>>` and `#zc_name`. Provides zero-copy accessors
        /// for dynamic fields and derefs to the ZC struct for fixed fields.
        #[repr(transparent)]
        #vis struct #view_name {
            __view: AccountView,
        }

        impl AsAccountView for #view_name {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.__view
            }
        }

        impl #view_name {
            #[inline(always)]
            pub fn realloc(
                &self,
                new_space: usize,
                payer: &AccountView,
                rent: Option<&Rent>,
            ) -> Result<(), ProgramError> {
                quasar_core::accounts::account::realloc_account(&self.__view, new_space, payer, rent)
            }

            #(#accessor_methods)*
            #(#write_methods)*

            #[inline(always)]
            pub fn dynamic_fields(&self) -> #fields_name<'_> {
                let __data = unsafe { self.__view.borrow_unchecked() };
                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };
                let mut __offset = #disc_len + core::mem::size_of::<#zc_name>();
                #(#fields_extract_stmts)*
                let _ = __offset;
                #fields_name { #(#fields_field_names),* }
            }

            #[inline(always)]
            pub fn set_dynamic_fields(&self, __payer: &impl AsAccountView, #(#set_dyn_params),*) -> Result<(), ProgramError> {
                let __view = &self.__view;
                let __data = unsafe { __view.borrow_unchecked() };
                let __zc = unsafe { &*(__data[#disc_len..].as_ptr() as *const #zc_name) };

                let mut __buf = [0u8; 0 #(#max_space_terms)*];
                let mut __buf_offset = 0usize;
                let mut __old_offset = #disc_len + core::mem::size_of::<#zc_name>();

                #(#set_dyn_buf_stmts)*

                let _ = __old_offset;
                let __new_total = #disc_len + core::mem::size_of::<#zc_name>() + __buf_offset;
                let __old_total = __data.len();

                if __new_total > __old_total {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }

                let __data = unsafe { __view.borrow_unchecked_mut() };
                let __tail_start = #disc_len + core::mem::size_of::<#zc_name>();
                __data[__tail_start..__tail_start + __buf_offset]
                    .copy_from_slice(&__buf[..__buf_offset]);

                let __zc = unsafe { &mut *(__data[#disc_len..].as_mut_ptr() as *mut #zc_name) };
                #(#set_dyn_zc_updates)*

                if __new_total < __old_total {
                    self.realloc(__new_total, __payer.to_account_view(), None)?;
                }

                Ok(())
            }
        }

        impl core::ops::Deref for #view_name {
            type Target = #zc_name;

            /// SAFETY: Bounds validated by AccountCheck::check. ZC struct has alignment 1.
            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*(self.__view.data_ptr().add(#disc_len) as *const #zc_name) }
            }
        }

        impl core::ops::DerefMut for #view_name {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *(self.__view.data_ptr().add(#disc_len) as *mut #zc_name) }
            }
        }

        impl ZeroCopyDeref for #name<'_> {
            type Target = #view_name;

            /// SAFETY: #view_name is #[repr(transparent)] over AccountView.
            #[inline(always)]
            fn deref_from(view: &AccountView) -> &Self::Target {
                unsafe { &*(view as *const AccountView as *const #view_name) }
            }

            #[inline(always)]
            fn deref_from_mut(view: &AccountView) -> &mut Self::Target {
                unsafe { &mut *(view as *const AccountView as *mut #view_name) }
            }
        }

        impl #name<'_> {
            pub const MIN_SPACE: usize = #disc_len + core::mem::size_of::<#zc_name>();
            pub const MAX_SPACE: usize = Self::MIN_SPACE #(#max_space_terms)*;

            #[inline(always)]
            fn __dynamic_space(&self) -> usize {
                Self::MIN_SPACE #(#space_terms)*
            }

            #[inline(always)]
            fn __serialize_dynamic(&self, __data: &mut [u8]) -> Result<(), ProgramError> {
                let __zc = unsafe { &mut *(__data.as_mut_ptr() as *mut #zc_name) };
                #(#zc_header_stmts)*
                let mut __var_offset = core::mem::size_of::<#zc_name>();
                #(#var_serialize_stmts)*
                Ok(())
            }

            #[inline(always)]
            pub fn init<'__init>(self, account: &mut Initialize<#name<'__init>>, payer: &AccountView, rent: Option<&Rent>) -> Result<(), ProgramError> {
                self.init_signed(account, payer, rent, &[])
            }

            #[inline(always)]
            pub fn init_signed<'__init>(self, account: &mut Initialize<#name<'__init>>, payer: &AccountView, rent: Option<&Rent>, signers: &[quasar_core::cpi::Signer]) -> Result<(), ProgramError> {
                #(#max_checks)*

                let view = account.to_account_view();
                let __space = self.__dynamic_space();

                {
                    let __existing = unsafe { view.borrow_unchecked() };
                    if __existing.len() >= #disc_len {
                        #(
                            if unsafe { *__existing.get_unchecked(#disc_indices) } != 0 {
                                return Err(QuasarError::AccountAlreadyInitialized.into());
                            }
                        )*
                    }
                }

                let lamports = match rent {
                    Some(rent_account) => unsafe { rent_account.get_unchecked() }.minimum_balance_unchecked(__space),
                    None => {
                        use quasar_core::sysvars::Sysvar;
                        quasar_core::sysvars::rent::Rent::get()?.minimum_balance_unchecked(__space)
                    }
                };

                if view.lamports() == 0 {
                    quasar_core::cpi::system::create_account(payer, view, lamports, __space as u64, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                } else {
                    let required = lamports.saturating_sub(view.lamports());
                    if required > 0 {
                        quasar_core::cpi::system::transfer(payer, view, required)
                            .invoke_with_signers(signers)?;
                    }
                    quasar_core::cpi::system::assign(view, &Self::OWNER)
                        .invoke_with_signers(signers)?;
                    unsafe { view.resize_unchecked(__space) }?;
                }

                let __data = unsafe { view.borrow_unchecked_mut() };
                __data[..Self::DISCRIMINATOR.len()].copy_from_slice(Self::DISCRIMINATOR);
                self.__serialize_dynamic(&mut __data[Self::DISCRIMINATOR.len()..])?;
                Ok(())
            }
        }
    }
    .into()
}
