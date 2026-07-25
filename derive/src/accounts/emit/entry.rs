//! Header plan: account-count expressions, SVM header parsing, and direct
//! dispatch setup for generated account structs.

use {
    super::{super::resolve, EmitCx},
    crate::helpers::strip_generics,
    quote::{format_ident, quote},
};

pub(crate) struct AccountsPlan {
    pub parse_steps: Vec<proc_macro2::TokenStream>,
    pub count_expr: proc_macro2::TokenStream,
    pub parse_body: proc_macro2::TokenStream,
}

struct ParseFieldPlan {
    field_name: syn::Ident,
    offset: SlotOffset,
    kind: ParseFieldKind,
}

enum ParseFieldKind {
    Single(HeaderPlan),
    Composite { inner_ty: proc_macro2::TokenStream },
}

/// The flattened-account-array index of a field: the number of preceding fixed
/// (single) accounts plus the `AccountCount::COUNT` of every preceding
/// composite. Emitted as `fixed + Σ <composite>::COUNT` (a const expression).
struct SlotOffset {
    fixed: usize,
    composites: Vec<syn::Type>,
}

impl SlotOffset {
    /// The slot index relative to `__offset`, the caller-supplied base of this
    /// struct's region in the flattened account array. Absolute indices are
    /// required: `parse_account_dup` resolves the SVM dup byte, which is an
    /// index into the whole transaction's account list, against `base`.
    fn to_tokens(&self) -> proc_macro2::TokenStream {
        let krate = crate::krate::lang_path();
        let terms = self.composites.iter().map(|ty| {
            let inner = composite_parse_ty(ty);
            quote! { <#inner as #krate::traits::AccountCount>::COUNT }
        });
        // The first account of a struct sits at `__offset` itself; emitting the
        // `+ 0usize` it would otherwise carry adds a term that reads as if the
        // slot were computed when it is not.
        if self.fixed == 0 {
            quote! { __offset #(+ #terms)* }
        } else {
            let fixed = self.fixed;
            quote! { __offset + #fixed #(+ #terms)* }
        }
    }

    /// The offset rendered for the debug log. Stringifying `to_tokens()` would
    /// bake a whole `<Ty as AccountCount>::COUNT` path into the binary.
    fn debug_string(&self) -> String {
        if self.composites.is_empty() {
            self.fixed.to_string()
        } else {
            format!("{}+{}composite", self.fixed, self.composites.len())
        }
    }
}

struct HeaderPlan {
    ty: proc_macro2::TokenStream,
    writable: bool,
    optional: bool,
    allow_dup: bool,
}

impl HeaderPlan {
    fn from_field_plan(fp: &resolve::specs::FieldPlan) -> Self {
        Self {
            ty: {
                let ty = &fp.effective_ty;
                quote! { #ty }
            },
            writable: fp.writable,
            optional: fp.optional,
            allow_dup: fp.dup,
        }
    }
}

pub(crate) fn build_accounts_plan(
    typed_plan: &resolve::specs::AccountsPlanTyped,
    cx: &EmitCx,
) -> AccountsPlan {
    let fields = build_parse_fields(&typed_plan.fields);
    AccountsPlan {
        parse_steps: emit_parse_account_steps(&fields),
        count_expr: emit_count_expr(&fields),
        parse_body: emit_full_parse_body(typed_plan, &fields, cx),
    }
}

fn build_parse_fields(field_plans: &[resolve::specs::FieldPlan]) -> Vec<ParseFieldPlan> {
    let mut fields = Vec::new();
    let mut fixed = 0usize;
    let mut composites: Vec<syn::Type> = Vec::new();

    for fp in field_plans {
        let offset = SlotOffset {
            fixed,
            composites: composites.clone(),
        };

        match fp.kind {
            resolve::FieldKind::Composite => {
                let inner_ty = composite_parse_ty(&fp.effective_ty);
                fields.push(ParseFieldPlan {
                    field_name: fp.ident.clone(),
                    offset,
                    kind: ParseFieldKind::Composite { inner_ty },
                });
                composites.push(fp.effective_ty.clone());
            }
            resolve::FieldKind::Single => {
                fields.push(ParseFieldPlan {
                    field_name: fp.ident.clone(),
                    offset,
                    kind: ParseFieldKind::Single(HeaderPlan::from_field_plan(fp)),
                });
                fixed += 1;
            }
        }
    }

    fields
}

fn composite_parse_ty(ty: &syn::Type) -> proc_macro2::TokenStream {
    if resolve::wrapper::classify_wrapper(ty) == resolve::wrapper::WrapperKind::AccountsArray {
        return quote! { #ty };
    }
    // Composite field types are path types; fall back to the whole type token
    // (localized trait error, never a cascade) if that ever fails to hold.
    strip_generics(ty).unwrap_or_else(|_| quote! { #ty })
}

fn emit_parse_account_steps(fields: &[ParseFieldPlan]) -> Vec<proc_macro2::TokenStream> {
    fields.iter().map(emit_parse_field_step).collect()
}

fn emit_parse_field_step(field: &ParseFieldPlan) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    match &field.kind {
        ParseFieldKind::Composite { inner_ty } => {
            let cur_offset = field.offset.to_tokens();
            quote! {
                input = unsafe {
                    // SAFETY: the generated caller passes an input slice with
                    // enough accounts for the statically computed COUNT.
                    <#inner_ty as #krate::traits::ParseAccountsRaw>::parse_accounts_raw(
                        input,
                        base,
                        #cur_offset,
                        __program_id,
                    )?
                };
            }
        }
        ParseFieldKind::Single(header) => {
            emit_single_parse_step(&field.field_name, header, &field.offset)
        }
    }
}

fn emit_single_parse_step(
    field_name: &syn::Ident,
    header: &HeaderPlan,
    offset: &SlotOffset,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let cur_offset = offset.to_tokens();
    let ty = &header.ty;
    let writable = header.writable;

    // The wrapper type and the required-writable bit are the whole header: the
    // parse helpers read `IS_SIGNER`/`IS_EXECUTABLE` off `T` through an
    // associated const, so no header constants reach the expansion.
    if header.optional || header.allow_dup {
        let log = debug_log_line(field_name, offset, "parsed (dup-aware)");
        let is_optional = header.optional;
        let allow_dup = header.allow_dup;

        return quote! {
            // SAFETY: parse_account_dup validates the current account and
            // advances within the pre-counted input slice.
            input = unsafe {
                #krate::__internal::parse_account_dup::<#ty, #writable>(
                    input,
                    base,
                    #cur_offset,
                    __program_id,
                    #krate::__internal::ParseFlags {
                        is_optional: #is_optional,
                        is_ref_mut: #writable,
                        allow_dup: #allow_dup,
                    },
                )?
            };
            #log
        };
    }

    let log = debug_log_line(field_name, offset, "validation passed");
    quote! {
        // SAFETY: parse_account validates the current account and advances
        // within the pre-counted input slice.
        input = unsafe {
            #krate::__internal::parse_account::<#ty, #writable>(input, base, #cur_offset)?
        };
        #log
    }
}

/// The per-account trace line, built at macro time so the expansion carries one
/// string literal instead of a `concat!`/`stringify!` tree.
fn debug_log_line(
    field_name: &syn::Ident,
    offset: &SlotOffset,
    outcome: &str,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    let message = format!("account {field_name} @{}: {outcome}", offset.debug_string());
    quote! { #krate::debug_log!(#message); }
}

fn emit_count_expr(fields: &[ParseFieldPlan]) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    if fields
        .iter()
        .all(|field| matches!(field.kind, ParseFieldKind::Single(_)))
    {
        let n = fields.len();
        quote! { #n }
    } else {
        let addends: Vec<proc_macro2::TokenStream> = fields
            .iter()
            .map(|field| match &field.kind {
                ParseFieldKind::Composite { inner_ty, .. } => {
                    quote! { <#inner_ty as #krate::traits::AccountCount>::COUNT }
                }
                ParseFieldKind::Single(_) => quote! { 1usize },
            })
            .collect();
        quote! { #(#addends)+* }
    }
}

fn emit_full_parse_body(
    typed_plan: &resolve::specs::AccountsPlanTyped,
    fields: &[ParseFieldPlan],
    cx: &EmitCx,
) -> proc_macro2::TokenStream {
    let inner_body = super::parse::emit_parse_body(typed_plan, cx);
    emit_parse_body_from_inner(fields, inner_body)
}

fn emit_parse_body_from_inner(
    fields: &[ParseFieldPlan],
    inner_body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let krate = crate::krate::lang_path();
    if fields
        .iter()
        .any(|field| matches!(field.kind, ParseFieldKind::Composite { .. }))
    {
        let mut field_lets: Vec<proc_macro2::TokenStream> = Vec::new();
        // Only a struct that hands out more than one chunk ever rebinds the
        // cursor, and the final chunk never needs the tail it leaves behind.
        let rebinds = fields.len() > 1;
        let cursor_mut = if rebinds { quote! { mut } } else { quote! {} };
        field_lets.push(quote! {
            let #cursor_mut __accounts_rest: &mut [#krate::__internal::AccountView] = accounts;
        });

        let last = fields.len() - 1;
        for (idx, field) in fields.iter().enumerate() {
            let (rest_pat, advance) = if idx == last {
                (quote! { _ }, quote! {})
            } else {
                (quote! { __rest }, quote! { __accounts_rest = __rest; })
            };
            match &field.kind {
                ParseFieldKind::Composite { inner_ty, .. } => {
                    let field_name = &field.field_name;
                    let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                    field_lets.push(quote! {
                        // SAFETY: `parse_accounts_raw` already proved this
                        // composite's COUNT accounts are present.
                        let (__chunk, #rest_pat) = unsafe {
                            __accounts_rest.split_at_mut_unchecked(<#inner_ty as #krate::traits::AccountCount>::COUNT)
                        };
                        #advance
                        // SAFETY: the raw parser above validated this composite
                        // account chunk.
                        let (#field_name, #bumps_var) = unsafe { <#inner_ty as #krate::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
                            __chunk,
                            __ix_data,
                            __program_id
                        ) }?;
                    });
                }
                ParseFieldKind::Single(_) => {
                    let field_name = &field.field_name;
                    field_lets.push(quote! {
                        // SAFETY: `parse_accounts_raw` already proved at least
                        // one account remains for this field.
                        let (__chunk, #rest_pat) = unsafe { __accounts_rest.split_at_mut_unchecked(1) };
                        #advance
                        // SAFETY: the one-element split above guarantees index 0.
                        let #field_name = unsafe { __chunk.get_unchecked_mut(0) };
                    });
                }
            }
        }

        quote! {
            #(#field_lets)*
            #inner_body
        }
    } else {
        let names: Vec<&syn::Ident> = fields.iter().map(|field| &field.field_name).collect();

        quote! {
            let [#(#names),*] = accounts else {
                // SAFETY: parse_accounts_raw enforces this exact static count
                // before unchecked parsing runs.
                unsafe { core::hint::unreachable_unchecked() }
            };
            #inner_body
        }
    }
}
