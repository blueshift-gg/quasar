#![allow(unexpected_cfgs)]
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 99)]
pub struct MyData {
    pub value: u64,
}

// Behavior module with missing `.value()` setter: tests that builder
// type errors produce readable compiler errors.
mod bad_behavior {
    use quasar_lang::prelude::*;

    pub struct Args;
    pub struct ArgsBuilder;

    impl Args {
        pub fn builder() -> ArgsBuilder {
            ArgsBuilder
        }
    }

    impl ArgsBuilder {
        // Missing `.value()` setter: the derive will call `.value(42u64)`
        // which doesn't exist.
        pub fn build_check(self) -> Result<Args, ProgramError> {
            Ok(Args)
        }
        pub fn build_init(self) -> Result<Args, ProgramError> {
            Ok(Args)
        }
        pub fn build_exit(self) -> Result<Args, ProgramError> {
            Ok(Args)
        }
    }

    pub struct Behavior;

    impl AccountBehavior<Account<super::MyData>> for Behavior {
        type Args<'a> = Args;
    }
}

// ERROR: no method named `value` found for struct `bad_behavior::ArgsBuilder`
#[derive(Accounts)]
pub struct Bad {
    #[account(bad_behavior(value = 42u64))]
    pub data: Account<MyData>,
}

fn main() {}
