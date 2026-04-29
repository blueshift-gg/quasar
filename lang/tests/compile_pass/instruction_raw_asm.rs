#![allow(unexpected_cfgs)]
//! Proves that inline assembly is valid inside a `#[instruction(raw)]` handler.
//! The raw handler is a plain function — no macro wrapping prevents `asm!()`.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Normal {
    pub signer: Signer,
}

#[program]
pub mod test_raw_asm {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn setup(ctx: Ctx<Normal>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = 1, raw)]
    pub fn fast_update(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.is_empty() {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        // Inline asm — works because raw emits the function unchanged.
        // Use a target-appropriate nop for the compile-pass test.
        unsafe {
            #[cfg(target_arch = "x86_64")]
            core::arch::asm!("nop");
            #[cfg(target_arch = "aarch64")]
            core::arch::asm!("nop");
            // On SBF, you would write sBPF instructions here:
            // core::arch::asm!("mov64 r0, 0");
        }

        Ok(())
    }
}

fn main() {}
