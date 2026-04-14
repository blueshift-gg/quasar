//! `#[event]` — generates event discriminator, serialization, and the `Event`
//! trait impl for emission via `sol_log_data` or self-CPI.

use {
    crate::helpers::InstructionArgs,
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
        data_size += size;
    }

    let total_buf_size = disc_len + data_size;
    let emit_log_method = quote! {
        impl #name {
            #[inline(always)]
            pub fn emit_log(&self) {
                let mut buf = core::mem::MaybeUninit::<[u8; #total_buf_size]>::uninit();
                let ptr = buf.as_mut_ptr() as *mut u8;
                // Use the extracted helper so the code path is covered by Kani
                // proof harnesses (see lang/src/event.rs kani_proofs).
                let data_offset = unsafe {
                    quasar_lang::event::write_log_disc(
                        ptr,
                        <Self as quasar_lang::traits::Event>::DISCRIMINATOR,
                    )
                };
                <Self as quasar_lang::traits::Event>::write_data(self, unsafe {
                    core::slice::from_raw_parts_mut(ptr.add(data_offset), #data_size)
                });
                quasar_lang::log::log_data(&[unsafe { buf.assume_init_ref() }]);
            }
        }
    };

    let data_size_lit = proc_macro2::Literal::usize_unsuffixed(data_size);

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

                // Use the extracted helper so the code path is covered by Kani
                // proof harnesses (see lang/src/event.rs kani_proofs).
                let data_offset = unsafe {
                    quasar_lang::event::write_cpi_disc(ptr, Self::DISCRIMINATOR)
                };

                self.write_data(unsafe {
                    core::slice::from_raw_parts_mut(
                        ptr.add(data_offset),
                        __DATA_SIZE,
                    )
                });

                f(unsafe { buf.assume_init_ref() })
            }
        }

        #emit_log_method
    }.into()
}
