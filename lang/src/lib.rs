//! Quasar: zero-copy Solana program framework.
//!
//! `quasar-lang` provides the runtime primitives for building Solana programs
//! with Anchor-compatible ergonomics and minimal compute unit overhead. Account
//! data is accessed through pointer casts to `#[repr(C)]` companion structs:
//! no deserialization, no heap allocation.
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
//! Every `unsafe` block is validated by Miri under Tree Borrows with symbolic
//! alignment checking.

#![no_std]
#![cfg_attr(
    any(target_os = "solana", target_arch = "bpf"),
    feature(asm_experimental_arch)
)]
#[cfg(any(feature = "debug", feature = "idl-build"))]
extern crate alloc;
extern crate self as quasar_lang;
#[cfg(target_arch = "bpf")]
use solana_compiler_builtins as _;

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

    /// Size of a duplicate account entry in the SVM input buffer.
    pub const DUP_ENTRY_SIZE: usize = core::mem::size_of::<u64>();

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
        // SAFETY: The SVM guarantees 8-byte alignment at buffer start and
        // after each account entry (padded strides).
        debug_assert!(
            input as usize & 7 == 0,
            "parse_all_accounts_unchecked: input pointer is not 8-byte aligned"
        );

        let mut ptr = input;
        for i in 0..count {
            // SAFETY: Early exit if we have reached the accounts boundary.
            // The SVM guarantees `count` entries fit, but this check is
            // defense-in-depth against a malformed buffer.
            if (ptr as *const u8) >= boundary {
                return Ok((i, ptr));
            }

            // SAFETY: `ptr` is within the accounts region (checked above)
            // and points to a valid `RuntimeAccount` header. The
            // `borrow_state` field is at offset 0 of the `#[repr(C)]`
            // struct.
            let raw = ptr as *mut RuntimeAccount;
            let borrow = unsafe { (*raw).borrow_state };

            if borrow == NOT_BORROWED {
                // SAFETY: Non-duplicate entry. `raw` is a valid
                // `RuntimeAccount` pointer. `AccountView::new_unchecked`
                // wraps it without copying.
                unsafe {
                    core::ptr::write(buf.add(i), AccountView::new_unchecked(raw));
                }
                // SAFETY: `account_stride` computes header + data_len
                // rounded to 8-byte alignment, matching the SVM's
                // serialization layout.
                let data_len = unsafe { (*raw).data_len as usize };
                ptr = unsafe { ptr.add(account_stride(data_len)) };
            } else {
                // SAFETY: Duplicate entry. `borrow_state` encodes the
                // index of the source non-dup account. The SVM
                // guarantees dup indices always point backward to a
                // previously-serialized non-dup entry.
                let orig_idx = borrow as usize;
                if orig_idx < i {
                    // SAFETY: `orig_idx < i` ensures the source slot is
                    // already initialized. `AccountView` does not impl
                    // `Drop` (verified by static assert in remaining.rs),
                    // so bitwise copy is safe. Note: the copy creates an
                    // aliased `AccountView`; both point to the same
                    // `RuntimeAccount`. The raw handler is responsible for
                    // avoiding simultaneous `borrow_unchecked_mut()` on
                    // aliased views.
                    unsafe {
                        core::ptr::write(buf.add(i), core::ptr::read(buf.add(orig_idx)));
                    }
                } else {
                    return Err(solana_program_error::ProgramError::InvalidAccountData);
                }
                // SAFETY: Dup entries are exactly `DUP_ENTRY_SIZE` (8)
                // bytes in the SVM buffer.
                ptr = unsafe { ptr.add(DUP_ENTRY_SIZE) };
            }
        }
        Ok((count, ptr))
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
        // SAFETY: `input` points to a valid `RuntimeAccount` header.
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
        // SAFETY: `raw` is valid for the current non-duplicate account.
        let data_len = unsafe { (*raw).data_len as usize };
        // SAFETY: Account entries are serialized as header + data + padding.
        let input = unsafe { input.add(ACCOUNT_HEADER.wrapping_add(data_len)) };
        // SAFETY: Advance over the SVM 8-byte alignment padding.
        let input = unsafe { input.add((input as usize).wrapping_neg() & 7) };
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
        // SAFETY: `input` points to a valid account or duplicate header.
        let actual_header = unsafe { *(raw as *const u32) };

        if (actual_header & 0xFF) == NOT_BORROWED as u32 {
            // Not a dup; validate flags.
            if flags.is_optional {
                // Optional: skip flag check if address == program_id (sentinel
                // for None).
                // SAFETY: Non-duplicate account header contains the address.
                let address = unsafe { &(*raw).address };
                if !crate::keys_eq(address, program_id) {
                    let expected_flags = flags.expected & flags.flag_mask;
                    if crate::utils::hint::unlikely(
                        (actual_header & flags.flag_mask) != expected_flags,
                    ) {
                        // Mirror `parse_account`: only surface a decodable error.
                        // `decode_header_error` returns 0 when the mismatched bit
                        // is outside the required mask, in which case this must
                        // fall through instead of returning `Err(from(0))`.
                        let err =
                            crate::decode_header_error(actual_header, flags.expected, flags.mask);
                        if err != 0 {
                            return Err(ProgramError::from(err));
                        }
                    }
                }
            } else {
                let expected_flags = flags.expected & flags.flag_mask;
                if crate::utils::hint::unlikely((actual_header & flags.flag_mask) != expected_flags)
                {
                    // Mirror `parse_account`: only surface a decodable error so a
                    // 0 result (mismatch outside the required mask) falls through
                    // instead of returning `Err(from(0))`.
                    let err = crate::decode_header_error(actual_header, flags.expected, flags.mask);
                    if err != 0 {
                        return Err(ProgramError::from(err));
                    }
                }
            }
            // SAFETY: `base.add(offset)` is within the caller-provided output
            // buffer, and `raw` is the current account header.
            unsafe { core::ptr::write(base.add(offset), AccountView::new_unchecked(raw)) };
            // SAFETY: `raw` is valid for the current non-duplicate account.
            let data_len = unsafe { (*raw).data_len as usize };
            // SAFETY: Account entries are serialized as header + data + padding.
            let input = unsafe { input.add(ACCOUNT_HEADER.wrapping_add(data_len)) };
            // SAFETY: Advance over the SVM 8-byte alignment padding.
            let input = unsafe { input.add((input as usize).wrapping_neg() & 7) };
            Ok(input)
        } else {
            // Dup branch: borrow_state != NOT_BORROWED means the SVM
            // deduplicated this account slot.
            let idx = (actual_header & 0xFF) as usize;
            if crate::utils::hint::unlikely(idx >= offset) {
                return Err(ProgramError::InvalidAccountData);
            }

            if flags.is_optional {
                // Optional None uses the program id as a sentinel. Repeated
                // sentinels may be serialized as SVM duplicate entries, but
                // they are not aliases of a real user account.
                let orig_view = unsafe { core::ptr::read(base.add(idx)) };
                if crate::keys_eq(orig_view.address(), program_id) {
                    unsafe { core::ptr::write(base.add(offset), orig_view) };
                    let input = unsafe { input.add(core::mem::size_of::<u64>()) };
                    return Ok(input);
                }

                if !flags.allow_dup {
                    return Err(ProgramError::AccountBorrowFailed);
                }

                unsafe { core::ptr::write(base.add(offset), orig_view) };
                let input = unsafe { input.add(core::mem::size_of::<u64>()) };
                return Ok(input);
            }

            if !flags.allow_dup {
                // Dups are only accepted for explicit #[account(dup)] fields.
                return Err(ProgramError::AccountBorrowFailed);
            }

            // SAFETY: `idx < offset` means the source slot is already
            // initialized; the destination slot is within the output buffer.
            unsafe { core::ptr::write(base.add(offset), core::ptr::read(base.add(idx))) };
            // SAFETY: Duplicate entries are exactly one u64 in the SVM input.
            let input = unsafe { input.add(core::mem::size_of::<u64>()) };
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
/// Program entrypoint macros (`dispatch!`, `no_alloc!`, `panic_handler!`).
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
    #[cfg(not(target_os = "solana"))]
    {
        a == b
    }
    #[cfg(target_os = "solana")]
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
#[allow(unused_variables)]
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
}

#[cfg(kani)]
#[path = "../kani/lib.rs"]
mod kani_proofs;
