//! Per-instruction specs and the `#[program(...)]` attribute.
//!
//! `InstructionSpec` is the resolved, validated description of one normal
//! instruction (discriminator, accounts type, client/IDL args, remaining-account
//! flag); `RawInstructionSpec` is the lighter `#[instruction(raw)]` variant.
//! `model.rs` builds these from the raw scan; `dispatch.rs` and `idl.rs` consume
//! them for codegen.

use {
    crate::helpers::{
        classify_borrowed_as_compact, classify_lifetime_arg, classify_option_pod_dynamic,
        classify_pod_dynamic, parse_max_attr, pascal_to_snake, prefix_bytes_to_rust_type,
        snake_to_pascal, PodDynField,
    },
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    syn::{FnArg, Ident, LitInt, Pat, Type},
};

/// Parsed attributes from `#[program(...)]`.
pub(super) struct ProgramArgs {
    pub no_entrypoint: bool,
}

impl syn::parse::Parse for ProgramArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut no_entrypoint = false;
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "no_entrypoint" {
                if no_entrypoint {
                    return Err(syn::Error::new(ident.span(), "duplicate `no_entrypoint`"));
                }
                no_entrypoint = true;
            } else {
                return Err(syn::Error::new(ident.span(), "expected `no_entrypoint`"));
            }
            let _ = input.parse::<Option<syn::Token![,]>>();
        }
        Ok(Self { no_entrypoint })
    }
}

/// Whether an instruction's discriminator was pinned or auto-assigned. Threaded
/// into the IDL so tooling can distinguish the two.
#[derive(Clone, Copy)]
pub(super) enum DiscriminatorSource {
    Auto,
    Explicit,
}

impl DiscriminatorSource {
    pub(super) fn idl_tokens(self) -> TokenStream2 {
        match self {
            Self::Auto => quote! {
                quasar_lang::idl_build::InstructionDiscriminatorSource::Auto
            },
            Self::Explicit => quote! {
                quasar_lang::idl_build::InstructionDiscriminatorSource::Explicit
            },
        }
    }
}

/// An instruction argument's name and type. The client-arg list stores the
/// off-chain mapped type (`String<N>` -> `DynString<P>`); the IDL-arg list keeps
/// the type as declared in the handler signature.
pub(super) struct ArgSpec {
    pub name: Ident,
    pub ty: Type,
}

/// Lightweight spec for `#[instruction(raw)]`: only discriminator + heap flag.
/// Raw instructions have no accounts type, no client args, no remaining.
pub(super) struct RawInstructionSpec {
    pub fn_name: Ident,
    pub disc_bytes: Vec<LitInt>,
    pub disc_values: Vec<u8>,
    pub discriminator_source: DiscriminatorSource,
    pub heap: bool,
    pub docs: Vec<String>,
}

pub(super) struct InstructionSpec {
    pub fn_name: Ident,
    pub disc_bytes: Vec<LitInt>,
    pub disc_values: Vec<u8>,
    pub discriminator_source: DiscriminatorSource,
    pub accounts_type: TokenStream2,
    pub accounts_type_str: String,
    pub heap: bool,
    pub client_struct_name: Ident,
    pub client_macro_ident: Ident,
    pub client_args: Vec<ArgSpec>,
    pub idl_args: Vec<ArgSpec>,
    pub has_remaining: bool,
    pub docs: Vec<String>,
}

/// Context wrapper classification for an instruction function.
#[derive(Clone, Copy)]
pub(super) struct CtxKind<'a>(&'a Type, bool);

impl<'a> CtxKind<'a> {
    /// Classify the first parameter of an instruction function.
    pub(super) fn classify(sig: &'a syn::Signature) -> syn::Result<Self> {
        let first_arg = match sig.inputs.first() {
            Some(FnArg::Typed(pt)) => pt,
            _ => {
                return Err(syn::Error::new_spanned(
                    &sig.ident,
                    "#[program]: instruction function must have ctx: Ctx<T> as first parameter",
                ));
            }
        };

        if let Some(inner) = crate::helpers::extract_generic_inner_type(&first_arg.ty, "Ctx") {
            return Ok(CtxKind(inner, false));
        }
        if let Some(inner) =
            crate::helpers::extract_generic_inner_type(&first_arg.ty, "CtxWithRemaining")
        {
            return Ok(CtxKind(inner, true));
        }

        Err(syn::Error::new_spanned(
            &first_arg.ty,
            "first parameter must be Ctx<T> or CtxWithRemaining<T>",
        ))
    }

    pub(super) fn inner_ty(&self) -> &'a Type {
        self.0
    }

    pub(super) fn has_remaining(&self) -> bool {
        self.1
    }
}

