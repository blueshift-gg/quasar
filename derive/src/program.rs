//! `#[program]`: generates the program entrypoint, instruction dispatch table,
//! and CPI method stubs. Scans all `#[instruction]` functions within the module
//! to build the discriminator -> handler routing.

use {
    crate::helpers::{
        classify_borrowed_as_compact, classify_lifetime_arg, classify_option_pod_dynamic,
        classify_pod_dynamic, extract_generic_inner_type, parse_discriminator_bytes,
        parse_max_attr, pascal_to_snake, prefix_bytes_to_rust_type, snake_to_pascal,
        InstructionArgs, PodDynField,
    },
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    std::collections::BTreeSet,
    syn::{parse_macro_input, FnArg, Ident, Item, ItemMod, LitInt, Meta, Pat, Type},
};

/// Emit the heap cursor init or poison block for a dispatch arm.
///
/// - `heap = true`: reset cursor to start (this endpoint uses the allocator)
/// - `heap = false, any_heap = true`: poison cursor in release, reset in debug
///   (this endpoint must NOT allocate: trap accidental allocations)
/// - `any_heap = false`: no-op (global heap init in entrypoint handles it)
fn emit_heap_cursor_block(heap: bool, any_heap: bool) -> TokenStream2 {
    if heap {
        quote! {
            #[cfg(feature = "alloc")]
            {
                unsafe {
                    // SAFETY: generated entrypoint owns the process-local bump allocator cursor.
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
                    // SAFETY: debug poison path resets the generated bump allocator cursor.
                    let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                    *(heap_start as *mut usize) =
                        heap_start + core::mem::size_of::<usize>();
                }
                #[cfg(not(feature = "debug"))]
                unsafe {
                    // SAFETY: release poison path writes the generated allocator cursor slot.
                    *(super::allocator::HEAP_START_ADDRESS as *mut usize) =
                        super::allocator::HEAP_CURSOR_POISONED;
                }
            }
        }
    } else {
        quote! {}
    }
}

fn emit_raw_context_setup(data_start: TokenStream2) -> TokenStream2 {
    quote! {
        let __raw_program_id: &[u8; 32] = unsafe {
            // SAFETY: the Solana entrypoint ABI stores the program id immediately
            // after the instruction data slice.
            &*(instruction_data.as_ptr().add(instruction_data.len()) as *const [u8; 32])
        };
        const __RAW_U64: usize = core::mem::size_of::<u64>();
        let __raw_num_accounts = unsafe {
            // SAFETY: the SVM input buffer starts with the account count.
            *(ptr as *const u64)
        };
        let __raw_accounts_start = unsafe {
            // SAFETY: account records start immediately after the count prefix.
            (ptr as *mut u8).add(__RAW_U64)
        };
        let __raw_boundary = unsafe {
            // SAFETY: the ABI places the instruction-data length prefix directly
            // before `instruction_data`.
            instruction_data.as_ptr().sub(__RAW_U64)
        };

        const __RAW_MAX: usize = 64;
        let __raw_count = core::cmp::min(__raw_num_accounts as usize, __RAW_MAX);
        let mut __raw_buf = core::mem::MaybeUninit::<
            [quasar_lang::__internal::AccountView; __RAW_MAX]
        >::uninit();

        let (__raw_parsed, __raw_remaining) = unsafe {
            // SAFETY: `__raw_buf` has room for `__raw_count` AccountView values,
            // and `__raw_boundary` bounds the account region.
            quasar_lang::__internal::parse_all_accounts_unchecked(
                __raw_accounts_start,
                __raw_buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView,
                __raw_count,
                __raw_boundary,
            )?
        };

        let __raw_accounts = unsafe {
            // SAFETY: `parse_all_accounts_unchecked` initialized the first
            // `__raw_parsed` AccountView slots.
            core::slice::from_raw_parts_mut(
                __raw_buf.as_mut_ptr() as *mut quasar_lang::__internal::AccountView,
                __raw_parsed,
            )
        };

        let __raw_ctx = Context {
            program_id: __raw_program_id,
            accounts: __raw_accounts,
            remaining_ptr: __raw_remaining,
            data: &instruction_data[#data_start..],
            accounts_boundary: __raw_boundary as *const u8,
        };
    }
}

/// Context wrapper classification for an instruction function.
struct CtxKind<'a>(&'a Type, bool);

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
            return Ok(CtxKind(inner, false));
        }
        if let Some(inner) = extract_generic_inner_type(&first_arg.ty, "CtxWithRemaining") {
            return Ok(CtxKind(inner, true));
        }

        Err(syn::Error::new_spanned(
            &first_arg.ty,
            "first parameter must be Ctx<T> or CtxWithRemaining<T>",
        ))
    }

    fn inner_ty(&self) -> &'a Type {
        self.0
    }

    fn has_remaining(&self) -> bool {
        self.1
    }
}

