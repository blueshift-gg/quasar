//! Quasar: zero-copy Solana program framework.
//!
//! `quasar-lang` provides the runtime primitives for building Solana programs
//! with Anchor-compatible ergonomics and minimal compute unit overhead. Account
//! data is accessed through pointer casts to `#[repr(C)]` companion structs:
//! no deserialization, no heap allocation.
//!
//! # Quick check
//!
//! Quasar's primitive account fields have alignment one, which is the basis
//! for its zero-copy layouts:
//!
//! ```rust
//! use quasar_lang::prelude::{PodBool, PodU64};
//!
//! assert_eq!(core::mem::align_of::<PodU64>(), 1);
//! assert_eq!(core::mem::align_of::<PodBool>(), 1);
//! ```
//!
//! # Crate structure
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`accounts`] | Zero-copy account wrapper types (`Account`, `Signer`, `UncheckedAccount`) |
//! | [`checks`] | Compile-time account validation traits |
//! | [`cpi`] | Const-generic cross-program invocation builder |
//! | [`pod`] | Alignment-1 integer types (re-exported from `zeropod`) |
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
//! Required CI runs the dedicated `quasar-lang` Miri integration target under
//! Tree Borrows, symbolic alignment checking, and strict provenance. That suite
//! exercises the off-chain unsafe paths represented in `tests/miri.rs`; it is
//! not a complete proof over every reachable `unsafe` operation.
//!
//! Miri cannot execute the SBF-only syscall wrappers gated on
//! `target_os = "solana"` (`pda`, `log`, `sysvars`, and the `cpi`
//! `invoke_raw`/return-data paths) or the generated `extern "C"` program
//! entrypoint. The on-chain integration suite exercises additional SBF
//! behavior, but it is not an undefined-behavior proof for every excluded path.

#![no_std]
#![warn(missing_docs)]
#![cfg_attr(
    any(target_os = "solana", target_arch = "bpf"),
    feature(asm_experimental_arch)
)]
#[cfg(any(feature = "debug", feature = "idl-build"))]
extern crate alloc;
extern crate self as quasar_lang;

