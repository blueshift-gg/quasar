//! `#[program]`: generates the program entrypoint, instruction dispatch table,
//! and CPI method stubs. Scans all `#[instruction]` functions within the module
//! to build the discriminator -> handler routing.
//!
//! Pipeline: [`scan`] parses each `#[instruction]` attribute once into a raw
//! list; [`model::ProgramModel`] assigns discriminators and runs every
//! scan-time rule; [`dispatch`] emits the router + entrypoint;
//! [`event_authority`] emits the program marker type + `EventAuthority` PDA;
//! [`idl`] emits the IDL fragments. `mod.rs` is only the orchestrator.

mod dispatch;
mod event_authority;
mod idl;
mod model;
mod scan;
mod spec;

use {
    crate::helpers::snake_to_pascal,
    model::ProgramModel,
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    spec::{InstructionSpec, ProgramArgs},
    syn::ItemMod,
};

pub(crate) fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    program_inner(attr.into(), item.into()).into()
}

pub(crate) fn program_inner(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    let program_args = match syn::parse2::<ProgramArgs>(attr) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };
    let mut module = match syn::parse2::<ItemMod>(item) {
        Ok(module) => module,
        Err(e) => return e.to_compile_error(),
    };
    let mod_name = module.ident.clone();
    let program_type_name = format_ident!("{}Program", snake_to_pascal(&mod_name.to_string()));

    let items = match module.content.as_ref() {
        Some((_, items)) => items,
        None => {
            return syn::Error::new_spanned(
                &module,
                "#[program] must be used on a module with a body",
            )
            .to_compile_error();
        }
    };

    // Scan the module body once, then resolve + validate into the model.
    let raw = match scan::scan(items) {
        Ok(raw) => raw,
        Err(e) => return e.to_compile_error(),
    };
    let model = match ProgramModel::build(&raw, &mod_name) {
        Ok(model) => model,
        Err(e) => return e.to_compile_error(),
    };

    let client_items: Vec<TokenStream2> = model
        .instruction_specs
        .iter()
        .map(InstructionSpec::client_item)
        .collect();

    // Push `__handle_event`, `__dispatch`, the entrypoint, and the cpi module
    // into the user's module body.
    dispatch::push_dispatch_items(&mut module, &model, &program_args, &client_items);

    let program_type = event_authority::emit_program_type(&program_type_name);

    // On host builds the entrypoint is cfg'd out, so `__dispatch` is unreachable
    // and every handler under it reads as dead. Suppress there only: an SBF
    // build has a live entrypoint, so dead code in the user's module is real
    // and worth reporting.
    module.attrs.push(syn::parse_quote!(
        #[cfg_attr(not(any(target_arch = "bpf", target_os = "solana")), allow(dead_code))]
    ));

    let idl = idl::emit_idl(&model, &mod_name);

    quote! {
        #program_type

        #module

        #[allow(unexpected_cfgs)]
        #[cfg(any(
            not(any(target_arch = "bpf", target_os = "solana")),
            feature = "alloc"
        ))]
        extern crate alloc;

        #[allow(unexpected_cfgs)]
        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        pub use #mod_name::cpi;

        #[allow(unexpected_cfgs)]
        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
            #krate::abort_program()
        }

        #[allow(unexpected_cfgs)]
        #[cfg(feature = "alloc")]
        #krate::heap_alloc!();

        #[allow(unexpected_cfgs)]
        #[cfg(not(feature = "alloc"))]
        #krate::no_alloc!();

        #idl
    }
}
