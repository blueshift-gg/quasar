//! Convenience re-exports for Quasar programs.
//!
//! Most programs only need `use quasar::prelude::*` to access all framework
//! types, traits, macros, and account wrappers.

pub use {
    crate::{
        account_load::AccountLoad,
        accounts::*,
        checks,
        context::{Context, Ctx, CtxWithRemaining},
        cpi::{
            system::{System, SYSTEM_PROGRAM_ID},
            CpiReturn, DynCpiCall, Seed,
        },
        dispatch, emit,
        error::QuasarError,
        instruction_arg::{InstructionArg, InstructionArgDecode, OptionZc},
        no_alloc, panic_handler,
        pod::{
            PodBool, PodI128, PodI16, PodI32, PodI64, PodString, PodU128, PodU16, PodU32, PodU64,
            PodVec,
        },
        require, require_eq, require_keys_eq,
        return_data::set_return_data,
        sysvars::{clock::Clock, rent::Rent},
        traits::{
            AccountCheck, AccountCount, AsAccountView, CheckOwner, Discriminator, Event, HasSeeds,
            Id, Owner, Owners, ParseAccounts, ProgramInterface, Space, StaticView, ZeroCopyDeref,
        },
    },
    core::ops::{Deref, DerefMut},
    quasar_derive::{
        account, declare_program, emit_cpi, error_code, event, instruction, program, Accounts,
        QuasarSerialize,
    },
    solana_account_view::AccountView,
    solana_address::{address, declare_id, Address},
    solana_program_error::ProgramError,
    solana_program_log::log,
};
