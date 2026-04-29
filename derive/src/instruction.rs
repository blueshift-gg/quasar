//! `#[instruction]` — generates instruction handler wrappers with context
//! deserialization, discriminator matching, and zero-copy argument decoding.
//!
//! Instruction args use the same zeropod layout as accounts:
//! - Fixed args: `ZeroPodFixed` pointer-cast + validate
//! - Dynamic args (`String<N>`, `Vec<T, N>`, `&str`, `&[T]`): `ZeroPodCompact`
//!   Ref views
//!
//! Borrowed args (`&'a str`, `&'a [T]`) are desugared to compact schema fields
//! via `#[max(N)]` annotations — the compact Ref returns zero-copy views.

use {
    crate::helpers::{
        classify_borrowed_as_compact, classify_lifetime_arg, classify_pod_dynamic,
        extract_generic_inner_type, is_unit_type, pod_dyn_to_compact_type, InstructionArgs,
        PodDynField,
    },
    proc_macro::TokenStream,
    quote::quote,
    syn::{parse_macro_input, FnArg, Ident, ItemFn, Pat, ReturnType, Type},
};

// Note: pre_hook/post_hook have been removed. Users should use
// #[accounts(validate)] for pre-handler validation, or write logic
// directly in the handler body.

/// Emit the ZeroPodFixed schema codegen block: derive struct, size check,
/// validate, cast, and per-field `from_zc` extraction.
fn emit_fixed_schema_stmts(
    param_ident: &Ident,
    field_names: &[Ident],
    field_types: &[syn::Type],
) -> Vec<syn::Stmt> {
    let mut stmts: Vec<syn::Stmt> = Vec::new();
    stmts.push(syn::parse_quote!(
        #[derive(zeropod::ZeroPod)]
        struct __InstructionDataSchema {
            #(#field_names: #field_types,)*
        }
    ));
    stmts.push(syn::parse_quote!(
        if #param_ident.data.len() < <__InstructionDataSchema as quasar_lang::ZeroPodFixed>::SIZE {
            return Err(ProgramError::InvalidInstructionData);
        }
    ));
    stmts.push(syn::parse_quote!(
        <__InstructionDataSchema as quasar_lang::ZeroPodFixed>::validate(
            &#param_ident.data[..<__InstructionDataSchema as quasar_lang::ZeroPodFixed>::SIZE]
        ).map_err(|_| ProgramError::InvalidInstructionData)?;
    ));
    stmts.push(syn::parse_quote!(
        let __zc = unsafe {
            <__InstructionDataSchema as quasar_lang::ZeroPodFixed>::from_bytes_unchecked(&#param_ident.data)
        };
    ));
    for (name, ty) in field_names.iter().zip(field_types.iter()) {
        stmts.push(syn::parse_quote!(
            let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__zc.#name);
        ));
    }
    stmts
}

/// Parse #[max(N)] or #[max(N, pfx = P)] from a function parameter's
/// attributes.
fn parse_max_attr_from_fn_arg(pt: &syn::PatType) -> Option<Result<(usize, usize), syn::Error>> {
    for attr in &pt.attrs {
        if attr.path().is_ident("max") {
            return Some(attr.parse_args_with(|stream: syn::parse::ParseStream| {
                let n: syn::LitInt = stream.parse()?;
                let max_n: usize = n.base10_parse()?;
                let mut pfx = 0usize;
                if !stream.is_empty() {
                    let _: syn::Token![,] = stream.parse()?;
                    let key: syn::Ident = stream.parse()?;
                    if key != "pfx" {
                        return Err(syn::Error::new(key.span(), "expected `pfx`"));
                    }
                    let _: syn::Token![=] = stream.parse()?;
                    let p: syn::LitInt = stream.parse()?;
                    pfx = p.base10_parse()?;
                }
                Ok((max_n, pfx))
            }));
        }
    }
    None
}

