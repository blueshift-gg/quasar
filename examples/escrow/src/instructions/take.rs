use {
    crate::{events::TakeEvent, state::Escrow},
    quasar_core::prelude::*,
    quasar_spl::{Mint, Token, TokenClose, TokenCpi},
};

#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: &'info mut Signer,
    #[account(
        has_one = maker,
        has_one = maker_ta_b,
        constraint = escrow.receive > 0,
        close = taker,
        seeds = [b"escrow", maker],
        bump = escrow.bump
    )]
    pub escrow: &'info mut Account<Escrow>,
    pub maker: &'info mut UncheckedAccount,
    pub mint_a: &'info Account<Mint>,
    pub mint_b: &'info Account<Mint>,
    #[account(init_if_needed, payer = taker, token::mint = mint_a, token::authority = taker)]
    pub taker_ta_a: &'info mut Account<Token>,
    pub taker_ta_b: &'info mut Account<Token>,
    #[account(init_if_needed, payer = taker, token::mint = mint_b, token::authority = maker)]
    pub maker_ta_b: &'info mut Account<Token>,
    pub vault_ta_a: &'info mut Account<Token>,
    pub rent: &'info Sysvar<Rent>,
    pub token_program: &'info Program<Token>,
    pub system_program: &'info Program<System>,
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

        self.vault_ta_a
            .close(self.token_program, self.taker, self.escrow)
            .invoke_signed(&seeds)
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(TakeEvent {
            escrow: *self.escrow.address(),
        });
        Ok(())
    }
}