mod compiler_builtins;

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

    /// The expected header `u32` for a non-duplicate account with the given
    /// required flags. The low byte is `NOT_BORROWED` (`0xFF`); each set flag
    /// bit lands in its byte (signer=byte 1, writable=byte 2, executable=byte
    /// 3). This is the single source of the header-bit layout: the accounts
    /// derive emits calls to it instead of open-coding `<< 8`/`<< 16`/`<< 24`.
    #[inline(always)]
    pub const fn header_expected(signer: bool, writable: bool, executable: bool) -> u32 {
        0xFF | (signer as u32) << 8 | (writable as u32) << 16 | (executable as u32) << 24
    }

    /// The header comparison mask: the borrow byte (`0xFF`) is always compared,
    /// plus every required flag's full byte. Extra (unrequired) permissions are
    /// masked out so they are silently accepted.
    #[inline(always)]
    pub const fn header_mask(signer: bool, writable: bool, executable: bool) -> u32 {
        0xFF | header_flag_mask(signer, writable, executable)
    }

    /// The flag portion of [`header_mask`] without the `0xFF` borrow byte — the
    /// mask used by the dup-aware parse path, which validates the borrow byte
    /// separately.
    #[inline(always)]
    pub const fn header_flag_mask(signer: bool, writable: bool, executable: bool) -> u32 {
        (if signer { 0xFFu32 << 8 } else { 0 })
            | (if writable { 0xFFu32 << 16 } else { 0 })
            | (if executable { 0xFFu32 << 24 } else { 0 })
    }

    /// Not borrowed, no flags required.
    pub const NODUP: u32 = header_expected(false, false, false);
    /// Not borrowed + signer.
    pub const NODUP_SIGNER: u32 = header_expected(true, false, false);
    /// Not borrowed + writable.
    pub const NODUP_MUT: u32 = header_expected(false, true, false);
    /// Not borrowed + signer + writable.
    pub const NODUP_MUT_SIGNER: u32 = header_expected(true, true, false);
    /// Not borrowed + executable.
    pub const NODUP_EXECUTABLE: u32 = header_expected(false, false, true);

    /// Size of the SVM account header: `RuntimeAccount` struct + 10 KiB
    /// realloc padding + trailing `u64` length.
    pub const ACCOUNT_HEADER: usize = core::mem::size_of::<RuntimeAccount>()
        + MAX_PERMITTED_DATA_INCREASE
        + core::mem::size_of::<u64>();

    /// Size of a duplicate account entry in the SVM input buffer.
    pub const DUP_ENTRY_SIZE: usize = core::mem::size_of::<u64>();

    /// sBPF stack frame size. Compile-time stack budgets subtract a
    /// per-site headroom from this; each site documents its margin.
    pub const SBF_STACK_FRAME: usize = 4096;

    /// Round `n` up to the next multiple of 8.
    #[inline(always)]
    pub const fn align_up_8(n: usize) -> usize {
        (n.wrapping_add(7)) & !7
    }

    /// Byte stride past a non-duplicate account entry in the SVM input buffer:
    /// header + data_len, rounded up to 8-byte alignment.
    #[inline(always)]
    pub const fn account_stride(data_len: usize) -> usize {
        align_up_8(ACCOUNT_HEADER.wrapping_add(data_len))
    }

    // Platform invariant: the SVM uses 64-bit pointers and u64 data_len.
    // The `data_len as usize` cast in account-walking code is only lossless
    // on 64-bit targets.
    const _: () = assert!(
        core::mem::size_of::<usize>() >= core::mem::size_of::<u64>(),
        "quasar requires usize >= u64 (SVM pointers are 64-bit)"
    );

    /// Walk the SVM input buffer and write `AccountView`s into a
    /// caller-provided stack buffer, without any validation (no signer,
    /// writable, owner, or discriminator checks). Used by
    /// `#[instruction(raw)]` dispatch codegen.
    ///
    /// Returns the number of accounts actually parsed and a pointer past
    /// the last account entry.
    ///
    /// # Safety
    ///
    /// - `input` must point to the first account entry in the SVM input buffer
    ///   and be 8-byte aligned (guaranteed by the SVM ABI).
    /// - `buf` must have space for at least `count` `AccountView` values.
    /// - `count` must not exceed the actual number of account entries between
    ///   `input` and `boundary` (the SVM's reported `num_accounts`, capped by
    ///   the caller).
    /// - `boundary` must point to the end of the accounts region (i.e.
    ///   `ix_data_ptr - sizeof(u64)`).
    #[inline(always)]
    pub unsafe fn parse_all_accounts_unchecked(
        input: *mut u8,
        buf: *mut AccountView,
        count: usize,
        boundary: *const u8,
    ) -> Result<(usize, *mut u8), solana_program_error::ProgramError> {
        use crate::svm::{Cursor, RawEntry};

        // SAFETY: The SVM guarantees 8-byte alignment at buffer start and
        // after each account entry (padded strides); `boundary` is the end of
        // the account region in the same allocation.
        let mut cursor = unsafe { Cursor::new(input, boundary) };
        for i in 0..count {
            // Early exit if we have reached the accounts boundary. The SVM
            // guarantees `count` entries fit, but this check is
            // defense-in-depth against a malformed buffer.
            if cursor.at_end() {
                return Ok((i, cursor.ptr()));
            }

            // SAFETY: `cursor` is not at end (checked above), so it points at
            // a valid account entry.
            match unsafe { cursor.next() } {
                RawEntry::Account(raw) => {
                    // SAFETY: Non-duplicate entry. `raw` is a valid
                    // `RuntimeAccount` pointer. `AccountView::new_unchecked`
                    // wraps it without copying.
                    unsafe {
                        core::ptr::write(buf.add(i), AccountView::new_unchecked(raw));
                    }
                }
                RawEntry::Dup(borrow) => {
                    // Duplicate entry. `borrow` is the loader's global index of
                    // the source non-dup account; in this flat buffer the slot
                    // index equals the global index, so it resolves against the
                    // accounts parsed so far (`buf[0..i]`). The SVM guarantees
                    // dup indices point backward to a previously-serialized
                    // non-dup entry (`resolve_dup` rejects a forward index).
                    // The copy creates an aliased `AccountView` (both
                    // point at the same `RuntimeAccount`); the raw handler is
                    // responsible for avoiding simultaneous
                    // `borrow_unchecked_mut()` on aliased views.
                    match crate::svm::resolve_dup(
                        borrow as usize,
                        crate::svm::DupSources::Buffer {
                            base: buf,
                            count: i,
                        },
                    ) {
                        // SAFETY: `buf.add(i)` is within the output buffer.
                        Some(view) => unsafe { core::ptr::write(buf.add(i), view) },
                        None => return Err(solana_program_error::ProgramError::InvalidAccountData),
                    }
                }
            }
        }
        Ok((count, cursor.ptr()))
    }

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
    ///
    /// # Safety
    ///
    /// The caller must ensure `input` is 8-byte aligned and points at a
    /// non-duplicate account entry whose full `RuntimeAccount` header
    /// (including `data_len`) is readable, and that `base.add(offset)` is a
    /// writable `AccountView` slot.
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
        // SAFETY: the header is the four flag bytes at the 8-aligned start of a
        // valid `RuntimeAccount`, so the aligned u32 read is in-bounds.
        let header = unsafe { *(raw as *const u32) };

        if crate::utils::hint::unlikely(header != expected) {
            let err = crate::decode_header_error(header, expected, mask);
            if err != 0 {
                return Err(solana_program_error::ProgramError::from(err));
            }
        }

        // SAFETY: `base.add(offset)` is within the caller-provided output
        // buffer, and `raw` is the current account header.
        unsafe { core::ptr::write(base.add(offset), AccountView::new_unchecked(raw)) };
        // SAFETY: `raw` is the current non-dup account, so `data_len` is valid
        // and `input` meets `advance_account_data`'s entry contract.
        let data_len = unsafe { (*raw).data_len as usize };
        // SAFETY: `input` points to the validated account header and
        // `data_len` was read from that header.
        let input = unsafe { crate::svm::advance_account_data(input, data_len) };
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
    ///
    /// # Safety
    ///
    /// The caller must ensure `input` is 8-byte aligned and points at an
    /// account or duplicate entry, that slots `base[0..offset]` are already
    /// initialized (the dup branch reads an earlier slot), and that
    /// `base.add(offset)` is a writable `AccountView` slot.
    #[inline(always)]
    pub unsafe fn parse_account_dup(
        input: *mut u8,
        base: *mut AccountView,
        offset: usize,
        program_id: &solana_address::Address,
        flags: ParseFlags,
    ) -> Result<*mut u8, solana_program_error::ProgramError> {
        debug_assert!(
            input as usize & 7 == 0,
            "parse_account_dup: input pointer is not 8-byte aligned"
        );
        let raw = input as *mut RuntimeAccount;
        // SAFETY: the header is the first four bytes at the 8-aligned start of a
        // valid account/dup entry, so the aligned u32 read is in-bounds.
        let actual_header = unsafe { *(raw as *const u32) };

        // Decode the dup/non-dup distinction through the single owner in
        // `svm.rs`. This parser keeps its tuned `advance_account_data` stride
        // form (see `svm::advance_account_data`) rather than driving a `Cursor`,
        // so it decodes the low header byte here instead of via `Cursor::next` —
        // but the `NOT_BORROWED` comparison itself lives in one place.
        if crate::svm::classify_borrow_state((actual_header & 0xFF) as u8).is_some() {
            // SAFETY: a dup entry at this 8-aligned input; the caller's
            // contract covers `base[0..=offset]`.
            return unsafe { copy_dup_slot(input, base, offset, actual_header, program_id, flags) };
        }

        // Optional None uses the program id as a sentinel and skips the flag
        // check.
        // SAFETY: a non-duplicate account header contains the address.
        let is_none_sentinel =
            flags.is_optional && crate::keys_eq(unsafe { &(*raw).address }, program_id);
        if !is_none_sentinel {
            check_header_flags(actual_header, flags)?;
        }

        // SAFETY: `base.add(offset)` is within the caller-provided output
        // buffer, and `raw` is the current account header.
        unsafe { core::ptr::write(base.add(offset), AccountView::new_unchecked(raw)) };
        // SAFETY: `raw` is the current non-dup account, so `data_len` is
        // valid and `input` meets `advance_account_data`'s entry contract.
        let data_len = unsafe { (*raw).data_len as usize };
        // SAFETY: `input` points to the validated account header and
        // `data_len` was read from that header.
        let input = unsafe { crate::svm::advance_account_data(input, data_len) };
        Ok(input)
    }

    /// Validate a non-duplicate account header against the field's expected
    /// flags, surfacing only decodable mismatches.
    #[inline(always)]
    fn check_header_flags(
        actual_header: u32,
        flags: ParseFlags,
    ) -> Result<(), solana_program_error::ProgramError> {
        let expected_flags = flags.expected & flags.flag_mask;
        if crate::utils::hint::unlikely((actual_header & flags.flag_mask) != expected_flags) {
            // `decode_header_error` returns 0 when the mismatched bit is
            // outside the required mask; that must fall through rather than
            // become `Err(from(0))`.
            let err = crate::decode_header_error(actual_header, flags.expected, flags.mask);
            if err != 0 {
                return Err(solana_program_error::ProgramError::from(err));
            }
        }
        Ok(())
    }

    /// Resolve one SVM duplicate entry against an earlier slot.
    ///
    /// The low header byte is the loader's global index; in this flat declared
    /// buffer that equals the output slot index, so it aliases an earlier
    /// slot. Kept in its tuned inline form rather than routed through
    /// `svm::resolve_dup` (which documents the same flat `Buffer` index
    /// space): reading `base[idx]` lazily with the `unlikely` bounds hint
    /// measured 3 CU cheaper on the multi-sentinel parse.
    ///
    /// # Safety
    ///
    /// `input` must point at a dup entry; slots `base[0..offset]` must be
    /// initialized and `base[offset]` writable (the `parse_account_dup`
    /// contract).
    #[inline(always)]
    unsafe fn copy_dup_slot(
        input: *mut u8,
        base: *mut AccountView,
        offset: usize,
        actual_header: u32,
        program_id: &solana_address::Address,
        flags: ParseFlags,
    ) -> Result<*mut u8, solana_program_error::ProgramError> {
        use solana_program_error::ProgramError;

        let idx = (actual_header & 0xFF) as usize;
        if crate::utils::hint::unlikely(idx >= offset) {
            return Err(ProgramError::InvalidAccountData);
        }

        // SAFETY: `idx < offset` (checked above), so `base[idx]` is an
        // initialized slot per the caller's contract.
        let orig_view = unsafe { core::ptr::read(base.add(idx)) };

        // Repeated optional-None sentinels may be serialized as duplicate
        // entries; they are not aliases of a real user account, so they copy
        // without requiring `#[account(dup)]`.
        let is_none_sentinel = flags.is_optional && crate::keys_eq(orig_view.address(), program_id);
        if !is_none_sentinel && !flags.allow_dup {
            return Err(ProgramError::AccountBorrowFailed);
        }

        // SAFETY: `base[offset]` is writable per the caller's contract; a dup
        // slot may copy the already validated view.
        unsafe { core::ptr::write(base.add(offset), orig_view) };
        // SAFETY: a duplicate entry is exactly `DUP_ENTRY_SIZE` bytes and the
        // caller guarantees the input contains a complete entry.
        Ok(unsafe { input.add(DUP_ENTRY_SIZE) })
    }
}

