//! Custom behavior: init params (no SPL). Proves behavior-driven init works.
#![allow(unexpected_cfgs)]
extern crate alloc;
use {quasar_derive::Accounts, quasar_lang::prelude::*};

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 43)]
pub struct MyState {
    pub owner: Address,
}

mod my_state_init {
    use quasar_lang::prelude::*;

    pub struct Args<'a> {
        pub owner: &'a AccountView,
    }

    pub struct ArgsBuilder<'a> {
        owner: Option<&'a AccountView>,
    }

    impl<'a> Args<'a> {
        pub fn builder() -> ArgsBuilder<'a> {
            ArgsBuilder { owner: None }
        }
    }

    impl<'a> ArgsBuilder<'a> {
        pub fn owner(mut self, v: &'a AccountView) -> Self {
            self.owner = Some(v);
            self
        }
        pub fn build_init(self) -> Result<Args<'a>, ProgramError> {
            Ok(Args {
                owner: self.owner.ok_or(ProgramError::InvalidArgument)?,
            })
        }
        pub fn build_check(self) -> Result<Args<'a>, ProgramError> {
            self.build_init()
        }
        pub fn build_exit(self) -> Result<Args<'a>, ProgramError> {
            self.build_init()
        }
    }

    pub struct Behavior;

    impl AccountBehavior<Account<super::MyState>> for Behavior {
        type Args<'a> = Args<'a>;
        const RUN_CHECK: bool = true;
        const RUN_AFTER_INIT: bool = true;

        fn after_init<'a>(
            account: &mut Account<super::MyState>,
            args: &Args<'a>,
        ) -> Result<(), ProgramError> {
            account.owner = *args.owner.address();
            Ok(())
        }

        fn check<'a>(
            account: &Account<super::MyState>,
            args: &Args<'a>,
        ) -> Result<(), ProgramError> {
            if !quasar_lang::keys_eq(&account.owner, args.owner.address()) {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(())
        }
    }
}

#[derive(Accounts)]
pub struct InitMyState {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(mut, init, my_state_init(owner = authority))]
    pub state: Account<MyState>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
