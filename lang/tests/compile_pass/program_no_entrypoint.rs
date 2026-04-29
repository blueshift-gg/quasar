#![allow(unexpected_cfgs)]
//! Proves that `#[program(no_entrypoint)]` compiles: the entrypoint is not
//! generated and `__dispatch` is pub, so a custom entrypoint can call it.
//! This follows Anchor 0.30's pattern where programs skip the entrypoint
//! and write their own, calling back into the framework for fallthrough.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Init {
    pub signer: Signer,
}

#[program(no_entrypoint)]
pub mod test_no_entrypoint {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<Init>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = 1, raw)]
    pub fn hot_path(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }
}

// Verify __dispatch is accessible outside the module (pub).
fn _verify_dispatch_is_pub() {
    let _: fn(*mut u8, &[u8]) -> Result<(), ProgramError> =
        test_no_entrypoint::__dispatch;
}

fn main() {}
