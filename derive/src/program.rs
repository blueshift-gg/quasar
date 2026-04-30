//! `#[program]` — generates the program entrypoint, instruction dispatch table,
//! and CPI method stubs. Scans all `#[instruction]` functions within the module
//! to build the discriminator → handler routing.

use {
    crate::helpers::{
        classify_borrowed_as_compact, classify_lifetime_arg, classify_pod_dynamic,
        extract_generic_inner_type, parse_discriminator_bytes, pascal_to_snake,
        prefix_bytes_to_rust_type, snake_to_pascal, InstructionArgs, PodDynField,
    },
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    syn::{parse_macro_input, FnArg, Ident, Item, ItemMod, LitInt, Pat, Type},
};

/// Emit the heap cursor init or poison block for a dispatch arm.
///
/// - `heap = true`: reset cursor to start (this endpoint uses the allocator)
/// - `heap = false, any_heap = true`: poison cursor in release, reset in debug
///   (this endpoint must NOT allocate — trap accidental allocations)
/// - `any_heap = false`: no-op (global heap init in entrypoint handles it)
fn emit_heap_cursor_block(heap: bool, any_heap: bool) -> TokenStream2 {
    if heap {
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
    } else if any_heap {
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
    } else {
        quote! {}
    }
}

/// Parse `#[max(N)]` or `#[max(N, pfx = P)]` from a function parameter's
/// attributes. Used by the `#[program]` macro to map borrowed args to
/// compact client types.
fn parse_max_attr_from_program_arg(pt: &syn::PatType) -> Option<(usize, usize)> {
    for attr in &pt.attrs {
        if attr.path().is_ident("max") {
            let parsed = attr.parse_args_with(|stream: syn::parse::ParseStream| {
                let n: LitInt = stream.parse()?;
                let max_n: usize = n.base10_parse()?;
                let mut pfx = 0usize;
                if !stream.is_empty() {
                    let _: syn::Token![,] = stream.parse()?;
                    let key: Ident = stream.parse()?;
                    if key != "pfx" {
                        return Err(syn::Error::new(key.span(), "expected `pfx`"));
                    }
                    let _: syn::Token![=] = stream.parse()?;
                    let p: LitInt = stream.parse()?;
                    pfx = p.base10_parse()?;
                }
                Ok((max_n, pfx))
            });
            return parsed.ok();
        }
    }
    None
}

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

struct ClientArgSpec {
    name: Ident,
    ty: Type,
}

/// Lightweight spec for `#[instruction(raw)]` — only discriminator + heap flag.
/// Raw instructions have no accounts type, no client args, no remaining.
struct RawInstructionSpec {
    fn_name: Ident,
    disc_bytes: Vec<LitInt>,
    disc_values: Vec<u8>,
    heap: bool,
}

struct InstructionSpec {
    fn_name: Ident,
    disc_bytes: Vec<LitInt>,
    disc_values: Vec<u8>,
    accounts_type: TokenStream2,
    heap: bool,
    client_struct_name: Ident,
    client_macro_ident: Ident,
    client_args: Vec<ClientArgSpec>,
    has_remaining: bool,
}

impl InstructionSpec {
    fn dispatch_arm(&self) -> TokenStream2 {
        let fn_name = &self.fn_name;
        let accounts_type = &self.accounts_type;
        let disc_bytes = &self.disc_bytes;
        quote! {
            [#(#disc_bytes),*] => #fn_name(#accounts_type)
        }
    }

    fn heap_dispatch_arm(&self, any_heap: bool) -> TokenStream2 {
        let cursor_init = emit_heap_cursor_block(self.heap, any_heap);
        let fn_name = &self.fn_name;
        let accounts_type = &self.accounts_type;
        let disc_bytes = &self.disc_bytes;
        quote! {
            [#(#disc_bytes),*] => { #cursor_init } => #fn_name(#accounts_type)
        }
    }

    fn client_item(&self) -> TokenStream2 {
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
        quote! {
            #macro_ident!(#struct_name, [#(#disc_values),*], {#(#arg_names : #arg_types),*} #remaining_arg);
        }
    }
}

/// Parsed attributes from `#[program(...)]`.
struct ProgramArgs {
    no_entrypoint: bool,
}

impl syn::parse::Parse for ProgramArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut no_entrypoint = false;
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            if ident == "no_entrypoint" {
                no_entrypoint = true;
            } else {
                return Err(syn::Error::new(ident.span(), "expected `no_entrypoint`"));
            }
            let _ = input.parse::<Option<syn::Token![,]>>();
        }
        Ok(Self { no_entrypoint })
    }
}

