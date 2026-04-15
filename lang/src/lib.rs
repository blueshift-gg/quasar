//! Quasar — zero-copy Solana program framework.
//!
//! `quasar-lang` provides the runtime primitives for building Solana programs
//! with Anchor-compatible ergonomics and minimal compute unit overhead. Account
//! data is accessed through pointer casts to `#[repr(C)]` companion structs —
//! no deserialization, no heap allocation.
//!
//! # Crate structure
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`accounts`] | Zero-copy account wrapper types (`Account`, `Signer`, `UncheckedAccount`) |
//! | [`checks`] | Compile-time account validation traits |
//! | [`cpi`] | Const-generic cross-program invocation builder |
//! | [`pod`] | Alignment-1 integer types (re-exported from `quasar-pod`) |
//! | [`traits`] | Core framework traits (`Owner`, `Discriminator`, `Space`, etc.) |
//! | [`prelude`] | Convenience re-exports for program code |
//!
//! # Safety model
//!
//! Quasar uses `unsafe` for zero-copy access, CPI syscalls, and pointer casts.
//! Soundness relies on:
//!
//! - **Alignment-1 guarantee**: Pod types and ZC companion structs are
//!   `#[repr(C)]` with alignment 1. Compile-time assertions verify this.
//! - **Bounds checking**: Account data length is validated during parsing
//!   before any pointer cast occurs.
//! - **Discriminator validation**: All-zero discriminators are banned at
//!   compile time. Account data is checked against the expected discriminator
//!   before access.
//!
//! Every `unsafe` block is validated by Miri under Tree Borrows with symbolic
//! alignment checking.

#![no_std]
#![cfg_attr(
    any(target_os = "solana", target_arch = "bpf"),
    feature(asm_experimental_arch)
)]
#[cfg(feature = "debug")]
extern crate alloc;
extern crate self as quasar_lang;

/// Internal re-exports for proc macro codegen. Not part of the public API.
/// Breaking changes to this module are not considered semver violations.
#[doc(hidden)]
pub mod __internal {
    pub use solana_account_view::{
        AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
    };

    // Header layout (little-endian u32):
    //
    // ```text
    // byte 0: borrow_state  (0xFF = NOT_BORROWED, 0 = mutably borrowed,
    //                         1..254 = immutable borrows remaining)
    // byte 1: is_signer     (0 or 1)
    // byte 2: is_writable   (0 or 1)
    // byte 3: executable    (0 or 1)
    // ```
    //
    // The generated `parse_accounts` code reads the header as a single u32
    // and compares it against the expected constant. On mismatch, the cold
    // `decode_header_error` path uses a mask-based minimum-requirements
    // check so that extra permissions (e.g. signer when not required) are
    // silently accepted.

    /// Not borrowed, no flags required.
    pub const NODUP: u32 = 0xFF;
    /// Not borrowed + signer.
    pub const NODUP_SIGNER: u32 = 0xFF | (1 << 8);
    /// Not borrowed + writable.
    pub const NODUP_MUT: u32 = 0xFF | (1 << 16);
    /// Not borrowed + signer + writable.
    pub const NODUP_MUT_SIGNER: u32 = 0xFF | (1 << 8) | (1 << 16);
    /// Not borrowed + executable.
    pub const NODUP_EXECUTABLE: u32 = 0xFF | (1 << 24);

    /// Size of the SVM account header: `RuntimeAccount` struct + 10 KiB
    /// realloc padding + trailing `u64` length.
    pub const ACCOUNT_HEADER: usize = core::mem::size_of::<RuntimeAccount>()
        + MAX_PERMITTED_DATA_INCREASE
        + core::mem::size_of::<u64>();

    /// Packed flags for [`parse_account_dup`]. Keeps the param count under the
    /// sBPF 5-register limit to avoid stack spills.
    #[derive(Clone, Copy)]
    pub struct ParseFlags {
        /// Expected header value (const).
        pub expected: u32,
        /// Required-mask for the cold-path minimum-requirements check.
        pub mask: u32,
        /// Flag-only mask (excludes borrow_state byte).
        pub flag_mask: u32,
        /// Whether this field is `Option<T>`.
        pub is_optional: bool,
        /// Whether the field reference is `&mut`.
        pub is_ref_mut: bool,
        /// Whether the field has `#[account(dup)]`.
        pub allow_dup: bool,
    }

