//! Op struct construction from group directive args.
//!
//! The derive emits UFCS calls like:
//! ```text
//! <token::Op<'_> as AccountOp<Account<Token>>>::after_load(
//!     &token::Op { mint: ..., authority: ..., token_program: ... },
//!     &field, &__ctx,
//! )?;
//! ```
//!
//! This module generates the `token::Op { ... }` struct literal.
//!
//! ## `target` arg
//!
//! When a group has `target = Type`, the value is a type parameter, not a
//! field value. `emit_op_type` puts it in the turbofish, `emit_op_struct`
//! excludes it from fields and adds `_target: PhantomData`.
//!
//! ## Field name awareness
//!
//! The arg transforms (`typed_arg`, `exit_arg`) only apply `.to_account_view()`
//! to idents that are account fields. Non-field idents (constants, scalars)
//! pass through unchanged. Field names are threaded via `OpEmitCtx`.

use {
    super::super::resolve::{GroupArg, GroupDirective},
    quote::quote,
};

/// Context for op emission — carries field names for transform disambiguation.
pub(crate) struct OpEmitCtx {
    pub field_names: Vec<String>,
}

/// Return the fully-qualified op type for a group, including generic params.
///
/// Without `target`: `#path::Op<'_>`
/// With `target = ConfigV2`: `#path::Op<'_, ConfigV2>`
pub(crate) fn emit_op_type(group: &GroupDirective) -> proc_macro2::TokenStream {
    let path = &group.path;
    let target = group.args.iter().find(|a| a.key == "target");
    match target {
        Some(arg) => {
            let ty = &arg.value;
            quote! { #path::Op<'_, #ty> }
        }
        None => quote! { #path::Op<'_> },
    }
}

/// Like `emit_op_type` but uses `'static` for const-assert contexts.
pub(crate) fn emit_op_type_static(group: &GroupDirective) -> proc_macro2::TokenStream {
    let path = &group.path;
    let target = group.args.iter().find(|a| a.key == "target");
    match target {
        Some(arg) => {
            let ty = &arg.value;
            quote! { #path::Op<'static, #ty> }
        }
        None => quote! { #path::Op<'static> },
    }
}

/// Emit an op struct literal from a group directive's args.
///
/// `arg_transform` converts each arg value to the correct expression
/// for the phase (raw slot refs for Phase 1, typed refs for Phase 3).
///
/// The `target` arg is excluded from struct fields (it's a type param).
/// When present, a `_target: core::marker::PhantomData` field is added.
pub(crate) fn emit_op_struct(
    group: &GroupDirective,
    arg_transform: impl Fn(&GroupArg, &OpEmitCtx) -> proc_macro2::TokenStream,
    ctx: &OpEmitCtx,
) -> proc_macro2::TokenStream {
    let path = &group.path;
    let has_target = group.args.iter().any(|a| a.key == "target");
    let target_arg = group.args.iter().find(|a| a.key == "target");

    let fields: Vec<proc_macro2::TokenStream> = group
        .args
        .iter()
        .filter(|arg| arg.key != "target")
        .map(|arg| {
            let key = &arg.key;
            let value = arg_transform(arg, ctx);
            quote! { #key: #value }
        })
        .collect();

    if has_target {
        let ty = &target_arg.unwrap().value;
        quote! {
            #path::Op::<'_, #ty> {
                #(#fields,)*
                _target: core::marker::PhantomData,
            }
        }
    } else {
        quote! {
            #path::Op {
                #(#fields,)*
            }
        }
    }
}

fn is_field_ident(expr: &syn::Expr, ctx: &OpEmitCtx) -> bool {
    if let syn::Expr::Path(ep) = expr {
        if ep.qself.is_none() && ep.path.segments.len() == 1 {
            let name = ep.path.segments[0].ident.to_string();
            return ctx.field_names.iter().any(|f| f == &name);
        }
    }
    false
}

/// Transform arg value for Phase 1 (pre-load): field idents reference raw
/// `&mut AccountView` slots, `Some(ident)` wraps in Some, everything else
/// passes through.
pub(crate) fn raw_slot_arg(arg: &GroupArg, _ctx: &OpEmitCtx) -> proc_macro2::TokenStream {
    transform_raw_expr(&arg.value)
}

fn transform_raw_expr(expr: &syn::Expr) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Path(ep)
            if ep.qself.is_none()
                && ep.path.segments.len() == 1
                && ep.path.segments[0].ident == "None" =>
        {
            quote! { None }
        }
        syn::Expr::Call(call)
            if matches!(&*call.func, syn::Expr::Path(p)
                if p.path.segments.len() == 1 && p.path.segments[0].ident == "Some")
                && call.args.len() == 1 =>
        {
            let inner = transform_raw_expr(&call.args[0]);
            quote! { Some(#inner) }
        }
        _ => quote! { #expr },
    }
}

/// Transform arg value for Phase 3 (post-load): field idents get
/// `.to_account_view()`, `Some(field)` becomes `Some(field.to_account_view())`,
/// non-field idents, `None`, and literals pass through.
pub(crate) fn typed_arg(arg: &GroupArg, ctx: &OpEmitCtx) -> proc_macro2::TokenStream {
    transform_typed_expr(&arg.value, ctx)
}

fn transform_typed_expr(expr: &syn::Expr, ctx: &OpEmitCtx) -> proc_macro2::TokenStream {
    match expr {
        // None → pass through
        syn::Expr::Path(ep)
            if ep.qself.is_none()
                && ep.path.segments.len() == 1
                && ep.path.segments[0].ident == "None" =>
        {
            quote! { None }
        }
        // Field ident → typed ref via to_account_view()
        _ if is_field_ident(expr, ctx) => {
            quote! { #expr.to_account_view() }
        }
        // Some(inner) → transform inner recursively, wrap in Some()
        syn::Expr::Call(call)
            if matches!(&*call.func, syn::Expr::Path(p)
                if p.path.segments.len() == 1 && p.path.segments[0].ident == "Some")
                && call.args.len() == 1 =>
        {
            let inner = transform_typed_expr(&call.args[0], ctx);
            quote! { Some(#inner) }
        }
        // Everything else (literals, consts, multi-segment paths) → pass through
        _ => {
            quote! { #expr }
        }
    }
}

/// Transform arg value for Phase 4 (exit): field idents get
/// `self.field.to_account_view()`, `Some(field)` becomes
/// `Some(self.field.to_account_view())`, non-field values pass through.
pub(crate) fn exit_arg(arg: &GroupArg, ctx: &OpEmitCtx) -> proc_macro2::TokenStream {
    transform_exit_expr(&arg.value, ctx)
}

fn transform_exit_expr(expr: &syn::Expr, ctx: &OpEmitCtx) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Path(ep)
            if ep.qself.is_none()
                && ep.path.segments.len() == 1
                && ep.path.segments[0].ident == "None" =>
        {
            quote! { None }
        }
        // Field ident → self.field.to_account_view()
        _ if is_field_ident(expr, ctx) => {
            if let syn::Expr::Path(ep) = expr {
                let ident = &ep.path.segments[0].ident;
                quote! { self.#ident.to_account_view() }
            } else {
                unreachable!()
            }
        }
        syn::Expr::Call(call)
            if matches!(&*call.func, syn::Expr::Path(p)
                if p.path.segments.len() == 1 && p.path.segments[0].ident == "Some")
                && call.args.len() == 1 =>
        {
            let inner = transform_exit_expr(&call.args[0], ctx);
            quote! { Some(#inner) }
        }
        // Everything else → pass through
        _ => {
            quote! { #expr }
        }
    }
}
