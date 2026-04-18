use {
    super::super::resolve::{BumpSyntax, FieldSemantics, FieldShape, PdaSource, SeedNode},
    quote::quote,
    syn::{Expr, Ident, Type},
};

#[derive(Clone, Copy)]
pub(crate) enum AccountWrapperKind {
    Account,
    InterfaceAccount,
}

#[derive(Clone)]
pub(crate) enum SeedEmitNode {
    Literal(Vec<u8>),
    TypePrefix(syn::Path),
    AccountAddress(syn::Expr),
    FieldBytes {
        expr: syn::Expr,
        root_ident: syn::Ident,
        inner_ty: Option<syn::Type>,
        wrapper_kind: Option<AccountWrapperKind>,
    },
    FieldRootedExpr {
        expr: syn::Expr,
        root_ident: syn::Ident,
        inner_ty: Option<syn::Type>,
        wrapper_kind: Option<AccountWrapperKind>,
    },
    InstructionArg {
        expr: syn::Expr,
        ty: syn::Type,
    },
    OpaqueExpr(syn::Expr),
}

#[derive(Clone, Copy)]
pub(crate) enum SeedRenderContext {
    Parse,
    Init,
    Method,
}

pub(crate) fn classify_seed(
    expr: Expr,
    field_names: &[String],
    field_types: &[(Ident, Type)],
    instruction_args: &Option<Vec<crate::accounts::InstructionArg>>,
) -> SeedNode {
    if let Expr::Lit(ref lit) = expr {
        if let syn::Lit::ByteStr(ref bs) = lit.lit {
            return SeedNode::Literal(bs.value());
        }
    }

    if let Expr::Path(ref ep) = expr {
        if ep.path.segments.len() == 1 && ep.qself.is_none() {
            let ident = &ep.path.segments[0].ident;
            let name = ident.to_string();

            if field_names.contains(&name) {
                return SeedNode::AccountAddress {
                    field: ident.clone(),
                };
            }

            if let Some(args) = instruction_args {
                if let Some(arg) = args.iter().find(|a| a.name == *ident) {
                    return SeedNode::InstructionArg {
                        name: arg.name.clone(),
                        ty: arg.ty.clone(),
                    };
                }
            }
        }
    }

    if let Some((root, path)) = extract_field_path(&expr) {
        let root_ty = lookup_field_effective_ty(&root, field_types);
        return SeedNode::FieldBytes {
            root,
            path,
            root_ty,
        };
    }

    if let Some(root) = extract_expr_root(&expr) {
        if field_names.contains(&root.to_string()) {
            let root_ty = lookup_field_effective_ty(&root, field_types);
            return SeedNode::FieldRootedExpr {
                root,
                expr,
                root_ty,
            };
        }
    }

    SeedNode::OpaqueExpr(expr)
}

pub(crate) fn lower_bump(bump: &Option<Option<Expr>>) -> Option<BumpSyntax> {
    match bump {
        None => None,
        Some(None) => Some(BumpSyntax::Bare),
        Some(Some(expr)) => Some(BumpSyntax::Explicit(expr.clone())),
    }
}

pub(crate) fn render_seed_expr(
    node: &SeedEmitNode,
    ctx: SeedRenderContext,
) -> proc_macro2::TokenStream {
    match node {
        SeedEmitNode::Literal(bytes) => {
            render_literal_seed(bytes, !matches!(ctx, SeedRenderContext::Parse))
        }
        SeedEmitNode::TypePrefix(type_path) => render_type_prefix(type_path),
        SeedEmitNode::AccountAddress(expr) => render_account_address(expr, ctx),
        SeedEmitNode::FieldBytes {
            expr,
            root_ident,
            inner_ty,
            wrapper_kind,
        } => render_field_bytes(
            expr,
            root_ident,
            inner_ty.as_ref(),
            wrapper_kind.as_ref(),
            ctx,
        ),
        SeedEmitNode::FieldRootedExpr {
            expr,
            root_ident,
            inner_ty,
            wrapper_kind,
        } => render_field_rooted_expr(
            expr,
            root_ident,
            inner_ty.as_ref(),
            wrapper_kind.as_ref(),
            ctx,
        ),
        SeedEmitNode::InstructionArg { expr, ty } => emit_instruction_arg_seed_bytes(expr, ty),
        SeedEmitNode::OpaqueExpr(expr) => render_expr_as_bytes(expr, ctx),
    }
}