/// Declarative macros: `define_account!`, `require!`, `require_eq!`, `emit!`.
#[macro_use]
pub mod macros;
/// Sysvar access and the `impl_sysvar_get!` helper macro.
#[macro_use]
pub mod sysvars;
/// Protocol-owned account behavior trait (`AccountBehavior`).
pub mod account_behavior;
/// Runtime init functions for program-owned accounts.
pub mod account_init;
/// Layout descriptor for zero-copy account wrappers (`AccountLayout`).
pub mod account_layout;
/// Trait-based account loading and validation (`AccountLoad`).
pub mod account_load;
/// Zero-copy account wrapper types for instruction handlers.
pub mod accounts;
/// Unified address verification trait (`AddressVerify`).
pub mod address;
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
/// Program runtime-environment macros (`no_alloc!`, `heap_alloc!`,
/// `panic_handler!`); dispatch is generated by `#[program]`.
pub mod entrypoint;
/// Framework error types.
pub mod error;
/// Event emission via `sol_log_data` and self-CPI.
pub mod event;
/// Trait for fixed-size instruction argument types with alignment-1 ZC
/// companions.
pub mod instruction_arg;
/// Low-level `sol_log_data` syscall wrapper.
pub mod log;
/// Op-dispatch: `OpCtx`, `SupportsRealloc`, structural ops (init,
/// realloc, close).
pub mod ops;
/// Program Derived Address creation and lookup.
pub mod pda;
/// Alignment-1 Pod integer types (re-exported from `zeropod`).
pub mod pod;
/// Convenience re-exports for program code.
pub mod prelude;
/// Zero-allocation remaining accounts iterator.
pub mod remaining;
/// `set_return_data` syscall wrapper.
pub mod return_data;
/// The single owner of the SVM account-buffer walk (`Cursor`).
pub(crate) mod svm;
/// Centralized sBPF ABI facts (entrypoint, input buffer, `SolBytes`).
pub mod svm_abi;
/// Core framework traits.
pub mod traits;
/// Utility functions
pub mod utils;
/// Runtime validation helpers for account constraints.
pub mod validation;

