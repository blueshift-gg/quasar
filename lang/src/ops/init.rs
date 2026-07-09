//! Init op: account initialization via system program CPI.
//!
//! `init::Op` calls `AccountInit::init` on the account type when
//! the account is owned by the system program (uninitialized). When
//! `idempotent = true`, already-initialized accounts are silently accepted.

use {
    super::{OpCtx, RentAccess},
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
/// Generic `Params` defaults to `()` for plain `#[account]` types.
/// Init contributors (token, mint, associated token) populate params via
/// capability traits before this op runs.
pub struct Op<'a, Params = ()> {
    pub payer: &'a AccountView,
    pub space: u64,
    pub signers: &'a [Signer<'a, 'a>],
    pub params: Params,
    pub idempotent: bool,
}

impl<'a, P> Op<'a, P> {
    /// Execute the init operation on a raw account slot.
    ///
    /// Unlike [`realloc::Op::apply`](crate::ops::realloc::Op::apply) and
    /// [`close::Op::apply`](crate::ops::close::Op::apply), `ctx` is bound to
    /// the op's own lifetime (`&'a OpCtx<'a, R>`) rather than a free
    /// `&OpCtx<'_, R>`. This is required, not incidental: [`InitCtx<'a>`]
    /// carries `payer`, `program_id`, `signers`, and `rent` under a single
    /// lifetime, and `payer`/ `signers` come from `Op<'a>`, so
    /// `ctx.program_id` and `&ctx.rent` must also outlive `'a`. Relaxing it
    /// would require decoupling `InitCtx`'s lifetimes.
    #[inline(always)]
    pub fn apply<F, R>(
        &self,
        slot: &mut AccountView,
        ctx: &'a OpCtx<'a, R>,
    ) -> Result<(), ProgramError>
    where
        F: AccountLoad + AccountInit<InitParams<'a> = P>,
        R: RentAccess,
    {
        if crate::is_system_program(slot.owner()) {
            // SAFETY: `slot` is borrowed for the duration of this inlined call.
            // `AccountInit::init` does not retain the `target` reference.
            let target = unsafe { &mut *(slot as *mut AccountView) };
            <F as AccountInit>::init(
                InitCtx {
                    payer: self.payer,
                    target,
                    program_id: ctx.program_id,
                    space: self.space,
                    signers: self.signers,
                    rent: &ctx.rent,
                },
                &self.params,
            )?;
        } else if !self.idempotent {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        Ok(())
    }
}
