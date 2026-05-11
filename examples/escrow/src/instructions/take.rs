use {
    crate::{events::TakeEvent, state::Escrow},
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
pub struct Take {
    #[account(mut)]
    pub taker: Signer,
    #[account(
        mut,
        has_one(maker),
        has_one(maker_ta_b),
        constraints(escrow.receive > 0),
        close(dest = taker),
        address = Escrow::seeds(maker.address())
    )]
    pub escrow: Account<Escrow>,
    #[account(mut)]
    pub maker: UncheckedAccount,
    pub mint_a: Account<Mint>,
    pub mint_b: Account<Mint>,
    #[account(init(idempotent), payer = taker, token(mint = mint_a, authority = taker, token_program = token_program))]
    pub taker_ta_a: Account<Token>,
    #[account(mut)]
    pub taker_ta_b: Account<Token>,
    #[account(init(idempotent), payer = taker, token(mint = mint_b, authority = maker, token_program = token_program))]
    pub maker_ta_b: Account<Token>,
    #[account(mut)]
    pub vault_ta_a: Account<Token>,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl Take {
    #[inline(always)]
    pub fn transfer_tokens(&mut self) -> Result<(), ProgramError> {
        self.token_program
            .transfer(
                &self.taker_ta_b,
                &self.maker_ta_b,
                &self.taker,
                self.escrow.receive,
            )
            .invoke()
    }

    #[inline(always)]
    pub fn withdraw_tokens_and_close(&self, bumps: &TakeBumps) -> Result<(), ProgramError> {
        let escrow_signer = self.escrow_signer(bumps);
        self.token_program
            .transfer(
                &self.vault_ta_a,
                &self.taker_ta_a,
                &self.escrow,
                self.vault_ta_a.amount(),
            )
            .invoke_signed(&escrow_signer)?;

        self.token_program
            .close_account(&self.vault_ta_a, &self.taker, &self.escrow)
            .invoke_signed(&escrow_signer)
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(TakeEvent {
            escrow: *self.escrow.address(),
        });
        Ok(())
    }
}
