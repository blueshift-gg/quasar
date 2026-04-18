use {
    super::{emit, resolve},
    crate::helpers::strip_generics,
    quote::{format_ident, quote},
};

pub(crate) struct AccountsPlan {
    pub parse_steps: Vec<proc_macro2::TokenStream>,
    pub count_expr: proc_macro2::TokenStream,
    pub typed_seed_asserts: proc_macro2::TokenStream,
    pub parse_body: proc_macro2::TokenStream,
}

struct ParseFieldPlan {
    field_name: syn::Ident,
    offset_expr: proc_macro2::TokenStream,
    kind: ParseFieldKind,
}

enum ParseFieldKind {
    Single(HeaderPlan),
    Composite { inner_ty: proc_macro2::TokenStream },
}

struct HeaderPlan {
    ty: proc_macro2::TokenStream,
    account_index: String,
    requires_signer: bool,
    requires_executable: bool,
    writable: bool,
    optional: bool,
    allow_dup: bool,
}

impl HeaderPlan {
    fn from_semantics(
        sem: &resolve::FieldSemantics,
        offset_expr: &proc_macro2::TokenStream,
    ) -> Self {
        Self {
            ty: {
                let ty = &sem.core.effective_ty;
                quote! { #ty }
            },
            account_index: offset_expr.to_string(),
            requires_signer: matches!(sem.core.shape, resolve::FieldShape::Signer)
                || sem.client_requires_signer(),
            requires_executable: matches!(
                sem.core.shape,
                resolve::FieldShape::Program { .. } | resolve::FieldShape::Interface { .. }
            ),
            writable: sem.is_writable(),
            optional: sem.core.optional,
            allow_dup: sem.core.dup,
        }
    }

    fn expected_expr(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let requires_signer = self.requires_signer;
        let writable_bit: u32 = if self.writable { 0x01 << 16 } else { 0 };
        let requires_executable = self.requires_executable;
        quote! {{
            const __S: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_SIGNER || #requires_signer;
            const __E: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE || #requires_executable;
            0xFFu32 | (__S as u32) << 8 | #writable_bit | (__E as u32) << 24
        }}
    }

    fn mask_expr(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let requires_signer = self.requires_signer;
        let writable_mask: u32 = if self.writable { 0xFF << 16 } else { 0 };
        let requires_executable = self.requires_executable;
        quote! {{
            const __S: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_SIGNER || #requires_signer;
            const __E: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE || #requires_executable;
            0xFFu32 | (if __S { 0xFFu32 << 8 } else { 0u32 }) | #writable_mask | (if __E { 0xFFu32 << 24 } else { 0u32 })
        }}
    }

    fn flag_mask_expr(&self) -> proc_macro2::TokenStream {
        let ty = &self.ty;
        let requires_signer = self.requires_signer;
        let writable_mask: u32 = if self.writable { 0xFF << 16 } else { 0 };
        let requires_executable = self.requires_executable;
        quote! {{
            const __S: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_SIGNER || #requires_signer;
            const __E: bool = <#ty as quasar_lang::account_load::AccountLoad>::IS_EXECUTABLE || #requires_executable;
            (if __S { 0xFFu32 << 8 } else { 0u32 }) | #writable_mask | (if __E { 0xFFu32 << 24 } else { 0u32 })
        }}
    }
}

pub(crate) fn build_accounts_plan(
    semantics: &[resolve::FieldSemantics],
    cx: &emit::EmitCx,
) -> syn::Result<AccountsPlan> {
    let fields = build_parse_fields(semantics);
    Ok(AccountsPlan {
        parse_steps: emit_parse_account_steps(&fields),
        count_expr: emit_count_expr(&fields),
        typed_seed_asserts: emit_typed_seed_asserts(semantics),
        parse_body: emit_full_parse_body(semantics, &fields, cx)?,
    })
}

fn build_parse_fields(semantics: &[resolve::FieldSemantics]) -> Vec<ParseFieldPlan> {
    let mut fields = Vec::new();
    let mut buf_offset_expr = quote! { 0usize };

    for sem in semantics {
        let offset_expr = buf_offset_expr.clone();

        if let Some(inner_ty) = composite_inner_ty(sem) {
            fields.push(ParseFieldPlan {
                field_name: sem.core.ident.clone(),
                offset_expr: offset_expr.clone(),
                kind: ParseFieldKind::Composite {
                    inner_ty: inner_ty.clone(),
                },
            });
            buf_offset_expr = quote! { #offset_expr + <#inner_ty as AccountCount>::COUNT };
        } else {
            fields.push(ParseFieldPlan {
                field_name: sem.core.ident.clone(),
                offset_expr: offset_expr.clone(),
                kind: ParseFieldKind::Single(HeaderPlan::from_semantics(sem, &offset_expr)),
            });
            buf_offset_expr = quote! { #offset_expr + 1usize };
        }
    }

    fields
}

fn emit_parse_account_steps(fields: &[ParseFieldPlan]) -> Vec<proc_macro2::TokenStream> {
    fields.iter().map(emit_parse_field_step).collect()
}

fn emit_parse_field_step(field: &ParseFieldPlan) -> proc_macro2::TokenStream {
    match &field.kind {
        ParseFieldKind::Composite { inner_ty } => {
            let cur_offset = &field.offset_expr;
            quote! {
                {
                    let mut __inner_buf = core::mem::MaybeUninit::<
                        [quasar_lang::__internal::AccountView; <#inner_ty as AccountCount>::COUNT]
                    >::uninit();
                    input = <#inner_ty>::parse_accounts(input, &mut __inner_buf, __program_id)?;
                    let __inner = unsafe { __inner_buf.assume_init() };
                    let mut __j = 0usize;
                    while __j < <#inner_ty as AccountCount>::COUNT {
                        unsafe { core::ptr::write(base.add(#cur_offset + __j), *__inner.as_ptr().add(__j)); }
                        __j += 1;
                    }
                }
            }
        }
        ParseFieldKind::Single(header) => {
            emit_single_parse_step(&field.field_name, header, &field.offset_expr)
        }
    }
}

fn emit_single_parse_step(
    field_name: &syn::Ident,
    header: &HeaderPlan,
    cur_offset: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let account_index = &header.account_index;
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
                #[cfg(feature = "debug")]
                quasar_lang::prelude::log(concat!(
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
                    quasar_lang::__internal::parse_account(
                        input, base, #cur_offset, __EXPECTED, __MASK,
                    )?
                };
                #[cfg(feature = "debug")]
                quasar_lang::prelude::log(concat!(
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
                ParseFieldKind::Composite { inner_ty } => {
                    quote! { <#inner_ty as AccountCount>::COUNT }
                }
                ParseFieldKind::Single(_) => quote! { 1usize },
            })
            .collect();
        quote! { #(#addends)+* }
    }
}

fn emit_full_parse_body(
    semantics: &[resolve::FieldSemantics],
    fields: &[ParseFieldPlan],
    cx: &emit::EmitCx,
) -> syn::Result<proc_macro2::TokenStream> {
    let inner_body = emit::emit_parse_body(semantics, cx)?;

    if fields
        .iter()
        .any(|field| matches!(field.kind, ParseFieldKind::Composite { .. }))
    {
        let mut field_lets: Vec<proc_macro2::TokenStream> = Vec::new();
        field_lets.push(quote! { let mut __accounts_rest = accounts; });

        for field in fields {
            match &field.kind {
                ParseFieldKind::Composite { inner_ty } => {
                    let field_name = &field.field_name;
                    let bumps_var = format_ident!("__composite_bumps_{}", field_name);
                    field_lets.push(quote! {
                        let (__chunk, __rest) = unsafe {
                            __accounts_rest.split_at_mut_unchecked(<#inner_ty as AccountCount>::COUNT)
                        };
                        __accounts_rest = __rest;
                        let (#field_name, #bumps_var) = unsafe { <#inner_ty as quasar_lang::traits::ParseAccountsUnchecked>::parse_unchecked(
                            __chunk,
                            __program_id
                        ) }?;
                    });
                }
                ParseFieldKind::Single(_) => {
                    let field_name = &field.field_name;
                    field_lets.push(quote! {
                        let (__chunk, __rest) = unsafe { __accounts_rest.split_at_mut_unchecked(1) };
                        __accounts_rest = __rest;
                        let #field_name = unsafe { __chunk.get_unchecked_mut(0) };
                    });
                }
            }
        }
        field_lets.push(quote! { let _ = __accounts_rest; });

        Ok(quote! {
            #(#field_lets)*
            #inner_body
        })
    } else {
        let names: Vec<&syn::Ident> = fields.iter().map(|field| &field.field_name).collect();

        Ok(quote! {
            let [#(#names),*] = accounts else {
                unsafe { core::hint::unreachable_unchecked() }
            };
            #inner_body
        })
    }
}

fn emit_typed_seed_asserts(semantics: &[resolve::FieldSemantics]) -> proc_macro2::TokenStream {
    let asserts: Vec<proc_macro2::TokenStream> = semantics
        .iter()
        .filter_map(|sem| match &sem.pda {
            Some(resolve::PdaConstraint {
                source: resolve::PdaSource::Typed { type_path, args },
                ..
            }) => {
                let arg_count = args.len();
                Some(quote! {
                    let _: [(); <#type_path as quasar_lang::traits::HasSeeds>::SEED_DYNAMIC_COUNT] = [(); #arg_count];
                })
            }
            _ => None,
        })
        .collect();

    quote! { #(#asserts)* }
}

fn composite_inner_ty(sem: &resolve::FieldSemantics) -> Option<proc_macro2::TokenStream> {
    matches!(sem.core.shape, resolve::FieldShape::Composite)
        .then(|| strip_generics(&sem.core.effective_ty))
}
