//! Op-dispatch traits for account lifecycle operations.
//!
//! `AccountOp<Field>` is the single trait that all account operations
//! implement. The derive dispatches ALL ops at ALL lifecycle phases via UFCS.
//! Each op's trait consts (`HAS_BEFORE_LOAD`, `HAS_EXIT`, `REQUIRES_MUT`)
//! determine which phases are real — the derive is phase-agnostic.
//!
//! | Phase | Method | Gate |
//! |-------|--------|------|
//! | 1 | `before_load` | `HAS_BEFORE_LOAD` |
//! | 3a | `after_load` | always |
//! | 3b | `after_load_mut` | `REQUIRES_MUT` |
//! | 4 | `exit` | `HAS_EXIT` |
//!
//! Users declare ops directly on fields: `token(...)`, `close_program(...)`,
//! `migration(...)`. The derive doesn't know which ops run at which phase.

pub mod close_program;
pub mod init;
pub mod pda;

use {solana_account_view::AccountView, solana_program_error::ProgramError};

/// Runtime context shared across all op calls within a single parse invocation.
///
/// Carries `program_id` (always available) and lazily-fetched `Rent` (only
/// fetched when an op actually needs it).
pub struct OpCtx<'a> {
    pub program_id: &'a solana_address::Address,
    rent: core::cell::OnceCell<crate::sysvars::rent::Rent>,
}

impl<'a> OpCtx<'a> {
    #[inline(always)]
    pub fn new(program_id: &'a solana_address::Address) -> Self {
        Self {
            program_id,
            rent: core::cell::OnceCell::new(),
        }
    }

    /// Get rent, fetching from sysvar on first access.
    #[inline(always)]
    pub fn rent(&self) -> Result<&crate::sysvars::rent::Rent, ProgramError> {
        if let Some(r) = self.rent.get() {
            return Ok(r);
        }
        let r = <crate::sysvars::rent::Rent as crate::sysvars::Sysvar>::get()?;
        // Ignore set failure — race would mean another path already populated.
        let _ = self.rent.set(r);
        Ok(self.rent.get().unwrap())
    }
}

/// Trait for account operations dispatched by the derive macro via UFCS.
///
/// Each op struct (e.g., `init::Op`, `token::Op`) implements this trait for
/// the field types it supports. The derive emits fully-qualified calls:
///
/// ```text
/// <token::Op<'_> as AccountOp<Account<Token>>>::after_load(&op, &field, &ctx)?;
/// ```
///
/// # Const Safety Contract
///
/// Any op that mutates an account in `before_load`, `after_load_mut`, or `exit`
/// **MUST** set `REQUIRES_MUT = true`. The header writable-bit and
/// duplicate-safety rejection depend on this const being honest.
pub trait AccountOp<Field> {
    /// Op requires the target field to be writable.
    const REQUIRES_MUT: bool = false;

    /// Op has meaningful before_load behavior (Phase 1, raw slot).
    const HAS_BEFORE_LOAD: bool = false;

    /// Op has meaningful exit behavior (Phase 4, epilogue).
    const HAS_EXIT: bool = false;

    /// Phase 1: before typed construction. Raw `AccountView` slot.
    #[inline(always)]
    fn before_load(&self, _slot: &mut AccountView, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Phase 3a: after ALL fields loaded. Shared field ref.
    /// Called for all ops (validation, checks).
    #[inline(always)]
    fn after_load(&self, _field: &Field, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Phase 3b: after ALL fields loaded. Mutable field ref.
    /// Called ONLY for ops with `REQUIRES_MUT = true`, after `load_mut`.
    /// Use for post-load mutations (realloc, compact flush).
    #[inline(always)]
    fn after_load_mut(&self, _field: &mut Field, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Phase 4: epilogue. Mutable field ref.
    /// Called ONLY for `exit()` groups.
    #[inline(always)]
    fn exit(&self, _field: &mut Field, _ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        Ok(())
    }

    /// Contribute init params. Default is no-op.
    ///
    /// Ops that contribute (token, mint, ata_init) override this. The derive
    /// calls this on ALL groups — the default no-op ensures non-contributing
    /// ops compile and optimize to nothing.
    ///
    /// `params` is a pointer to `InitParams<'_>` for the field's init type.
    /// Overriding impls cast to the concrete params type. Sound because the
    /// derive constructs params with the correct associated type.
    #[inline(always)]
    fn apply_init_params(&self, _params: *mut u8) -> Result<(), ProgramError> {
        Ok(())
    }
}

/// Marker trait for account types that support realloc.
///
/// The `realloc::Op` in `quasar-spl` requires `F::BehaviorTarget:
/// SupportsRealloc` to ensure only realloc-capable accounts are used with
/// `realloc(...)`.
pub trait SupportsRealloc {}
