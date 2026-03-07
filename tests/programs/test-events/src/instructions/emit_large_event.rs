use quasar_core::prelude::*;

use crate::events::LargeEvent;

#[derive(Accounts)]
pub struct EmitLargeEvent<'info> {
    pub signer: &'info Signer,
}

impl<'info> EmitLargeEvent<'info> {
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn handler(
        &self,
        a: u64,
        b: u64,
        c: u64,
        d: u64,
        e: Address,
        f: Address,
        g: u128,
        h: u128,
    ) -> Result<(), ProgramError> {
        emit!(LargeEvent {
            a,
            b,
            c,
            d,
            e,
            f,
            g,
            h,
        });
        Ok(())
    }
}
