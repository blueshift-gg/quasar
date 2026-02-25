use {
    crate::{events::RefundEvent, state::EscrowAccount},
    quasar_core::prelude::*,
    quasar_spl::{TokenAccount, TokenProgram},
};

#[derive(Accounts)]
pub struct Refund<'info> {
    pub maker: &'info mut Signer,
    #[account(
        has_one = maker,
        seeds = [b"escrow", maker],
        bump = escrow.bump
    )]
    pub escrow: &'info mut Account<EscrowAccount>,
    pub maker_ta_a: &'info mut Account<TokenAccount>,
    pub vault_ta_a: &'info mut Account<TokenAccount>,
    pub token_program: &'info TokenProgram,
}

impl<'info> Refund<'info> {
    #[inline(always)]
    pub fn withdraw_tokens_and_close(&mut self, bumps: &RefundBumps) -> Result<(), ProgramError> {
        let seeds = bumps.escrow_seeds();

        self.token_program
            .transfer(
                self.vault_ta_a,
                self.maker_ta_a,
                self.escrow,
                self.vault_ta_a.amount(),
            )
            .invoke_signed(&seeds)?;

        self.token_program
            .close_account(self.vault_ta_a, self.maker, self.escrow)
            .invoke_signed(&seeds)
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(RefundEvent {
            escrow: *self.escrow.address(),
        });
        Ok(())
    }

    #[inline(always)]
    pub fn close_escrow(&mut self) -> Result<(), ProgramError> {
        self.escrow.close(self.maker.to_account_view())
    }
}
