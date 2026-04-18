#![allow(unexpected_cfgs)]
//! Proves that a borrowed QuasarSerialize struct with &'a str and &'a [T]
//! fields passes macro expansion and type-checking via compact Ref decode.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(QuasarSerialize)]
pub struct MintArgs<'a> {
    pub amount: u64,
    #[max(32)]
    pub name: &'a str,
    #[max(10)]
    pub recipients: &'a [Address],
}

#[derive(Accounts)]
pub struct Mint {
    pub signer: Signer,
}

#[program]
pub mod test_grouped_borrowed {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn mint(ctx: Ctx<Mint>, args: MintArgs<'_>) -> Result<(), ProgramError> {
        let _ = (args.amount, args.name, args.recipients);
        Ok(())
    }
}

fn main() {}
