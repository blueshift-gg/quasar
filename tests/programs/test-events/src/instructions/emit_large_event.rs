use {crate::events::LargeEvent, quasar_derive::Accounts, quasar_lang::prelude::*};

#[derive(Accounts)]
pub struct EmitLargeEvent {
    pub signer: Signer,
}

impl EmitLargeEvent {
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
