//! Compact-schema IR: the single source of `#[zeropod(compact)]` emission.
//!
//! Four codegen paths lower a list of fields into a compact zeropod schema —
//! the `#[instruction]` handler decode, the struct-level `#[instruction(...)]`
//! extractor (`accounts/emit/ix_args.rs`), the borrowed `QuasarSerialize`
//! struct, and the dynamic `#[account]`. They previously each hand-wrote the
//! `#[derive(zeropod::ZeroPod)] #[zeropod(compact)] struct { .. }` boilerplate.
//! Here it is emitted once from a shared [`SchemaIR`].
//!
//! The IR is pure `syn`/token data (no behavior). The per-field *element*
//! spelling for `PodVec` is supplied by the caller (see the divergence note on
//! [`LayoutClass::Vec`]): each caller's schema field type must match the
//! element type its own accessor yields, so the element token is caller data
//! rather than recomputed here.

use {
    crate::helpers::{map_to_pod_type, PodDynField},
    proc_macro2::TokenStream,
    quote::quote,
    syn::{Expr, Ident, Type, Visibility},
};

/// The wire class of one compact-schema field.
pub(crate) enum LayoutClass {
    /// Inline fixed field: the native type is emitted verbatim into the schema.
    Fixed { ty: TokenStream },
    /// `PodString<max, prefix>` tail field.
    Str { max: Expr, prefix_bytes: usize },
    /// `PodVec<elem, max, prefix>` tail field.
    ///
    /// `elem` is the already-resolved element token supplied by the caller. The
    /// dynamic-`#[account]` path passes `map_to_pod_type(elem)` (the `ZcField`
    /// companion, matching its read accessors); the `#[instruction]` / borrowed
    /// paths pass the raw element (matching their `&[elem]` accessor). These
    /// spellings are equivalent for the align-1 element types that reach here,
    /// but are kept caller-supplied so the schema and accessor never disagree.
    Vec {
        elem: TokenStream,
        max: Expr,
        prefix_bytes: usize,
    },
    /// `Option<inner>` tail field (instruction args only).
    OptionalDyn { inner: Box<LayoutClass> },
}

impl LayoutClass {
    /// Build a `LayoutClass` from a [`PodDynField`] with a caller-chosen `Vec`
    /// element spelling.
    pub(crate) fn from_pod_dyn(pd: &PodDynField, vec_elem: impl Fn(&Type) -> TokenStream) -> Self {
        match pd {
            PodDynField::Str { max, prefix_bytes } => LayoutClass::Str {
                max: max.clone(),
                prefix_bytes: *prefix_bytes,
            },
            PodDynField::Vec {
                elem,
                max,
                prefix_bytes,
            } => LayoutClass::Vec {
                elem: vec_elem(elem),
                max: max.clone(),
                prefix_bytes: *prefix_bytes,
            },
        }
    }

    fn is_dynamic(&self) -> bool {
        !matches!(self, LayoutClass::Fixed { .. })
    }

