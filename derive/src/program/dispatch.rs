//! Instruction dispatch + entrypoint codegen.
//!
//! Emits, into the `#[program]` module body: the `__handle_event` event-CPI
//! stub, the `__dispatch` router (event fast-path, raw table/match, then the
//! normal discriminator match), the `extern "C" entrypoint`, and the off-chain
//! `cpi` client module.

use {
    super::{
        model::ProgramModel,
        spec::{InstructionSpec, ProgramArgs},
    },
    proc_macro2::{Literal, TokenStream as TokenStream2},
    quote::{format_ident, quote},
    syn::{Ident, ItemMod},
};

/// Account-count crossover at which the normal dispatch arm calls the direct
/// `__quasar_direct_*` parser instead of buffering into a `MaybeUninit` array:
/// below it the buffered path removes a Context/Ctx layer more cheaply, at or
/// above it the direct parse wins. (Interpolated as a `usize`, so it emits the
/// historical `8usize` literal.)
const DIRECT_PARSE_MIN_ACCOUNTS: usize = 8;

/// Instruction-count threshold above which the `0xFF` event fast-path is only
/// emitted when an account set can actually service event CPI. Small tables
/// compile smaller keeping the explicit invalid-instruction return; larger
/// tables benefit from erasing it. (Derive-side comparison, not emitted.)
const EVENT_FASTPATH_MIN_IX: usize = 4;

/// Stack-allocated `AccountView` buffer cap for raw dispatch: a raw handler
/// sees up to this many accounts inlined on the entrypoint frame, the rest flow
/// through the remaining-accounts cursor. Bounds the stack footprint of the raw
/// path. (Emitted as an unsuffixed `64` literal to match the historical shape.)
const RAW_INLINE_ACCOUNT_CAP: usize = 64;

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
    let raw_max = Literal::usize_unsuffixed(RAW_INLINE_ACCOUNT_CAP);
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

        const __RAW_MAX: usize = #raw_max;
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

        // SAFETY: `__raw_remaining` and `__raw_boundary` came from the raw
        // account walk over this same SVM input buffer.
        let __raw_ctx = unsafe {
            Context::from_raw_parts(
                __raw_program_id,
                __raw_accounts,
                &instruction_data[#data_start..],
                __raw_remaining,
                __raw_boundary as *const u8,
            )
        };
    }
}

/// Emit the normal-dispatch match arm for one instruction: the account-count
/// guard, then the buffered parse (small account lists) or the direct parser
/// (>= `DIRECT_PARSE_MIN_ACCOUNTS`); handlers with remaining accounts always
/// use the buffered path.
fn guarded_match_arm(spec: &InstructionSpec, any_heap: bool, disc_len: usize) -> TokenStream2 {
    let cursor_init = emit_heap_cursor_block(spec.heap, any_heap);
    let fn_name = &spec.fn_name;
    let direct_fn_name = format_ident!("__quasar_direct_{}", fn_name);
    let accounts_type = &spec.accounts_type;
    let disc_bytes = &spec.disc_bytes;
    let min_direct = DIRECT_PARSE_MIN_ACCOUNTS;
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
            let __data_after_disc = #data_after_disc;
            // SAFETY: `parse_accounts` returned the remaining-region
            // pointer for this SVM buffer, and the ABI places the
            // instruction-data length prefix directly before
            // `instruction_data`, giving the accounts boundary.
            #fn_name(unsafe {
                Context::from_raw_parts(
                    __program_id,
                    &mut __accounts,
                    __data_after_disc,
                    __remaining_ptr,
                    instruction_data.as_ptr().sub(__U64_SIZE),
                )
            })
        }
    };

    let body = if spec.has_remaining {
        buffered_body
    } else {
        quote! {
            // The direct helper removes one generated Context/Ctx::new layer.
            // On small account lists the buffered path is cheaper, so the
            // derive selects the lower-CU shape from the account count.
            if <#accounts_type as AccountCount>::COUNT >= #min_direct {
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

/// Build the raw-instruction dispatch block: an O(1) function-pointer table for
/// contiguous 1-byte discriminators, else a match chain.
fn emit_raw_dispatch_block(model: &ProgramModel) -> TokenStream2 {
    let raw_instruction_specs = &model.raw_specs;
    let disc_len_lit = model.disc_len;
    let any_heap = model.any_heap;

    if raw_instruction_specs.is_empty() {
        return quote! {};
    }

    // Sort raw specs by discriminator value for table construction.
    let mut sorted_raw: Vec<&_> = raw_instruction_specs.iter().collect();
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
}

/// Build the `0xFF` event-CPI fast-path block.
fn emit_event_dispatch_block(model: &ProgramModel) -> TokenStream2 {
    let instruction_specs = &model.instruction_specs;
    if model.raw_specs.is_empty() {
        let accounts_types: Vec<&TokenStream2> = instruction_specs
            .iter()
            .map(|spec| &spec.accounts_type)
            .collect();
        // Small dispatch tables compile smaller with the explicit 0xFF
        // invalid-instruction fast path. Larger tables benefit from
        // erasing it unless an account set can actually service event CPI.
        if instruction_specs.len() >= EVENT_FASTPATH_MIN_IX {
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
    }
}

/// Build the normal dispatch tail: a single discriminator match picking the
/// lower-CU account parser shape per instruction.
fn emit_normal_dispatch_tail(model: &ProgramModel) -> TokenStream2 {
    let instruction_specs = &model.instruction_specs;
    let disc_len_lit = model.disc_len;
    let any_heap = model.any_heap;

    if instruction_specs.is_empty() {
        // All instructions are raw: no normal dispatch needed.
        return quote! { Err(ProgramError::InvalidInstructionData) };
    }
    let normal_match_arms: Vec<TokenStream2> = instruction_specs
        .iter()
        .map(|spec| guarded_match_arm(spec, any_heap, disc_len_lit))
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
}

/// Push `__handle_event`, `__dispatch`, the `extern "C"` entrypoint, and the
/// off-chain `cpi` module into the `#[program]` module body.
pub(super) fn push_dispatch_items(
    module: &mut ItemMod,
    model: &ProgramModel,
    program_args: &ProgramArgs,
    client_items: &[TokenStream2],
) {
    let any_heap = model.any_heap;
    let raw_dispatch_block = emit_raw_dispatch_block(model);

    let Some((_, ref mut items)) = module.content else {
        return;
    };

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

    let event_dispatch_block = emit_event_dispatch_block(model);
    let normal_dispatch_tail = emit_normal_dispatch_tail(model);

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