/// IDL fragment collection for compile-time IDL generation.
/// Feature-gated behind `idl-build`.
#[cfg(feature = "idl-build")]
pub mod idl_build;

pub use crate::pod::{PodString as String, PodVec as Vec};
/// Re-export `inventory` for proc macro codegen (hidden).
#[cfg(feature = "idl-build")]
#[doc(hidden)]
pub use inventory as __private_inventory;
#[doc(hidden)]
pub use solana_program_error as __solana_program_error;
/// Re-export of the `zeropod` crate so that `#[derive(ZeroPod)]` expansion
/// inside framework-generated code can resolve `zeropod::*` paths without
/// downstream crates adding a direct dependency.
#[doc(hidden)]
pub use zeropod as __zeropod;
/// The `#[derive(ZeroPod)]` macro for defining zero-copy account and
/// instruction schemas.
///
/// This is the stable path for framework plugins that define their own
/// zero-copy schema types (see [`pod`] for the alignment-1 field types).
///
/// Note: the derive expands to unqualified `zeropod::` paths, so a crate using
/// `#[derive(quasar_lang::ZeroPod)]` must also bring the `zeropod` crate into
/// scope (e.g. `use quasar_lang::__zeropod as zeropod;`) until the derive
/// gains a crate-path override.
pub use zeropod::ZeroPod;
// Re-export zeropod traits for framework integration.
pub use zeropod::{
    ZcElem, ZcField, ZcValidate, ZeroPodCompact, ZeroPodError, ZeroPodFixed, ZeroPodSchema,
};