    /// The compact-schema field type tokens (`zeropod::pod::PodString<..>`,
    /// etc.). All callers alias `quasar_lang::__zeropod` as `zeropod` in scope.
    fn field_type(&self) -> TokenStream {
        match self {
            LayoutClass::Fixed { ty } => ty.clone(),
            LayoutClass::Str { max, prefix_bytes } => {
                quote! { zeropod::pod::PodString<#max, #prefix_bytes> }
            }
            LayoutClass::Vec {
                elem,
                max,
                prefix_bytes,
            } => quote! { zeropod::pod::PodVec<#elem, #max, #prefix_bytes> },
            LayoutClass::OptionalDyn { inner } => {
                let inner_ty = inner.field_type();
                quote! { Option<#inner_ty> }
            }
        }
    }
}

/// One field of a compact schema: name, visibility, passthrough attributes
/// (`#[zeropod(...)]` on account fields), and wire class.
pub(crate) struct SchemaField {
    pub ident: Ident,
    pub vis: Visibility,
    pub attrs: Vec<syn::Attribute>,
    pub class: LayoutClass,
}

impl SchemaField {
    /// A private field with no passthrough attributes (instruction /
    /// serialize).
    pub(crate) fn private(ident: Ident, class: LayoutClass) -> Self {
        Self {
            ident,
            vis: Visibility::Inherited,
            attrs: Vec::new(),
            class,
        }
    }
}

/// A validated compact schema: fixed fields precede all dynamic ones.
pub(crate) struct SchemaIR {
    fields: Vec<SchemaField>,
}

impl SchemaIR {
    /// The sole constructor: enforces fixed-before-dynamic ordering. The error
    /// is spanned at the offending field ident with a generic message; callers
    /// that need a domain-specific ordering diagnostic validate first (E1's
    /// `check_fixed_before_dynamic`) and reach here already ordered.
    pub(crate) fn new(fields: Vec<SchemaField>) -> syn::Result<Self> {
        let is_dynamic: Vec<bool> = fields.iter().map(|f| f.class.is_dynamic()).collect();
        let first_dynamic = is_dynamic.iter().position(|&d| d);
        let last_fixed = is_dynamic.iter().rposition(|&d| !d);
        if let (Some(fd), Some(lf)) = (first_dynamic, last_fixed) {
            if lf > fd {
                return Err(syn::Error::new_spanned(
                    &fields[lf].ident,
                    "fixed fields must precede all dynamic (String/Vec) fields",
                ));
            }
        }
        Ok(Self { fields })
    }

    pub(crate) fn fields(&self) -> &[SchemaField] {
        &self.fields
    }
}

/// Emit the shared `#[derive(zeropod::ZeroPod)] #[zeropod(compact)] struct` for
/// a compact schema. `struct_vis` is the struct visibility (`pub` for accounts,
/// inherited for instruction/serialize). This is the ONLY place the
/// `#[zeropod(compact)]` attribute is constructed.
pub(crate) fn emit_compact_schema(
    name: &Ident,
    ir: &SchemaIR,
    struct_vis: &Visibility,
) -> TokenStream {
    let field_defs = ir.fields().iter().map(|f| {
        let vis = &f.vis;
        let ident = &f.ident;
        let attrs = &f.attrs;
        let ty = f.class.field_type();
        quote! { #(#attrs)* #vis #ident: #ty }
    });
    quote! {
        #[derive(zeropod::ZeroPod)]
        #[zeropod(compact)]
        #struct_vis struct #name {
            #(#field_defs,)*
        }
    }
}

/// How a compact decode should treat fixed fields and which error to raise.
pub(crate) struct DecodeOpts {
    /// The schema struct ident (for the `ZeroPodCompact::validate` call).
    pub schema_name: Ident,
    /// The generated `Ref` type ident.
    pub ref_name: Ident,
    /// The slice expression to validate/decode (`__ix_data`, `&ctx.data`, ...).
    pub data: TokenStream,
    /// The error expression for `validate().map_err(|_| <err>)`.
    pub err: TokenStream,
    /// Whether to `validate_zc` each fixed field before `from_zc` (the handler
    /// decode does; the borrowed-serialize decode does not).
    pub validate_fixed: bool,
}

/// Emit the shared compact decode statements: `validate` + `Ref::new_unchecked`
/// + per-field bindings, in schema order. Each dynamic field binds a zero-copy
/// accessor view; each fixed field decodes via `InstructionArg` (validating
/// first when `validate_fixed`). Returned as statements so callers can splice
/// the bindings directly into the enclosing scope.
pub(crate) fn emit_compact_decode(ir: &SchemaIR, opts: &DecodeOpts) -> Vec<syn::Stmt> {
    let krate = crate::krate::lang_path();
    let DecodeOpts {
        schema_name,
        ref_name,
        data,
        err,
        validate_fixed,
    } = opts;

    let mut stmts: Vec<syn::Stmt> = Vec::new();
    stmts.push(syn::parse_quote! {
        <#schema_name as #krate::ZeroPodCompact>::validate(#data)
            .map_err(|_| #err)?;
    });
    stmts.push(syn::parse_quote! {
        // SAFETY: `validate` succeeded on this exact slice above.
        let __ref = unsafe { #ref_name::new_unchecked(#data) };
    });

    for f in ir.fields() {
        let name = &f.ident;
        if f.class.is_dynamic() {
            stmts.push(syn::parse_quote! { let #name = __ref.#name(); });
        } else {
            let LayoutClass::Fixed { ty } = &f.class else {
                ice!("non-dynamic LayoutClass must be Fixed")
            };
            if *validate_fixed {
                stmts.push(syn::parse_quote! {
                    <#ty as #krate::instruction_arg::InstructionArg>::validate_zc(&__ref.#name)
                        .map_err(|_| #err)?;
                });
            }
            stmts.push(syn::parse_quote! {
                let #name = <#ty as #krate::instruction_arg::InstructionArg>::from_zc(&__ref.#name);
            });
        }
    }

    stmts
}

// ---------------------------------------------------------------------------
// Size model
// ---------------------------------------------------------------------------

/// The tail-region byte size contribution of one dynamic field, given a
/// length expression (`name.len()`, `self.name.len()`, or the `max` bound).
/// Collapses the three previously hand-written tail-size terms in
/// `account/dynamic.rs`.
pub(crate) fn field_size_expr(pd: &PodDynField, len: TokenStream) -> TokenStream {
    match pd {
        PodDynField::Str { .. } => quote! { + #len },
        PodDynField::Vec { elem, .. } => {
            let mapped = map_to_pod_type(elem);
            quote! { + #len * core::mem::size_of::<#mapped>() }
        }
    }
}

/// The on-chain byte size of one `#[event]` field. Preserves the rejection of
/// unsupported field types (only primitive integers, `bool`, and `Address`).
pub(crate) fn event_field_size(ty: &Type) -> syn::Result<usize> {
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
