//! Convenience re-exports for Quasar programs.
//!
//! Most programs only need `use quasar_lang::prelude::*` to access all
//! framework types, traits, macros, and account wrappers.
//
// Umbrella-crate decision (OPEN): there is deliberately no `quasar` library
// crate that re-exports this prelude as `quasar::prelude`. The short name
// `quasar` is owned by the CLI package (`cli/`), so shipping a `quasar` lib
// would collide with it. Whether to publish a thin umbrella lib (renaming the
// CLI, or namespacing it) is a maintainer decision left open by Workstream H;
// until then, programs depend on `quasar-lang` directly. Because generated code
// resolves the runtime crate by its dependency name (see `derive/src/krate.rs`),
// a future rename to `quasar` would not require touching any emitter.
//
// Author-facing vs plumbing: the traits re-exported by [`internal`] below are
// compiler/runtime plumbing that authors rarely name directly. They stay in
// this top-level glob because macro-generated code relies on them being in
// scope through `use quasar_lang::prelude::*` (it calls trait methods like
// `.to_account_view()`, `Self::COUNT`, `.epilogue()`, and emits
// `unsafe impl StaticView for ...`). Moving them out of the glob would break
// every program's generated code, so the split is documentation-only: `internal`
// re-exports the same items for readers and for explicit imports.

pub use {
    crate::{
        // --- author-facing: the surface programs write against ---
        account_behavior::AccountBehavior,
        account_init::{AccountInit, InitCtx},
        account_load::AccountLoad,
        accounts::*,
        address::AddressVerify,
        checks,
        context::{Context, Ctx, CtxWithRemaining},
        cpi::{
            system::{SystemProgram, SYSTEM_PROGRAM_ID},
            CpiCall, CpiDynamic, CpiReturn, CpiSignerSeeds,
        },
        emit,
        error::QuasarError,
        heap_alloc, no_alloc, panic_handler,
        instruction_arg::{InstructionArg, OptionZc},
        pod::{
            PodBool, PodI128, PodI16, PodI32, PodI64, PodString, PodU128, PodU16, PodU32, PodU64,
            PodVec,
        },
        remaining::{Remaining, RemainingAccount, RemainingAccounts},
        require, require_eq, require_keys_eq,
        return_data::set_return_data,
        sysvars::{clock::Clock, rent::Rent},
        // `String`/`Vec` are REBINDS, not `alloc`'s heap collections: they are
        // `pod::PodString<N>` / `pod::PodVec<T, N>`, fixed-capacity value types
        // (`#[account]`/`#[instruction]` field types) that need no allocator.
        // The rebind lets programs write `String<32>` / `Vec<T, N>` in a
        // `#![no_std]`, `no_alloc!` crate. Do not `use alloc::string::String`
        // alongside this glob.
        String, Vec,
        // --- plumbing: also grouped under `internal` (see module note above) ---
        traits::{
            AccountCount, AccountGroup, AsAccountView, CheckOwner, Discriminator, Event, HasSeeds,
            Id, Owner, Owners, ParseAccounts, ProgramInterface, Space, StaticView, ZeroCopyDeref,
        },
        ZcElem, ZcField, ZcValidate, ZeroPodError,
    },
    core::ops::{Deref, DerefMut},
    quasar_derive::{
        account, declare_program, emit_cpi, error_code, event, instruction, program, Accounts,
        QuasarSerialize, Seeds,
    },
    solana_account_view::AccountView,
    solana_address::{address, declare_id, Address},
    solana_program_error::ProgramError,
    solana_program_log::log,
};

/// Compiler/runtime plumbing traits, grouped for documentation and explicit
/// import.
///
/// These are the traits macro-generated code depends on but that programs
/// rarely name directly (owner/discriminator checks, account counting, the
/// zero-copy borrow machinery, the `repr(transparent)` [`StaticView`] guard).
/// They are ALSO re-exported from the top-level prelude glob because generated
/// code needs them in scope via `use quasar_lang::prelude::*`; this module does
/// not remove them from the glob, it only names the plumbing subset so readers
/// can see the author/plumbing boundary and import it deliberately if they glob
/// only the author-facing surface.
pub mod internal {
    pub use crate::traits::{
        AccountCount, AccountGroup, AsAccountView, CheckOwner, Discriminator, HasSeeds, Owners,
        ParseAccounts, ProgramInterface, StaticView, ZeroCopyDeref,
    };
    pub use crate::{ZcElem, ZcField, ZcValidate, ZeroPodError};
}
