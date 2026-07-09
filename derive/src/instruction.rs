//! `#[instruction]`: generates instruction handler wrappers with context
//! deserialization, discriminator matching, and zero-copy argument decoding.
//!
//! Instruction args use the same zeropod layout as accounts:
//! - Fixed args: `ZeroPodFixed` pointer-cast + validate
//! - Dynamic args (`String<N>`, `Vec<T, N>`, `&str`, `&[T]`): `ZeroPodCompact`
//!   Ref views
//!
//! Borrowed args (`&'a str`, `&'a [T]`) are desugared to compact schema fields
//! via `#[max(N)]` annotations: the compact Ref returns zero-copy views.

use {
    crate::helpers::{
        classify_borrowed_as_compact, classify_lifetime_arg, classify_option_pod_dynamic,
        classify_pod_dynamic, extract_generic_inner_type, is_unit_type, parse_discriminator_bytes,
        parse_max_attr, pod_dyn_to_compact_type, InstructionArgs, PodDynField,
    },
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    syn::{FnArg, Ident, ItemFn, Pat, ReturnType, Type},
};

/// Emit the fixed-argument decode block.
///
/// This deliberately avoids deriving a local `ZeroPodFixed` schema for the
/// common fixed-only path. The derive had enough information to emit the ZC
/// layout directly, which removes the schema wrapper's duplicate length check
/// and lets each field validate through `InstructionArg::validate_zc`.
fn emit_fixed_schema_stmts(
    param_ident: &Ident,
    field_names: &[Ident],
    field_types: &[&syn::Type],
) -> Vec<syn::Stmt> {
    let mut stmts: Vec<syn::Stmt> = Vec::new();
    stmts.push(syn::parse_quote!(
        #[repr(C)]
        struct __InstructionDataZc {
            #(#field_names: <#field_types as quasar_lang::instruction_arg::InstructionArg>::Zc,)*
        }
    ));
    stmts.push(syn::parse_quote!(
        const _: () = assert!(
            core::mem::align_of::<__InstructionDataZc>() == 1,
            "fixed instruction data ZC layout must have alignment 1"
        );
    ));
    stmts.push(syn::parse_quote!(
        const __INSTRUCTION_DATA_SIZE: usize = core::mem::size_of::<__InstructionDataZc>();
    ));
    stmts.push(syn::parse_quote!(
        if #param_ident.data.len() < __INSTRUCTION_DATA_SIZE {
            return Err(ProgramError::InvalidInstructionData);
        }
    ));
    stmts.push(syn::parse_quote!(
        let __zc = unsafe { &*(#param_ident.data.as_ptr() as *const __InstructionDataZc) };
    ));
    for (name, ty) in field_names.iter().zip(field_types.iter()) {
        stmts.push(syn::parse_quote!(
            <#ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__zc.#name)
                .map_err(|_| ProgramError::InvalidInstructionData)?;
        ));
        stmts.push(syn::parse_quote!(
            let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__zc.#name);
        ));
    }
    stmts
}

/// Build the handler tail: user body + epilogue, with optional return-data
/// wrapping. This is the single canonical emission point for the instruction
/// lifecycle after validate has run.
///
/// Lifecycle: parse -> validate -> handler -> epilogue
fn emit_handler_tail(
    param_ident: &Ident,
    stmts: &[syn::Stmt],
    has_return_data: bool,
    return_ok_type: Option<&Type>,
) -> Vec<syn::Stmt> {
    let user_body: proc_macro2::TokenStream = stmts.iter().map(|s| quote!(#s)).collect();
    let mut tail = Vec::new();

    // Const-elide via HAS_EPILOGUE. When the struct has no close/sweep/migrate,
    // this branch is eliminated at compile time: saving ~2-7 CU on sBPF.
    let epilogue_call = quote! {
        if #param_ident.has_epilogue() {
            #param_ident.accounts.epilogue()?;
        }
    };

    if has_return_data {
        let ok_ty = return_ok_type
            .unwrap_or_else(|| ice!("return_ok_type must be set when has_return_data is true"));
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

fn emit_decode_and_tail(
    param_ident: &Ident,
    remaining: &[syn::PatType],
    stmts: &[syn::Stmt],
    has_return_data: bool,
    return_ok_type: Option<&Type>,
) -> syn::Result<Vec<syn::Stmt>> {
    let mut out = Vec::new();

    if !remaining.is_empty() {
        let mut field_names: Vec<Ident> = Vec::with_capacity(remaining.len());
        for pt in remaining {
            match &*pt.pat {
                Pat::Ident(pat_ident) => field_names.push(pat_ident.ident.clone()),
                _ => {
                    return Err(syn::Error::new_spanned(
                        &pt.pat,
                        "#[instruction] parameters must be simple identifiers",
                    ));
                }
            }
        }

        /// Per-arg classification: fixed-size or dynamic (compact) decode.
        enum ArgClass {
            Fixed,
            PodDyn(PodDynField),
            OptionalPodDyn(PodDynField),
        }

        let mut arg_classes: Vec<ArgClass> = Vec::with_capacity(remaining.len());
        for pt in remaining {
            // Reject an invalid explicit length-prefix (e.g. `String<16, f32>`)
            // before it silently defaults to a 1-byte prefix in the schema.
            crate::helpers::validate_dynamic_prefix(&pt.ty)?;
            if let Some(pd) = classify_pod_dynamic(&pt.ty) {
                arg_classes.push(ArgClass::PodDyn(pd));
            } else if let Some(pd) = classify_option_pod_dynamic(&pt.ty) {
                arg_classes.push(ArgClass::OptionalPodDyn(pd));
            } else if matches!(&*pt.ty, Type::Reference(_)) {
                // Borrowed arg: desugar to compact via #[max(N)]
                match parse_max_attr(&pt.attrs) {
                    Some(Ok((max_n, pfx))) => {
                        match classify_borrowed_as_compact(&pt.ty, max_n, pfx) {
                            Some(pd) => arg_classes.push(ArgClass::PodDyn(pd)),
                            None => {
                                return Err(syn::Error::new_spanned(
                                    &pt.ty,
                                    "unsupported borrowed type; use &str or &[T]",
                                ));
                            }
                        }
                    }
                    Some(Err(e)) => return Err(e),
                    None => {
                        return Err(syn::Error::new_spanned(
                            &pt.ty,
                            "borrowed instruction args require #[max(N)] annotation",
                        ));
                    }
                }
            } else if classify_lifetime_arg(&pt.ty) {
                // Grouped borrowed struct (e.g., MintArgs<'a>): must be the
                // only arg.
                if remaining.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        &pt.ty,
                        "a grouped borrowed struct must be the only instruction arg (besides ctx)",
                    ));
                }

                // Generate: let arg_name = <ArgType>::decode_compact(&ctx.data)?;
                let arg_name = &field_names[0];
                let arg_ty = &pt.ty;
                out.push(syn::parse_quote!(
                    let #arg_name = <#arg_ty>::decode_compact(&#param_ident.data)?;
                ));

                out.push(syn::parse_quote!(
                    #param_ident.data = &[];
                ));

                out.extend(emit_handler_tail(
                    param_ident,
                    stmts,
                    has_return_data,
                    return_ok_type,
                ));
                return Ok(out);
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
                return Err(syn::Error::new_spanned(
                    &remaining[lf],
                    "fixed instruction args must precede all dynamic or borrowed args",
                ));
            }
        }

        for cls in &arg_classes {
            if let ArgClass::PodDyn(PodDynField::Vec { elem, .. })
            | ArgClass::OptionalPodDyn(PodDynField::Vec { elem, .. }) = cls
            {
                out.push(syn::parse_quote!(
                    const _: () = assert!(
                        core::mem::align_of::<#elem>() == 1,
                        "instruction Vec element type must have alignment 1"
                    );
                ));
            }
        }

        let has_pod_dyn = arg_classes
            .iter()
            .any(|cls| matches!(cls, ArgClass::PodDyn(_) | ArgClass::OptionalPodDyn(_)));

        if has_pod_dyn {
            // Alias quasar_lang's re-export so `zeropod::*` paths emitted by
            // the ZeroPod derive resolve without a direct crate dependency.
            out.push(syn::parse_quote!(
                use quasar_lang::__zeropod as zeropod;
            ));

            // Compact schema contains fixed fields, length prefixes, then tail data.
            let compact_field_types: Vec<proc_macro2::TokenStream> = arg_classes
                .iter()
                .zip(remaining.iter())
                .map(|(cls, pt)| match cls {
                    ArgClass::Fixed => {
                        let ty = &pt.ty;
                        quote!(#ty)
                    }
                    ArgClass::PodDyn(ref pd) => pod_dyn_to_compact_type(pd),
                    ArgClass::OptionalPodDyn(ref pd) => {
                        let inner = pod_dyn_to_compact_type(pd);
                        quote!(Option<#inner>)
                    }
                })
                .collect();

            out.push(syn::parse_quote!(
                #[derive(zeropod::ZeroPod)]
                #[zeropod(compact)]
                struct __InstructionDataCompact {
                    #(#field_names: #compact_field_types,)*
                }
            ));

            out.push(syn::parse_quote!(
                <__InstructionDataCompact as quasar_lang::ZeroPodCompact>::validate(
                    &#param_ident.data
                ).map_err(|_| ProgramError::InvalidInstructionData)?;
            ));

            out.push(syn::parse_quote!(
                let __ref = unsafe {
                    __InstructionDataCompactRef::new_unchecked(&#param_ident.data)
                };
            ));

            for (i, cls) in arg_classes.iter().enumerate() {
                let name = &field_names[i];
                let ty = &remaining[i].ty;
                match cls {
                    ArgClass::Fixed => {
                        // Match the fixed-only path: validate each fixed field's
                        // ZC bytes before reading, so a malformed inline value
                        // (bad bool/enum/etc.) is rejected rather than decoded.
                        out.push(syn::parse_quote!(
                            <#ty as quasar_lang::instruction_arg::InstructionArg>::validate_zc(&__ref.#name)
                                .map_err(|_| ProgramError::InvalidInstructionData)?;
                        ));
                        out.push(syn::parse_quote!(
                            let #name = <#ty as quasar_lang::instruction_arg::InstructionArg>::from_zc(&__ref.#name);
                        ));
                    }
                    ArgClass::PodDyn(_) | ArgClass::OptionalPodDyn(_) => {
                        out.push(syn::parse_quote!(
                            let #name = __ref.#name();
                        ));
                    }
                }
            }
        } else {
            let zc_field_orig_types: Vec<_> = remaining.iter().map(|pt| pt.ty.as_ref()).collect();

            out.extend(emit_fixed_schema_stmts(
                param_ident,
                &field_names,
                &zc_field_orig_types,
            ));
        }

        out.push(syn::parse_quote!(
            #param_ident.data = &[];
        ));
    }

    out.extend(emit_handler_tail(
        param_ident,
        stmts,
        has_return_data,
        return_ok_type,
    ));
    Ok(out)
}

pub(crate) fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    instruction_inner(attr.into(), item.into()).into()
}

pub(crate) fn instruction_inner(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let args = match syn::parse2::<InstructionArgs>(attr) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };
    let mut func = match syn::parse2::<ItemFn>(item) {
        Ok(func) => func,
        Err(e) => return e.to_compile_error(),
    };
    if let Some(disc_bytes) = &args.discriminator {
        let disc_len = disc_bytes.len();
        let disc_values = match parse_discriminator_bytes(disc_bytes) {
            Ok(values) => values,
            Err(e) => return e.to_compile_error(),
        };

        // Reject multi-byte all-zero discriminators: zeroed instruction data could
        // accidentally match. Single-byte discriminators are fine (the dispatch
        // macro's length check rejects empty instruction data).
        if disc_len > 1 && disc_values.iter().all(|&byte| byte == 0) {
            return syn::Error::new_spanned(
                &disc_bytes[0],
                "instruction discriminator must contain at least one non-zero byte; all-zero \
                 multi-byte discriminators are dangerous because zeroed instruction data would \
                 match",
            )
            .to_compile_error();
        }
    }

    if args.raw {
        let first_arg = match func.sig.inputs.first() {
            Some(FnArg::Typed(pt)) => pt.clone(),
            _ => {
                return syn::Error::new_spanned(
                    &func.sig.ident,
                    "#[instruction(raw)] requires a single parameter of type Context",
                )
                .to_compile_error();
            }
        };

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
            .to_compile_error();
        }

        if func.sig.inputs.len() > 1 {
            return syn::Error::new_spanned(
                &func.sig,
                "#[instruction(raw)] handler must have exactly one parameter: Context",
            )
            .to_compile_error();
        }

        return quote!(#func);
    }

    let first_arg = match func.sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt.clone(),
        _ => {
            return syn::Error::new_spanned(
                &func.sig.ident,
                "#[instruction] requires ctx: Ctx<T> as first parameter",
            )
            .to_compile_error();
        }
    };

    let param_ident = match &*first_arg.pat {
        Pat::Ident(pat_ident) => pat_ident.ident.clone(),
        _ => {
            return syn::Error::new_spanned(
                &first_arg.pat,
                "#[instruction] ctx parameter must be an identifier",
            )
            .to_compile_error();
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

    let fn_name = func.sig.ident.clone();

    // Single classifier, shared with `#[program]`: rejects a non-`Ctx` first
    // parameter identically and reports whether the direct entry applies.
    let (accounts_ty, has_remaining) = {
        let ctx_kind = match crate::ctx::CtxKind::classify(&func.sig) {
            Ok(k) => k,
            Err(e) => return e.to_compile_error(),
        };
        (ctx_kind.inner_ty().clone(), ctx_kind.has_remaining())
    };

    // Decode + user body + epilogue, emitted exactly ONCE into `__{name}_body`.
    // Both thin entries call it; `#[inline(always)]` folds it back into each so
    // the codegen (and CU) match the previous per-entry inline shape while the
    // front-end pays the decode-lowering cost a single time.
    let stmts = std::mem::take(&mut func.block.stmts);
    let decoded_tail = match emit_decode_and_tail(
        &param_ident,
        &remaining,
        &stmts,
        has_return_data,
        return_ok_type.as_ref(),
    ) {
        Ok(stmts) => stmts,
        Err(e) => return e.to_compile_error(),
    };
    let body_fn_name = format_ident!("__{}_body", fn_name);
    let body_fn = quote! {
        #[inline(always)]
        fn #body_fn_name(mut #param_ident: #param_type) -> Result<(), ProgramError> {
            #(#decoded_tail)*
        }
    };

    // Normal entry: a thin wrapper through the length-checked constructor.
    func.sig.inputs = syn::punctuated::Punctuated::new();
    func.sig
        .inputs
        .push(syn::parse_quote!(mut context: Context));
    // SAFETY: the generated dispatch parsed exactly `COUNT` validated account
    // views into this `Context` before invoking the handler.
    let entry_call: syn::Expr = syn::parse_quote!(
        #body_fn_name(unsafe { <#param_type>::new_unchecked(context) }?)
    );
    func.block.stmts = vec![syn::Stmt::Expr(entry_call, None)];

    // Direct entry: skipped when the handler carries remaining accounts (the
    // direct parser has no remaining-account path).
    let direct_fn = if has_remaining {
        quote! {}
    } else {
        let direct_name = format_ident!("__quasar_direct_{}", fn_name);
        quote! {
            #[inline(always)]
            fn #direct_name(
                __program_id: &[u8; 32],
                __accounts_start: *mut u8,
                __ix_data: &[u8],
            ) -> Result<(), ProgramError> {
                let __program_id_addr = unsafe {
                    &*(__program_id as *const [u8; 32] as *const quasar_lang::prelude::Address)
                };
                let (__accounts, __bumps) = unsafe {
                    <#accounts_ty>::parse_direct_with_instruction_data_unchecked(
                        __accounts_start,
                        __ix_data,
                        __program_id_addr,
                    )?
                };
                #body_fn_name(quasar_lang::context::Ctx {
                    accounts: __accounts,
                    bumps: __bumps,
                    program_id: __program_id,
                    data: __ix_data,
                })
            }
        }
    };

    quote!(
        #func
        #body_fn
        #direct_fn
    )
}
