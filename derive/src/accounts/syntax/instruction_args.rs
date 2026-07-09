use {
    crate::helpers::{classify_pod_dynamic, pod_dyn_to_compact_type, PodDynField},
    quote::quote,
    syn::{parse::ParseStream, DeriveInput, Ident, Token, Type},
};

pub(crate) struct InstructionArg {
    pub name: Ident,
    pub ty: Type,
}

pub(crate) fn parse_struct_instruction_args(
    input: &DeriveInput,
) -> syn::Result<Option<Vec<InstructionArg>>> {
    let attr = match input
        .attrs
        .iter()
        .find(|a| a.path().is_ident("instruction"))
    {
        Some(attr) => attr,
        None => return Ok(None),
    };

    let args = attr.parse_args_with(|stream: ParseStream| {
        let mut args = Vec::new();
        while !stream.is_empty() {
            let name: Ident = stream.parse()?;
            let _: Token![:] = stream.parse()?;
            let ty: Type = stream.parse()?;
            if args.iter().any(|arg: &InstructionArg| arg.name == name) {
                return Err(syn::Error::new_spanned(
                    &name,
                    format!("duplicate instruction arg `{name}`"),
                ));
            }
            args.push(InstructionArg { name, ty });
            if !stream.is_empty() {
                let _: Token![,] = stream.parse()?;
            }
        }
        Ok(args)
    })?;

    Ok(Some(args))
}

/// Generate code that extracts struct-level `#[instruction(..)]` args from
/// `__ix_data` (the discriminator-stripped instruction data).
///
/// The declared arg list must match the handler's parameters through the last
/// dynamic arg: both decoders read the same buffer, so a divergent prefix would
/// desync every arg after it.
///
/// When every declared arg is fixed, they are read via a zero-copy `#[repr(C)]`
/// struct pointer cast. When any arg is dynamic (`String<N>` / `Vec<T, N>`),
/// the whole list is decoded through the canonical zeropod **compact** layout —
/// inline fixed fields, then all tail length-prefixes, then all tail payloads —
/// which is exactly what the handler macro (`derive/src/instruction.rs`) and
/// the generated client emit. Decoding an interleaved layout here would make
/// the accounts-side constraints read different bytes than the handler once two
/// or more dynamic args are present.
pub(crate) fn generate_instruction_arg_extraction(
    ix_args: &[InstructionArg],
) -> proc_macro2::TokenStream {
    if ix_args.is_empty() {
        return quote! {};
    }

    let pod_dyns: Vec<Option<PodDynField>> = ix_args
        .iter()
        .map(|arg| classify_pod_dynamic(&arg.ty))
        .collect();

    let has_dynamic = pod_dyns.iter().any(|pd| pd.is_some());

    let vec_align_asserts: Vec<proc_macro2::TokenStream> = pod_dyns
        .iter()
        .filter_map(|pd| match pd {
            Some(PodDynField::Vec { elem, .. }) => Some(quote! {
                const _: () = assert!(
                    core::mem::align_of::<#elem>() == 1,
                    "instruction Vec element type must have alignment 1"
                );
            }),
            _ => None,
        })
        .collect();

    let mut stmts: Vec<proc_macro2::TokenStream> = vec_align_asserts;

    if !has_dynamic {
        // Pure-fixed path: read every arg from a `#[repr(C)]` ZC struct cast.
        let zc_field_names: Vec<Ident> = ix_args.iter().map(|arg| arg.name.clone()).collect();
        let zc_field_types: Vec<proc_macro2::TokenStream> = ix_args
            .iter()
            .map(|arg| {
                let ty = &arg.ty;
                quote! { <#ty as quasar_lang::instruction_arg::InstructionArg>::Zc }
            })
            .collect();

        stmts.push(quote! {
            #[repr(C)]
            struct __IxArgsZc {
                #(#zc_field_names: #zc_field_types,)*
            }
        });

        stmts.push(quote! {
            const _: () = assert!(
                core::mem::align_of::<__IxArgsZc>() == 1,
                "instruction args ZC struct must have alignment 1"
            );
        });

        stmts.push(quote! {
            if __ix_data.len() < core::mem::size_of::<__IxArgsZc>() {
                return Err(ProgramError::InvalidInstructionData);
            }
        });

        stmts.push(quote! {
            // SAFETY: `__IxArgsZc` has alignment 1 and the preceding length check
            // guarantees the fixed ZC block is present.
            let __ix_zc = unsafe { &*(__ix_data.as_ptr() as *const __IxArgsZc) };
        });

        for arg in ix_args {
            let name = &arg.name;
            let ty = &arg.ty;
            stmts.push(quote! {
                <#ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__ix_zc.#name)?;
                let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ix_zc.#name);
            });
        }

        return quote! { #(#stmts)* };
    }

    // Mixed/dynamic path: decode via the canonical zeropod compact layout, the
    // same schema the handler macro and the generated client use. Fixed args
    // stay inline; dynamic args (String<N>/Vec<T, N>) become compact fields whose
    // length prefixes group ahead of their payloads. `validate` enforces the
    // `#[max]` bounds, so no separate bound checks are emitted here.
    //
    // Alias quasar_lang's re-export so `zeropod::*` paths emitted by the ZeroPod
    // derive resolve without a direct crate dependency.
    stmts.push(quote! {
        use quasar_lang::__zeropod as zeropod;
    });

    let compact_field_names: Vec<Ident> = ix_args.iter().map(|arg| arg.name.clone()).collect();
    let compact_field_types: Vec<proc_macro2::TokenStream> = ix_args
        .iter()
        .zip(pod_dyns.iter())
        .map(|(arg, pd)| match pd {
            Some(pd) => pod_dyn_to_compact_type(pd),
            None => {
                let ty = &arg.ty;
                quote! { #ty }
            }
        })
        .collect();

    stmts.push(quote! {
        #[derive(zeropod::ZeroPod)]
        #[zeropod(compact)]
        struct __IxArgsCompact {
            #(#compact_field_names: #compact_field_types,)*
        }
    });

    stmts.push(quote! {
        <__IxArgsCompact as quasar_lang::ZeroPodCompact>::validate(__ix_data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;
    });

    stmts.push(quote! {
        // SAFETY: `validate` succeeded on this exact slice above.
        let __ref = unsafe { __IxArgsCompactRef::new_unchecked(__ix_data) };
    });

    for (arg, pd) in ix_args.iter().zip(pod_dyns.iter()) {
        let name = &arg.name;
        match pd {
            Some(_) => {
                // Dynamic accessor: returns a zero-copy `&str` / `&[T]` view.
                stmts.push(quote! {
                    let #name = __ref.#name();
                });
            }
            None => {
                let ty = &arg.ty;
                // Per-field semantic validation before from_zc: the compact
                // schema validate() checks layout/prefix bounds but not
                // InstructionArg-level invariants (e.g. Option tags), matching
                // the handler macro's compact decode path.
                stmts.push(quote! {
                    <#ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__ref.#name)
                        .map_err(|_| ProgramError::InvalidInstructionData)?;
                    let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ref.#name);
                });
            }
        }
    }

    quote! { #(#stmts)* }
}
