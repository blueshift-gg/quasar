//! Quasar — zero-copy Solana program framework.
//!
//! `quasar-core` provides the runtime primitives for building Solana programs
//! with Anchor-compatible ergonomics and minimal compute unit overhead. Account
//! data is accessed through pointer casts to `#[repr(C)]` companion structs —
//! no deserialization, no heap allocation.
//!
//! # Crate structure
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`accounts`] | Zero-copy account wrapper types (`Account`, `Initialize`, `Signer`) |
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
//! - **Alignment-1 guarantee**: Pod types and ZC companion structs are `#[repr(C)]`
//!   with alignment 1. Compile-time assertions verify this.
//! - **Bounds checking**: Account data length is validated during parsing before
//!   any pointer cast occurs.
//! - **Discriminator validation**: All-zero discriminators are banned at compile
//!   time. Account data is checked against the expected discriminator before access.
//!
//! Every `unsafe` block is validated by Miri under Tree Borrows with symbolic
//! alignment checking.

#![no_std]
extern crate self as quasar_core;

/// Internal re-exports for proc macro codegen. Not part of the public API.
/// Breaking changes to this module are not considered semver violations.
#[doc(hidden)]
pub mod __internal {
    pub use solana_account_view::{
        AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
    };

    // Header validation constants (little-endian u32)
    // Byte 0: borrow_state (0xFF = NOT_BORROWED)
    // Byte 1: is_signer (bit 8)
    // Byte 2: is_writable (bit 16)
    // Byte 3: executable (bit 24)
    pub const NODUP: u32 = 0xFF; // 0x000000FF - not borrowed only
    pub const NODUP_SIGNER: u32 = 0xFF | (1 << 8); // 0x000001FF - not borrowed + signer
    pub const NODUP_MUT: u32 = 0xFF | (1 << 16); // 0x000100FF - not borrowed + writable
    pub const NODUP_MUT_SIGNER: u32 = 0xFF | (1 << 8) | (1 << 16); // 0x000101FF - not borrowed + signer + writable
    pub const NODUP_EXECUTABLE: u32 = 0xFF | (1 << 24); // 0x010000FF - not borrowed + executable

    /// Allocation-free logging helper for generated code.
    /// Wraps solana_program_log::log for use in derive macro output.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn log_str(msg: &str) {
        solana_program_log::log(msg);
    }
}

/// Declarative macros: `define_account!`, `require!`, `require_eq!`, `emit!`.
#[macro_use]
pub mod macros;
/// Sysvar access and the `impl_sysvar_get!` helper macro.
#[macro_use]
pub mod sysvars;
/// Zero-copy account wrapper types for instruction handlers.
pub mod accounts;
/// Borsh-compatible serialization primitives for CPI instruction data.
pub mod borsh;
/// Compile-time account validation traits (`Address`, `Owner`, `Executable`, `Mutable`, `Signer`).
pub mod checks;
/// Off-chain instruction building utilities. Only compiled for non-SBF targets.
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub mod client;
/// Instruction context types (`Context`, `Ctx`).
pub mod context;
/// Const-generic cross-program invocation with stack-allocated account arrays.
pub mod cpi;
/// Marker types for dynamic fields (`String<P, N>`, `Vec<T, P, N>`) and codec helpers.
pub mod dynamic;
/// Program entrypoint macros (`dispatch!`, `no_alloc!`, `panic_handler!`).
pub mod entrypoint;
/// Framework error types.
pub mod error;
/// Event emission via `sol_log_data` and self-CPI.
pub mod event;
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

/// 32-byte address comparison via four `read_unaligned` u64 words.
///
/// Short-circuits on first mismatch. Uses `read_unaligned` to avoid
/// bounds-checked slicing, `Result` construction, and panic paths.
#[inline(always)]
pub fn keys_eq(a: &solana_address::Address, b: &solana_address::Address) -> bool {
    let a = a.as_array().as_ptr() as *const u64;
    let b = b.as_array().as_ptr() as *const u64;
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
/// Cold path — called only when the header comparison fails. Decomposes
/// the header `[borrow_state, is_signer, is_writable, executable]` to
/// determine which flag validation failed.
#[cold]
#[inline(never)]
#[allow(unused_variables)]
pub fn decode_header_error(header: u32, expected: u32) -> solana_program_error::ProgramError {
    use solana_program_error::ProgramError;

    let [borrow, signer, writable, _exec] = header.to_le_bytes();
    let [exp_borrow, exp_signer, exp_writable, exp_exec] = expected.to_le_bytes();

    if borrow != exp_borrow {
        #[cfg(feature = "debug")]
        solana_program_log::log("duplicate account detected");
        return ProgramError::AccountBorrowFailed;
    }
    if signer != exp_signer {
        #[cfg(feature = "debug")]
        solana_program_log::log("missing required signature");
        return ProgramError::MissingRequiredSignature;
    }
    if writable != exp_writable {
        #[cfg(feature = "debug")]
        solana_program_log::log("account not writable");
        return ProgramError::Immutable;
    }

    #[cfg(feature = "debug")]
    solana_program_log::log("account not executable");
    ProgramError::InvalidAccountData
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_address::Address;

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
