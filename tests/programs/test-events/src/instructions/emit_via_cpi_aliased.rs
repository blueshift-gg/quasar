use {
    crate::{events::SimpleEvent, EventAuthority, QuasarTestEventsProgram},
    quasar_derive::Accounts,
    quasar_lang::prelude::*,
};

/// Event-CPI accounts whose program field is deliberately NOT named `program`.
/// Proves `#[derive(Accounts)]` detects the program field by type
/// (`Program<T>`) and wires `emit_cpi!` through the generated `EventCpi` impl,
/// so the macro no longer hard-codes the `program`/`event_authority` names.
#[derive(Accounts)]
pub struct EmitViaCpiAliased {
    pub signer: Signer,
    pub event_authority: EventAuthority,
    pub emitter: Program<QuasarTestEventsProgram>,
}

impl EmitViaCpiAliased {
    #[inline(always)]
    pub fn handler(&self, value: u64) -> Result<(), ProgramError> {
        emit_cpi!(SimpleEvent { value })?;
        Ok(())
    }
}
