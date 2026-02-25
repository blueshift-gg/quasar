use {
    crate::helpers::InstructionArgs,
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, Type},
};

fn event_field_size(ty: &Type) -> usize {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" | "i8" | "bool" => 1,
                "u16" | "i16" => 2,
                "u32" | "i32" => 4,
                "u64" | "i64" => 8,
                "u128" | "i128" => 16,
                "Address" => 32,
                _ => panic!("unsupported event field type `{}`; only primitive integers, bool, and Address are supported", seg.ident),
            };
        }
    }
    panic!("unsupported event field type");
}

fn event_field_write(name: &Ident, ty: &Type, offset: usize) -> proc_macro2::TokenStream {
    let size = event_field_size(ty);
    let end = offset + size;
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            return match seg.ident.to_string().as_str() {
                "u8" => quote! { buf[#offset] = self.#name; },
                "i8" => quote! { buf[#offset] = self.#name as u8; },
                "bool" => quote! { buf[#offset] = self.#name as u8; },
                "Address" => quote! { buf[#offset..#end].copy_from_slice(self.#name.as_ref()); },
                _ => quote! { buf[#offset..#end].copy_from_slice(&self.#name.to_le_bytes()); },
            };
        }
    }
    unreachable!()
}

pub(crate) fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let disc_bytes = &args.discriminator;
    let disc_len = disc_bytes.len();

    let fields_data = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("#[event] requires named fields"),
        },
        _ => panic!("#[event] can only be used on structs"),
    };

    let mut data_size: usize = 0;
    let mut write_stmts: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields_data.iter() {
        let field_name = field.ident.as_ref().unwrap();
        let size = event_field_size(&field.ty);
        write_stmts.push(event_field_write(field_name, &field.ty, data_size));
        data_size += size;
    }

    let total_buf_size = disc_len + data_size;
    let emit_log_method = quote! {
        impl #name {
            #[inline(always)]
            pub fn emit_log(&self) {
                let mut buf = [0u8; #total_buf_size];
                buf[..#disc_len].copy_from_slice(<Self as quasar_core::traits::Event>::DISCRIMINATOR);
                <Self as quasar_core::traits::Event>::write_data(self, &mut buf[#disc_len..]);
                quasar_core::log::log_data(&[&buf]);
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

        impl quasar_core::traits::Event for #name {
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
                const __EVENT_DISC_LEN: usize = #disc_len;
                const __DATA_SIZE: usize = #data_size;
                const __BUF_SIZE: usize = 1 + __EVENT_DISC_LEN + __DATA_SIZE;

                let mut buf = core::mem::MaybeUninit::<[u8; __BUF_SIZE]>::uninit();
                let ptr = buf.as_mut_ptr() as *mut u8;

                unsafe {
                    core::ptr::write(ptr, 0xFF);
                    core::ptr::copy_nonoverlapping(
                        Self::DISCRIMINATOR.as_ptr(),
                        ptr.add(1),
                        __EVENT_DISC_LEN,
                    );
                }

                self.write_data(unsafe {
                    core::slice::from_raw_parts_mut(
                        ptr.add(1 + __EVENT_DISC_LEN),
                        __DATA_SIZE,
                    )
                });

                f(unsafe { buf.assume_init_ref() })
            }
        }

        #emit_log_method
    }.into()
}
