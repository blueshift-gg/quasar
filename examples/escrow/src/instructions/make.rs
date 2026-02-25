use quasar_core::prelude::*;
use quasar_spl::{TokenAccount, TokenProgram};

use crate::{events::MakeEvent, state::EscrowAccount};

#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: &'info mut Signer,
    #[account(seeds = [b"escrow", maker], bump)]
    pub escrow: &'info mut Initialize<EscrowAccount>,
    pub maker_ta_a: &'info mut Account<TokenAccount>,
    pub maker_ta_b: &'info Account<TokenAccount>,
    pub vault_ta_a: &'info mut Account<TokenAccount>,
    pub rent: &'info Sysvar<Rent>,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

impl<'info> Make<'info> {
    #[inline(always)]
    pub fn make_escrow(&mut self, receive: u64, bumps: &MakeBumps) -> Result<(), ProgramError> {
        let seeds = bumps.escrow_seeds();

        EscrowAccount {
            maker: *self.maker.address(),
            mint_a: *self.maker_ta_a.mint(),
            mint_b: *self.maker_ta_b.mint(),
            maker_ta_b: *self.maker_ta_b.address(),
            receive,
            bump: bumps.escrow,
        }
        .init_signed(
            self.escrow,
            self.maker.to_account_view(),
            Some(&**self.rent),
            &[quasar_core::cpi::Signer::from(&seeds)],
        )
    }

    #[inline(always)]
    pub fn emit_event(&self, deposit: u64, receive: u64) -> Result<(), ProgramError> {
        emit!(MakeEvent {
            escrow: *self.escrow.address(),
            maker: *self.maker.address(),
            mint_a: *self.maker_ta_a.mint(),
            mint_b: *self.maker_ta_b.mint(),
            deposit,
            receive,
        });
        Ok(())
    }

    #[inline(always)]
    pub fn deposit_tokens(&mut self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .transfer(self.maker_ta_a, self.vault_ta_a, self.maker, amount)
            .invoke()
    }
}