struct ClientArgSpec {
    name: Ident,
    ty: Type,
}

/// Lightweight spec for `#[instruction(raw)]`: only discriminator + heap flag.
/// Raw instructions have no accounts type, no client args, no remaining.
struct RawInstructionSpec {
    fn_name: Ident,
    disc_bytes: Vec<LitInt>,
    disc_values: Vec<u8>,
    discriminator_source: DiscriminatorSource,
    heap: bool,
}

#[derive(Clone, Copy)]
enum DiscriminatorSource {
    Auto,
    Explicit,
}

impl DiscriminatorSource {
    fn idl_tokens(self) -> TokenStream2 {
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

/// Original arg (name + type) as declared in the handler, for IDL emission.
struct IdlArgSpec {
    name: Ident,
    ty: Type,
}

struct InstructionSpec {
    fn_name: Ident,
    disc_bytes: Vec<LitInt>,
    disc_values: Vec<u8>,
    discriminator_source: DiscriminatorSource,
    accounts_type: TokenStream2,
    accounts_type_str: String,
    heap: bool,
    client_struct_name: Ident,
    client_macro_ident: Ident,
    client_args: Vec<ClientArgSpec>,
    idl_args: Vec<IdlArgSpec>,
    has_remaining: bool,
}

impl InstructionSpec {
    fn has_compact_client_layout(&self) -> bool {
        self.idl_args.iter().any(|arg| {
            classify_pod_dynamic(&arg.ty).is_some()
                || classify_option_pod_dynamic(&arg.ty).is_some()
        })
    }

    fn guarded_match_arm(&self, any_heap: bool, disc_len: usize) -> TokenStream2 {
        let cursor_init = emit_heap_cursor_block(self.heap, any_heap);
        let fn_name = &self.fn_name;
        let direct_fn_name = format_ident!("__quasar_direct_{}", fn_name);
        let accounts_type = &self.accounts_type;
        let disc_bytes = &self.disc_bytes;
        let data_after_disc = quote! {
            unsafe {
                // SAFETY: dispatch checks `instruction_data.len() >= disc_len`
                // before matching this arm.
                instruction_data.get_unchecked(#disc_len..)
            }
        };

        let buffered_body = quote! {
            {
                let mut __buf = core::mem::MaybeUninit::<
                    [AccountView; <#accounts_type as AccountCount>::COUNT]
                >::uninit();
                let __remaining_ptr = unsafe {
                    // SAFETY: the account count check above guarantees the
                    // fixed account parser has enough records to read.
                    <#accounts_type>::parse_accounts(
                        __accounts_start,
                        &mut __buf,
                        unsafe {
                            // SAFETY: Address is represented by the same 32-byte
                            // value as the ABI program id.
                            &*(__program_id as *const [u8; 32] as *const quasar_lang::prelude::Address)
                        },
                    )?
                };
                let mut __accounts = unsafe {
                    // SAFETY: `parse_accounts` initialized exactly COUNT slots
                    // before returning `Ok`.
                    __buf.assume_init()
                };
                #fn_name(Context {
                    program_id: __program_id,
                    accounts: &mut __accounts,
                    remaining_ptr: __remaining_ptr,
                    data: #data_after_disc,
                    accounts_boundary: unsafe {
                        // SAFETY: the ABI places the instruction-data length
                        // prefix directly before `instruction_data`.
                        instruction_data.as_ptr().sub(__U64_SIZE)
                    },
                })
            }
        };

        let body = if self.has_remaining {
            buffered_body
        } else {
            quote! {
                // The direct helper removes one generated Context/Ctx::new layer.
                // On small account lists the buffered path is cheaper, so the
                // derive selects the lower-CU shape from the account count.
                if <#accounts_type as AccountCount>::COUNT >= 8usize {
                    #direct_fn_name(
                        __program_id,
                        __accounts_start,
                        #data_after_disc,
                    )
                } else {
                    #buffered_body
                }
            }
        };

