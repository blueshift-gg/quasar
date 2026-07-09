use {
    crate::{
        events::MakeEvent,
        state::{Escrow, EscrowInner},
    },
    quasar_lang::prelude::*,
    quasar_spl::prelude::*,
};

/// Canonical `#[derive(Accounts)]` tutorial: the maker opens an escrow and
/// deposits into a vault. Each directive is annotated inline.
#[derive(Accounts)]
pub struct Make {
    // `mut`: the maker pays rent/fees, so its lamports change -> must be writable.
    #[account(mut)]
    pub maker: Signer,
    // `init`: create + fund this PDA account (payer = maker). `address = ...`
    // pins it to the typed-seeds PDA, so the derive also fills `bumps.escrow`.
    #[account(init, payer = maker, address = Escrow::seeds(maker.address()))]
    pub escrow: Account<Escrow>,
    // Plain `Account<Mint>`: owner + discriminator checked, read-only.
    pub mint_a: Account<Mint>,
    pub mint_b: Account<Mint>,
    // `mut`: tokens leave this account during the deposit CPI.
    #[account(mut)]
    pub maker_ta_a: Account<Token>,
    // `init(idempotent)` + `token(...)`: create the token account if it does not
    // already exist. `token(...)` is an SPL *behavior* (owned by quasar-spl, not
    // the derive); it runs the SPL init CPI with the given mint/authority.
    #[account(init(idempotent), payer = maker, token(mint = mint_b, authority = maker, token_program = token_program))]
    pub maker_ta_b: Account<Token>,
    // The vault's authority is the `escrow` PDA, so only this program can move
    // its tokens.
    #[account(init(idempotent), payer = maker, token(mint = mint_a, authority = escrow, token_program = token_program))]
    pub vault_ta_a: Account<Token>,
    // `Sysvar<Rent>` / `Program<...>`: type-checked well-known accounts the init
    // and CPI machinery needs; no directive required.
    pub rent: Sysvar<Rent>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}

impl Make {
    #[inline(always)]
    pub fn make_escrow(&mut self, receive: u64, bumps: &MakeBumps) -> Result<(), ProgramError> {
        self.escrow.set_inner(EscrowInner {
            maker: *self.maker.address(),
            mint_a: *self.mint_a.address(),
            mint_b: *self.mint_b.address(),
            maker_ta_b: *self.maker_ta_b.address(),
            receive,
            bump: bumps.escrow,
        });
        Ok(())
    }

    #[inline(always)]
    pub fn emit_event(&self, deposit: u64, receive: u64) -> Result<(), ProgramError> {
        emit!(MakeEvent {
            escrow: *self.escrow.address(),
            maker: *self.maker.address(),
            mint_a: *self.mint_a.address(),
            mint_b: *self.mint_b.address(),
            deposit,
            receive,
        });
        Ok(())
    }

    #[inline(always)]
    pub fn deposit_tokens(&mut self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .transfer(&self.maker_ta_a, &self.vault_ta_a, &self.maker, amount)
            .invoke()
    }
}
