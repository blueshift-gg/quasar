//! A vault that pays out only with the approval of a threshold of its
//! members, who sign as remaining accounts. Current Quasar's multisig
//! example, written here.
#![no_std]

use quasar_lang::{prelude::*, zeropod::ArrayString};

declare_id!("44444444444444444444444444444444444444444444");

/// The most members a multisig has, and so the most approvals an execution
/// counts.
pub const MAX_MEMBERS: usize = 10;

#[account(discriminator = 1)]
#[seeds(b"multisig", creator: Address)]
pub struct MultisigConfig {
    pub creator: Address,
    pub threshold: u8,
    pub bump: u8,
    #[max_len(32)]
    pub label: String,
    #[max_len(10)]
    pub members: Vec<Address>,
}

#[derive(Seeds)]
#[seeds(b"vault", config: Address)]
pub struct MultisigVault;

/// How many members must approve a payout.
#[derive(InstructionData)]
pub struct Threshold {
    pub threshold: u8,
}

/// Lamports to move.
#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

#[derive(InstructionData)]
pub struct Label {
    pub label: ArrayString<32>,
}

#[derive(Accounts)]
pub struct Create {
    #[account(mut)]
    pub creator: Signer,
    #[account(init(payer = creator), seeds(creator))]
    pub config: Pda<Account<MultisigConfig>>,
    pub rent: Sysvar<Rent>,
    pub system_program: Program<SystemProgram>,
    /// The members, each signing; duplicates are rejected as they load.
    pub members: Remaining<Signer>,
}

#[derive(Accounts)]
pub struct Deposit {
    #[account(mut)]
    pub depositor: Signer,
    pub config: Account<MultisigConfig>,
    #[account(mut, seeds(config))]
    pub vault: Pda<UncheckedAccount, MultisigVault>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct SetLabel {
    #[account(mut)]
    pub creator: Signer,
    #[account(mut, has_one(creator), seeds(creator), realloc(payer = creator))]
    pub config: Pda<Account<MultisigConfig>>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct ExecuteTransfer {
    #[account(has_one(creator), seeds(creator))]
    pub config: Pda<Account<MultisigConfig>>,
    pub creator: UncheckedAccount,
    #[account(mut, seeds(config))]
    pub vault: Pda<UncheckedAccount, MultisigVault>,
    #[account(mut)]
    pub recipient: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
    /// The approving members, each signing.
    pub approvals: Remaining<Signer>,
}
#[program]
mod multisig {
    use super::*;

    #[instruction(0)]
    pub fn create(mut ctx: Ctx<Create, Threshold>) -> Result<()> {
        let threshold = ctx.data.threshold;
        let mut members = [Address::default(); MAX_MEMBERS];
        let mut count = 0;
        for member in &mut ctx.accounts.members {
            let slot = members.get_mut(count).ok_or(ProgramError::InvalidArgument)?;
            *slot = *member?.address();
            count += 1;
        }
        require!(threshold > 0, ProgramError::InvalidArgument);
        require!(usize::from(threshold) <= count, ProgramError::InvalidArgument);
        let config = &mut ctx.accounts.config;
        let bump = config.bump();
        config.set_creator(*ctx.accounts.creator.address())?;
        config.set_threshold(threshold)?;
        config.set_bump(bump)?;
        config.set_members(&members[..count])?;
        Ok(())
    }

    #[instruction(1)]
    pub fn deposit(mut ctx: Ctx<Deposit, Amount>) -> Result<()> {
        let Deposit { depositor, vault, system_program, .. } = &mut ctx.accounts;
        system_program.transfer(depositor, vault, ctx.data.amount).invoke()
    }

    #[instruction(2)]
    pub fn set_label(mut ctx: Ctx<SetLabel, Label>) -> Result<()> {
        ctx.accounts.config.set_label(&ctx.data.label)?;
        Ok(())
    }

    #[instruction(3)]
    pub fn execute_transfer(ctx: Ctx<ExecuteTransfer, Amount>) -> Result<()> {
        let ExecuteTransfer { config, mut vault, mut recipient, system_program, approvals, .. } =
            ctx.accounts;
        require!(approvals.len() <= MAX_MEMBERS, ProgramError::InvalidArgument);
        let mut approved = 0;
        for approval in approvals {
            // Loaded accounts are distinct, so each member counts once.
            approved += u8::from(config.members().contains(approval?.address()));
        }
        require!(approved >= config.threshold(), ProgramError::MissingRequiredSignature);
        let config_key = *config.address();
        let signer = MultisigVault::seeds(&config_key).with_bump(vault.bump());
        system_program.transfer(&mut vault, &mut recipient, ctx.data.amount).invoke_signed(&signer)
    }
}
