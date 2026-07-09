//! `#[event]`: generates event discriminator, serialization, and the `Event`
//! trait impl for emission via `sol_log_data` or self-CPI.

use {
    crate::helpers::{parse_discriminator_bytes, InstructionArgs},
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, Data, DeriveInput, Fields, Type},
};

fn event_field_size(ty: &Type) -> syn::Result<usize> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" | "i8" | "bool" => Ok(1),
                "u16" | "i16" => Ok(2),
                "u32" | "i32" => Ok(4),
                "u64" | "i64" => Ok(8),
                "u128" | "i128" => Ok(16),
                "Address" => Ok(32),
                _ => Err(syn::Error::new_spanned(
                    ty,
                    format!(
                        "unsupported event field type `{}`; only primitive integers, bool, and \
                         Address are supported",
                        seg.ident
                    ),
                )),
            };
        }
    }
    Err(syn::Error::new_spanned(ty, "unsupported event field type"))
}

pub(crate) fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let disc_bytes = match &args.discriminator {
        Some(d) => d,
        None => {
            return syn::Error::new_spanned(
                &input.ident,
                "#[event] requires `discriminator = [...]`",
            )
            .to_compile_error()
            .into();
        }
    };
    let disc_len = disc_bytes.len();

    let fields_data = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(&input, "#[event] requires named fields")
                    .to_compile_error()
                    .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "#[event] can only be used on structs")
                .to_compile_error()
                .into();
        }
    };

    let mut data_size: usize = 0;
    for field in fields_data.iter() {
        let size = match event_field_size(&field.ty) {
            Ok(s) => s,
            Err(e) => return e.to_compile_error().into(),
        };
        data_size = match data_size.checked_add(size) {
            Some(total) => total,
            None => {
                return syn::Error::new_spanned(&field.ty, "event data size exceeds usize::MAX")
                    .to_compile_error()
                    .into();
            }
        };
    }

    let total_buf_size = disc_len + data_size;
    let emit_log_method = quote! {
        impl #name {
            #[inline(always)]
            pub fn emit_log(&self) {
                let mut buf = core::mem::MaybeUninit::<[u8; #total_buf_size]>::uninit();
                let ptr = buf.as_mut_ptr() as *mut u8;
                // SAFETY: `ptr` points to the start of a `#total_buf_size`
                // byte buffer, which is `disc_len + data_size`.
                let data_offset = unsafe {
                    quasar_lang::event::write_log_disc(
                        ptr,
                        <Self as quasar_lang::traits::Event>::DISCRIMINATOR,
                    )
                };
                // SAFETY: `write_log_disc` initialized the discriminator bytes
                // and returned the payload offset. The remaining `#data_size`
                // bytes fit exactly in the buffer.
                <Self as quasar_lang::traits::Event>::write_data(self, unsafe {
                    core::slice::from_raw_parts_mut(ptr.add(data_offset), #data_size)
                });
                // SAFETY: Discriminator and payload bytes were initialized
                // above before exposing the full buffer as a slice.
                quasar_lang::log::log_data(&[unsafe { buf.assume_init_ref() }]);
            }
        }
    };

    let data_size_lit = proc_macro2::Literal::usize_unsuffixed(data_size);

    // IDL fragment emission
    let name_str = name.to_string();
    let disc_values = match parse_discriminator_bytes(disc_bytes) {
        Ok(values) => values,
        Err(e) => return e.to_compile_error().into(),
    };
    let field_defs: Vec<proc_macro2::TokenStream> = fields_data
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

    let idl_fragment = quote! {
        #[cfg(feature = "idl-build")]
        quasar_lang::__private_inventory::submit! {
            quasar_lang::idl_build::EventFragment {
                build: {
                    fn __build() -> (
                        quasar_lang::idl_build::__reexport::IdlEventDef,
                        quasar_lang::idl_build::__reexport::IdlTypeDef,
                    ) {
                        (
                            quasar_lang::idl_build::__reexport::IdlEventDef {
                                name: quasar_lang::idl_build::s(#name_str),
                                discriminator: quasar_lang::idl_build::vec![#(#disc_values),*],
                                docs: quasar_lang::idl_build::Vec::new(),
                                ty: None,
                            },
                            quasar_lang::idl_build::__reexport::IdlTypeDef {
                                name: quasar_lang::idl_build::s(#name_str),
                                kind: quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct,
                                docs: quasar_lang::idl_build::Vec::new(),
                                generics: quasar_lang::idl_build::Vec::new(),
                                fields: quasar_lang::idl_build::vec![#(#field_defs),*],
                                variants: quasar_lang::idl_build::Vec::new(),
                                repr: None,
                                alias: None,
                                fallback: None,
                                codec: None,
                                layout: None,
                                space: None,
                                semantics: None,
                            },
                        )
                    }
                    __build
                },
            }
        }
    };

    quote! {
        #[repr(C)]
        #input

        const _: () = assert!(
            core::mem::size_of::<#name>() == #data_size_lit,
            "event struct has padding; cannot use memcpy serialization"
        );

        impl quasar_lang::traits::Event for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_bytes),*];
            const DATA_SIZE: usize = #data_size;

            #[inline(always)]
            fn write_data(&self, buf: &mut [u8]) {
                // SAFETY: The compile-time size assertion above proves `Self`
                // has exactly `DATA_SIZE` bytes with no padding, and callers
                // pass a buffer of that length.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        self as *const Self as *const u8,
                        buf.as_mut_ptr(),
                        #data_size_lit,
                    );
                }
            }

            #[inline(always)]
            fn emit(&self, f: impl FnOnce(&[u8]) -> Result<(), ProgramError>) -> Result<(), ProgramError> {
                const __DATA_SIZE: usize = #data_size;
                const __BUF_SIZE: usize = 1 + #disc_len + __DATA_SIZE;

                let mut buf = core::mem::MaybeUninit::<[u8; __BUF_SIZE]>::uninit();
                let ptr = buf.as_mut_ptr() as *mut u8;

                // SAFETY: `ptr` points to the start of a buffer sized for the
                // self-CPI sentinel, discriminator, and payload.
                let data_offset = unsafe {
                    quasar_lang::event::write_cpi_disc(ptr, Self::DISCRIMINATOR)
                };

                // SAFETY: `write_cpi_disc` initialized the prefix bytes and
                // returned the payload offset. The remaining `__DATA_SIZE`
                // bytes fit exactly in the buffer.
                self.write_data(unsafe {
                    core::slice::from_raw_parts_mut(
                        ptr.add(data_offset),
                        __DATA_SIZE,
                    )
                });

                // SAFETY: Prefix and payload bytes were initialized above.
                f(unsafe { buf.assume_init_ref() })
            }
        }

        #emit_log_method

        #idl_fragment
    }.into()
}