/// Map a handler argument to its off-chain client type. Fixed args pass through;
/// dynamic (`String<N>`/`Vec<T,N>`), optional-dynamic, and borrowed (`&str`/
/// `&[T]` + `#[max]`) args map to the compact `DynString`/`DynVec` client types.
fn map_client_arg_type(pt: &syn::PatType) -> syn::Result<Type> {
    let ty = if classify_lifetime_arg(&pt.ty) {
        // Borrowed struct (has lifetime param): the off-chain client
        // takes pre-serialized bytes. The user is responsible for
        // encoding the struct into the wire format.
        syn::parse_quote!(::alloc::vec::Vec<u8>)
    } else if let Some(pod_dyn) = classify_pod_dynamic(&pt.ty) {
        match pod_dyn {
            PodDynField::Str { prefix_bytes, .. } => {
                let pfx_ty = prefix_bytes_to_rust_type(prefix_bytes);
                syn::parse_quote!(quasar_lang::client::DynString<#pfx_ty>)
            }
            PodDynField::Vec {
                elem, prefix_bytes, ..
            } => {
                let pfx_ty = prefix_bytes_to_rust_type(prefix_bytes);
                syn::parse_quote!(quasar_lang::client::DynVec<#elem, #pfx_ty>)
            }
        }
    } else if let Some(pod_dyn) = classify_option_pod_dynamic(&pt.ty) {
        match pod_dyn {
            PodDynField::Str { prefix_bytes, .. } => {
                let pfx_ty = prefix_bytes_to_rust_type(prefix_bytes);
                syn::parse_quote!(Option<quasar_lang::client::DynString<#pfx_ty>>)
            }
            PodDynField::Vec {
                elem, prefix_bytes, ..
            } => {
                let pfx_ty = prefix_bytes_to_rust_type(prefix_bytes);
                syn::parse_quote!(Option<quasar_lang::client::DynVec<#elem, #pfx_ty>>)
            }
        }
    } else if matches!(&*pt.ty, Type::Reference(_)) {
        // Borrowed arg (&str, &[T]): parse #[max(N)] and map
        // to compact client type, same wire format as String<N>/Vec<T,N>.
        let (max_n, pfx) = match parse_max_attr(&pt.attrs) {
            Some(Ok(value)) => value,
            Some(Err(e)) => return Err(e),
            None => (0, 0),
        };
        if let Some(pd) = classify_borrowed_as_compact(&pt.ty, max_n, pfx) {
            match pd {
                PodDynField::Str { prefix_bytes, .. } => {
                    let pfx_ty = prefix_bytes_to_rust_type(prefix_bytes);
                    syn::parse_quote!(quasar_lang::client::DynString<#pfx_ty>)
                }
                PodDynField::Vec {
                    elem, prefix_bytes, ..
                } => {
                    let pfx_ty = prefix_bytes_to_rust_type(prefix_bytes);
                    syn::parse_quote!(quasar_lang::client::DynVec<#elem, #pfx_ty>)
                }
            }
        } else {
            // Unsupported borrowed type: pass through; #[instruction]
            // will emit the real error.
            (*pt.ty).clone()
        }
    } else {
        (*pt.ty).clone()
    };
    Ok(ty)
}

impl InstructionSpec {
    /// Build the spec for one normal instruction handler. The discriminator has
    /// already been resolved (auto or explicit) by `model.rs`; this collects the
    /// accounts type, client/IDL arg lists, and remaining-account flag.
    pub(super) fn from_handler(
        func: &syn::ItemFn,
        disc_bytes: Vec<LitInt>,
        disc_values: Vec<u8>,
        discriminator_source: DiscriminatorSource,
        heap: bool,
        ctx_kind: CtxKind<'_>,
    ) -> syn::Result<Self> {
        let fn_name = &func.sig.ident;
        let inner_ty = ctx_kind.inner_ty();
        let accounts_type = quote!(#inner_ty);

        let struct_name = format_ident!("{}Instruction", snake_to_pascal(&fn_name.to_string()));
        // Resolve the fragment name from the last path segment sans
        // generics (`Ctx<instructions::Deposit>` -> "Deposit"), so
        // it matches the bare accounts-struct ident on the other
        // side of the join and never feeds `::`/`<..>` into
        // `format_ident!` (which would panic).
        let accounts_type_str = crate::helpers::last_type_segment_name(inner_ty);
        let macro_ident = format_ident!("__{}_instruction", pascal_to_snake(&accounts_type_str));

        let mut idl_args: Vec<ArgSpec> = Vec::new();
        let mut client_args: Vec<ArgSpec> = Vec::new();
        for arg in func.sig.inputs.iter().skip(1) {
            let FnArg::Typed(pt) = arg else {
                continue;
            };
            let name = match &*pt.pat {
                Pat::Ident(pi) => pi.ident.clone(),
                _ => continue,
            };
            idl_args.push(ArgSpec {
                name: name.clone(),
                ty: (*pt.ty).clone(),
            });
            let ty = map_client_arg_type(pt)?;
            client_args.push(ArgSpec { name, ty });
        }

        Ok(InstructionSpec {
            fn_name: fn_name.clone(),
            disc_bytes,
            disc_values,
            discriminator_source,
            accounts_type,
            accounts_type_str,
            heap,
            client_struct_name: struct_name,
            client_macro_ident: macro_ident,
            client_args,
            idl_args,
            has_remaining: ctx_kind.has_remaining(),
            docs: crate::helpers::extract_doc_lines(&func.attrs),
        })
    }

    pub(super) fn has_compact_client_layout(&self) -> bool {
        self.idl_args
            .iter()
            .any(|arg| crate::helpers::instruction_arg_is_compact(&arg.ty))
    }

    pub(super) fn client_item(&self) -> TokenStream2 {
        let struct_name = &self.client_struct_name;
        let macro_ident = &self.client_macro_ident;
        let disc_values = &self.disc_values;
        let arg_names: Vec<&Ident> = self.client_args.iter().map(|arg| &arg.name).collect();
        let arg_types: Vec<&Type> = self.client_args.iter().map(|arg| &arg.ty).collect();
        let remaining_arg = if self.has_remaining {
            quote!(, remaining)
        } else {
            quote!()
        };
        if self.has_compact_client_layout() {
            quote! {
                #macro_ident!(#struct_name, [#(#disc_values),*], {#(#arg_names : #arg_types),*}, compact #remaining_arg);
            }
        } else {
            quote! {
                #macro_ident!(#struct_name, [#(#disc_values),*], {#(#arg_names : #arg_types),*} #remaining_arg);
            }
        }
    }
}