pub(crate) fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    let program_args = parse_macro_input!(attr as ProgramArgs);
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
    let mut instruction_specs = Vec::new();
    let mut raw_instruction_specs: Vec<RawInstructionSpec> = Vec::new();
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

                    if args.raw {
                        // Raw instruction — no CtxKind, no accounts_type, no client args.
                        raw_instruction_specs.push(RawInstructionSpec {
                            fn_name: fn_name.clone(),
                            disc_bytes: disc_bytes.clone(),
                            disc_values: disc_values.clone(),
                            heap: args.heap,
                        });
                        break;
                    }

                    let ctx_kind = match CtxKind::classify(&func.sig) {
                        Ok(k) => k,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    let inner_ty = ctx_kind.inner_ty();
                    let accounts_type = quote!(#inner_ty);

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
                        let ty = if classify_lifetime_arg(&pt.ty) {
                            // Borrowed struct (has lifetime param) — the off-chain client
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
                        } else if matches!(&*pt.ty, Type::Reference(_)) {
                            // Borrowed arg (&str, &[T]) — parse #[max(N)] and map
                            // to compact client type, same wire format as String<N>/Vec<T,N>.
                            let (max_n, pfx) =
                                parse_max_attr_from_program_arg(pt).unwrap_or((0, 0));
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
                                // Unsupported borrowed type — pass through; #[instruction]
                                // will emit the real error.
                                (*pt.ty).clone()
                            }
                        } else {
                            (*pt.ty).clone()
                        };
                        remaining_args.push((name, ty));
                    }

                    let client_args = remaining_args
                        .into_iter()
                        .map(|(name, ty)| ClientArgSpec { name, ty })
                        .collect();

                    instruction_specs.push(InstructionSpec {
                        fn_name: fn_name.clone(),
                        disc_bytes: disc_bytes.clone(),
                        disc_values,
                        accounts_type,
                        heap: args.heap,
                        client_struct_name: struct_name,
                        client_macro_ident: macro_ident,
                        client_args,
                        has_remaining: ctx_kind.has_remaining(),
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

    let dispatch_arms: Vec<TokenStream2> = instruction_specs
        .iter()
        .map(InstructionSpec::dispatch_arm)
        .collect();

    let client_items: Vec<TokenStream2> = instruction_specs
        .iter()
        .map(InstructionSpec::client_item)
        .collect();

    let any_heap = instruction_specs.iter().any(|spec| spec.heap)
        || raw_instruction_specs.iter().any(|spec| spec.heap);

    // ── Raw dispatch early-return block ──────────────────────────
    // Each raw instruction gets a match arm that walks the SVM buffer
    // to build a `Context` with all accounts, then calls the handler
    // directly. This block is inserted in __dispatch before the
    // normal dispatch! call.
    let raw_dispatch_block = if raw_instruction_specs.is_empty() {
        quote! {}
    } else {
        // Sort raw specs by discriminator value for table construction.
        let mut sorted_raw: Vec<&RawInstructionSpec> = raw_instruction_specs.iter().collect();
        sorted_raw.sort_by_key(|spec| spec.disc_values.clone());

        // For 1-byte discriminators with contiguous values: use O(1) function
        // pointer table dispatch. The SVM verifier accepts callx (indirect
        // calls) — verified by the callx_dispatch integration test.
        //
        // Contiguity check: sorted disc values must form a gap-free sequence.
        // If not (e.g. raw discs 1,2,5 with normal at 3,4), fall back to match.
        let is_contiguous = sorted_raw.len() == 1
            || sorted_raw
                .windows(2)
                .all(|w| w[1].disc_values[0] == w[0].disc_values[0] + 1);
        let use_table = disc_len_lit == 1 && is_contiguous && !sorted_raw.is_empty();

        if use_table {
            let raw_min = sorted_raw.first().unwrap().disc_values[0];
            let raw_max = sorted_raw.last().unwrap().disc_values[0];
            let table_size = (raw_max - raw_min + 1) as usize;

            // Build the function pointer table. Slots are filled for each
            // raw disc value. Gaps (if any non-raw instruction sits between
            // raw ones) should not exist — the disc collision check prevents it.
            let raw_fn_names: Vec<&Ident> = sorted_raw.iter().map(|s| &s.fn_name).collect();
            let table_size_lit = table_size;
            let raw_min_lit = raw_min;

            // Heap: init once before dispatch if any raw instruction uses heap.
            let heap_init = emit_heap_cursor_block(sorted_raw.iter().any(|s| s.heap), any_heap);

            quote! {
                if instruction_data.len() >= 1 {
                    let __raw_disc_byte = instruction_data[0];
                    let __raw_idx = __raw_disc_byte.wrapping_sub(#raw_min_lit) as usize;
                    if __raw_idx < #table_size_lit {
                        #heap_init

                        // SAFETY: Program ID follows instruction data in the SVM buffer.
                        let __raw_program_id: &[u8; 32] = unsafe {
                            &*(instruction_data.as_ptr().add(instruction_data.len())
                                as *const [u8; 32])
                        };
                        const __RAW_U64: usize = core::mem::size_of::<u64>();
                        let __raw_num_accounts = unsafe { *(ptr as *const u64) };
                        let __raw_accounts_start = unsafe { (ptr as *mut u8).add(__RAW_U64) };
                        let __raw_boundary = unsafe { instruction_data.as_ptr().sub(__RAW_U64) };

                        const __RAW_MAX: usize = 64;
                        let __raw_count = core::cmp::min(__raw_num_accounts as usize, __RAW_MAX);
                        let mut __raw_buf = core::mem::MaybeUninit::<
                            [quasar_lang::__internal::AccountView; __RAW_MAX]
                        >::uninit();

                        let (__raw_parsed, __raw_remaining) = unsafe {
                            quasar_lang::__internal::parse_all_accounts_unchecked(
                                __raw_accounts_start,
                                __raw_buf.as_mut_ptr()
                                    as *mut quasar_lang::__internal::AccountView,
                                __raw_count,
                                __raw_boundary,
                            )?
                        };

                        let __raw_accounts = unsafe {
                            core::slice::from_raw_parts_mut(
                                __raw_buf.as_mut_ptr()
                                    as *mut quasar_lang::__internal::AccountView,
                                __raw_parsed,
                            )
                        };

                        let __raw_ctx = Context {
                            program_id: __raw_program_id,
                            accounts: __raw_accounts,
                            remaining_ptr: __raw_remaining,
                            data: &instruction_data[1..],
                            accounts_boundary: __raw_boundary as *const u8,
                        };

                        // O(1) dispatch: function pointer table indexed by
                        // discriminator byte. LLVM emits `callx` for the
                        // indirect call — ~5 CU constant overhead.
                        type __RawHandler = fn(Context) -> Result<(), ProgramError>;
                        let __raw_table: [__RawHandler; #table_size_lit] = [
                            #(#raw_fn_names),*
                        ];
                        return __raw_table[__raw_idx](__raw_ctx);
                    }
                }
            }
        } else {
            // Multi-byte discriminators: fall back to match chain.
            let raw_disc_patterns: Vec<TokenStream2> = raw_instruction_specs
                .iter()
                .map(|spec| {
                    let disc_bytes = &spec.disc_bytes;
                    quote! { [#(#disc_bytes),*] }
                })
                .collect();

            let raw_call_arms: Vec<TokenStream2> = raw_instruction_specs
                .iter()
                .map(|spec| {
                    let fn_name = &spec.fn_name;
                    let disc_bytes = &spec.disc_bytes;
                    let cursor_init = emit_heap_cursor_block(spec.heap, any_heap);
                    quote! {
                        [#(#disc_bytes),*] => {
                            #cursor_init
                            return #fn_name(__raw_ctx);
                        }
                    }
                })
                .collect();

            quote! {
                if instruction_data.len() >= #disc_len_lit {
                    let __raw_disc: [u8; #disc_len_lit] = unsafe {
                        *(instruction_data.as_ptr() as *const [u8; #disc_len_lit])
                    };

                    if matches!(__raw_disc, #(#raw_disc_patterns)|*) {
                        let __raw_program_id: &[u8; 32] = unsafe {
                            &*(instruction_data.as_ptr().add(instruction_data.len())
                                as *const [u8; 32])
                        };
                        const __RAW_U64: usize = core::mem::size_of::<u64>();
                        let __raw_num_accounts = unsafe { *(ptr as *const u64) };
                        let __raw_accounts_start = unsafe { (ptr as *mut u8).add(__RAW_U64) };
                        let __raw_boundary = unsafe { instruction_data.as_ptr().sub(__RAW_U64) };

                        const __RAW_MAX: usize = 64;
                        let __raw_count = core::cmp::min(__raw_num_accounts as usize, __RAW_MAX);
                        let mut __raw_buf = core::mem::MaybeUninit::<
                            [quasar_lang::__internal::AccountView; __RAW_MAX]
                        >::uninit();

                        let (__raw_parsed, __raw_remaining) = unsafe {
                            quasar_lang::__internal::parse_all_accounts_unchecked(
                                __raw_accounts_start,
                                __raw_buf.as_mut_ptr()
                                    as *mut quasar_lang::__internal::AccountView,
                                __raw_count,
                                __raw_boundary,
                            )?
                        };

                        let __raw_accounts = unsafe {
                            core::slice::from_raw_parts_mut(
                                __raw_buf.as_mut_ptr()
                                    as *mut quasar_lang::__internal::AccountView,
                                __raw_parsed,
                            )
                        };

                        let __raw_ctx = Context {
                            program_id: __raw_program_id,
                            accounts: __raw_accounts,
                            remaining_ptr: __raw_remaining,
                            data: &instruction_data[#disc_len_lit..],
                            accounts_boundary: __raw_boundary as *const u8,
                        };

                        match __raw_disc {
                            #(#raw_call_arms)*
                            _ => unsafe { core::hint::unreachable_unchecked() }
                        }
                    }
                }
            }
        }
    };

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

        // Normal dispatch tail: either dispatch! with heap arms, dispatch!
        // with simple arms, or just InvalidInstructionData if all instructions
        // are raw.
        let normal_dispatch_tail = if instruction_specs.is_empty() {
            // All instructions are raw — no normal dispatch needed.
            quote! { Err(ProgramError::InvalidInstructionData) }
        } else if any_heap {
            let heap_dispatch_arms: Vec<proc_macro2::TokenStream> = instruction_specs
                .iter()
                .map(|spec| spec.heap_dispatch_arm(any_heap))
                .collect();
            quote! {
                dispatch!(ptr, instruction_data, #disc_len_lit, {
                    #(#heap_dispatch_arms),*
                })
            }
        } else {
            quote! {
                dispatch!(ptr, instruction_data, #disc_len_lit, {
                    #(#dispatch_arms),*
                })
            }
        };

        // When no_entrypoint is set, __dispatch is pub so users can call
        // it from a custom entrypoint. Otherwise it stays module-private.
        let dispatch_vis = if program_args.no_entrypoint {
            quote! { pub }
        } else {
            quote! {}
        };

        let dispatch_heap_init = emit_heap_cursor_block(true, true);

        if any_heap {
            items.push(syn::parse_quote! {
                #[inline(always)]
                #dispatch_vis fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                    #dispatch_heap_init

                    if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                        return __handle_event(ptr, instruction_data);
                    }

                    #raw_dispatch_block

                    #normal_dispatch_tail
                }
            });
        } else {
            items.push(syn::parse_quote! {
                #[inline(always)]
                #dispatch_vis fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                    if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                        return __handle_event(ptr, instruction_data);
                    }

                    #raw_dispatch_block

                    #normal_dispatch_tail
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

        // When no_entrypoint is set, skip the generated entrypoint so the
        // user can write their own extern "C" fn entrypoint that calls
        // module::__dispatch() for fallthrough (Anchor 0.30 pattern).
        if !program_args.no_entrypoint {
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
        }

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

        impl quasar_lang::account_load::AccountLoad for EventAuthority {
            type BehaviorTarget = Self;

            #[inline(always)]
            fn check(
                view: &AccountView,
                _field_name: &str,
            ) -> Result<(), ProgramError> {
                if !quasar_lang::keys_eq(view.address(), &Self::ADDRESS) {
                    return Err(ProgramError::InvalidSeeds);
                }
                Ok(())
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
