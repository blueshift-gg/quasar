//! Unified codegen for `#[account]` types.

use {proc_macro::TokenStream, syn::DeriveInput};

/// Info about each field needed for codegen.
pub(super) struct PodFieldInfo<'a> {
    pub field: &'a syn::Field,
    pub pod_dyn: Option<crate::helpers::PodDynField>,
}

pub(super) fn generate_account(
    name: &syn::Ident,
    disc_bytes: &[syn::LitInt],
    disc_len: usize,
    disc_indices: &[usize],
    field_infos: &[PodFieldInfo<'_>],
    input: &DeriveInput,
    gen_set_inner: bool,
) -> TokenStream {
    let vis = &input.vis;
    let attrs = &input.attrs;
    let has_dynamic = field_infos.iter().any(|fi| fi.pod_dyn.is_some());

    let zc = super::layout::build_zc_spec(name, field_infos, has_dynamic);
    let bump_offset_impl =
        super::layout::emit_bump_offset_impl(field_infos, has_dynamic, disc_len, &zc.zc_path);
    let dynamic = super::dynamic::build_dynamic_pieces(field_infos, disc_len, &zc.zc_path);

    let zc_definition =
        super::layout::emit_zc_definition(name, has_dynamic, &zc, &dynamic.align_asserts);
    let account_wrapper =
        super::layout::emit_account_wrapper(attrs, vis, name, disc_len, &zc.zc_path);
    let discriminator_impl =
        super::traits::emit_discriminator_impl(name, disc_bytes, &bump_offset_impl);
    let owner_impl = super::traits::emit_owner_impl(name);
    let space_impl = super::traits::emit_space_impl(
        name,
        field_infos,
        has_dynamic,
        disc_len,
        &zc.zc_path,
        dynamic.prefix_total,
    );
    let account_check_impl =
        super::traits::emit_account_check_impl(super::traits::AccountCheckSpec {
            name,
            has_dynamic,
            disc_len,
            disc_indices,
            disc_bytes,
            zc_path: &zc.zc_path,
            prefix_total: dynamic.prefix_total,
            validation_stmts: &dynamic.validation_stmts,
        });
    let dynamic_impl_block =
        super::dynamic::emit_dynamic_impl_block(name, has_dynamic, disc_len, &zc.zc_path, &dynamic);
    let dyn_guard =
        super::dynamic::emit_dyn_guard(name, has_dynamic, disc_len, &zc.zc_name, &dynamic);
    let dyn_writer =
        super::dynamic::emit_dyn_writer(name, has_dynamic, disc_len, &zc.zc_name, &dynamic);
    let set_inner_impl = super::methods::emit_set_inner_impl(super::methods::SetInnerSpec {
        name,
        vis,
        field_infos,
        has_dynamic,
        disc_len,
        zc_name: &zc.zc_name,
        zc_path: &zc.zc_path,
        gen_set_inner,
    });

    quote::quote! {
        #account_wrapper

        #zc_definition

        #discriminator_impl

        #owner_impl

        #space_impl

        #account_check_impl

        #dynamic_impl_block

        #dyn_guard

        #dyn_writer

        #set_inner_impl
    }
    .into()
}
