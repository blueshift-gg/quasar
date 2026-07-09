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
    pub direct_parse_body: proc_macro2::TokenStream,
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
    fn to_tokens(&self) -> proc_macro2::TokenStream {
        let fixed = self.fixed;
        let terms = self.composites.iter().map(|ty| {
            let inner = composite_parse_ty(ty);
            quote! { <#inner as AccountCount>::COUNT }
        });
        quote! { #fixed #(+ #terms)* }
    }

    /// The offset rendered as a string literal for the debug log.
    fn debug_string(&self) -> String {
        self.to_tokens().to_string()
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

    fn expected_expr(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let writable_bit: u32 = if self.writable { 0x01 << 16 } else { 0 };
        // IS_SIGNER and IS_EXECUTABLE come from the type's AccountLoad impl:
        // no domain knowledge needed here.
        quote! {{
            const __S: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
            const __E: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
            0xFFu32 | (__S as u32) << 8 | #writable_bit | (__E as u32) << 24
        }}
    }

    fn mask_expr(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let writable_mask: u32 = if self.writable { 0xFF << 16 } else { 0 };
        quote! {{
            const __S: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
            const __E: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
            0xFFu32 | (if __S { 0xFFu32 << 8 } else { 0u32 }) | #writable_mask | (if __E { 0xFFu32 << 24 } else { 0u32 })
        }}
    }

    fn flag_mask_expr(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let writable_mask: u32 = if self.writable { 0xFF << 16 } else { 0 };
        quote! {{
            const __S: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_SIGNER;
            const __E: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE;
            (if __S { 0xFFu32 << 8 } else { 0u32 }) | #writable_mask | (if __E { 0xFFu32 << 24 } else { 0u32 })
        }}
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
        direct_parse_body: emit_direct_parse_body(typed_plan, &fields, cx),
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
    match &field.kind {
        ParseFieldKind::Composite { inner_ty } => {
            let cur_offset = field.offset.to_tokens();
            quote! {
                {
                    input = unsafe {
                        // SAFETY: the generated caller passes an input slice with
                        // enough accounts for the statically computed COUNT.
                        <#inner_ty as quasar_lang::traits::ParseAccountsRaw>::parse_accounts_raw(
                            input,
                            base,
                            #cur_offset,
                            __program_id,
                        )?
                    };
                }
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
    let cur_offset = offset.to_tokens();
    let account_index = offset.debug_string();
    let expected_expr = header.expected_expr();
    let mask_expr = header.mask_expr();

    if header.optional || header.allow_dup {
        let flag_mask_expr = header.flag_mask_expr();
        let is_optional = header.optional;
        let is_ref_mut = header.writable;
        let allow_dup = header.allow_dup;

        quote! {
            {
                const __EXPECTED: u32 = #expected_expr;
                const __MASK: u32 = #mask_expr;
                const __FLAG_MASK: u32 = #flag_mask_expr;
                input = unsafe {
                    // SAFETY: parse_account_dup validates the current account
                    // and advances within the pre-counted input slice.
                    quasar_lang::__internal::parse_account_dup(
                        input,
                        base,
                        #cur_offset,
                        __program_id,
                        quasar_lang::__internal::ParseFlags {
                            expected: __EXPECTED,
                            mask: __MASK,
                            flag_mask: __FLAG_MASK,
                            is_optional: #is_optional,
                            is_ref_mut: #is_ref_mut,
                            allow_dup: #allow_dup,
                        },
                    )?
                };
                quasar_lang::debug_log!(concat!(
                    "Account '", stringify!(#field_name),
                    "' (index ", #account_index, "): parsed (dup-aware)"
                ));
            }
        }
    } else {
        quote! {
            {
                const __EXPECTED: u32 = #expected_expr;
                const __MASK: u32 = #mask_expr;
                input = unsafe {
                    // SAFETY: parse_account validates the current account and
                    // advances within the pre-counted input slice.
                    quasar_lang::__internal::parse_account(
                        input, base, #cur_offset, __EXPECTED, __MASK,
                    )?
                };
                quasar_lang::debug_log!(concat!(
                    "Account '", stringify!(#field_name),
                    "' (index ", #account_index, "): validation passed"
                ));
            }
        }
    }
}

fn emit_count_expr(fields: &[ParseFieldPlan]) -> proc_macro2::TokenStream {
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
                    quote! { <#inner_ty as AccountCount>::COUNT }
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
    if fields
        .iter()
        .any(|field| matches!(field.kind, ParseFieldKind::Composite { .. }))
    {
        let mut field_lets: Vec<proc_macro2::TokenStream> = Vec::new();
        field_lets.push(quote! {
            let mut __accounts_rest: &mut [quasar_lang::__internal::AccountView] = accounts;
        });

        for field in fields {
            match &field.kind {
                ParseFieldKind::Composite { inner_ty, .. } => {
                    let field_name = &field.field_name;
                    let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                    field_lets.push(quote! {
                        // SAFETY: `parse_accounts_raw` already proved this
                        // composite's COUNT accounts are present.
                        let (__chunk, __rest) = unsafe {
                            __accounts_rest.split_at_mut_unchecked(<#inner_ty as AccountCount>::COUNT)
                        };
                        __accounts_rest = __rest;
                        // SAFETY: the raw parser above validated this composite
                        // account chunk.
                        let (#field_name, #bumps_var) = unsafe { <#inner_ty as quasar_lang::traits::ParseAccountsUnchecked>::parse_with_instruction_data_unchecked(
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
                        let (__chunk, __rest) = unsafe { __accounts_rest.split_at_mut_unchecked(1) };
                        __accounts_rest = __rest;
                        // SAFETY: the one-element split above guarantees index 0.
                        let #field_name = unsafe { __chunk.get_unchecked_mut(0) };
                    });
                }
            }
        }
        field_lets.push(quote! { let _ = __accounts_rest; });

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

fn emit_direct_parse_body(
    typed_plan: &resolve::specs::AccountsPlanTyped,
    fields: &[ParseFieldPlan],
    cx: &EmitCx,
) -> proc_macro2::TokenStream {
    let count_expr = emit_count_expr(fields);
    let fallback_body = emit_parse_body_without_behavior_assertions(typed_plan, fields, cx);
    quote! {
        let mut __buf = core::mem::MaybeUninit::<
            [quasar_lang::__internal::AccountView; #count_expr]
        >::uninit();
        let _ = Self::parse_accounts(input, &mut __buf, __program_id)?;
        // SAFETY: parse_accounts initializes the whole fixed-size buffer before
        // returning Ok.
        let mut __accounts = unsafe { __buf.assume_init() };
        let accounts = &mut __accounts;
        let __parsed_result: Result<
            (Self, <Self as quasar_lang::traits::ParseAccounts>::Bumps),
            ProgramError,
        > = {
            #fallback_body
        };
        let (__parsed_accounts, __parsed_bumps) = __parsed_result?;
        Ok((__parsed_accounts, __parsed_bumps))
    }
}

fn emit_parse_body_without_behavior_assertions(
    typed_plan: &resolve::specs::AccountsPlanTyped,
    fields: &[ParseFieldPlan],
    cx: &EmitCx,
) -> proc_macro2::TokenStream {
    let inner_body = super::parse::emit_parse_body_without_behavior_assertions(typed_plan, cx);
    emit_parse_body_from_inner(fields, inner_body)
}
