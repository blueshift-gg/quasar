#![allow(unexpected_cfgs)]
//! Proves that borrowed instruction args (&str, &[T]) with #[max(N)]
//! pass macro expansion and type-checking via compact Ref desugaring.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct BorrowedArgs {
    pub signer: Signer,
}

#[program]
pub mod test_borrowed_args {
    use super::*;

    #[instruction(discriminator = 1)]
    pub fn with_borrowed_str(
        ctx: Ctx<BorrowedArgs>,
        #[max(64)] label: &str,
    ) -> Result<(), ProgramError> {
        let _ = label.len();
        Ok(())
    }

    #[instruction(discriminator = 2)]
    pub fn with_borrowed_slice(
        ctx: Ctx<BorrowedArgs>,
        #[max(10)] data: &[u8],
    ) -> Result<(), ProgramError> {
        let _ = data.len();
        Ok(())
    }

    #[instruction(discriminator = 3)]
    pub fn with_mixed_args(
        ctx: Ctx<BorrowedArgs>,
        amount: u64,
        #[max(32)] name: &str,
    ) -> Result<(), ProgramError> {
        let _ = (amount, name);
        Ok(())
    }
}

fn main() {}
