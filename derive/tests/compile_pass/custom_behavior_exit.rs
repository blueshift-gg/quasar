//! Custom behavior: exit (epilogue). Proves behavior-driven exit works.
#![allow(unexpected_cfgs)]
extern crate alloc;
use {quasar_derive::Accounts, quasar_lang::prelude::*};

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 44)]
pub struct Counter {
    pub value: u64,
}

mod bump_counter {
    use quasar_lang::prelude::*;

    pub struct Args {
        pub amount: u64,
    }

    pub struct ArgsBuilder {
        amount: Option<u64>,
    }

    impl Args {
        pub fn builder() -> ArgsBuilder {
            ArgsBuilder { amount: None }
        }
    }

    impl ArgsBuilder {
        pub fn amount(mut self, v: u64) -> Self {
            self.amount = Some(v);
            self
        }
        pub fn build_check(self) -> Result<Args, ProgramError> {
            Ok(Args {
                amount: self.amount.ok_or(ProgramError::InvalidArgument)?,
            })
        }
        pub fn build_init(self) -> Result<Args, ProgramError> {
            self.build_check()
        }
        pub fn build_exit(self) -> Result<Args, ProgramError> {
            self.build_check()
        }
    }

    pub struct Behavior;

    impl AccountBehavior<Account<super::Counter>> for Behavior {
        type Args<'a> = Args;
        const RUN_EXIT: bool = true;

        fn exit<'a>(
            account: &mut Account<super::Counter>,
            args: &Args,
        ) -> Result<(), ProgramError> {
            account.value = account.value.saturating_add(args.amount);
            Ok(())
        }
    }
}

#[derive(Accounts)]
pub struct BumpCounter {
    #[account(mut, bump_counter(amount = 1u64))]
    pub counter: Account<Counter>,
}

fn main() {}
