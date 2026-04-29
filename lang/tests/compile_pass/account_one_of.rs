#![allow(unexpected_cfgs)]
//! Proves that `#[account(one_of)]` compiles on enum declarations,
//! generates the ref enum, variant(), is_X(), and typed accessors.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// ── Account types (shared owner, different discriminators) ──

#[account(discriminator = 1)]
pub struct Settings {
    pub authority: Address,
    pub fee_bps: PodU16,
}

#[account(discriminator = 2)]
pub struct Policy {
    pub authority: Address,
    pub max_amount: PodU64,
}

// ── one_of enum ──

#[account(one_of)]
pub enum ConsensusAccount {
    Settings(Settings),
    Policy(Policy),
}

// ── Accounts struct using one_of ──

#[derive(Accounts)]
pub struct ReadConsensus {
    pub signer: Signer,

    pub consensus: Account<ConsensusAccount>,
}

// ── Program ──

#[program]
pub mod test_one_of {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn read_consensus(ctx: Ctx<ReadConsensus>) -> Result<(), ProgramError> {
        match ctx.accounts.consensus.variant() {
            ConsensusAccountRef::Settings(s) => {
                let _fee: u16 = s.fee_bps.into();
            }
            ConsensusAccountRef::Policy(p) => {
                let _max: u64 = p.max_amount.into();
            }
        }

        // Typed accessors
        if ctx.accounts.consensus.is_settings() {
            let _s = ctx.accounts.consensus.settings().unwrap();
        }

        Ok(())
    }
}

fn main() {}
