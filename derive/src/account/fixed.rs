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
    let bump_offset_impl =
        super::layout::emit_bump_offset_impl(field_infos, has_dynamic, disc_len, &zc.zc_path);
    let dynamic = super::dynamic::build_dynamic_pieces(field_infos, disc_len, &zc.zc_mod);

    let zc_definition = super::layout::emit_zc_definition(name, has_dynamic, &zc);
    let account_wrapper =
        super::layout::emit_account_wrapper(attrs, vis, name, disc_len, &zc.zc_path);
    // Custom accounts skip Owner, Discriminator, AccountCheck — the user's
    // check() method replaces all framework validation. Instead we generate
    // a direct AccountLoad impl that delegates to Self::check().
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
                    type BehaviorTarget = Self;

                    #[inline(always)]
                    fn check(
                        view: &quasar_lang::__internal::AccountView,
                        field_name: &str,
                    ) -> Result<(), quasar_lang::prelude::ProgramError> {
                        #name::check(view, field_name)
                    }
                }

                impl quasar_lang::traits::FieldLifecycle for #name {}
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
        } else {
            let disc = super::traits::emit_discriminator_impl(name, disc_bytes, &bump_offset_impl);
            let owner = super::traits::emit_owner_impl(name);
            let space = super::traits::emit_space_impl(
                name,
                field_infos,
                has_dynamic,
                disc_len,
                &zc.zc_mod,
            );
            let check = super::traits::emit_account_check_impl(super::traits::AccountCheckSpec {
                name,
                has_dynamic,
                disc_len,
                disc_indices,
                disc_bytes,
                zc_path: &zc.zc_path,
                zc_mod: &zc.zc_mod,
            });
            (disc, owner, space, check, quote::quote! {})
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

            impl quasar_lang::ops::close_program::AccountClose for #name {
                #[inline(always)]
                fn close(
                    view: &mut quasar_lang::__internal::AccountView,
                    dest: &quasar_lang::__internal::AccountView,
                ) -> Result<(), quasar_lang::prelude::ProgramError> {
                    quasar_lang::ops::close_program::close_program_account(
                        view,
                        dest,
                        <Self as quasar_lang::traits::Discriminator>::DISCRIMINATOR.len(),
                    )
                }
            }

            impl quasar_lang::ops::SupportsRealloc for #name {}
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
    }
    .into()
}
