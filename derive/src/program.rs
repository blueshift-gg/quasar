//! `#[program]` — generates the program entrypoint, instruction dispatch table,
//! and CPI method stubs. Scans all `#[instruction]` functions within the module
//! to build the discriminator → handler routing.

use {
    crate::helpers::{
        classify_pod_string, classify_pod_vec, extract_generic_inner_type,
        parse_discriminator_bytes, pascal_to_snake, snake_to_pascal, InstructionArgs,
    },
    proc_macro::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_macro_input, FnArg, Ident, Item, ItemMod, Pat, Type},
};

/// Context wrapper kind, classified once per instruction function.
enum CtxKind<'a> {
    Ctx { inner_ty: &'a Type },
    CtxWithRemaining { inner_ty: &'a Type },
}

impl<'a> CtxKind<'a> {
    /// Classify the first parameter of an instruction function.
    fn classify(sig: &'a syn::Signature) -> syn::Result<Self> {
        let first_arg = match sig.inputs.first() {
            Some(FnArg::Typed(pt)) => pt,
            _ => {
                return Err(syn::Error::new_spanned(
                    &sig.ident,
                    "#[program]: instruction function must have ctx: Ctx<T> as first parameter",
                ));
            }
        };

        if let Some(inner) = extract_generic_inner_type(&first_arg.ty, "Ctx") {
            return Ok(CtxKind::Ctx { inner_ty: inner });
        }
        if let Some(inner) = extract_generic_inner_type(&first_arg.ty, "CtxWithRemaining") {
            return Ok(CtxKind::CtxWithRemaining { inner_ty: inner });
        }

        Err(syn::Error::new_spanned(
            &first_arg.ty,
            "first parameter must be Ctx<T> or CtxWithRemaining<T>",
        ))
    }

    fn inner_ty(&self) -> &'a Type {
        match self {
            CtxKind::Ctx { inner_ty } | CtxKind::CtxWithRemaining { inner_ty } => inner_ty,
        }
    }

    fn has_remaining(&self) -> bool {
        matches!(self, CtxKind::CtxWithRemaining { .. })
    }
}

