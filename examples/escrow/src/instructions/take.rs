use {
    crate::{events::TakeEvent, state::Escrow},
    quasar_lang::{cpi::Seed, prelude::*},
    quasar_spl::prelude::*,
};

/// Canonical tutorial for the *validation* directives: the taker fulfills the
/// escrow. See `make.rs` for the `init`/behavior directives.
#[derive(Accounts)]
pub struct Take {
    #[account(mut)]
    pub taker: Signer,
    // A compound directive set on one account, evaluated in order:
    //  - `has_one(maker)` / `has_one(maker_ta_b)`: the stored field must equal the same-named
    //    account passed in (relationship check).
    //  - `constraints(...)`: an arbitrary boolean the account must satisfy.
    //  - `close(dest = taker)`: after the handler, zero the account and refund its rent lamports
    //    to `taker`.
    //  - `address = Escrow::seeds(maker.address())`: verify this is the expected PDA (and populate
    //    `bumps.escrow`).
    #[account(
        mut,
        has_one(maker),
        has_one(maker_ta_b),
        constraints(escrow.receive > 0),
        close(dest = taker),
        address = Escrow::seeds(maker.address())
    )]
    pub escrow: Account<Escrow>,
    // `UncheckedAccount`: no owner/type check. Sound here because `has_one(maker)`
    // above already pinned it to the address stored in `escrow`.
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
        let bump = [bumps.escrow];
        let seeds = [
            Seed::from(b"escrow" as &[u8]),
            Seed::from(self.maker.address().as_ref()),
            Seed::from(bump.as_ref()),
        ];

        self.token_program
            .transfer(
                &self.vault_ta_a,
                &self.taker_ta_a,
                &self.escrow,
                self.vault_ta_a.amount(),
            )
            .invoke_signed(&seeds)?;

        self.token_program
            .close_account(&self.vault_ta_a, &self.taker, &self.escrow)
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
