#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Copy, Clone, QuasarSerialize)]
pub struct WalletConfig {
    threshold: u64,
    flags: u8,
}

#[derive(Copy, Clone, QuasarSerialize)]
pub struct CreateWalletArgs<T: InstructionArg> {
    config: WalletConfig,
    nonce: T,
}

#[derive(Accounts)]
pub struct CreateWallet {
    pub authority: Signer,
}

#[program]
mod test_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn create_wallet(
        _ctx: Ctx<CreateWallet>,
        _args: CreateWalletArgs<u64>,
    ) -> Result<(), ProgramError> {
        Ok(())
    }
}

fn main() {}