pub(crate) fn program(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut module = parse_macro_input!(item as ItemMod);
    let mod_name = module.ident.clone();
    let program_type_name = format_ident!("{}", snake_to_pascal(&mod_name.to_string()));

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
    let mut heap_flags: Vec<bool> = Vec::new();
    // Per-instruction data for inline dispatch generation (used when any_heap)
    let mut arm_disc_tokens: Vec<Vec<proc_macro2::TokenStream>> = Vec::new();
    let mut arm_fn_names: Vec<Ident> = Vec::new();
    let mut arm_accounts_types: Vec<proc_macro2::TokenStream> = Vec::new();

    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("instruction") {
                    let args: InstructionArgs = match attr.parse_args() {
                        Ok(a) => a,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    let disc_bytes = match &args.discriminator {
                        Some(d) => d,
                        None => {
                            return syn::Error::new_spanned(
                                attr,
                                "#[program]: instruction requires `discriminator = [...]`",
                            )
                            .to_compile_error()
                            .into();
                        }
                    };
                    let fn_name = &func.sig.ident;
                    let ctx_kind = match CtxKind::classify(&func.sig) {
                        Ok(k) => k,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    let inner_ty = ctx_kind.inner_ty();
                    let accounts_type = quote!(#inner_ty);

                    // Validate same length across all instructions
                    match disc_len {
                        Some(len) => {
                            if disc_bytes.len() != len {
                                return syn::Error::new_spanned(
                                    attr,
                                    format!(
                                        "all instruction discriminators must have the same \
                                         length: expected {} byte(s), found {}",
                                        len,
                                        disc_bytes.len()
                                    ),
                                )
                                .to_compile_error()
                                .into();
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
                    heap_flags.push(args.heap);
                    arm_disc_tokens.push(disc_bytes.iter().map(|b| quote!(#b)).collect());
                    arm_fn_names.push(fn_name.clone());
                    arm_accounts_types.push(accounts_type.clone());

                    // Collect data for client module generation — invoke the macro_rules
                    // bridge emitted by derive(Accounts)
                    let struct_name =
                        format_ident!("{}Instruction", snake_to_pascal(&fn_name.to_string()));
                    let accounts_type_str = accounts_type.to_string().replace(' ', "");
                    let macro_ident =
                        format_ident!("__{}_instruction", pascal_to_snake(&accounts_type_str));

                    let mut remaining_args: Vec<(Ident, Type)> = Vec::new();
                    for arg in func.sig.inputs.iter().skip(1) {
                        let FnArg::Typed(pt) = arg else {
                            continue;
                        };
                        let name = match &*pt.pat {
                            Pat::Ident(pi) => pi.ident.clone(),
                            _ => continue,
                        };
                        let ty = if classify_pod_string(&pt.ty).is_some() {
                            // String<N> / PodString<N> → DynBytes<u8> (u8 prefix)
                            syn::parse_quote!(quasar_lang::client::DynBytes<u8>)
                        } else if let Some((elem, _max)) = classify_pod_vec(&pt.ty) {
                            // Vec<T, N> / PodVec<T, N> → DynVec<T, u16> (u16 prefix)
                            syn::parse_quote!(quasar_lang::client::DynVec<#elem, u16>)
                        } else {
                            (*pt.ty).clone()
                        };
                        remaining_args.push((name, ty));
                    }

                    let arg_names: Vec<&Ident> = remaining_args.iter().map(|(n, _)| n).collect();
                    let arg_types: Vec<&Type> = remaining_args.iter().map(|(_, t)| t).collect();

                    let remaining_arg = if ctx_kind.has_remaining() {
                        quote!(, remaining)
                    } else {
                        quote!()
                    };
                    client_items.push(quote! {
                        #macro_ident!(#struct_name, [#(#disc_values),*], {#(#arg_names : #arg_types),*} #remaining_arg);
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
                "instruction `{}` has a discriminator starting with 0xFF which is reserved for \
                 events",
                fn_name
            ),
        )
        .to_compile_error()
        .into();
    }

    let any_heap = heap_flags.iter().any(|&h| h);

    // Append dispatch + entrypoint to the module
    if let Some((_, ref mut items)) = module.content {
        items.push(syn::parse_quote! {
            #[inline(always)]
            fn __handle_event(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                // SAFETY: `ptr` is the SVM input buffer from the entrypoint.
                unsafe {
                    quasar_lang::event::handle_event(
                        ptr,
                        instruction_data,
                        &super::EventAuthority::ADDRESS,
                    )
                }
            }
        });

        if any_heap {
            // When any instruction opts into heap, build per-arm cursor init
            // blocks and feed them into the extended dispatch! macro form.
            // Heap endpoints get normal cursor init; non-heap endpoints poison
            // the cursor (clean alloc trap). Debug builds exempt non-heap
            // endpoints so alloc::format! works in error paths.
            let heap_dispatch_arms: Vec<proc_macro2::TokenStream> = arm_disc_tokens
                .iter()
                .zip(arm_fn_names.iter())
                .zip(arm_accounts_types.iter())
                .zip(heap_flags.iter())
                .map(|(((disc_toks, fn_name), accounts_type), &is_heap)| {
                    let cursor_init = if is_heap {
                        quote! {
                            #[cfg(feature = "alloc")]
                            {
                                unsafe {
                                    let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                                    *(heap_start as *mut usize) =
                                        heap_start + core::mem::size_of::<usize>();
                                }
                            }
                        }
                    } else {
                        quote! {
                            #[cfg(feature = "alloc")]
                            {
                                #[cfg(feature = "debug")]
                                unsafe {
                                    let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                                    *(heap_start as *mut usize) =
                                        heap_start + core::mem::size_of::<usize>();
                                }
                                #[cfg(not(feature = "debug"))]
                                unsafe {
                                    *(super::allocator::HEAP_START_ADDRESS as *mut usize) =
                                        super::allocator::HEAP_CURSOR_POISONED;
                                }
                            }
                        }
                    };
                    quote! {
                        [#(#disc_toks),*] => { #cursor_init } => #fn_name(#accounts_type)
                    }
                })
                .collect();

            items.push(syn::parse_quote! {
                #[inline(always)]
                fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                    // Initialize cursor to a valid state before the event check.
                    // __handle_event itself is allocation-free, but this prevents
                    // UB if it ever changes or if debug error paths allocate.
                    #[cfg(feature = "alloc")]
                    unsafe {
                        let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                        *(heap_start as *mut usize) =
                            heap_start + core::mem::size_of::<usize>();
                    }

                    if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                        return __handle_event(ptr, instruction_data);
                    }
                    // Per-arm cursor init overrides the safe default above.
                    dispatch!(ptr, instruction_data, #disc_len_lit, {
                        #(#heap_dispatch_arms),*
                    })
                }
            });
        } else {
            // No heap annotations — use the simple dispatch! macro form
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
        }

        // When per-endpoint heap is used, cursor init is in the dispatch
        // arms — the entrypoint does NOT init the cursor. Otherwise, init
        // the cursor once in the entrypoint.
        let cursor_init = if any_heap {
            quote! {}
        } else {
            quote! {
                #[cfg(feature = "alloc")]
                {
                    let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                    *(heap_start as *mut usize) = heap_start + core::mem::size_of::<usize>();
                }
            }
        };

        items.push(syn::parse_quote! {
            #[unsafe(no_mangle)]
            #[cfg(any(target_os = "solana", target_arch = "bpf"))]
            #[allow(unexpected_cfgs)]
            pub unsafe extern "C" fn entrypoint(ptr: *mut u8, instruction_data: *const u8) -> u64 {
                #cursor_init
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

        // Add CPI module inside the program module (instruction builders only —
        // the full client with account/event types is generated by the IDL).
        let cpi_mod: syn::Item = syn::parse2(quote! {
            #[cfg(not(any(target_arch = "bpf", target_os = "solana")))]
            pub mod cpi {
                use super::*;

                #(#client_items)*
            }
        })
        .unwrap_or_else(|e| syn::Item::Verbatim(e.to_compile_error()));
        items.push(cpi_mod);
    }

    // Generate the named program type outside the module
    let program_type = quote! {
        quasar_lang::define_account!(pub struct #program_type_name => [quasar_lang::checks::Executable, quasar_lang::checks::Address]);

        impl quasar_lang::traits::Id for #program_type_name {
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
            const __PDA: (Address, u8) = quasar_lang::pda::find_program_address_const(
                &[b"__event_authority"],
                &crate::ID,
            );
            pub const ADDRESS: Address = Self::__PDA.0;
            pub const BUMP: u8 = Self::__PDA.1;

            #[inline(always)]
            pub fn from_account_view(view: &AccountView) -> Result<&Self, ProgramError> {
                if !quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
                    return Err(ProgramError::InvalidSeeds);
                }
                Ok(unsafe { &*(view as *const AccountView as *const Self) })
            }

            /// Construct without validation.
            ///
            /// # Safety
            /// Caller must ensure account address matches the expected PDA.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
                &*(view as *const AccountView as *const Self)
            }
        }

        impl #program_type_name {
            #[inline(always)]
            pub fn emit_event<E: quasar_lang::traits::Event>(
                &self,
                event: &E,
                event_authority: &EventAuthority,
            ) -> Result<(), ProgramError> {
                let program = self.to_account_view();
                let ea = event_authority.to_account_view();
                event.emit(|data| {
                    quasar_lang::event::emit_event_cpi(program, ea, data, EventAuthority::BUMP)
                })
            }
        }
    };

    // Suppress dead_code warnings on the user's #[program] module.
    // Instruction handlers and account structs inside it are only referenced
    // from macro-generated dispatch code, which the compiler can't see.
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
        pub use #mod_name::cpi;

        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        #[panic_handler]
        fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
            quasar_lang::abort_program()
        }

        #[allow(unexpected_cfgs)]
        #[cfg(feature = "alloc")]
        quasar_lang::heap_alloc!();

        #[allow(unexpected_cfgs)]
        #[cfg(not(feature = "alloc"))]
        quasar_lang::no_alloc!();
    }
    .into()
}