    /// Parse a non-duplicate account from the SVM input buffer (hot path).
    ///
    /// Reads the 4-byte header, compares against `expected`. On exact match,
    /// writes the `AccountView` and advances `input` past the account data +
    /// alignment padding. On mismatch, the cold `decode_header_error` path
    /// checks minimum requirements.
    ///
    /// Returns the updated input pointer on success.
    #[inline(always)]
    pub unsafe fn parse_account(
        input: *mut u8,
        base: *mut AccountView,
        offset: usize,
        expected: u32,
        mask: u32,
    ) -> Result<*mut u8, solana_program_error::ProgramError> {
        debug_assert!(
            input as usize & 7 == 0,
            "parse_account: input pointer is not 8-byte aligned"
        );
        let raw = input as *mut RuntimeAccount;
        let header = *(raw as *const u32);

        if crate::utils::hint::unlikely(header != expected) {
            let err = crate::decode_header_error(header, expected, mask);
            if err != 0 {
                return Err(solana_program_error::ProgramError::from(err));
            }
        }

        core::ptr::write(base.add(offset), AccountView::new_unchecked(raw));
        let input = input.add(ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
        let input = input.add((input as usize).wrapping_neg() & 7);
        Ok(input)
    }

    /// Parse an account that may be a duplicate or optional (cold-ish path).
    ///
    /// Handles:
    /// - Optional sentinel guards (program_id == account address means None)
    /// - Duplicate account reuse with borrow-state tracking
    /// - Mutable dup rejection when `!flags.allow_dup`
    /// - Mask-based flag checks
    ///
    /// Returns the updated input pointer on success.
    #[inline(always)]
    pub unsafe fn parse_account_dup(
        input: *mut u8,
        base: *mut AccountView,
        offset: usize,
        program_id: &solana_address::Address,
        flags: ParseFlags,
    ) -> Result<*mut u8, solana_program_error::ProgramError> {
        use solana_program_error::ProgramError;

        debug_assert!(
            input as usize & 7 == 0,
            "parse_account_dup: input pointer is not 8-byte aligned"
        );
        let raw = input as *mut RuntimeAccount;
        let actual_header = *(raw as *const u32);

        if (actual_header & 0xFF) == NOT_BORROWED as u32 {
            // Not a dup — validate flags.
            if flags.is_optional {
                // Optional: skip flag check if address == program_id (sentinel
                // for None).
                if !crate::keys_eq(&(*raw).address, program_id) {
                    let expected_flags = flags.expected & flags.flag_mask;
                    if crate::utils::hint::unlikely(
                        (actual_header & flags.flag_mask) != expected_flags,
                    ) {
                        return Err(ProgramError::from(crate::decode_header_error(
                            actual_header,
                            flags.expected,
                            flags.mask,
                        )));
                    }
                }
            } else {
                let expected_flags = flags.expected & flags.flag_mask;
                if crate::utils::hint::unlikely((actual_header & flags.flag_mask) != expected_flags)
                {
                    return Err(ProgramError::from(crate::decode_header_error(
                        actual_header,
                        flags.expected,
                        flags.mask,
                    )));
                }
            }
            core::ptr::write(base.add(offset), AccountView::new_unchecked(raw));
            let input = input.add(ACCOUNT_HEADER.wrapping_add((*raw).data_len as usize));
            let input = input.add((input as usize).wrapping_neg() & 7);
            Ok(input)
        } else {
            // Dup branch: borrow_state != NOT_BORROWED means the SVM
            // deduplicated this account slot.
            if flags.is_ref_mut && !flags.allow_dup {
                // Mutable dups without #[account(dup)] are rejected.
                return Err(ProgramError::AccountBorrowFailed);
            }

            let idx = (actual_header & 0xFF) as usize;
            if crate::utils::hint::unlikely(idx >= offset) {
                return Err(ProgramError::InvalidAccountData);
            }

            if flags.is_ref_mut {
                // Mutable dup: claim exclusive access.
                let orig_view = core::ptr::read(base.add(idx));
                let bs_ptr = orig_view.account_ptr() as *mut u8;
                let bs = *bs_ptr;
                if crate::utils::hint::unlikely(bs != NOT_BORROWED) {
                    return Err(ProgramError::AccountBorrowFailed);
                }
                *bs_ptr = 0;
            } else {
                // Immutable dup: consume one immutable borrow slot.
                let orig_view = core::ptr::read(base.add(idx));
                let bs_ptr = orig_view.account_ptr() as *mut u8;
                let bs = *bs_ptr;
                if crate::utils::hint::unlikely(bs <= 1) {
                    return Err(ProgramError::AccountBorrowFailed);
                }
                *bs_ptr = bs - 1;
            }

            core::ptr::write(base.add(offset), core::ptr::read(base.add(idx)));
            let input = input.add(core::mem::size_of::<u64>());
            Ok(input)
        }
    }
}

/// Declarative macros: `define_account!`, `require!`, `require_eq!`, `emit!`.
#[macro_use]
pub mod macros;
/// Sysvar access and the `impl_sysvar_get!` helper macro.
#[macro_use]
pub mod sysvars;
/// Runtime exit functions for program-owned accounts (close).
pub mod account_exit;
/// Runtime init functions for program-owned accounts.
pub mod account_init;
/// Trait-based account loading and validation (`AccountLoad`).
pub mod account_load;
/// Zero-copy account wrapper types for instruction handlers.
pub mod accounts;
/// Compile-time account validation traits (`Address`, `Owner`, `Executable`,
/// `Mutable`, `Signer`).
pub mod checks;
/// Off-chain instruction building utilities. Only compiled for non-SBF targets.
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub mod client;
/// Instruction context types (`Context`, `Ctx`).
pub mod context;
/// Const-generic cross-program invocation with stack-allocated account arrays.
pub mod cpi;
/// Program entrypoint macros (`dispatch!`, `no_alloc!`, `panic_handler!`).
pub mod entrypoint;
/// Framework error types.
pub mod error;
/// Event emission via `sol_log_data` and self-CPI.
pub mod event;
/// Trait for fixed-size instruction argument types with alignment-1 ZC
/// companions.
pub mod instruction_arg;
/// Instruction data deserialization for dynamic fields (strings and vecs).
pub mod instruction_data;
/// Low-level `sol_log_data` syscall wrapper.
pub mod log;
/// Program Derived Address creation and lookup.
pub mod pda;
/// Alignment-1 Pod integer types (re-exported from `quasar-pod`).
pub mod pod;
/// Convenience re-exports for program code.
pub mod prelude;
/// Zero-allocation remaining accounts iterator.
pub mod remaining;
/// `set_return_data` syscall wrapper.
pub mod return_data;
/// Core framework traits.
pub mod traits;
/// Utility functions
pub mod utils;
/// Runtime validation helpers for account constraints.
pub mod validation;

/// 32-byte address comparison via four `read_unaligned` u64 words.
///
/// Short-circuits on first mismatch. Uses `read_unaligned` to avoid
/// bounds-checked slicing, `Result` construction, and panic paths.
#[inline(always)]
pub fn keys_eq(a: &solana_address::Address, b: &solana_address::Address) -> bool {
    let a = a.as_array().as_ptr() as *const u64;
    let b = b.as_array().as_ptr() as *const u64;
    // SAFETY: `Address` is a 32-byte array. Reading four u64 words covers
    // all 32 bytes. `read_unaligned` is used because `Address` has align 1.
    unsafe {
        core::ptr::read_unaligned(a) == core::ptr::read_unaligned(b)
            && core::ptr::read_unaligned(a.add(1)) == core::ptr::read_unaligned(b.add(1))
            && core::ptr::read_unaligned(a.add(2)) == core::ptr::read_unaligned(b.add(2))
            && core::ptr::read_unaligned(a.add(3)) == core::ptr::read_unaligned(b.add(3))
    }
}

/// Check if an address is all zeros (the System program address).
///
/// OR-folds four u64 words — half the loads of a full comparison.
#[inline(always)]
pub fn is_system_program(addr: &solana_address::Address) -> bool {
    let a = addr.as_array().as_ptr() as *const u64;
    // SAFETY: Same as `keys_eq` — 32 bytes read as four u64 words.
    // `read_unaligned` handles the align-1 `Address` layout.
    unsafe {
        (core::ptr::read_unaligned(a)
            | core::ptr::read_unaligned(a.add(1))
            | core::ptr::read_unaligned(a.add(2))
            | core::ptr::read_unaligned(a.add(3)))
            == 0
    }
}

/// Decode a failed u32 header check into the appropriate error.
///
/// Cold path — called only when the exact header comparison fails.
/// Uses `required_mask` to perform a minimum-requirements check: if the
/// account has all required flags (even with extras like an unexpected
/// signer bit), returns `0` to signal "acceptable, proceed with parse."
///
/// Returns:
/// - `0` — acceptable mismatch (extra flags but requirements met)
/// - non-zero — actual error (dup, missing signer, etc.)
#[cold]
#[inline(never)]
#[allow(unused_variables)]
pub fn decode_header_error(header: u32, expected: u32, required_mask: u32) -> u64 {
    use solana_program_error::ProgramError;

    let [borrow, signer, writable, exec] = header.to_le_bytes();
    let [exp_borrow, exp_signer, exp_writable, exp_exec] = expected.to_le_bytes();

    #[cfg(feature = "debug")]
    {
        solana_program_log::log("account header mismatch — actual vs expected:");
        crate::log::log_data(&[
            &[borrow, signer, writable, exec],
            &[exp_borrow, exp_signer, exp_writable, exp_exec],
        ]);
    }

    // Dup: borrow_state is a dup index, not NOT_BORROWED.
    if borrow != exp_borrow {
        #[cfg(feature = "debug")]
        solana_program_log::log(
            "=> duplicate account (borrow_state is a dup index, not NOT_BORROWED)",
        );
        return u64::from(ProgramError::AccountBorrowFailed);
    }

    // Mask-based minimum requirements: if all required flags are present,
    // accept even with extras (e.g. signer when not required).
    if (header & required_mask) == (expected & required_mask) {
        #[cfg(feature = "debug")]
        solana_program_log::log("=> extra flags present but minimum requirements met — accepted");
        return 0;
    }

    // Actual flag mismatch — only reject if a required flag is missing.
    if exp_signer != 0 && signer == 0 {
        #[cfg(feature = "debug")]
        solana_program_log::log("=> signer required but account is not a signer");
        return u64::from(ProgramError::MissingRequiredSignature);
    }
    if exp_writable != 0 && writable == 0 {
        #[cfg(feature = "debug")]
        solana_program_log::log("=> writable required but account is read-only");
        return u64::from(ProgramError::Immutable);
    }

    #[cfg(feature = "debug")]
    solana_program_log::log("=> executable required but account is not executable");
    u64::from(ProgramError::InvalidAccountData)
}

/// Immediately terminate the program with `ProgramError::Custom(0)`.
///
/// On-chain: emits two SBF instructions (`lddw r0, 0x100000000; exit`).
/// Off-chain: panics with a descriptive message for test ergonomics.
#[inline(always)]
pub fn abort_program() -> ! {
    #[cfg(target_os = "solana")]
    unsafe {
        core::arch::asm!("lddw r0, 0x100000000", "exit", options(noreturn));
    }

    // bpfel-unknown-none uses LLVM's BPF dialect (different asm syntax).
    #[cfg(all(target_arch = "bpf", not(target_os = "solana")))]
    unsafe {
        core::arch::asm!("r0 = 0x100000000 ll", "exit", options(noreturn));
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    panic!("program aborted");
}
