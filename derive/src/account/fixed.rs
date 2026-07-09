//! Unified codegen for `#[account]` types.

use {proc_macro2::TokenStream, syn::DeriveInput};

/// Info about each field needed for codegen.
pub(super) struct PodFieldInfo<'a> {
    pub field: &'a syn::Field,
    pub pod_dyn: Option<crate::helpers::PodDynField>,
}

/// All inputs to `generate_account`, bundled so the entry point takes one spec
/// instead of eight positional arguments.
pub(super) struct AccountCodegenSpec<'a> {
    pub name: &'a syn::Ident,
    pub disc_bytes: &'a [syn::LitInt],
    pub disc_values: &'a [u8],
    pub disc_len: usize,
    pub disc_indices: &'a [usize],
    pub field_infos: &'a [PodFieldInfo<'a>],
    pub input: &'a DeriveInput,
    pub gen_set_inner: bool,
}

pub(super) fn generate_account(spec: AccountCodegenSpec<'_>) -> TokenStream {
    let krate = crate::krate::lang_path();
    let AccountCodegenSpec {
        name,
        disc_bytes,
        disc_values,
        disc_len,
        disc_indices,
        field_infos,
        input,
        gen_set_inner,
    } = spec;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let has_dynamic = field_infos.iter().any(|fi| fi.pod_dyn.is_some());

    let zc = super::layout::build_zc_spec(name, field_infos, has_dynamic);
    let bump_offset_impl = super::layout::emit_bump_offset_impl(field_infos, disc_len, &zc.zc_path);
    let dynamic = super::dynamic::build_dynamic_pieces(field_infos, disc_len, &zc.zc_mod);

    let zc_definition = super::layout::emit_zc_definition(name, has_dynamic, &zc);
    let account_wrapper =
        super::layout::emit_account_wrapper(attrs, vis, name, disc_len, &zc.zc_path);
    let discriminator_impl =
        super::traits::emit_discriminator_impl(name, disc_bytes, &bump_offset_impl);
    let owner_impl = super::traits::emit_owner_impl(name);
    let space_impl = super::traits::emit_space_impl(name, has_dynamic, disc_len, &zc.zc_mod);
    let account_load_impl = if has_dynamic {
        // Dynamic/compact accounts: inline validation into AccountLoad::check.
        super::traits::emit_dynamic_account_load(super::traits::AccountLoadSpec {
            name,
            disc_len,
            disc_indices,
            disc_bytes,
            zc_mod: &zc.zc_mod,
        })
    } else {
        // Fixed accounts: emit AccountLayout + composed checks.
        // AccountLoad::check is the single source of truth, composing
        // Discriminator + ZeroPod (ZeroPod self-guards its length check, so
        // checks::DataLen is not emitted here).
        let disc_len_lit = disc_len;
        let zc_mod_ident = &zc.zc_mod;
        quote::quote! {
            impl #krate::account_layout::AccountLayout for #name {
                type Schema = #zc_mod_ident::__Schema;
                const DATA_OFFSET: usize = #disc_len_lit;
            }

            impl #krate::checks::Discriminator for #name {}
            impl #krate::checks::ZeroPod for #name {}

            impl #krate::account_load::AccountLoad for #name {
                #[inline(always)]
                fn check(view: &#krate::__internal::AccountView) -> Result<(), #krate::__solana_program_error::ProgramError> {
                    <#name as #krate::checks::Discriminator>::check(view)?;
                    <#name as #krate::checks::ZeroPod>::check(view)?;
                    Ok(())
                }

                #[inline(always)]
                fn check_checked(view: &#krate::__internal::AccountView) -> Result<(), #krate::__solana_program_error::ProgramError> {
                    <#name as #krate::checks::Discriminator>::check_checked(view)?;
                    <#name as #krate::checks::ZeroPod>::check_checked(view)?;
                    Ok(())
                }
            }

        }
    };
    let dynamic_impl_block =
        super::dynamic::emit_dynamic_impl_block(name, has_dynamic, disc_len, &zc.zc_mod, &dynamic);
    let compact_mut = super::dynamic::emit_compact_mut(
        name,
        has_dynamic,
        disc_len,
        &zc.zc_mod,
        &zc.zc_path,
        &dynamic,
    );
    let dyn_writer = super::dynamic::emit_dyn_writer(
        name,
        has_dynamic,
        disc_len,
        &zc.zc_mod,
        &zc.zc_path,
        &dynamic,
    );
    let set_inner_impl = super::methods::emit_set_inner_impl(super::methods::SetInnerSpec {
        name,
        vis,
        field_infos,
        has_dynamic,
        disc_len,
        zc_mod: &zc.zc_mod,
        zc_path: &zc.zc_path,
        gen_set_inner,
    });

    let lifecycle_impls = quote::quote! {
        impl #krate::account_init::AccountInit for #name {
            type InitParams<'a> = ();

            #[inline(always)]
            fn init<'a, R: #krate::ops::RentAccess>(
                ctx: #krate::account_init::InitCtx<'a, R>,
                _params: &(),
            ) -> Result<(), #krate::prelude::ProgramError> {
                #krate::account_init::init_account(
                    ctx.payer,
                    ctx.target,
                    ctx.space,
                    ctx.program_id,
                    ctx.signers,
                    ctx.rent.get()?,
                    <Self as #krate::traits::Discriminator>::DISCRIMINATOR,
                )
            }
        }

        impl #krate::ops::SupportsRealloc for #name {}
    };

    // IDL fragment emission (feature-gated)
    let idl_fragment = {
        let name_str = name.to_string();
        let mut inline_field_names: Vec<String> = Vec::new();
        let mut tail_field_names: Vec<String> = Vec::new();

        let field_defs: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .map(|fi| {
                let fname = fi
                    .field
                    .ident
                    .as_ref()
                    .unwrap_or_else(|| ice!("account fields are validated as named before codegen"))
                    .to_string();
                let fty = crate::idl::type_to_idl_type_tokens(&fi.field.ty);
                let codec_tokens = crate::idl::type_to_idl_codec_tokens(&fi.field.ty);
                let fdocs = crate::helpers::docs_tokens(&fi.field.attrs);

                if fi.pod_dyn.is_some() {
                    tail_field_names.push(fname.clone());
                } else {
                    inline_field_names.push(fname.clone());
                }

                quote::quote! {
                    #krate::idl_build::__reexport::IdlFieldDef {
                        name: #krate::idl_build::s(#fname),
                        ty: #fty,
                        codec: #codec_tokens,
                        docs: #fdocs,
                    }
                }
            })
            .collect();

        // Fixed layout when all fields are inline; Compact otherwise — via the
        // single-source `emit_idl_layout` shared with the instruction fragments.
        let layout_tokens = crate::idl::emit_idl_layout(&inline_field_names, &tail_field_names);

        // Emit the account's on-chain footprint. The fragment builder runs
        // host-side (in the `__quasar_emit_idl` test), where the account's
        // `Space::SPACE` associated const is evaluable. `min` is the minimum
        // byte size including the discriminator.
        let space_tokens = quote::quote! {
            Some(#krate::idl_build::__reexport::IdlSpace {
                discriminator: Some(#disc_len),
                min: <#name as #krate::traits::Space>::SPACE as u64,
                max: None,
                formula: None,
            })
        };

        let struct_docs = crate::helpers::docs_tokens(attrs);

        quote::quote! {
            #[cfg(feature = "idl-build")]
            #krate::__private_inventory::submit! {
                #krate::idl_build::AccountFragment {
                    build: {
                        fn __build() -> (
                            #krate::idl_build::__reexport::IdlAccountDef,
                            #krate::idl_build::__reexport::IdlTypeDef,
                        ) {
                            (
                                #krate::idl_build::__reexport::IdlAccountDef {
                                    name: #krate::idl_build::s(#name_str),
                                    discriminator: #krate::idl_build::vec![#(#disc_values),*],
                                    docs: #struct_docs,
                                    space: #space_tokens,
                                },
                                #krate::idl_build::__reexport::IdlTypeDef {
                                    name: #krate::idl_build::s(#name_str),
                                    kind: #krate::idl_build::__reexport::IdlTypeDefKind::Struct,
                                    docs: #krate::idl_build::Vec::new(),
                                    fields: #krate::idl_build::vec![#(#field_defs),*],
                                    variants: #krate::idl_build::Vec::new(),
                                    repr: None,
                                    alias: None,
                                    fallback: None,
                                    codec: None,
                                    layout: #layout_tokens,
                                    space: None,
                                    semantics: None,
                                },
                            )
                        }
                        __build
                    },
                }
            }
        }
    };

    quote::quote! {
        #account_wrapper

        #zc_definition

        #discriminator_impl

        #owner_impl

        #space_impl

        #account_load_impl

        #lifecycle_impls

        #dynamic_impl_block

        #compact_mut

        #dyn_writer

        #set_inner_impl

        #idl_fragment
    }
}