/// Build the handler tail: user body + epilogue, with optional return-data
/// wrapping. This is the single canonical emission point for the instruction
/// lifecycle after validate has run.
///
/// Lifecycle: parse → validate → handler → epilogue
fn emit_handler_tail(
    param_ident: &Ident,
    stmts: &[syn::Stmt],
    has_return_data: bool,
    return_ok_type: Option<&Type>,
) -> Vec<syn::Stmt> {
    let user_body: proc_macro2::TokenStream = stmts.iter().map(|s| quote!(#s)).collect();
    let mut tail = Vec::new();

    // Const-elide via HAS_EPILOGUE. When the struct has no close/sweep/migrate,
    // this branch is eliminated at compile time — saving ~2-7 CU on sBPF.
    let epilogue_call = quote! {
        if #param_ident.has_epilogue() {
            #param_ident.accounts.epilogue()?;
        }
    };

    if has_return_data {
        let ok_ty =
            return_ok_type.expect("return_ok_type must be set when has_return_data is true");
        tail.push(syn::parse_quote!(
            const _: () = assert!(
                core::mem::align_of::<<#ok_ty as quasar_lang::instruction_arg::InstructionArg>::Zc>() == 1,
                "return data type must implement InstructionArg with an alignment-1 Zc companion"
            );
        ));
        tail.push(syn::parse_quote!(
            {
                let __result: Result<#ok_ty, ProgramError> = (|| { #user_body })();
                match __result {
                    Ok(ref __val) => {
                        #epilogue_call
                        let __zc =
                            <#ok_ty as quasar_lang::instruction_arg::InstructionArg>::to_zc(__val);
                        let __bytes = unsafe {
                            core::slice::from_raw_parts(
                                &__zc as *const <#ok_ty as quasar_lang::instruction_arg::InstructionArg>::Zc as *const u8,
                                core::mem::size_of::<<#ok_ty as quasar_lang::instruction_arg::InstructionArg>::Zc>(),
                            )
                        };
                        quasar_lang::return_data::set_return_data(__bytes);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
        ));
    } else {
        tail.push(syn::parse_quote!({
            let __user_result: Result<(), ProgramError> = { #user_body };
            __user_result?;
            #epilogue_call
            Ok(())
        }));
    }

    tail
}

pub(crate) fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as InstructionArgs);
    let mut func = parse_macro_input!(item as ItemFn);
    let disc_bytes = match &args.discriminator {
        Some(d) => d,
        None => {
            return syn::Error::new_spanned(
                &func.sig.ident,
                "#[instruction] requires `discriminator = [...]`",
            )
            .to_compile_error()
            .into();
        }
    };
    let disc_len = disc_bytes.len();

    // Reject multi-byte all-zero discriminators — zeroed instruction data could
    // accidentally match. Single-byte discriminators are fine (the dispatch
    // macro's length check rejects empty instruction data).
    if disc_len > 1
        && disc_bytes
            .iter()
            .all(|lit| matches!(lit.base10_parse::<u8>(), Ok(0)))
    {
        return syn::Error::new_spanned(
            &disc_bytes[0],
            "instruction discriminator must contain at least one non-zero byte; all-zero \
             multi-byte discriminators are dangerous because zeroed instruction data would match",
        )
        .to_compile_error()
        .into();
    }

    // ── Raw path ────────────────────────────────────────────────────
    // When `raw` is set, the handler receives a bare `Context` — no
    // Ctx<T>, no arg decoding, no validate(), no epilogue. The macro
    // just validates the signature and emits the function unchanged.
    // The `#[program]` codegen is responsible for generating the
    // dispatch arm that constructs and passes the `Context`.
    if args.raw {
        let first_arg = match func.sig.inputs.first() {
            Some(FnArg::Typed(pt)) => pt.clone(),
            _ => {
                return syn::Error::new_spanned(
                    &func.sig.ident,
                    "#[instruction(raw)] requires a single parameter of type Context",
                )
                .to_compile_error()
                .into();
            }
        };

        // Check the type path ends with "Context".
        let is_context = match &*first_arg.ty {
            Type::Path(tp) => tp
                .path
                .segments
                .last()
                .is_some_and(|seg| seg.ident == "Context"),
            _ => false,
        };
        if !is_context {
            return syn::Error::new_spanned(
                &first_arg.ty,
                "#[instruction(raw)] parameter must be of type Context",
            )
            .to_compile_error()
            .into();
        }

        if func.sig.inputs.len() > 1 {
            return syn::Error::new_spanned(
                &func.sig,
                "#[instruction(raw)] handler must have exactly one parameter: Context",
            )
            .to_compile_error()
            .into();
        }

        // Emit the function unchanged — no wrapping, no codegen.
        return quote!(#func).into();
    }

    // ── Normal path ──────────────────────────────────────────────────
    let first_arg = match func.sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt.clone(),
        _ => {
            return syn::Error::new_spanned(
                &func.sig.ident,
                "#[instruction] requires ctx: Ctx<T> as first parameter",
            )
            .to_compile_error()
            .into();
        }
    };

    let param_name = &first_arg.pat;
    let param_ident = match &*first_arg.pat {
        Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        _ => {
            return syn::Error::new_spanned(
                &first_arg.pat,
                "#[instruction] ctx parameter must be an identifier",
            )
            .to_compile_error()
            .into();
        }
    };
    let param_type = &first_arg.ty;

    let return_ok_type = match &func.sig.output {
        ReturnType::Type(_, ty) => extract_generic_inner_type(ty, "Result").cloned(),
        _ => None,
    };
    let has_return_data = return_ok_type
        .as_ref()
        .is_some_and(|ok_ty| !is_unit_type(ok_ty));

    if has_return_data {
        func.sig.output = syn::parse_quote!(-> Result<(), ProgramError>);
    }

    let remaining: Vec<_> = func
        .sig
        .inputs
        .iter()
        .skip(1)
        .filter_map(|arg| match arg {
            FnArg::Typed(pt) => Some(pt.clone()),
            _ => None,
        })
        .collect();

    func.sig.inputs = syn::punctuated::Punctuated::new();
    func.sig
        .inputs
        .push(syn::parse_quote!(mut context: Context));

    let stmts = std::mem::take(&mut func.block.stmts);
    let mut new_stmts: Vec<syn::Stmt> = vec![
        // Skip past the discriminator prefix. The dispatch! macro in the
        // entrypoint already verified the discriminator matches via a
        // fixed-size array comparison, so no need to re-check here.
        syn::parse_quote!(
            context.data = &context.data[#disc_len..];
        ),
        syn::parse_quote!(
            let mut #param_name: #param_type = <#param_type>::new(context)?;
        ),
        // Call validate() only when the user overrides it. The const bool
        // is known at compile time so this branch is fully elided when false,
        // avoiding a dead Result branch that sBPF doesn't optimize away.
        syn::parse_quote!(
            if #param_ident.has_validate() {
                #param_ident.accounts.validate()?;
            }
        ),
    ];

    if !remaining.is_empty() {
        let mut field_names: Vec<Ident> = Vec::with_capacity(remaining.len());
        for pt in &remaining {
            match &*pt.pat {
                Pat::Ident(pat_ident) => field_names.push(pat_ident.ident.clone()),
                _ => {
                    return syn::Error::new_spanned(
                        &pt.pat,
                        "#[instruction] parameters must be simple identifiers",
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }

        /// Per-arg classification: fixed-size or dynamic (compact) decode.
        enum ArgClass {
            Fixed,
            PodDyn(PodDynField),
        }

        let mut arg_classes: Vec<ArgClass> = Vec::with_capacity(remaining.len());
        for pt in &remaining {
            if let Some(pd) = classify_pod_dynamic(&pt.ty) {
                arg_classes.push(ArgClass::PodDyn(pd));
            } else if matches!(&*pt.ty, Type::Reference(_)) {
                // Borrowed arg — desugar to compact via #[max(N)]
                match parse_max_attr_from_fn_arg(pt) {
                    Some(Ok((max_n, pfx))) => {
                        match classify_borrowed_as_compact(&pt.ty, max_n, pfx) {
                            Some(pd) => arg_classes.push(ArgClass::PodDyn(pd)),
                            None => {
                                return syn::Error::new_spanned(
                                    &pt.ty,
                                    "unsupported borrowed type; use &str or &[T]",
                                )
                                .to_compile_error()
                                .into();
                            }
                        }
                    }
                    Some(Err(e)) => return e.to_compile_error().into(),
                    None => {
                        return syn::Error::new_spanned(
                            &pt.ty,
                            "borrowed instruction args require #[max(N)] annotation",
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            } else if classify_lifetime_arg(&pt.ty) {
                // Grouped borrowed struct (e.g., MintArgs<'a>) — must be the only arg
                if remaining.len() != 1 {
                    return syn::Error::new_spanned(
                        &pt.ty,
                        "a grouped borrowed struct must be the only instruction arg (besides ctx)",
                    )
                    .to_compile_error()
                    .into();
                }

                // Generate: let arg_name = <ArgType>::decode_compact(&ctx.data)?;
                let arg_name = &field_names[0];
                let arg_ty = &pt.ty;
                new_stmts.push(syn::parse_quote!(
                    let #arg_name = <#arg_ty>::decode_compact(&#param_ident.data)?;
                ));

                // Clear ctx.data after extraction
                new_stmts.push(syn::parse_quote!(
                    #param_ident.data = &[];
                ));

                new_stmts.extend(emit_handler_tail(
                    &param_ident,
                    &stmts,
                    has_return_data,
                    return_ok_type.as_ref(),
                ));
                func.block.stmts = new_stmts;

                return quote!(#func).into();
            } else {
                arg_classes.push(ArgClass::Fixed);
            }
        }

        let first_dynamic = arg_classes
            .iter()
            .position(|cls| !matches!(cls, ArgClass::Fixed));
        let last_fixed = arg_classes
            .iter()
            .rposition(|cls| matches!(cls, ArgClass::Fixed));
        if let (Some(fd), Some(lf)) = (first_dynamic, last_fixed) {
            if lf > fd {
                return syn::Error::new_spanned(
                    &remaining[lf],
                    "fixed instruction args must precede all dynamic or borrowed args",
                )
                .to_compile_error()
                .into();
            }
        }

        let vec_align_asserts: Vec<proc_macro2::TokenStream> = arg_classes
            .iter()
            .filter_map(|cls| match cls {
                ArgClass::PodDyn(PodDynField::Vec { elem, .. }) => Some(quote! {
                    const _: () = assert!(
                        core::mem::align_of::<#elem>() == 1,
                        "instruction Vec element type must have alignment 1"
                    );
                }),
                _ => None,
            })
            .collect();

        for assert_stmt in vec_align_asserts {
            new_stmts.push(
                syn::parse2(assert_stmt)
                    .expect("failed to parse generated Vec alignment assert statement"),
            );
        }

        let has_pod_dyn = arg_classes
            .iter()
            .any(|cls| matches!(cls, ArgClass::PodDyn(_)));

        // Alias quasar_lang's re-export so `zeropod::*` paths emitted by
        // the ZeroPod derive resolve without a direct crate dependency.
        new_stmts.push(syn::parse_quote!(
            use quasar_lang::__zeropod as zeropod;
        ));

        if has_pod_dyn {
            // Compact path: a single zeropod compact schema with ALL fields
            // (fixed + dynamic). The header contains fixed fields and length
            // prefixes; tail data follows immediately after the header.
            let compact_field_names: Vec<_> = field_names.clone();
            let compact_field_types: Vec<proc_macro2::TokenStream> = arg_classes
                .iter()
                .zip(remaining.iter())
                .map(|(cls, pt)| match cls {
                    ArgClass::Fixed => {
                        let ty = &pt.ty;
                        quote!(#ty)
                    }
                    ArgClass::PodDyn(ref pd) => pod_dyn_to_compact_type(pd),
                })
                .collect();

            new_stmts.push(syn::parse_quote!(
                #[derive(zeropod::ZeroPod)]
                #[zeropod(compact)]
                struct __InstructionDataCompact {
                    #(#compact_field_names: #compact_field_types,)*
                }
            ));

            new_stmts.push(syn::parse_quote!(
                <__InstructionDataCompact as quasar_lang::ZeroPodCompact>::validate(
                    &#param_ident.data
                ).map_err(|_| ProgramError::InvalidInstructionData)?;
            ));

            new_stmts.push(syn::parse_quote!(
                let __ref = unsafe {
                    __InstructionDataCompactRef::new_unchecked(&#param_ident.data)
                };
            ));

            for (i, cls) in arg_classes.iter().enumerate() {
                let name = &field_names[i];
                let ty = &remaining[i].ty;
                match cls {
                    ArgClass::Fixed => {
                        new_stmts.push(syn::parse_quote!(
                            let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ref.#name);
                        ));
                    }
                    ArgClass::PodDyn(_) => {
                        new_stmts.push(syn::parse_quote!(
                            let #name = __ref.#name();
                        ));
                    }
                }
            }
        } else {
            // Fixed-only path: keep the current ZeroPodFixed schema.
            let zc_field_names: Vec<_> = field_names.clone();
            let zc_field_orig_types: Vec<_> = remaining.iter().map(|pt| (*pt.ty).clone()).collect();

            new_stmts.extend(emit_fixed_schema_stmts(
                &param_ident,
                &zc_field_names,
                &zc_field_orig_types,
            ));
        }

        // Clear ctx.data after extraction
        new_stmts.push(syn::parse_quote!(
            #param_ident.data = &[];
        ));
    }

    new_stmts.extend(emit_handler_tail(
        &param_ident,
        &stmts,
        has_return_data,
        return_ok_type.as_ref(),
    ));
    func.block.stmts = new_stmts;

    quote!(#func).into()
}
