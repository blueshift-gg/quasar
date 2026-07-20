#![no_std]
#![allow(dead_code)]
#![cfg_attr(target_os = "solana", feature(asm_experimental_arch))]

use quasar_lang::prelude::*;

declare_id!("RaW1111111111111111111111111111111111111112");

use quasar_derive::Accounts;

#[derive(Accounts)]
pub struct NormalInit {
    pub signer: Signer,
}

#[program]
mod quasar_test_raw {
    use super::*;

    /// Verifies normal instruction handling still works alongside raw handlers.
    #[instruction(discriminator = 0)]
    pub fn normal(ctx: Ctx<NormalInit>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    /// Writes a u64 from instruction data into the first account at offset 8.
    #[instruction(discriminator = 1, raw)]
    pub fn raw_write(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        let authority = &ctx.accounts[1];
        if !authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        if ctx.data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let target = &mut ctx.accounts[0];
        let value_bytes: [u8; 8] = ctx.data[..8].try_into().unwrap();
        // SAFETY: `target` is exclusively borrowed from the context; the
        // length check precedes the in-bounds eight-byte copy.
        unsafe {
            let data = target.borrow_unchecked_mut();
            if data.len() < 16 {
                return Err(ProgramError::AccountDataTooSmall);
            }
            core::ptr::copy_nonoverlapping(value_bytes.as_ptr(), data.as_mut_ptr().add(8), 8);
        }

        Ok(())
    }

    /// Writes through the account helper instead of manual pointer copying.
    #[instruction(discriminator = 6, raw)]
    pub fn raw_helper_write(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.is_empty() {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        if ctx.data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        ctx.accounts[0].write_bytes(8, &ctx.data[..8])
    }

    /// Copies a u64 with inline sBPF assembly in raw handlers.
    #[instruction(discriminator = 2, raw)]
    pub fn raw_asm_write(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        if !ctx.accounts[1].is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if ctx.data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        const WRITE_OFFSET: usize = 8;

        let dest = ctx.accounts[0].data_mut_ptr();
        let src = ctx.data.as_ptr();

        #[cfg(target_os = "solana")]
        unsafe {
            core::arch::asm!(
                "ldxdw r3, [r2+0]",
                "stxdw [r1+{offset}], r3",
                in("r1") dest,
                in("r2") src,
                offset = const WRITE_OFFSET,
                out("r3") _,
            );
        }

        #[cfg(not(target_os = "solana"))]
        // SAFETY: the test runtime supplies at least `WRITE_OFFSET + 8` bytes
        // for the target and `ctx.data` was checked to contain eight bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(src, dest.add(WRITE_OFFSET), 8);
        }

        Ok(())
    }

    /// Exercises indirect function-pointer dispatch in a raw handler.
    #[instruction(discriminator = 5, raw)]
    pub fn callx_dispatch(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.is_empty() {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        if ctx.data.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let target = &mut ctx.accounts[0];
        let selector = ctx.data[0] as usize;

        fn write_aa(view: &mut AccountView) {
            // SAFETY: this raw-handler fixture is invoked only with target
            // accounts containing byte offset eight.
            unsafe { *view.borrow_unchecked_mut().get_unchecked_mut(8) = 0xAA };
        }
        fn write_bb(view: &mut AccountView) {
            // SAFETY: same fixture contract as `write_aa`.
            unsafe { *view.borrow_unchecked_mut().get_unchecked_mut(8) = 0xBB };
        }

        type Handler = fn(&mut AccountView);
        let table: [Handler; 2] = [write_aa, write_bb];

        if selector < table.len() {
            table[selector](target);
        } else {
            return Err(ProgramError::InvalidInstructionData);
        }

        Ok(())
    }
}
