#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[repr(transparent)]
pub struct ParamAccount {
    view: AccountView,
}

impl AsAccountView for ParamAccount {
    fn to_account_view(&self) -> &AccountView {
        &self.view
    }
}

#[derive(Default)]
pub struct ParamAccountParams {
    pub expected_owner: Option<Address>,
}

impl AccountLoad for ParamAccount {
    type BehaviorTarget = Self;
    type Params = ParamAccountParams;

    fn check(_view: &AccountView, _field_name: &str) -> Result<(), ProgramError> {
        Ok(())
    }

    fn validate(&self, params: &Self::Params) -> Result<(), ProgramError> {
        if let Some(expected_owner) = &params.expected_owner {
            if !quasar_lang::keys_eq(self.to_account_view().owner(), expected_owner) {
                return Err(ProgramError::IllegalOwner);
            }
        }
        Ok(())
    }
}

impl AccountInit for ParamAccount {
    type InitParams<'a> = ();

    fn init<'a>(_ctx: InitCtx<'a>, _params: &()) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitIfNeededParam {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        mut,
        init_if_needed,
        payer = payer,
        space = 0,
        param::expected_owner = Some(*expected_owner.address())
    )]
    pub target: ParamAccount,
    pub expected_owner: UncheckedAccount,
    pub system_program: Program<System>,
}

fn main() {}
