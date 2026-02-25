use {
    crate::{events::TakeEvent, state::EscrowAccount},
    quasar_core::prelude::*,
    quasar_spl::{TokenAccount, TokenProgram},
};

#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: &'info mut Signer,
    #[account(
        has_one = maker,
        has_one = maker_ta_b,
        constraint = escrow.receive > 0,
        seeds = [b"escrow", maker],
        bump = escrow.bump
    )]
    pub escrow: &'info mut Account<EscrowAccount>,
    pub maker: &'info mut UncheckedAccount,
    pub taker_ta_a: &'info mut Account<TokenAccount>,
    pub taker_ta_b: &'info mut Account<TokenAccount>,
    pub maker_ta_b: &'info mut Account<TokenAccount>,
    pub vault_ta_a: &'info mut Account<TokenAccount>,
    pub token_program: &'info TokenProgram,
}

impl<'info> Take<'info> {
    #[inline(always)]
    pub fn transfer_tokens(&mut self) -> Result<(), ProgramError> {
        self.token_program
            .transfer(
                self.taker_ta_b,
                self.maker_ta_b,
                self.taker,
                self.escrow.receive,
            )
            .invoke()
    }

    #[inline(always)]
    pub fn withdraw_tokens_and_close(&mut self, bumps: &TakeBumps) -> Result<(), ProgramError> {
        let seeds = bumps.escrow_seeds();

        self.token_program
            .transfer(
                self.vault_ta_a,
                self.taker_ta_a,
                self.escrow,
                self.vault_ta_a.amount(),
            )
            .invoke_signed(&seeds)?;

        self.token_program
            .close_account(self.vault_ta_a, self.taker, self.escrow)
            .invoke_signed(&seeds)
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(TakeEvent {
            escrow: *self.escrow.address(),
        });
        Ok(())
    }

    #[inline(always)]
    pub fn close_escrow(&mut self) -> Result<(), ProgramError> {
        self.escrow.close(self.maker.to_account_view())
    }
}
