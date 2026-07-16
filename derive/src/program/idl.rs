//! IDL instruction fragments + the `__quasar_build_idl` assembler.
//!
//! Under `feature = "idl-build"` each instruction (normal and raw) submits an
//! `InstructionFragment` into the crate-wide inventory; `build_idl` joins them
//! with the account metadata registered by `#[derive(Accounts)]`.

use {
    super::model::ProgramModel, proc_macro2::TokenStream as TokenStream2, quote::quote, syn::Ident,
};

/// Emit the per-instruction IDL fragments and the `__quasar_build_idl` fn.
pub(super) fn emit_idl(model: &ProgramModel, mod_name: &Ident) -> TokenStream2 {
    let krate = crate::krate::lang_path();
    let idl_instruction_fragments: Vec<TokenStream2> = model
        .instruction_specs
        .iter()
        .map(|spec| {
            let fn_name_str = spec.fn_name.to_string();
            let disc_values = &spec.disc_values;
            let discriminator_source = spec.discriminator_source.idl_tokens();
            let accounts_type_str = &spec.accounts_type_str;
            let ix_docs = crate::helpers::docs_tokens_from_lines(&spec.docs);
            let arg_defs: Vec<TokenStream2> = spec.idl_args.iter().map(|arg| {
                let arg_name = arg.name.to_string();
                let idl_type_tokens = crate::idl::type_to_idl_type_tokens(&arg.ty);
                let codec_tokens = crate::idl::type_to_idl_codec_tokens(&arg.ty);
                quote! {
                    #krate::idl_build::__reexport::IdlArg {
                        name: #krate::idl_build::s(#arg_name),
                        ty: #idl_type_tokens,
                        codec: #codec_tokens,
                        docs: #krate::idl_build::Vec::new(),
                    }
                }
            }).collect();

            // No args -> no layout; otherwise project the ordered args (with the
            // same compact classification that drives the wire schema) into the
            // inline/tail split via the single-source projector.
            let layout_tokens = if spec.idl_args.is_empty() {
                quote! { None }
            } else {
                let layout_fields: Vec<(String, bool)> = spec
                    .idl_args
                    .iter()
                    .map(|arg| {
                        (
                            arg.name.to_string(),
                            crate::helpers::instruction_arg_is_compact(&arg.ty),
                        )
                    })
                    .collect();
                crate::idl::project_idl_layout(&layout_fields)
            };

            let remaining_tokens = if let (Some(item), Some(max)) =
                (&spec.remaining_item, &spec.remaining_max)
            {
                let signer = if crate::helpers::last_type_segment_name(item) == "Signer" {
                    quote! {
                        #krate::idl_build::__reexport::AccountFlag::Fixed(true)
                    }
                } else {
                    quote! {
                        #krate::idl_build::__reexport::AccountFlag::Dynamic(
                            #krate::idl_build::__reexport::AccountFlagDynamic::Input,
                        )
                    }
                };
                quote! {
                    Some(#krate::idl_build::__reexport::IdlRemainingAccounts {
                        kind: #krate::idl_build::__reexport::RemainingAccountsKind::Append,
                        name: #krate::idl_build::s("remainingAccounts"),
                        min: 0,
                        max: Some(
                            ((#max) as usize)
                                .checked_mul(
                                    <#item as #krate::remaining::RemainingItem<'static>>::COUNT,
                                )
                                .expect("typed remaining-account IDL maximum overflows usize"),
                        ),
                        item: #krate::idl_build::__reexport::RemainingAccountItem {
                            client_type: #krate::idl_build::s("accountMeta"),
                            signer: #signer,
                            writable: #krate::idl_build::__reexport::AccountFlag::Dynamic(
                                #krate::idl_build::__reexport::AccountFlagDynamic::Input,
                            ),
                        },
                        policy: #krate::idl_build::__reexport::RemainingAccountPolicy {
                            position: #krate::idl_build::__reexport::RemainingPosition::AfterDeclaredAccounts,
                            order: #krate::idl_build::__reexport::RemainingOrder::PreserveInput,
                        },
                    })
                }
            } else if spec.has_remaining {
                quote! {
                    Some(#krate::idl_build::__reexport::IdlRemainingAccounts {
                        kind: #krate::idl_build::__reexport::RemainingAccountsKind::Append,
                        name: #krate::idl_build::s("remainingAccounts"),
                        min: 0,
                        max: None,
                        item: #krate::idl_build::__reexport::RemainingAccountItem {
                            client_type: #krate::idl_build::s("accountMeta"),
                            signer: #krate::idl_build::__reexport::AccountFlag::Dynamic(
                                #krate::idl_build::__reexport::AccountFlagDynamic::Input,
                            ),
                            writable: #krate::idl_build::__reexport::AccountFlag::Dynamic(
                                #krate::idl_build::__reexport::AccountFlagDynamic::Input,
                            ),
                        },
                        policy: #krate::idl_build::__reexport::RemainingAccountPolicy {
                            position: #krate::idl_build::__reexport::RemainingPosition::AfterDeclaredAccounts,
                            order: #krate::idl_build::__reexport::RemainingOrder::PreserveInput,
                        },
                    })
                }
            } else {
                quote! { None }
            };

            quote! {
                #[cfg(feature = "idl-build")]
                #krate::__private_inventory::submit! {
                    #krate::idl_build::InstructionFragment {
                        build: {
                            fn __build() -> #krate::idl_build::__reexport::IdlInstruction {
                                #krate::idl_build::__reexport::IdlInstruction {
                                    name: #krate::idl_build::s(#fn_name_str),
                                    discriminator: #krate::idl_build::vec![#(#disc_values),*],
                                    docs: #ix_docs,
                                    accounts: #krate::idl_build::Vec::new(),
                                    args: #krate::idl_build::vec![#(#arg_defs),*],
                                    layout: #layout_tokens,
                                    remaining_accounts: #remaining_tokens,
                                }
                            }
                            __build
                        },
                        accounts_struct_name: #accounts_type_str,
                        discriminator_source: #discriminator_source,
                    }
                }
            }
        })
        .collect();

    let idl_raw_instruction_fragments: Vec<TokenStream2> = model
        .raw_specs
        .iter()
        .map(|spec| {
            let fn_name_str = spec.fn_name.to_string();
            let disc_values = &spec.disc_values;
            let discriminator_source = spec.discriminator_source.idl_tokens();
            let ix_docs = crate::helpers::docs_tokens_from_lines(&spec.docs);
            quote! {
                #[cfg(feature = "idl-build")]
                #krate::__private_inventory::submit! {
                    #krate::idl_build::InstructionFragment {
                        build: {
                            fn __build() -> #krate::idl_build::__reexport::IdlInstruction {
                                #krate::idl_build::__reexport::IdlInstruction {
                                    name: #krate::idl_build::s(#fn_name_str),
                                    discriminator: #krate::idl_build::vec![#(#disc_values),*],
                                    docs: #ix_docs,
                                    accounts: #krate::idl_build::Vec::new(),
                                    args: #krate::idl_build::Vec::new(),
                                    layout: None,
                                    remaining_accounts: None,
                                }
                            }
                            __build
                        },
                        accounts_struct_name: "",
                        discriminator_source: #discriminator_source,
                    }
                }
            }
        })
        .collect();

    let idl_build_fn = {
        let mod_name_str = mod_name.to_string();
        quote! {
            /// Assemble all IDL fragments and return JSON.
            #[cfg(feature = "idl-build")]
            pub fn __quasar_build_idl() -> #krate::idl_build::String {
                let address = #krate::idl_build::address_to_base58(&crate::ID);
                let idl = #krate::idl_build::build_idl(
                    &address,
                    #mod_name_str,
                    env!("CARGO_PKG_NAME"),
                    env!("CARGO_PKG_VERSION"),
                );
                #krate::idl_build::__reexport::serde_json::to_string_pretty(&idl)
                    .expect("generated IDL should serialize")
            }

            #[allow(unexpected_cfgs)]
            #[cfg(all(feature = "idl-build", test, not(any(target_os = "solana", target_arch = "bpf"))))]
            #[test]
            fn __quasar_emit_idl() {
                extern crate std;
                std::println!("__QUASAR_IDL_JSON_BEGIN__");
                std::println!("{}", __quasar_build_idl());
                std::println!("__QUASAR_IDL_JSON_END__");
            }
        }
    };

    quote! {
        #(#idl_instruction_fragments)*
        #(#idl_raw_instruction_fragments)*
        #idl_build_fn
    }
}