        quote! {
            [#(#disc_bytes),*] => {
                #cursor_init
                if (__num_accounts as usize) < <#accounts_type as AccountCount>::COUNT {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                #body
            }
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

fn parse_instruction_attr(attr: &syn::Attribute) -> syn::Result<InstructionArgs> {
    match &attr.meta {
        Meta::Path(_) => Ok(InstructionArgs {
            discriminator: None,
            heap: false,
            raw: false,
        }),
        Meta::List(_) => attr.parse_args(),
        Meta::NameValue(_) => Err(syn::Error::new_spanned(
            attr,
            "expected `#[instruction]` or `#[instruction(...)]`",
        )),
    }
}

fn auto_discriminator(
    used: &mut BTreeSet<u8>,
    next_auto: &mut u16,
    span: proc_macro2::Span,
) -> syn::Result<(Vec<LitInt>, Vec<u8>)> {
    while *next_auto <= 254 {
        let value = *next_auto as u8;
        *next_auto += 1;
        if used.insert(value) {
            return Ok((vec![LitInt::new(&value.to_string(), span)], vec![value]));
        }
    }

    Err(syn::Error::new(
        span,
        "automatic instruction discriminators exhausted the 1-byte space; pin explicit \
         discriminators or split the program",
    ))
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

    // Scan explicit discriminators first so automatic values skip every pinned
    // value, including values that appear later in the module.
    let mut instruction_specs = Vec::new();
    let mut raw_instruction_specs: Vec<RawInstructionSpec> = Vec::new();
    let mut seen_discriminators: Vec<(Vec<u8>, String)> = Vec::new();
    let mut disc_len: Option<usize> = None;
    let mut has_auto_discriminators = false;
    let mut explicit_discriminators: Vec<(Vec<u8>, String)> = Vec::new();

    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("instruction") {
                    let args = match parse_instruction_attr(attr) {
                        Ok(a) => a,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    let fn_name = func.sig.ident.to_string();
                    let Some(disc_bytes) = &args.discriminator else {
                        has_auto_discriminators = true;
                        break;
                    };

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

                    let disc_values = match parse_discriminator_bytes(disc_bytes) {
                        Ok(v) => v,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    if let Some((_, prev_fn)) = explicit_discriminators
                        .iter()
                        .find(|(v, _)| *v == disc_values)
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
                    explicit_discriminators.push((disc_values, fn_name));
                    break;
                }
            }
        }
    }

    if has_auto_discriminators {
        if let Some(len) = disc_len {
            if len != 1 {
                return syn::Error::new_spanned(
                    &module.ident,
                    "automatic instruction discriminators require 1-byte program discriminators; \
                     pin every instruction when using multi-byte discriminators",
                )
                .to_compile_error()
                .into();
            }
        } else {
            disc_len = Some(1);
        }
    }

    let mut used_auto_discriminators = explicit_discriminators
        .iter()
        .filter_map(|(values, _)| values.first().copied())
        .collect::<BTreeSet<_>>();
    let mut next_auto = 0u16;

    for item in items {
        if let Item::Fn(func) = item {
            for attr in &func.attrs {
                if attr.path().is_ident("instruction") {
                    let args: InstructionArgs = match parse_instruction_attr(attr) {
                        Ok(a) => a,
                        Err(e) => return e.to_compile_error().into(),
                    };
                    let fn_name = &func.sig.ident;
                    let (disc_bytes, disc_values, discriminator_source) = match &args.discriminator
                    {
                        Some(disc_bytes) => {
                            let disc_values = match parse_discriminator_bytes(disc_bytes) {
                                Ok(v) => v,
                                Err(e) => return e.to_compile_error().into(),
                            };
                            (
                                disc_bytes.clone(),
                                disc_values,
                                DiscriminatorSource::Explicit,
                            )
                        }
                        None => match auto_discriminator(
                            &mut used_auto_discriminators,
                            &mut next_auto,
                            fn_name.span(),
                        ) {
                            Ok((disc_bytes, disc_values)) => {
                                (disc_bytes, disc_values, DiscriminatorSource::Auto)
                            }
                            Err(e) => return e.to_compile_error().into(),
                        },
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
                        // Raw instruction: no CtxKind, no accounts_type, no client args.
                        raw_instruction_specs.push(RawInstructionSpec {
                            fn_name: fn_name.clone(),
                            disc_bytes: disc_bytes.clone(),
                            disc_values: disc_values.clone(),
                            discriminator_source,
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

                    let struct_name =
                        format_ident!("{}Instruction", snake_to_pascal(&fn_name.to_string()));
                    let accounts_type_str = accounts_type.to_string().replace(' ', "");
                    let macro_ident =
                        format_ident!("__{}_instruction", pascal_to_snake(&accounts_type_str));

                    let mut idl_args: Vec<IdlArgSpec> = Vec::new();
                    let mut remaining_args: Vec<(Ident, Type)> = Vec::new();
                    for arg in func.sig.inputs.iter().skip(1) {
                        let FnArg::Typed(pt) = arg else {
                            continue;
                        };
                        let name = match &*pt.pat {
                            Pat::Ident(pi) => pi.ident.clone(),
                            _ => continue,
                        };
                        idl_args.push(IdlArgSpec {
                            name: name.clone(),
                            ty: (*pt.ty).clone(),
                        });
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
                                Some(Err(e)) => return e.to_compile_error().into(),
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
                        discriminator_source,
                        accounts_type,
                        accounts_type_str,
                        heap: args.heap,
                        client_struct_name: struct_name,
                        client_macro_ident: macro_ident,
                        client_args,
                        idl_args,
                        has_remaining: ctx_kind.has_remaining(),
                    });

                    break;
                }
            }
        }
    }

    let disc_len_lit = disc_len.unwrap_or(1);

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

    let client_items: Vec<TokenStream2> = instruction_specs
        .iter()
        .map(InstructionSpec::client_item)
        .collect();

    let any_heap = instruction_specs.iter().any(|spec| spec.heap)
        || raw_instruction_specs.iter().any(|spec| spec.heap);

    // -- Raw dispatch early-return block --------------------------
    // Each raw instruction gets a match arm that walks the SVM buffer
    // to build a `Context` with all accounts, then calls the handler
    // directly. This block is inserted in __dispatch before the
    // normal dispatch! call.
    let raw_dispatch_block = if raw_instruction_specs.is_empty() {
        quote! {}
    } else {
        // Sort raw specs by discriminator value for table construction.
        let mut sorted_raw: Vec<&RawInstructionSpec> = raw_instruction_specs.iter().collect();
        sorted_raw.sort_by(|a, b| a.disc_values.cmp(&b.disc_values));

        // For 1-byte discriminators with contiguous values: use O(1) function
        // pointer table dispatch. The SVM verifier accepts callx (indirect
        // calls): verified by the callx_dispatch integration test.
        //
        // Contiguity check: sorted disc values must form a gap-free sequence.
        // If not (e.g. raw discs 1,2,5 with normal at 3,4), fall back to match.
        let is_contiguous = sorted_raw.len() == 1
            || sorted_raw
                .windows(2)
                .all(|w| w[1].disc_values[0] == w[0].disc_values[0] + 1);
        let use_table = disc_len_lit == 1 && is_contiguous;

        if use_table {
            let raw_min = sorted_raw[0].disc_values[0];
            let raw_max = sorted_raw[sorted_raw.len() - 1].disc_values[0];
            let table_size = (raw_max - raw_min + 1) as usize;
            let raw_context_setup = emit_raw_context_setup(quote!(1usize));

            // Build the function pointer table. Slots are filled for each
            // raw disc value. Gaps (if any non-raw instruction sits between
            // raw ones) should not exist: the disc collision check prevents it.
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

                        #raw_context_setup

                        // O(1) dispatch: function pointer table indexed by
                        // discriminator byte. LLVM emits `callx` for the
                        // indirect call: ~5 CU constant overhead.
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
            let raw_context_setup = emit_raw_context_setup(quote!(#disc_len_lit));
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
                        // SAFETY: the length guard above proves the discriminator
                        // bytes are present.
                        *(instruction_data.as_ptr() as *const [u8; #disc_len_lit])
                    };

                    if matches!(__raw_disc, #(#raw_disc_patterns)|*) {
                        #raw_context_setup

                        match __raw_disc {
                            #(#raw_call_arms)*
                            _ => unsafe {
                                // SAFETY: `matches!` above admits only the
                                // generated discriminator patterns.
                                core::hint::unreachable_unchecked()
                            }
                        }
                    }
                }
            }
        }
    };

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

        let event_dispatch_block = if raw_instruction_specs.is_empty() {
            let accounts_types: Vec<&TokenStream2> = instruction_specs
                .iter()
                .map(|spec| &spec.accounts_type)
                .collect();
            // Small dispatch tables compile smaller with the explicit 0xFF
            // invalid-instruction fast path. Larger tables benefit from
            // erasing it unless an account set can actually service event CPI.
            if instruction_specs.len() >= 4 {
                quote! {
                    const __QUASAR_NEEDS_EVENT_CPI: bool =
                        false #(|| <#accounts_types as AccountCount>::NEEDS_EVENT_CPI)*;
                    if __QUASAR_NEEDS_EVENT_CPI {
                        if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                            return __handle_event(ptr, instruction_data);
                        }
                    }
                }
            } else {
                quote! {
                    const __QUASAR_NEEDS_EVENT_CPI: bool =
                        false #(|| <#accounts_types as AccountCount>::NEEDS_EVENT_CPI)*;
                    if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                        if __QUASAR_NEEDS_EVENT_CPI {
                            return __handle_event(ptr, instruction_data);
                        }
                        return Err(ProgramError::InvalidInstructionData);
                    }
                }
            }
        } else {
            quote! {
                if !instruction_data.is_empty() && instruction_data[0] == 0xFF {
                    return __handle_event(ptr, instruction_data);
                }
            }
        };

        // Normal dispatch tail: use a single match and pick the lower-CU
        // account parser shape per instruction.
        let normal_dispatch_tail = if instruction_specs.is_empty() {
            // All instructions are raw: no normal dispatch needed.
            quote! { Err(ProgramError::InvalidInstructionData) }
        } else {
            let normal_match_arms: Vec<proc_macro2::TokenStream> = instruction_specs
                .iter()
                .map(|spec| spec.guarded_match_arm(any_heap, disc_len_lit))
                .collect();
            quote! {
                {
                    let __program_id: &[u8; 32] = unsafe {
                        // SAFETY: the Solana entrypoint ABI stores the program
                        // id immediately after the instruction data slice.
                        &*(instruction_data.as_ptr().add(instruction_data.len()) as *const [u8; 32])
                    };
                    const __U64_SIZE: usize = core::mem::size_of::<u64>();
                    let __num_accounts = unsafe {
                        // SAFETY: the SVM input buffer starts with the account count.
                        *(ptr as *const u64)
                    };
                    let __accounts_start = unsafe {
                        // SAFETY: account records start immediately after the
                        // count prefix.
                        (ptr as *mut u8).add(__U64_SIZE)
                    };

                    if instruction_data.len() < #disc_len_lit {
                        return Err(ProgramError::InvalidInstructionData);
                    }

                    let __disc: [u8; #disc_len_lit] = unsafe {
                        // SAFETY: the length guard above proves the discriminator
                        // bytes are present.
                        *(instruction_data.as_ptr() as *const [u8; #disc_len_lit])
                    };

                    match __disc {
                        #(#normal_match_arms)*
                        _ => Err(ProgramError::InvalidInstructionData),
                    }
                }
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

                    #event_dispatch_block

                    #raw_dispatch_block

                    #normal_dispatch_tail
                }
            });
        } else {
            items.push(syn::parse_quote! {
                #[inline(always)]
                #dispatch_vis fn __dispatch(ptr: *mut u8, instruction_data: &[u8]) -> Result<(), ProgramError> {
                    #event_dispatch_block

                    #raw_dispatch_block

                    #normal_dispatch_tail
                }
            });
        }