pub(crate) fn seeds_to_emit_nodes(
    source: &PdaSource,
    all_semantics: &[FieldSemantics],
) -> Vec<SeedEmitNode> {
    let mut emit_nodes = Vec::new();
    let seed_nodes = match source {
        PdaSource::Raw { seeds } => seeds,
        PdaSource::Typed { type_path, args } => {
            emit_nodes.push(SeedEmitNode::TypePrefix(type_path.clone()));
            args
        }
    };

    emit_nodes.extend(
        seed_nodes
            .iter()
            .map(|node| lower_seed_emit_node(node, all_semantics)),
    );

    emit_nodes
}

pub(crate) fn emit_instruction_arg_seed_bytes(
    expr: &syn::Expr,
    ty: &syn::Type,
) -> proc_macro2::TokenStream {
    let is_address_like = matches!(ty, syn::Type::Path(tp)
        if tp.path.segments.last().is_some_and(|s| s.ident == "Address" || s.ident == "Pubkey")
    );

    if is_address_like {
        quote! { #expr.as_ref() }
    } else {
        quote! { &#expr.to_le_bytes() }
    }
}

fn render_type_prefix(type_path: &syn::Path) -> proc_macro2::TokenStream {
    quote! { <#type_path as quasar_lang::traits::HasSeeds>::SEED_PREFIX }
}

fn render_account_address(expr: &syn::Expr, ctx: SeedRenderContext) -> proc_macro2::TokenStream {
    match ctx {
        SeedRenderContext::Parse => quote! { (#expr).to_account_view().address().as_ref() },
        SeedRenderContext::Init => quote! { (#expr).address().as_ref() },
        SeedRenderContext::Method => quote! { (self.#expr).to_account_view().address().as_ref() },
    }
}

fn render_field_bytes(
    expr: &syn::Expr,
    root_ident: &syn::Ident,
    inner_ty: Option<&syn::Type>,
    wrapper_kind: Option<&AccountWrapperKind>,
    ctx: SeedRenderContext,
) -> proc_macro2::TokenStream {
    match ctx {
        SeedRenderContext::Parse => render_raw_bytes(quote! { #expr }),
        SeedRenderContext::Method => render_raw_bytes(quote! { self.#expr }),
        SeedRenderContext::Init => {
            if let Some(ty) = inner_ty {
                let base_ty = crate::helpers::strip_generics(ty);
                let path_after_root = extract_path_after_root(expr);
                let typed_cast = emit_typed_cast(wrapper_kind, &base_ty, root_ident);
                quote! { unsafe {
                    let __typed = #typed_cast;
                    core::slice::from_raw_parts(
                        &(*__typed)#path_after_root as *const _ as *const u8,
                        core::mem::size_of_val(&(*__typed)#path_after_root),
                    )
                } }
            } else {
                render_raw_bytes(quote! { #expr })
            }
        }
    }
}

fn lower_seed_emit_node(node: &SeedNode, all_semantics: &[FieldSemantics]) -> SeedEmitNode {
    match node {
        SeedNode::Literal(bytes) => SeedEmitNode::Literal(bytes.clone()),
        SeedNode::AccountAddress { field } => {
            let expr: syn::Expr = syn::parse_quote!(#field);
            SeedEmitNode::AccountAddress(expr)
        }
        SeedNode::FieldBytes {
            root,
            path,
            root_ty,
        } => {
            let expr = path_expr(root, path);
            let (inner_ty, wrapper_kind) = resolve_field_type_info(root, root_ty, all_semantics);
            SeedEmitNode::FieldBytes {
                expr,
                root_ident: root.clone(),
                inner_ty,
                wrapper_kind,
            }
        }
        SeedNode::FieldRootedExpr {
            root,
            expr,
            root_ty,
        } => {
            let (inner_ty, wrapper_kind) = resolve_field_type_info(root, root_ty, all_semantics);
            SeedEmitNode::FieldRootedExpr {
                expr: expr.clone(),
                root_ident: root.clone(),
                inner_ty,
                wrapper_kind,
            }
        }
        SeedNode::InstructionArg { name, ty } => {
            let expr: syn::Expr = syn::parse_quote!(#name);
            SeedEmitNode::InstructionArg {
                expr,
                ty: ty.clone(),
            }
        }
        SeedNode::OpaqueExpr(expr) => SeedEmitNode::OpaqueExpr(expr.clone()),
    }
}

fn render_field_rooted_expr(
    expr: &syn::Expr,
    root_ident: &syn::Ident,
    inner_ty: Option<&syn::Type>,
    wrapper_kind: Option<&AccountWrapperKind>,
    ctx: SeedRenderContext,
) -> proc_macro2::TokenStream {
    match (ctx, inner_ty) {
        (SeedRenderContext::Init, Some(ty)) => {
            let base_ty = crate::helpers::strip_generics(ty);
            let rewritten = rewrite_init_expr_root(expr, root_ident, wrapper_kind, &base_ty);
            quote! { (#rewritten) as &[u8] }
        }
        (SeedRenderContext::Method, Some(_)) => {
            let rewritten = rewrite_method_expr_root(expr, root_ident);
            quote! { (#rewritten) as &[u8] }
        }
        _ => render_expr_as_bytes(expr, ctx),
    }
}

fn path_expr(root: &Ident, path: &[Ident]) -> syn::Expr {
    if path.is_empty() {
        syn::parse_quote!(#root)
    } else {
        let mut tokens: syn::Expr = syn::parse_quote!(#root);
        for p in path {
            tokens = syn::parse_quote!(#tokens.#p);
        }
        tokens
    }
}

fn render_literal_seed(bytes: &[u8], cast_slice: bool) -> proc_macro2::TokenStream {
    let byte_tokens: Vec<proc_macro2::TokenStream> = bytes.iter().map(|b| quote! { #b }).collect();
    if cast_slice {
        quote! { &[#(#byte_tokens),*] as &[u8] }
    } else {
        quote! { &[#(#byte_tokens),*] }
    }
}

fn render_expr_as_bytes(expr: &syn::Expr, ctx: SeedRenderContext) -> proc_macro2::TokenStream {
    match ctx {
        SeedRenderContext::Method => {
            if matches!(expr, syn::Expr::Path(_)) {
                quote! { quasar_lang::pda::seed_bytes(&(#expr)) }
            } else {
                quote! { quasar_lang::pda::seed_bytes(&(self.#expr)) }
            }
        }
        SeedRenderContext::Parse | SeedRenderContext::Init => {
            quote! { quasar_lang::pda::seed_bytes(&(#expr)) }
        }
    }
}

fn render_raw_bytes(expr: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! { unsafe {
        core::slice::from_raw_parts(
            &(#expr) as *const _ as *const u8,
            core::mem::size_of_val(&(#expr)),
        )
    } }
}

fn resolve_field_type_info(
    root: &Ident,
    root_ty: &Option<syn::Type>,
    all_semantics: &[FieldSemantics],
) -> (Option<syn::Type>, Option<AccountWrapperKind>) {
    if let Some(sem) = all_semantics.iter().find(|s| s.core.ident == *root) {
        match &sem.core.shape {
            FieldShape::Account { inner_ty } => {
                return (Some(inner_ty.clone()), Some(AccountWrapperKind::Account));
            }
            FieldShape::InterfaceAccount { inner_ty } => {
                return (
                    Some(inner_ty.clone()),
                    Some(AccountWrapperKind::InterfaceAccount),
                );
            }
            _ => {}
        }
    }
    (root_ty.clone(), None)
}

fn lookup_field_effective_ty(root: &Ident, field_types: &[(Ident, Type)]) -> Option<Type> {
    field_types
        .iter()
        .find(|(name, _)| name == root)
        .map(|(_, ty)| ty.clone())
}

fn extract_expr_root(expr: &Expr) -> Option<Ident> {
    match expr {
        Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
            Some(ep.path.segments[0].ident.clone())
        }
        Expr::Field(f) => extract_expr_root(&f.base),
        Expr::Paren(p) => extract_expr_root(&p.expr),
        Expr::MethodCall(m) => extract_expr_root(&m.receiver),
        Expr::Reference(r) => extract_expr_root(&r.expr),
        _ => None,
    }
}

fn extract_field_path(expr: &Expr) -> Option<(Ident, Vec<Ident>)> {
    let mut current = expr;
    let mut segments = Vec::new();

    loop {
        match current {
            Expr::Field(field_expr) => {
                if let syn::Member::Named(ident) = &field_expr.member {
                    segments.push(ident.clone());
                } else {
                    return None;
                }
                current = &field_expr.base;
            }
            Expr::Paren(paren_expr) => current = &paren_expr.expr,
            Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
                let root = ep.path.segments[0].ident.clone();
                segments.reverse();
                return (!segments.is_empty()).then_some((root, segments));
            }
            _ => return None,
        }
    }
}

fn extract_path_after_root(expr: &syn::Expr) -> proc_macro2::TokenStream {
    fn collect_path(expr: &syn::Expr, parts: &mut Vec<syn::Ident>) -> bool {
        match expr {
            syn::Expr::Field(field) => {
                if !collect_path(&field.base, parts) {
                    return false;
                }
                if let syn::Member::Named(ident) = &field.member {
                    parts.push(ident.clone());
                }
                true
            }
            syn::Expr::Path(_) => true,
            _ => false,
        }
    }

    let mut parts = Vec::new();
    collect_path(expr, &mut parts);
    let path = parts.iter().map(|p| quote! { .#p });
    quote! { #(#path)* }
}

fn emit_typed_cast(
    wrapper_kind: Option<&AccountWrapperKind>,
    base_ty: &proc_macro2::TokenStream,
    root_ident: &syn::Ident,
) -> proc_macro2::TokenStream {
    match wrapper_kind {
        Some(AccountWrapperKind::InterfaceAccount) => {
            quote! {
                quasar_lang::accounts::interface_account::InterfaceAccount::<#base_ty>::from_account_view_unchecked(#root_ident)
            }
        }
        _ => {
            quote! {
                quasar_lang::accounts::account::Account::<#base_ty>::from_account_view_unchecked(#root_ident)
            }
        }
    }
}

fn rewrite_init_expr_root(
    expr: &syn::Expr,
    root_ident: &syn::Ident,
    wrapper_kind: Option<&AccountWrapperKind>,
    base_ty: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
            let ident = &ep.path.segments[0].ident;
            if ident == root_ident {
                let typed_cast = emit_typed_cast(wrapper_kind, base_ty, root_ident);
                quote! { unsafe { #typed_cast } }
            } else {
                quote! { #expr }
            }
        }
        syn::Expr::Field(field_expr) => {
            let base = rewrite_init_expr_root(&field_expr.base, root_ident, wrapper_kind, base_ty);
            let member = &field_expr.member;
            quote! { (#base).#member }
        }
        syn::Expr::Paren(paren_expr) => {
            let inner = rewrite_init_expr_root(&paren_expr.expr, root_ident, wrapper_kind, base_ty);
            quote! { (#inner) }
        }
        syn::Expr::MethodCall(method_call) => {
            let receiver =
                rewrite_init_expr_root(&method_call.receiver, root_ident, wrapper_kind, base_ty);
            let method = &method_call.method;
            let turbofish = &method_call.turbofish;
            let args: Vec<_> = method_call.args.iter().collect();
            quote! { (#receiver).#method #turbofish ( #(#args),* ) }
        }
        syn::Expr::Reference(reference_expr) => {
            let inner =
                rewrite_init_expr_root(&reference_expr.expr, root_ident, wrapper_kind, base_ty);
            let mutability = &reference_expr.mutability;
            quote! { &#mutability (#inner) }
        }
        _ => quote! { #expr },
    }
}

fn rewrite_method_expr_root(expr: &syn::Expr, root_ident: &syn::Ident) -> proc_macro2::TokenStream {
    match expr {
        syn::Expr::Path(ep) if ep.path.segments.len() == 1 && ep.qself.is_none() => {
            let ident = &ep.path.segments[0].ident;
            if ident == root_ident {
                quote! { self.#root_ident }
            } else {
                quote! { #expr }
            }
        }
        syn::Expr::Field(field_expr) => {
            let base = rewrite_method_expr_root(&field_expr.base, root_ident);
            let member = &field_expr.member;
            quote! { (#base).#member }
        }
        syn::Expr::Paren(paren_expr) => {
            let inner = rewrite_method_expr_root(&paren_expr.expr, root_ident);
            quote! { (#inner) }
        }
        syn::Expr::MethodCall(method_call) => {
            let receiver = rewrite_method_expr_root(&method_call.receiver, root_ident);
            let method = &method_call.method;
            let turbofish = &method_call.turbofish;
            let args: Vec<_> = method_call.args.iter().collect();
            quote! { (#receiver).#method #turbofish ( #(#args),* ) }
        }
        syn::Expr::Reference(reference_expr) => {
            let inner = rewrite_method_expr_root(&reference_expr.expr, root_ident);
            let mutability = &reference_expr.mutability;
            quote! { &#mutability (#inner) }
        }
        _ => quote! { #expr },
    }
}
