//! Init op: Phase 1 account initialization via system program CPI.
//!
//! `init::Op` calls `AccountInit::init` on the field's behavior target when
//! the account is owned by the system program (uninitialized). When
//! `idempotent = true`, already-initialized accounts are silently accepted.

use {
    super::{AccountOp, OpCtx},
    crate::{
        account_init::{AccountInit, InitCtx},
        account_load::AccountLoad,
        cpi::Signer,
    },
    solana_account_view::AccountView,
    solana_program_error::ProgramError,
};

/// Init operation. Constructed by the derive macro from `init(...)` syntax.
///
/// Generic `Params` defaults to `()` for plain `#[account]` types. SPL ops
/// override `AccountOp::apply_init_params` to contribute their own init
/// params (e.g., `token(mint = ..., authority = ...)` → `TokenInitParams`).
pub struct Op<'a, Params = ()> {
    pub payer: &'a AccountView,
    pub space: u64,
    pub signers: &'a [Signer<'a, 'a>],
    pub params: Params,
    pub idempotent: bool,
}

impl<'a, F, P> AccountOp<F> for Op<'a, P>
where
    F: AccountLoad,
    <F as AccountLoad>::BehaviorTarget: AccountInit<InitParams<'a> = P>,
{
    const HAS_BEFORE_LOAD: bool = true;
    const REQUIRES_MUT: bool = true;

    #[inline(always)]
    fn before_load(&self, slot: &mut AccountView, ctx: &OpCtx<'_>) -> Result<(), ProgramError> {
        if crate::is_system_program(slot.owner()) {
            type Target<F2> = <F2 as AccountLoad>::BehaviorTarget;
            // SAFETY: All references (payer, slot, program_id, signers, rent)
            // are live for the duration of this #[inline(always)] call.
            // InitCtx<'a> requires a single lifetime but our references come
            // from different sources — the pointer casts unify them. Sound
            // because init() completes before any reference is dropped.
            let target = unsafe { &mut *(slot as *mut AccountView) };
            let program_id = unsafe { &*(ctx.program_id as *const solana_address::Address) };
            let rent = ctx.rent()?;
            let rent = unsafe { &*(rent as *const crate::sysvars::rent::Rent) };
            <Target<F> as AccountInit>::init(
                InitCtx {
                    payer: self.payer,
                    target,
                    program_id,
                    space: self.space,
                    signers: self.signers,
                    rent,
                },
                &self.params,
            )?;
        } else if !self.idempotent {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        Ok(())
    }
}