        // When per-endpoint heap is used, cursor init is in the dispatch
        // arms: the entrypoint does NOT init the cursor. Otherwise, init
        // the cursor once in the entrypoint.
        let cursor_init = if any_heap {
            quote! {}
        } else {
            quote! {
                #[cfg(feature = "alloc")]
                {
                    let heap_start = super::allocator::HEAP_START_ADDRESS as usize;
                    unsafe {
                        // SAFETY: generated entrypoint owns the process-local
                        // bump allocator cursor.
                        *(heap_start as *mut usize) = heap_start + core::mem::size_of::<usize>();
                    }
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
                        // SAFETY: the Solana entrypoint ABI stores the
                        // instruction-data length in the eight bytes before
                        // the data pointer.
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
                Ok(unsafe {
                    // SAFETY: `EventAuthority` is repr(transparent) over
                    // AccountView, and the PDA address was validated above.
                    &*(view as *const AccountView as *const Self)
                })
            }

            /// Construct without validation.
            ///
            /// # Safety
            /// Caller must ensure account address matches the expected PDA.
            #[inline(always)]
            pub unsafe fn from_account_view_unchecked(view: &AccountView) -> &Self {
                unsafe {
                    // SAFETY: caller guarantees the account is the event
                    // authority PDA.
                    &*(view as *const AccountView as *const Self)
                }
            }
        }

        impl quasar_lang::account_load::AccountLoad for EventAuthority {

            #[inline(always)]
            fn check(view: &AccountView) -> Result<(), ProgramError> {
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

    let idl_instruction_fragments: Vec<TokenStream2> = instruction_specs
        .iter()
        .map(|spec| {
            let fn_name_str = spec.fn_name.to_string();
            let disc_values = &spec.disc_values;
            let discriminator_source = spec.discriminator_source.idl_tokens();
            let accounts_type_str = &spec.accounts_type_str;
            let arg_defs: Vec<TokenStream2> = spec.idl_args.iter().map(|arg| {
                let arg_name = arg.name.to_string();
                let idl_type_tokens = crate::helpers::type_to_idl_type_tokens(&arg.ty);
                let codec_tokens = crate::helpers::type_to_idl_codec_tokens(&arg.ty);
                quote! {
                    quasar_lang::idl_build::__reexport::IdlArg {
                        name: quasar_lang::idl_build::s(#arg_name),
                        ty: #idl_type_tokens,
                        codec: #codec_tokens,
                        docs: quasar_lang::idl_build::Vec::new(),
                    }
                }
            }).collect();

            let has_dynamic = spec.idl_args.iter().any(|arg| {
                crate::helpers::classify_pod_dynamic(&arg.ty).is_some()
                    || crate::helpers::classify_option_dynamic(&arg.ty)
            });
            let layout_tokens = if has_dynamic {
                let inline_fields: Vec<String> = spec.idl_args.iter()
                    .filter(|arg| {
                        crate::helpers::classify_pod_dynamic(&arg.ty).is_none()
                            && !crate::helpers::classify_option_dynamic(&arg.ty)
                    })
                    .map(|arg| arg.name.to_string())
                    .collect();
                let tail_fields: Vec<String> = spec.idl_args.iter()
                    .filter(|arg| {
                        crate::helpers::classify_pod_dynamic(&arg.ty).is_some()
                            || crate::helpers::classify_option_dynamic(&arg.ty)
                    })
                    .map(|arg| arg.name.to_string())
                    .collect();
                quote! {
                    Some(quasar_lang::idl_build::__reexport::IdlLayout::Compact {
                        inline_fields: quasar_lang::idl_build::vec![#(quasar_lang::idl_build::s(#inline_fields)),*],
                        tail_fields: quasar_lang::idl_build::vec![#(quasar_lang::idl_build::s(#tail_fields)),*],
                        wire: quasar_lang::idl_build::__reexport::CompactWire::InlineFieldsThenTailHeadersThenTailPayloads,
                    })
                }
            } else if spec.idl_args.is_empty() {
                quote! { None }
            } else {
                let field_names: Vec<String> = spec.idl_args.iter().map(|arg| arg.name.to_string()).collect();
                quote! {
                    Some(quasar_lang::idl_build::__reexport::IdlLayout::Fixed {
                        fields: quasar_lang::idl_build::vec![#(quasar_lang::idl_build::s(#field_names)),*],
                    })
                }
            };

            let remaining_tokens = if spec.has_remaining {
                quote! {
                    Some(quasar_lang::idl_build::__reexport::IdlRemainingAccounts {
                        kind: quasar_lang::idl_build::__reexport::RemainingAccountsKind::Append,
                        name: quasar_lang::idl_build::s("remainingAccounts"),
                        min: 0,
                        max: None,
                        item: quasar_lang::idl_build::__reexport::RemainingAccountItem {
                            client_type: quasar_lang::idl_build::s("accountMeta"),
                            signer: quasar_lang::idl_build::__reexport::AccountFlag::Dynamic(
                                quasar_lang::idl_build::__reexport::AccountFlagDynamic::Input,
                            ),
                            writable: quasar_lang::idl_build::__reexport::AccountFlag::Dynamic(
                                quasar_lang::idl_build::__reexport::AccountFlagDynamic::Input,
                            ),
                        },
                        policy: quasar_lang::idl_build::__reexport::RemainingAccountPolicy {
                            position: quasar_lang::idl_build::__reexport::RemainingPosition::AfterDeclaredAccounts,
                            order: quasar_lang::idl_build::__reexport::RemainingOrder::PreserveInput,
                        },
                    })
                }
            } else {
                quote! { None }
            };

            quote! {
                #[cfg(feature = "idl-build")]
                quasar_lang::__private_inventory::submit! {
                    quasar_lang::idl_build::InstructionFragment {
                        build: {
                            fn __build() -> quasar_lang::idl_build::__reexport::IdlInstruction {
                                quasar_lang::idl_build::__reexport::IdlInstruction {
                                    name: quasar_lang::idl_build::s(#fn_name_str),
                                    discriminator: quasar_lang::idl_build::vec![#(#disc_values),*],
                                    docs: quasar_lang::idl_build::Vec::new(),
                                    accounts: quasar_lang::idl_build::Vec::new(),
                                    args: quasar_lang::idl_build::vec![#(#arg_defs),*],
                                    layout: #layout_tokens,
                                    returns: None,
                                    effects: quasar_lang::idl_build::Vec::new(),
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

    let idl_raw_instruction_fragments: Vec<TokenStream2> = raw_instruction_specs
        .iter()
        .map(|spec| {
            let fn_name_str = spec.fn_name.to_string();
            let disc_values = &spec.disc_values;
            let discriminator_source = spec.discriminator_source.idl_tokens();
            quote! {
                #[cfg(feature = "idl-build")]
                quasar_lang::__private_inventory::submit! {
                    quasar_lang::idl_build::InstructionFragment {
                        build: {
                            fn __build() -> quasar_lang::idl_build::__reexport::IdlInstruction {
                                quasar_lang::idl_build::__reexport::IdlInstruction {
                                    name: quasar_lang::idl_build::s(#fn_name_str),
                                    discriminator: quasar_lang::idl_build::vec![#(#disc_values),*],
                                    docs: quasar_lang::idl_build::Vec::new(),
                                    accounts: quasar_lang::idl_build::Vec::new(),
                                    args: quasar_lang::idl_build::Vec::new(),
                                    layout: None,
                                    returns: None,
                                    effects: quasar_lang::idl_build::Vec::new(),
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
            pub fn __quasar_build_idl() -> quasar_lang::idl_build::String {
                let address = quasar_lang::idl_build::address_to_base58(&crate::ID);
                let idl = quasar_lang::idl_build::build_idl(
                    &address,
                    #mod_name_str,
                    env!("CARGO_PKG_VERSION"),
                );
                quasar_lang::idl_build::__reexport::serde_json::to_string_pretty(&idl)
                    .expect("generated IDL should serialize")
            }

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

        #(#idl_instruction_fragments)*
        #(#idl_raw_instruction_fragments)*

        #idl_build_fn
    }
    .into()
}
