#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1, set_inner)]
pub struct Profile {
    pub bump: u8,
    pub name: String<32>,
    pub scores: Vec<u8, 10>,
}

#[derive(Accounts)]
pub struct InitProfile {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub profile: Account<Profile>,
    pub rent: Sysvar<Rent>,
}

impl InitProfile {
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        // Explicit path: rent from the declared sysvar field.
        self.profile.set_inner_with_rent(
            ProfileInner {
                bump: 1,
                name: "leo",
                scores: &[1, 2, 3],
            },
            self.payer.to_account_view(),
            self.rent.get(),
        )?;
        // Syscall path: no rent argument at all.
        self.profile.set_inner(
            ProfileInner {
                bump: 2,
                name: "leo",
                scores: &[4, 5, 6],
            },
            self.payer.to_account_view(),
        )
    }
}

fn main() {}
