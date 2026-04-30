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

    /// Normal instruction — verifies framework pipeline still works alongside
    /// raw.
    #[instruction(discriminator = 0)]
    pub fn normal(ctx: Ctx<NormalInit>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    /// Raw instruction — reads a u64 from instruction data and writes it to the
    /// first account's data at offset 8 (past the 8-byte discriminator region).
    /// Verifies: signer check on account[1], data length validation, direct
    /// account mutation via AccountView.
    #[instruction(discriminator = 1, raw)]
    pub fn raw_write(ctx: Context) -> Result<(), ProgramError> {
        if ctx.accounts.len() < 2 {
            return Err(ProgramError::NotEnoughAccountKeys);
        }

        // Manual signer check on the second account.
        let authority = &ctx.accounts[1];
        if !authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }

        // Validate instruction data has at least 8 bytes.
        if ctx.data.len() < 8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        // Write the u64 from instruction data into account[0] data at offset 8.
        let target = &mut ctx.accounts[0];
        let value_bytes: [u8; 8] = ctx.data[..8].try_into().unwrap();
        unsafe {
            let data = target.borrow_unchecked_mut();
            if data.len() < 16 {
                return Err(ProgramError::AccountDataTooSmall);
            }
            core::ptr::copy_nonoverlapping(value_bytes.as_ptr(), data.as_mut_ptr().add(8), 8);
        }

        Ok(())
    }

    /// Raw + inline asm — uses sBPF `ldxdw`/`stxdw` to copy a u64 from
    /// instruction data into account[0] data at a fixed offset.
    /// Proves inline assembly works end-to-end in a raw handler.
    ///
    /// Accounts: [0] writable data account, [1] signer authority
    /// Data: 8 bytes (u64 little-endian value to write)
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

        const WRITE_OFFSET: usize = 8; // past discriminator region

        let dest = ctx.accounts[0].data_mut_ptr();
        let src = ctx.data.as_ptr();

        // On SBF: use inline asm to load 8 bytes from instruction data
        // and store them into account data at WRITE_OFFSET.
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

        // On non-SBF (host tests): equivalent in safe Rust.
        #[cfg(not(target_os = "solana"))]
        unsafe {
            core::ptr::copy_nonoverlapping(src, dest.add(WRITE_OFFSET), 8);
        }

        Ok(())
    }

    /// callx dispatch test — proves the SVM verifier accepts indirect calls
    /// through function pointers loaded at runtime. This is the foundation
    /// for O(1) jump table dispatch of raw instructions.
    ///
    /// Uses a 2-entry function pointer table. The discriminator data byte
    /// selects which function to call: 0 = write 0xAA, 1 = write 0xBB.
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
            unsafe { *view.borrow_unchecked_mut().get_unchecked_mut(8) = 0xAA };
        }
        fn write_bb(view: &mut AccountView) {
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