/// 32-byte address comparison via four `read_unaligned` u64 words.
///
/// Short-circuits on first mismatch. Uses `read_unaligned` to avoid
/// bounds-checked slicing, `Result` construction, and panic paths.
#[inline(always)]
pub fn keys_eq(a: &solana_address::Address, b: &solana_address::Address) -> bool {
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        a == b
    }
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
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
}

/// Const-compatible 32-byte address comparison for use in compile-time
/// assertions (e.g. `one_of` owner checks, migration owner equality).
/// Not intended for runtime use; prefer [`keys_eq`] which is branchless
/// and optimized for sBPF.
#[inline(always)]
pub const fn keys_eq_const(a: &solana_address::Address, b: &solana_address::Address) -> bool {
    let a = a.as_array();
    let b = b.as_array();
    let mut i = 0;
    while i < 32 {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Check if an address is all zeros (the System program address).
///
/// OR-folds four u64 words; half the loads of a full comparison.
#[inline(always)]
pub fn is_system_program(addr: &solana_address::Address) -> bool {
    let a = addr.as_array().as_ptr() as *const u64;
    // SAFETY: Same as `keys_eq`: 32 bytes read as four u64 words.
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
/// Cold path, called only when the exact header comparison fails.
/// Uses `required_mask` to perform a minimum-requirements check: if the
/// account has all required flags (even with extras like an unexpected
/// signer bit), returns `0` to signal "acceptable, proceed with parse."
///
/// Returns:
/// - `0`: acceptable mismatch (extra flags but requirements met)
/// - non-zero: actual error (dup, missing signer, etc.)
#[cold]
#[inline(never)]
// `exec`/`exp_exec` are read only on the `debug` logging path.
#[allow(unused_variables)]
#[doc(hidden)]
pub fn decode_header_error(header: u32, expected: u32, required_mask: u32) -> u64 {
    use solana_program_error::ProgramError;

    let [borrow, signer, writable, exec] = header.to_le_bytes();
    let [exp_borrow, exp_signer, exp_writable, exp_exec] = expected.to_le_bytes();

    #[cfg(feature = "debug")]
    {
        solana_program_log::log("account header mismatch: actual vs expected:");
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
        solana_program_log::log("=> extra flags present but minimum requirements met: accepted");
        return 0;
    }

    // Actual flag mismatch: only reject if a required flag is missing.
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
    // SAFETY: This is the Solana SBF abort sequence: place
    // `ProgramError::Custom(0)` in r0 and exit without returning.
    unsafe {
        core::arch::asm!("lddw r0, 0x100000000", "exit", options(noreturn));
    }

    // bpfel-unknown-none uses LLVM's BPF dialect (different asm syntax).
    #[cfg(all(target_arch = "bpf", not(target_os = "solana")))]
    // SAFETY: Same abort sequence as above, expressed in LLVM's BPF asm
    // dialect for bpfel-unknown-none.
    unsafe {
        core::arch::asm!("r0 = 0x100000000 ll", "exit", options(noreturn));
    }

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    panic!("program aborted");
}

#[cfg(test)]
mod tests {
    use {super::*, solana_address::Address};

    #[test]
    fn keys_eq_identical() {
        let a = Address::new_from_array([0xAB; 32]);
        assert!(keys_eq(&a, &a));
    }

    #[test]
    fn keys_eq_first_word_mismatch() {
        let a = Address::new_from_array([0xFF; 32]);
        let mut b_bytes = [0xFF; 32];
        b_bytes[0] = 0x00;
        let b = Address::new_from_array(b_bytes);
        assert!(!keys_eq(&a, &b));
    }

    #[test]
    fn keys_eq_last_word_mismatch() {
        let a = Address::new_from_array([0xFF; 32]);
        let mut b_bytes = [0xFF; 32];
        b_bytes[31] = 0x00;
        let b = Address::new_from_array(b_bytes);
        assert!(!keys_eq(&a, &b));
    }

    #[test]
    fn keys_eq_all_zero() {
        let a = Address::new_from_array([0; 32]);
        let b = Address::new_from_array([0; 32]);
        assert!(keys_eq(&a, &b));
    }

    #[test]
    fn is_system_program_zero() {
        let addr = Address::new_from_array([0; 32]);
        assert!(is_system_program(&addr));
    }

    #[test]
    fn is_system_program_nonzero() {
        let mut bytes = [0u8; 32];
        bytes[16] = 1;
        let addr = Address::new_from_array(bytes);
        assert!(!is_system_program(&addr));
    }

    /// The `header_expected` const fn reproduces the five `NODUP_*` constants
    /// and the raw bit layout for every flag combination, so the derive-emitted
    /// calls and the runtime constants can never drift apart.
    #[test]
    fn header_bits_match_nodup_constants() {
        use super::__internal::{
            header_expected, header_flag_mask, header_mask, NODUP, NODUP_EXECUTABLE, NODUP_MUT,
            NODUP_MUT_SIGNER, NODUP_SIGNER,
        };

        assert_eq!(header_expected(false, false, false), 0xFF);
        assert_eq!(header_expected(true, false, false), 0xFF | (1 << 8));
        assert_eq!(header_expected(false, true, false), 0xFF | (1 << 16));
        assert_eq!(
            header_expected(true, true, false),
            0xFF | (1 << 8) | (1 << 16)
        );
        assert_eq!(header_expected(false, false, true), 0xFF | (1 << 24));

        assert_eq!(NODUP, 0xFF);
        assert_eq!(NODUP_SIGNER, 0xFF | (1 << 8));
        assert_eq!(NODUP_MUT, 0xFF | (1 << 16));
        assert_eq!(NODUP_MUT_SIGNER, 0xFF | (1 << 8) | (1 << 16));
        assert_eq!(NODUP_EXECUTABLE, 0xFF | (1 << 24));

        // mask = borrow byte + flag mask; flag mask = mask without the borrow byte.
        for &(s, w, e) in &[
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            assert_eq!(header_mask(s, w, e), 0xFF | header_flag_mask(s, w, e));
            let expected_flag = (if s { 0xFFu32 << 8 } else { 0 })
                | (if w { 0xFFu32 << 16 } else { 0 })
                | (if e { 0xFFu32 << 24 } else { 0 });
            assert_eq!(header_flag_mask(s, w, e), expected_flag);
        }
    }
}

#[cfg(kani)]
#[path = "../kani/lib.rs"]
mod kani_proofs;
