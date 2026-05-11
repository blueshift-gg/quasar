use {
    crate::{events::RefundEvent, state::Escrow},
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

#[derive(Accounts)]
pub struct Refund {
    #[account(mut)]
    pub maker: Signer,
    #[account(
        mut,
        has_one(maker),
        close(dest = maker),
        address = Escrow::seeds(maker.address())
    )]
    pub escrow: Account<Escrow>,
    pub mint_a: Account<Mint>,
    #[account(init(idempotent), payer = maker, token(mint = mint_a, authority = maker, token_program = token_program))]
    pub maker_ta_a: Account<Token>,
    #[account(mut)]
    pub vault_ta_a: Account<Token>,
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl Refund {
    #[inline(always)]
    pub fn withdraw_tokens_and_close(&self, bumps: &RefundBumps) -> Result<(), ProgramError> {
        let escrow_signer = self.escrow_signer(bumps);
        self.token_program
            .transfer(
                &self.vault_ta_a,
                &self.maker_ta_a,
                &self.escrow,
                self.vault_ta_a.amount(),
            )
            .invoke_signed(&escrow_signer)?;

        self.token_program
            .close_account(&self.vault_ta_a, &self.maker, &self.escrow)
            .invoke_signed(&escrow_signer)
    }

    #[inline(always)]
    pub fn emit_event(&self) -> Result<(), ProgramError> {
        emit!(RefundEvent {
            escrow: *self.escrow.address(),
        });
        Ok(())
    }
}
