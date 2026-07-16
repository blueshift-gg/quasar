use {quasar_lang::prelude::*, quasar_spl::prelude::*};

#[account(discriminator = 2)]
#[seeds(b"lifecycle", owner: Address)]
pub struct LifecycleAuthority {
    pub owner: Address,
    pub bump: u8,
}

/// Sweeps and closes a token vault before closing its PDA authority.
///
/// The authority is deliberately declared before the token account. This
/// catches epilogue implementations that close state in field order and make
/// the PDA unavailable to sign later lifecycle CPIs.
#[derive(Accounts)]
pub struct PdaSweepAndClose {
    #[account(mut)]
    pub owner: Signer,
    #[account(
        mut,
        close(dest = owner),
        address = LifecycleAuthority::seeds(owner.address())
    )]
    pub authority: Account<LifecycleAuthority>,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program),
        token_close(dest = owner, authority = authority, token_program = token_program)
    )]
    pub source: Account<Token>,
    #[account(mut)]
    pub receiver: Account<Token>,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
}

impl PdaSweepAndClose {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}
