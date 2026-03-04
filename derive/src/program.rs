use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, Ident, Item, ItemMod, Pat, Type};

use crate::helpers::{
    extract_generic_inner_type, is_dynamic_string, parse_discriminator_bytes, pascal_to_snake,
    snake_to_pascal, InstructionArgs,
};

/// Extracts the inner type `T` from a `Ctx<T>` or `CtxWithRemaining<T>` first parameter.
fn extract_ctx_inner_type(sig: &syn::Signature) -> syn::Result<proc_macro2::TokenStream> {
    let first_arg = match sig.inputs.first() {
        Some(FnArg::Typed(pt)) => pt,
        _ => {
            return Err(syn::Error::new_spanned(
                &sig.ident,
                "#[program]: instruction function must have ctx: Ctx<T> as first parameter",
            ));
        }
    };

    extract_generic_inner_type(&first_arg.ty, "Ctx")
        .or_else(|| extract_generic_inner_type(&first_arg.ty, "CtxWithRemaining"))
        .map(|ty| Ok(quote!(#ty)))
        .unwrap_or_else(|| {
            Err(syn::Error::new_spanned(
                &first_arg.ty,
                "first parameter must be Ctx<T> or CtxWithRemaining<T>",
            ))
        })
}

pub(crate) fn program(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as ItemMod);
    let mod_name = module.ident.clone();
    let program_type_name = format_ident!("{}Program", snake_to_pascal(&mod_name.to_string()));

    let (_, items) = match module.content.as_ref() {
        Some(content) => content,
        None => {
            return syn::Error::new_spanned(
                &module,
                "#[program] must be used on a module with a body",
            )
            .to_compile_error()
            .into();
        }
    };

    // Scan for #[instruction(discriminator = ...)] functions
    let mut dispatch_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut client_items: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut seen_discriminators: Vec<(Vec<u8>, String)> = Vec::new();
    let mut disc_len: Option<usize> = None;

    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("instruction") {
                    let args: InstructionArgs = match attr.parse_args() {
                        Ok(a) => a,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    let disc_bytes = &args.discriminator;
                    let fn_name = &func.sig.ident;
                    let accounts_type = match extract_ctx_inner_type(&func.sig) {
                        Ok(ty) => ty,
                        Err(e) => return e.to_compile_error().into(),
                    };

                    // Validate same length across all instructions
                    match disc_len {
                        Some(len) => {
                            if disc_bytes.len() != len {
                                return syn::Error::new_spanned(
                                    attr,
                                    format!(
                                        "all instruction discriminators must have the same length: expected {} byte(s), found {}",
                                        len, disc_bytes.len()
                                    ),
                                ).to_compile_error().into();
                            }
                        }
                        None => disc_len = Some(disc_bytes.len()),
                    }

                    // Check for duplicates
                    let disc_values = match parse_discriminator_bytes(disc_bytes) {
                        Ok(v) => v,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    if let Some((_, prev_fn)) =
                        seen_discriminators.iter().find(|(v, _)| *v == disc_values)
                    {
                        return syn::Error::new_spanned(
                            attr,
                            format!(
                                "duplicate discriminator {:?}: already used by `{}`",
                                disc_values, prev_fn
                            ),
                        )
                        .to_compile_error()
                        .into();
                    }
                    seen_discriminators.push((disc_values.clone(), fn_name.to_string()));

                    dispatch_arms.push(quote! {
                        [#(#disc_bytes),*] => #fn_name(#accounts_type)
                    });

                    // Collect data for client module generation — invoke the macro_rules
                    // bridge emitted by derive(Accounts)
                    let struct_name =
                        format_ident!("{}Instruction", snake_to_pascal(&fn_name.to_string()));
                    let accounts_type_str = accounts_type.to_string().replace(' ', "");
                    let macro_ident =
                        format_ident!("__{}_instruction", pascal_to_snake(&accounts_type_str));

                    let remaining_args: Vec<(Ident, Type)> = func
                        .sig
                        .inputs
                        .iter()
                        .skip(1)
                        .filter_map(|arg| match arg {
                            FnArg::Typed(pt) => {
                                let name = match &*pt.pat {
                                    Pat::Ident(pi) => pi.ident.clone(),
                                    _ => return None,
                                };
                                let ty = if is_dynamic_string(&pt.ty, false).is_some() {
                                    syn::parse_quote!(alloc::vec::Vec<u8>)
                                } else {
                                    (*pt.ty).clone()
                                };
                                Some((name, ty))
                            }
                            _ => None,
                        })
                        .collect();

                    let arg_names: Vec<&Ident> = remaining_args.iter().map(|(n, _)| n).collect();
                    let arg_types: Vec<&Type> = remaining_args.iter().map(|(_, t)| t).collect();

                    let disc_byte_lits: Vec<proc_macro2::TokenStream> = disc_values
                        .iter()
                        .map(|b| {
                            let lit = proc_macro2::Literal::u8_unsuffixed(*b);
                            quote! { #lit }
                        })
                        .collect();

                    client_items.push(quote! {
                        #macro_ident!(#struct_name, [#(#disc_byte_lits),*], {#(#arg_names : #arg_types),*});
                    });

                    break;
                }
            }
        }
    }

    let disc_len_lit = disc_len.unwrap_or(1);

    // Check no instruction discriminator starts with 0xFF (reserved for events)
    if let Some((_, fn_name)) = seen_discriminators
        .iter()
        .find(|(v, _)| v.first() == Some(&0xFF))
    {
        return syn::Error::new_spanned(
            &module.ident,
            format!(
                "instruction `{}` has a discriminator starting with 0xFF which is reserved for events",
                fn_name
            ),
        ).to_compile_error().into();
    }

    // Append dispatch + entrypoint to the module
    if let Some((_, ref mut items)) = module.content {
        items.push(syn::parse_quote! {
            fn __handle_event(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                // SAFETY: Pointer arithmetic follows the SVM input buffer layout. The u64 casts
                // for address comparison are technically misaligned (Address is align 1), but SBF
                // handles unaligned access natively — this 4×u64 compare saves ~20 CU vs memcmp.
                unsafe {
                    let raw = ptr.add(core::mem::size_of::<u64>()) as *const quasar_core::__internal::RuntimeAccount;

                    if (*raw).is_signer == 0 {
                        return Err(ProgramError::MissingRequiredSignature);
                    }

                    let addr = &(*raw).address as *const _ as *const u64;
                    let expected = super::EventAuthority::ADDRESS.as_ref().as_ptr() as *const u64;
                    if *addr != *expected
                        || *addr.add(1) != *expected.add(1)
                        || *addr.add(2) != *expected.add(2)
                        || *addr.add(3) != *expected.add(3)
                    {
                        return Err(ProgramError::InvalidSeeds);
                    }
                }

                if instruction_data.len() <= 1 {
                    return Err(ProgramError::InvalidInstructionData);
                }

                quasar_core::log::log_data(&[&instruction_data[1..]]);
                Ok(())
            }
        });

        items.push(syn::parse_quote! {
            #[inline(always)]
            fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                    return __handle_event(ptr, instruction_data);
                }
                dispatch!(ptr, instruction_data, #disc_len_lit, {
                    #(#dispatch_arms),*
                })
            }
        });

        items.push(syn::parse_quote! {
            #[unsafe(no_mangle)]
            #[cfg(target_os = "solana")]
            #[allow(unexpected_cfgs)]
            pub unsafe extern "C" fn entrypoint(ptr: *mut u8, instruction_data: *const u8) -> u64 {
                // SAFETY: Initialize bump allocator cursor. The SVM maps and zero-inits the heap
                // region before execution. This write sets the cursor past the 8-byte cursor slot
                // itself, eliminating the per-allocation zero-check branch in BumpAllocator::alloc.
                // Assumes re-entrancy is forbidden by the SVM — cursor reset on re-entry would
                // alias previous allocations.
                #[cfg(feature = "alloc")]
                {
                    let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                    *(heap_start as *mut usize) = heap_start + core::mem::size_of::<usize>();
                }

                // SAFETY: SVM places instruction data length as u64 at offset -8 from the data
                // pointer. The read is technically misaligned in the abstract machine, but the SVM
                // buffer is 8-byte aligned and SBF handles unaligned access natively.
                let instruction_data = unsafe {
                    core::slice::from_raw_parts(
                        instruction_data,
                        *(instruction_data.sub(8) as *const u64) as usize,
                    )
                };
                match __dispatch(ptr, instruction_data) {
                    Ok(_) => 0,
                    Err(e) => e.into(),
                }
            }
        });

        // Add client module inside the program module
        let client_mod: syn::Item = syn::parse2(quote! {
            #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
            pub mod client {
                use alloc::vec;
                use super::*;

                #(#client_items)*
            }
        })
        .unwrap_or_else(|e| syn::Item::Verbatim(e.to_compile_error()));
        items.push(client_mod);
    }

    // Generate the named program type outside the module
    let program_type = quote! {
        quasar_core::define_account!(pub struct #program_type_name => [quasar_core::checks::Executable, quasar_core::checks::Address]);

        impl quasar_core::traits::Id for #program_type_name {
            const ID: Address = crate::ID;
        }

        #[repr(transparent)]
        pub struct EventAuthority {
            view: AccountView,
        }

        impl AsAccountView for EventAuthority {
            #[inline(always)]
            fn to_account_view(&self) -> &AccountView {
                &self.view
            }
        }

        impl EventAuthority {
            const __PDA: (Address, u8) = quasar_core::pda::find_program_address_const(
                &[b"__event_authority"],
                &crate::ID,
            );
            pub const ADDRESS: Address = Self::__PDA.0;
            pub const BUMP: u8 = Self::__PDA.1;

            #[inline(always)]
            pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
                if *view.address() != Self::ADDRESS {
                    return Err(ProgramError::InvalidSeeds);
                }
                Ok(unsafe { &*(view as *const AccountView as *const Self) })
            }
        }

        impl #program_type_name {
            #[inline(always)]
            pub fn emit_event<E: quasar_core::traits::Event>(
                &self,
                event: &E,
                event_authority: &EventAuthority,
            ) -> Result<(), ProgramError> {
                let program = self.to_account_view();
                let ea = event_authority.to_account_view();
                event.emit(|data| {
                    quasar_core::event::emit_event_cpi(program, ea, data, EventAuthority::BUMP)
                })
            }
        }
    };

    module.attrs.push(syn::parse_quote!(#[allow(dead_code)]));

    quote! {
        #program_type

        #module

        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        extern crate alloc;

        #[allow(unexpected_cfgs)]
        #[cfg(all(any(target_os = "solana", target_arch = "bpf"), feature = "alloc"))]
        extern crate alloc;

        #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
        pub use #mod_name::client;

        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
            quasar_core::prelude::log("PANIC");
            loop {}
        }

        #[allow(unexpected_cfgs)]
        #[cfg(feature = "alloc")]
        quasar_core::heap_alloc!();

        #[allow(unexpected_cfgs)]
        #[cfg(not(feature = "alloc"))]
        quasar_core::no_alloc!();
    }
    .into()
}
