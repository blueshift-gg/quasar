//! A program the features program calls, which records what each call gave it.
#![no_std]

use quasar_lang::prelude::*;

declare_id!("9d4dX5c6ntXTtzdRapGwT4nMzRwpvWPvzNdab4oFLCJ");

#[account(discriminator = 1)]
pub struct Tally {
    pub count: u64,
}

#[derive(Accounts)]
pub struct Record {
    #[account(mut)]
    pub record: UncheckedAccount,
    pub rest: Remaining<UncheckedAccount>,
}

#[derive(Accounts)]
pub struct Bump {
    #[account(mut)]
    pub tally: Account<Tally>,
}

/// A count only its holder raises.
#[account(discriminator = 2)]
pub struct Purse {
    pub holder: Address,
    pub count: u64,
}

#[derive(Accounts)]
pub struct Touch {
    pub holder: Signer,
    #[account(mut, has_one(holder))]
    pub purse: Account<Purse>,
}

#[derive(Accounts)]
pub struct Nothing {
    pub rest: Remaining<UncheckedAccount>,
}

/// A value `record` writes first.
#[derive(InstructionData)]
pub struct Payload {
    pub payload: u64,
}
#[program]
mod callee {
    use super::*;

    /// Writes `payload`, then each remaining account's key and its signer and
    /// writable flags, into `record`.
    #[instruction(0)]
    pub fn record(ctx: Ctx<Record, Payload>) -> Result<()> {
        let Record { mut record, rest } = ctx.accounts;
        let record = record.data_mut();
        record[..8].copy_from_slice(&ctx.data.payload.to_le_bytes());
        for (index, account) in rest.enumerate() {
            let account = account?;
            let at = 8 + index * 34;
            record[at..at + 32].copy_from_slice(account.address().as_ref());
            record[at + 32] = u8::from(account.entry().is_signer());
            record[at + 33] = u8::from(account.entry().is_writable());
        }
        Ok(())
    }

    #[instruction(1)]
    pub fn bump(mut ctx: Ctx<Bump>) -> Result<()> {
        let count = ctx.accounts.tally.count();
        ctx.accounts.tally.set_count(count + 1)?;
        Ok(())
    }

    #[instruction(2)]
    pub fn noop(_ctx: Ctx<Nothing>) -> Result<()> {
        Ok(())
    }

    #[instruction(3)]
    pub fn touch(mut ctx: Ctx<Touch>) -> Result<()> {
        let count = ctx.accounts.purse.count();
        ctx.accounts.purse.set_count(count + 1)?;
        Ok(())
    }
}
