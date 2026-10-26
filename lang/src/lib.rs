//! Quasar's runtime. It turns the input the Solana runtime hands a program
//! into typed accounts, and gives handlers the effects they may perform.
//!
//! `#[inline(always)]` marks the hot path. Each one was measured on the
//! vault benchmark, and `make bench-check` fails if removing one costs.
#![no_std]

// Account fields name `String` and `Vec`, and zeropod reads them in place:
// nothing is allocated on chain. Off chain, `std` supplies the panic handler
// and allocator that a `no_std` program's host build (for tests) would lack.
extern crate alloc;
#[cfg(not(target_os = "solana"))]
extern crate std;

/// On chain nothing allocates: every read and write is in place. An account's
/// `String` and `Vec` fields still name `alloc`'s types, so the program links
/// `alloc`, whose allocator this is: every request fails.
#[cfg(target_os = "solana")]
#[global_allocator]
static NO_ALLOCATION: Refuse = Refuse;

#[cfg(target_os = "solana")]
struct Refuse;

// SAFETY: `alloc` never returns memory, which `GlobalAlloc` allows.
#[cfg(target_os = "solana")]
unsafe impl alloc::alloc::GlobalAlloc for Refuse {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {}
}

pub mod account;
#[cfg(not(target_os = "solana"))]
pub mod client;
pub mod context;
pub mod cpi;
pub mod data;
pub mod effects;
pub mod error;
pub mod field;
pub mod input;
pub mod math;
pub mod pda;
pub mod relation;
pub mod remaining;
pub mod system;
pub mod sysvar;
#[cfg(kani)]
pub mod verify;
pub mod wrapper;

mod syscalls;

#[cfg(test)]
mod miri;

pub use solana_address::Address;
pub use solana_program_error::ProgramError;
pub use zeropod;

/// Both `Result<()>` and `Result<(), ProgramError>` read naturally.
pub type Result<T = (), E = ProgramError> = core::result::Result<T, E>;

pub use context::Accounts;
pub use data::InstructionData;
pub use quasar_derive::{
    Accounts, InstructionData, Seeds, account, error_code, event, instruction, program, property,
};

/// Keeps generated client code out of the SBF build. The `cfg` lives here, so
/// program crates never need to know the `solana` target.
#[cfg(not(target_os = "solana"))]
#[macro_export]
macro_rules! host_only {
    ($($item:item)*) => { $($item)* };
}

#[cfg(target_os = "solana")]
#[macro_export]
macro_rules! host_only {
    ($($item:item)*) => {};
}

/// Export the VM entrypoint on chain and a crate-specific symbol on the host.
/// Keeping the host function reachable also checks dispatch's const assertions.
#[doc(hidden)]
#[cfg(not(target_os = "solana"))]
#[macro_export]
macro_rules! export_entrypoint {
    ($item:item) => {
        #[unsafe(export_name = concat!(module_path!(), "__quasar_entrypoint"))]
        $item
    };
}

#[doc(hidden)]
#[cfg(target_os = "solana")]
#[macro_export]
macro_rules! export_entrypoint {
    ($item:item) => {
        #[unsafe(no_mangle)]
        $item
    };
}

/// Keeps verification code out of every build but Kani's.
#[cfg(kani)]
#[macro_export]
macro_rules! kani_only {
    ($($item:item)*) => { $($item)* };
}

#[cfg(not(kani))]
#[macro_export]
macro_rules! kani_only {
    ($($item:item)*) => {};
}

/// Declares the program's address as `ID`.
#[macro_export]
macro_rules! declare_id {
    ($address:literal) => {
        pub const ID: $crate::Address = $crate::Address::from_str_const($address);

        /// Carries each account kind's `UniqueDiscriminator` impl.
        #[doc(hidden)]
        pub struct __Kinds;

        /// This program, for `Program<..>`; `#[program]` names it.
        #[doc(hidden)]
        pub struct __Program;

        impl $crate::wrapper::ProgramId for __Program {
            const ID: $crate::Address = ID;
        }
    };
}

pub mod prelude {
    // Account fields as written: `#[max_len(32)] label: String`.
    pub use alloc::{string::String, vec::Vec};

    pub use crate::{
        Accounts, Address, InstructionData, ProgramError, Result, Seeds, account,
        account::{Account, InterfaceAccount},
        context::Ctx,
        declare_id, emit, error_code, event, instruction,
        pda::Pda,
        program, property,
        remaining::Remaining,
        require,
        sysvar::{Clock, Rent, Sysvar, SysvarData},
        wrapper::{
            Interface, Program, Signer, SystemAccount, SystemProgram, UncheckedAccount, Wrapper,
        },
    };
}
