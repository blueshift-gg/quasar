//! Custom behavior: check-only (no SPL). Proves the plugin system works.
#![allow(unexpected_cfgs)]
extern crate alloc;
use {quasar_derive::Accounts, quasar_lang::prelude::*};

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 42)]
pub struct MyData {
    pub value: u64,
}

mod min_value {
    use quasar_lang::prelude::*;

    pub struct Args {
        pub min: u64,
    }

    pub struct ArgsBuilder {
        min: Option<u64>,
    }

    impl Args {
        pub fn builder() -> ArgsBuilder {
            ArgsBuilder { min: None }
        }
    }

    impl ArgsBuilder {
        pub fn min(mut self, v: u64) -> Self {
            self.min = Some(v);
            self
        }
    }

    impl quasar_lang::account_behavior::BehaviorArgsBuilder for ArgsBuilder {
        type Init = Args;
        type Check = Args;
        type Exit = Args;
        fn build_check(self) -> Result<Args, ProgramError> {
            Ok(Args {
                min: self.min.ok_or(ProgramError::InvalidArgument)?,
            })
        }
        fn build_init(self) -> Result<Args, ProgramError> {
            self.build_check()
        }
        fn build_exit(self) -> Result<Args, ProgramError> {
            self.build_check()
        }
    }

    pub struct Behavior;

    impl AccountBehavior<Account<super::MyData>> for Behavior {
        type Args<'a> = Args;

        fn check<'a>(
            account: &Account<super::MyData>,
            args: &Args,
        ) -> Result<(), ProgramError> {
            if account.value < args.min {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(())
        }
    }
}

#[derive(Accounts)]
pub struct UseCustomBehavior {
    #[account(min_value(min = 10u64))]
    pub data: Account<MyData>,
}

fn main() {}
