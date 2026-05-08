//! Unified codegen for `#[account]` types.

use {proc_macro::TokenStream, syn::DeriveInput};

/// Info about each field needed for codegen.
pub(super) struct PodFieldInfo<'a> {
    pub field: &'a syn::Field,
    pub pod_dyn: Option<crate::helpers::PodDynField>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    field_infos: &[PodFieldInfo<'_>],
    input: &DeriveInput,
    gen_set_inner: bool,
    custom: bool,
) -> TokenStream {
    let vis = &input.vis;
    let attrs = &input.attrs;
    let has_dynamic = field_infos.iter().any(|fi| fi.pod_dyn.is_some());

    let zc = super::layout::build_zc_spec(name, field_infos, has_dynamic);
    let bump_offset_impl = super::layout::emit_bump_offset_impl(field_infos, disc_len, &zc.zc_path);
    let dynamic = super::dynamic::build_dynamic_pieces(field_infos, disc_len, &zc.zc_mod);

    let zc_definition = super::layout::emit_zc_definition(name, has_dynamic, &zc);
    let account_wrapper =
        super::layout::emit_account_wrapper(attrs, vis, name, disc_len, &zc.zc_path);
    // Custom accounts skip Owner, Discriminator, and generated AccountLoad checks —
    // the user's check() method replaces all framework validation. Instead we
    // generate a direct AccountLoad impl that delegates to Self::check().
    let (discriminator_impl, owner_impl, space_impl, account_check_impl, custom_account_load) =
        if custom {
            let space = super::traits::emit_space_impl(
                name,
                field_infos,
                has_dynamic,
                disc_len,
                &zc.zc_mod,
            );
            let account_load = quote::quote! {
                impl quasar_lang::account_load::AccountLoad for #name {

                    #[inline(always)]
                    fn check(view: &quasar_lang::__internal::AccountView) -> Result<(), quasar_lang::prelude::ProgramError> {
                        #name::check(view)
                    }
                }

            };
            // Custom accounts do NOT get generated AccountInit/AccountExit —
            // the user provides manual trait impls if needed.
            (
                quote::quote! {},
                quote::quote! {},
                space,
                quote::quote! {},
                account_load,
            )
        } else if has_dynamic {
            // Dynamic/compact accounts: inline validation into AccountLoad::check.
            let disc = super::traits::emit_discriminator_impl(name, disc_bytes, &bump_offset_impl);
            let owner = super::traits::emit_owner_impl(name);
            let space = super::traits::emit_space_impl(
                name,
                field_infos,
                has_dynamic,
                disc_len,
                &zc.zc_mod,
            );
            let account_load =
                super::traits::emit_dynamic_account_load(super::traits::AccountLoadSpec {
                    name,
                    has_dynamic,
                    disc_len,
                    disc_indices,
                    disc_bytes,
                    zc_path: &zc.zc_path,
                    zc_mod: &zc.zc_mod,
                });
            (disc, owner, space, quote::quote! {}, account_load)
        } else {
            // Fixed accounts: emit AccountLayout + composed checks.
            // AccountLoad::check is the single source of truth, composing
            // Discriminator + DataLen + ZeroPod.
            let disc = super::traits::emit_discriminator_impl(name, disc_bytes, &bump_offset_impl);
            let owner = super::traits::emit_owner_impl(name);
            let space = super::traits::emit_space_impl(
                name,
                field_infos,
                has_dynamic,
                disc_len,
                &zc.zc_mod,
            );
            let disc_len_lit = disc_len;
            let zc_mod_ident = &zc.zc_mod;
            let account_load = quote::quote! {
                impl quasar_lang::account_layout::AccountLayout for #name {
                    type Schema = #zc_mod_ident::__Schema;
                    type Target = <#zc_mod_ident::__Schema as quasar_lang::__zeropod::ZeroPodFixed>::Zc;
                    const DATA_OFFSET: usize = #disc_len_lit;
                }

                impl quasar_lang::checks::Discriminator for #name {}
                impl quasar_lang::checks::DataLen for #name {}
                impl quasar_lang::checks::ZeroPod for #name {}

                impl quasar_lang::account_load::AccountLoad for #name {
                    #[inline(always)]
                    fn check(view: &quasar_lang::__internal::AccountView) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
                        <#name as quasar_lang::checks::Discriminator>::check(view)?;
                        <#name as quasar_lang::checks::ZeroPod>::check(view)?;
                        Ok(())
                    }

                    #[inline(always)]
                    fn check_checked(view: &quasar_lang::__internal::AccountView) -> Result<(), quasar_lang::__solana_program_error::ProgramError> {
                        <#name as quasar_lang::checks::Discriminator>::check_checked(view)?;
                        <#name as quasar_lang::checks::ZeroPod>::check_checked(view)?;
                        Ok(())
                    }
                }

            };
            // Composed checks are the single generated validation path.
            (disc, owner, space, quote::quote! {}, account_load)
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

    // Generate AccountInit + AccountExit for non-custom accounts.
    // Custom accounts and one_of enums skip these — the user provides
    // manual impls if needed.
    let lifecycle_impls = if custom {
        quote::quote! {}
    } else {
        quote::quote! {
            impl quasar_lang::account_init::AccountInit for #name {
                type InitParams<'a> = ();

                #[inline(always)]
                fn init<'a>(
                    ctx: quasar_lang::account_init::InitCtx<'a>,
                    _params: &(),
                ) -> Result<(), quasar_lang::prelude::ProgramError> {
                    quasar_lang::account_init::init_account(
                        ctx.payer,
                        ctx.target,
                        ctx.space,
                        ctx.program_id,
                        ctx.signers,
                        ctx.rent,
                        <Self as quasar_lang::traits::Discriminator>::DISCRIMINATOR,
                    )
                }
            }

            impl quasar_lang::ops::close::AccountClose for #name {
                #[inline(always)]
                fn close(
                    view: &mut quasar_lang::__internal::AccountView,
                    dest: &quasar_lang::__internal::AccountView,
                ) -> Result<(), quasar_lang::prelude::ProgramError> {
                    quasar_lang::ops::close::close_account(
                        view,
                        dest,
                        <Self as quasar_lang::traits::Discriminator>::DISCRIMINATOR.len(),
                    )
                }
            }

            impl quasar_lang::ops::SupportsRealloc for #name {}
        }
    };

    // IDL fragment emission (feature-gated)
    let idl_fragment = {
        let name_str = name.to_string();
        let disc_values: Vec<u8> = disc_bytes
            .iter()
            .map(|lit| lit.base10_parse::<u8>().unwrap_or(0))
            .collect();

        let mut inline_field_names: Vec<String> = Vec::new();
        let mut tail_field_names: Vec<String> = Vec::new();

        let field_defs: Vec<proc_macro2::TokenStream> = field_infos
            .iter()
            .map(|fi| {
                let fname = fi
                    .field
                    .ident
                    .as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_default();
                let fty = crate::helpers::type_to_idl_type_tokens(&fi.field.ty);
                let codec_tokens = crate::helpers::type_to_idl_codec_tokens(&fi.field.ty);

                if fi.pod_dyn.is_some() {
                    tail_field_names.push(fname.clone());
                } else {
                    inline_field_names.push(fname.clone());
                }

                quote::quote! {
                    quasar_lang::idl_build::__reexport::IdlFieldDef {
                        name: quasar_lang::idl_build::s(#fname),
                        ty: #fty,
                        codec: #codec_tokens,
                        docs: quasar_lang::idl_build::Vec::new(),
                    }
                }
            })
            .collect();

        let layout_tokens = if tail_field_names.is_empty() {
            // All fields are fixed — emit Fixed layout
            let all_names = inline_field_names.iter().collect::<Vec<_>>();
            quote::quote! {
                Some(quasar_lang::idl_build::__reexport::IdlLayout::Fixed {
                    fields: quasar_lang::idl_build::vec![
                        #(quasar_lang::idl_build::s(#all_names)),*
                    ],
                })
            }
        } else {
            // Has dynamic fields — emit Compact layout
            let inline_refs = inline_field_names.iter().collect::<Vec<_>>();
            let tail_refs = tail_field_names.iter().collect::<Vec<_>>();
            quote::quote! {
                Some(quasar_lang::idl_build::__reexport::IdlLayout::Compact {
                    inline_fields: quasar_lang::idl_build::vec![
                        #(quasar_lang::idl_build::s(#inline_refs)),*
                    ],
                    tail_fields: quasar_lang::idl_build::vec![
                        #(quasar_lang::idl_build::s(#tail_refs)),*
                    ],
                    wire: quasar_lang::idl_build::__reexport::CompactWire::InlineFieldsThenTailHeadersThenTailPayloads,
                })
            }
        };

        quote::quote! {
            #[cfg(feature = "idl-build")]
            quasar_lang::__private_inventory::submit! {
                quasar_lang::idl_build::AccountFragment {
                    build: {
                        fn __build() -> (
                            quasar_lang::idl_build::__reexport::IdlAccountDef,
                            quasar_lang::idl_build::__reexport::IdlTypeDef,
                        ) {
                            (
                                quasar_lang::idl_build::__reexport::IdlAccountDef {
                                    name: quasar_lang::idl_build::s(#name_str),
                                    discriminator: quasar_lang::idl_build::vec![#(#disc_values),*],
                                    docs: quasar_lang::idl_build::Vec::new(),
                                    space: None,
                                },
                                quasar_lang::idl_build::__reexport::IdlTypeDef {
                                    name: quasar_lang::idl_build::s(#name_str),
                                    kind: quasar_lang::idl_build::__reexport::IdlTypeDefKind::Struct,
                                    docs: quasar_lang::idl_build::Vec::new(),
                                    generics: quasar_lang::idl_build::Vec::new(),
                                    fields: quasar_lang::idl_build::vec![#(#field_defs),*],
                                    variants: quasar_lang::idl_build::Vec::new(),
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

        #account_check_impl

        #custom_account_load

        #lifecycle_impls

        #dynamic_impl_block

        #compact_mut

        #dyn_writer

        #set_inner_impl

        #idl_fragment
    }
    .into()
}
