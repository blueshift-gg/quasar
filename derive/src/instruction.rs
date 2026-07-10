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
        check_fixed_before_dynamic, classify_instruction_arg, extract_generic_inner_type,
        is_unit_type, validate_discriminator, wire_layout, ArgClass, ArgSite, DiscCtx,
        InstructionArgs, PodDynField, WireLayout,
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
    let krate = crate::krate::lang_path();
    let mut stmts: Vec<syn::Stmt> = Vec::new();
    stmts.push(syn::parse_quote!(
        #[repr(C)]
        struct __InstructionDataZc {
            #(#field_names: <#field_types as #krate::instruction_arg::InstructionArg>::Zc,)*
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
            return Err(#krate::__solana_program_error::ProgramError::InvalidInstructionData);
        }
    ));
    stmts.push(syn::parse_quote!(
        let __zc = unsafe { &*(#param_ident.data.as_ptr() as *const __InstructionDataZc) };
    ));
    for (name, ty) in field_names.iter().zip(field_types.iter()) {
        stmts.push(syn::parse_quote!(
            <#ty as #krate::instruction_arg::InstructionArg>::validate_zc(&__zc.#name)
                .map_err(|_| #krate::__solana_program_error::ProgramError::InvalidInstructionData)?;
        ));
        stmts.push(syn::parse_quote!(
            let #name = <#ty as #krate::instruction_arg::InstructionArg>::from_zc(&__zc.#name);
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
    let krate = crate::krate::lang_path();
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
                core::mem::align_of::<<#ok_ty as #krate::instruction_arg::InstructionArg>::Zc>() == 1,
                "return data type must implement InstructionArg with an alignment-1 Zc companion"
            );
        ));
        tail.push(syn::parse_quote!(
            {
                let __result: Result<#ok_ty, #krate::__solana_program_error::ProgramError> = (|| { #user_body })();
                match __result {
                    Ok(ref __val) => {
                        #epilogue_call
                        let __zc =
                            <#ok_ty as #krate::instruction_arg::InstructionArg>::to_zc(__val);
                        let __bytes = unsafe {
                            core::slice::from_raw_parts(
                                &__zc as *const <#ok_ty as #krate::instruction_arg::InstructionArg>::Zc as *const u8,
                                core::mem::size_of::<<#ok_ty as #krate::instruction_arg::InstructionArg>::Zc>(),
                            )
                        };
                        #krate::return_data::set_return_data(__bytes);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
        ));
    } else {
        tail.push(syn::parse_quote!({
            let __user_result: Result<(), #krate::__solana_program_error::ProgramError> = { #user_body };
            __user_result?;
            #epilogue_call
            Ok(())
        }));
    }

    tail
}

/// Map a classified instruction arg to its compact schema layout class. The
/// handler decode uses the raw `PodVec` element (its accessor yields
/// `&[elem]`).
fn compact_layout_class(cls: &ArgClass) -> crate::schema_ir::LayoutClass {
    use crate::schema_ir::LayoutClass;
    match cls {
        ArgClass::Fixed(ty) => LayoutClass::Fixed { ty: quote!(#ty) },
        ArgClass::PodDyn(pd) | ArgClass::Borrowed(pd) => {
            LayoutClass::from_pod_dyn(pd, |e| quote!(#e))
        }
        ArgClass::OptionPodDyn(pd) => LayoutClass::OptionalDyn {
            inner: Box::new(LayoutClass::from_pod_dyn(pd, |e| quote!(#e))),
        },
        ArgClass::BorrowedGroup(_) => {
            ice!("grouped borrowed struct handled before compact lowering")
        }
    }
}

fn emit_decode_and_tail(
    param_ident: &Ident,
    remaining: &[syn::PatType],
    stmts: &[syn::Stmt],
    has_return_data: bool,
    return_ok_type: Option<&Type>,
) -> syn::Result<Vec<syn::Stmt>> {
    let krate = crate::krate::lang_path();
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

        // Single shared classifier: fixed / compact-tail (pod-dyn, optional
        // pod-dyn, borrowed) / grouped-borrowed-struct.
        let mut arg_classes: Vec<ArgClass> = Vec::with_capacity(remaining.len());
        for pt in remaining {
            arg_classes.push(classify_instruction_arg(
                &pt.ty,
                &pt.attrs,
                ArgSite::Handler,
            )?);
        }

        // A grouped borrowed struct is decoded whole and must be the only arg.
        if let Some(pos) = arg_classes
            .iter()
            .position(|cls| matches!(cls, ArgClass::BorrowedGroup(_)))
        {
            if remaining.len() != 1 {
                return Err(syn::Error::new_spanned(
                    &remaining[pos].ty,
                    "a grouped borrowed struct must be the only instruction arg (besides ctx)",
                ));
            }

            // Generate: let arg_name = <ArgType>::decode_compact(&ctx.data)?;
            let ArgClass::BorrowedGroup(arg_ty) = &arg_classes[pos] else {
                ice!("position() matched BorrowedGroup")
            };
            let arg_name = &field_names[0];
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
        }

        let is_dynamic: Vec<bool> = arg_classes.iter().map(ArgClass::is_dynamic).collect();
        check_fixed_before_dynamic(
            remaining,
            &is_dynamic,
            "fixed instruction args must precede all dynamic or borrowed args",
        )?;

        for cls in &arg_classes {
            if let ArgClass::PodDyn(PodDynField::Vec { elem, .. })
            | ArgClass::OptionPodDyn(PodDynField::Vec { elem, .. })
            | ArgClass::Borrowed(PodDynField::Vec { elem, .. }) = cls
            {
                out.push(syn::parse_quote!(
                    const _: () = assert!(
                        core::mem::align_of::<#elem>() == 1,
                        "instruction Vec element type must have alignment 1"
                    );
                ));
            }
        }

        if let WireLayout::Compact = wire_layout(&arg_classes) {
            // Alias quasar_lang's re-export so `zeropod::*` paths emitted by
            // the ZeroPod derive resolve without a direct crate dependency.
            out.push(syn::parse_quote!(
                use #krate::__zeropod as zeropod;
            ));

            // Build the compact schema IR: inline fixed fields, then dynamic
            // tail fields (raw `PodVec` element, matching the `&[elem]` accessor
            // views this decode yields), then emit via the single-source
            // emitters.
            let schema_fields: Vec<crate::schema_ir::SchemaField> = arg_classes
                .iter()
                .zip(field_names.iter())
                .map(|(cls, name)| {
                    crate::schema_ir::SchemaField::private(name.clone(), compact_layout_class(cls))
                })
                .collect();
            let ir = crate::schema_ir::SchemaIR::new(schema_fields)?;
            let schema_name: Ident = syn::parse_quote!(__InstructionDataCompact);
            let ref_name: Ident = syn::parse_quote!(__InstructionDataCompactRef);

            let schema_struct = crate::schema_ir::emit_compact_schema(
                &schema_name,
                &ir,
                &syn::Visibility::Inherited,
            );
            out.push(syn::parse_quote!(#schema_struct));
            out.extend(crate::schema_ir::emit_compact_decode(
                &ir,
                &crate::schema_ir::DecodeOpts {
                    schema_name,
                    ref_name,
                    data: quote!(&#param_ident.data),
                    err: quote!(#krate::__solana_program_error::ProgramError::InvalidInstructionData),
                    validate_fixed: true,
                },
            ));
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
    let krate = crate::krate::lang_path();
    let args = match syn::parse2::<InstructionArgs>(attr) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };
    let mut func = match syn::parse2::<ItemFn>(item) {
        Ok(func) => func,
        Err(e) => return e.to_compile_error(),
    };
    if let Some(disc_bytes) = &args.discriminator {
        // Single-source all-zero policy (single-byte 0x00 is safe; multi-byte
        // all-zero is rejected — see `validate_discriminator`).
        if let Err(e) = validate_discriminator(disc_bytes, DiscCtx::Instruction) {
            return e.to_compile_error();
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
        func.sig.output = syn::parse_quote!(-> Result<(), #krate::__solana_program_error::ProgramError>);
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
        fn #body_fn_name(mut #param_ident: #param_type) -> Result<(), #krate::__solana_program_error::ProgramError> {
            #(#decoded_tail)*
        }
    };

    // Normal entry: a thin wrapper through the length-checked constructor.
    func.sig.inputs = syn::punctuated::Punctuated::new();
    func.sig
        .inputs
        .push(syn::parse_quote!(mut context: #krate::context::Context));
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
            ) -> Result<(), #krate::__solana_program_error::ProgramError> {
                let __program_id_addr = unsafe {
                    &*(__program_id as *const [u8; 32] as *const #krate::prelude::Address)
                };
                let (__accounts, __bumps) = unsafe {
                    <#accounts_ty>::parse_direct_with_instruction_data_unchecked(
                        __accounts_start,
                        __ix_data,
                        __program_id_addr,
                    )?
                };
                #body_fn_name(#krate::context::Ctx {
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
