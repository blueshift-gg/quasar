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
/// Marker types for dynamic account fields (`String<'a, N>`, `Vec<'a, T, N>`).
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

/// 32-byte address comparison via four u64 word comparisons.
///
/// Short-circuits on the first non-matching word — wrong owner fails fast
/// on the first 8 bytes. Native-width u64 ops on SBF (64-bit target).
#[inline(always)]
pub fn keys_eq(a: &solana_address::Address, b: &solana_address::Address) -> bool {
    let a: &[u8] = a.as_ref();
    let b: &[u8] = b.as_ref();
    u64::from_le_bytes(a[..8].try_into().unwrap()) == u64::from_le_bytes(b[..8].try_into().unwrap())
        && u64::from_le_bytes(a[8..16].try_into().unwrap())
            == u64::from_le_bytes(b[8..16].try_into().unwrap())
        && u64::from_le_bytes(a[16..24].try_into().unwrap())
            == u64::from_le_bytes(b[16..24].try_into().unwrap())
        && u64::from_le_bytes(a[24..32].try_into().unwrap())
            == u64::from_le_bytes(b[24..32].try_into().unwrap())
}

/// Checks if an address is all zeros (the System program address).
///
/// OR-folds four u64 words — half the loads of a full comparison since
/// there's no second operand.
#[inline(always)]
pub fn is_system_program(addr: &solana_address::Address) -> bool {
    let a: &[u8] = addr.as_ref();
    u64::from_le_bytes(a[..8].try_into().unwrap())
        | u64::from_le_bytes(a[8..16].try_into().unwrap())
        | u64::from_le_bytes(a[16..24].try_into().unwrap())
        | u64::from_le_bytes(a[24..32].try_into().unwrap())
        == 0
}
