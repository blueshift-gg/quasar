//! Token sweep behavior module.
//!
//! Provides exit behavior for sweeping all tokens out before close.
//!
//! ```rust,ignore
//! use quasar_spl::accounts::token_sweep;
//! #[account(mut, token_sweep(
//!     receiver = receiver, mint = mint,
//!     authority = authority, token_program = token_program,
//! ))]
//! pub vault: Account<Token>,
//! ```

use quasar_lang::prelude::*;

pub struct Args<'a> {
    pub receiver: &'a AccountView,
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: &'a AccountView,
}

pub struct ArgsBuilder<'a> {
    receiver: Option<&'a AccountView>,
    mint: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            receiver: None,
            mint: None,
            authority: None,
            token_program: None,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
    #[inline(always)]
    pub fn receiver(mut self, v: &'a AccountView) -> Self {
        self.receiver = Some(v);
        self
    }

    #[inline(always)]
    pub fn mint(mut self, v: &'a AccountView) -> Self {
        self.mint = Some(v);
        self
    }

    #[inline(always)]
    pub fn authority(mut self, v: &'a AccountView) -> Self {
        self.authority = Some(v);
        self
    }

    #[inline(always)]
    pub fn token_program(mut self, v: &'a AccountView) -> Self {
        self.token_program = Some(v);
        self
    }
}

impl<'a> quasar_lang::account_behavior::BehaviorArgsBuilder for ArgsBuilder<'a> {
    type Init = Args<'a>;
    type Check = Args<'a>;
    type Exit = Args<'a>;

    #[inline(always)]
    fn build_check(self) -> Result<Args<'a>, ProgramError> {
        self.build_exit()
    }

    #[inline(always)]
    fn build_init(self) -> Result<Args<'a>, ProgramError> {
        self.build_exit()
    }

    #[inline(always)]
    fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            receiver: self.receiver.ok_or(ProgramError::InvalidArgument)?,
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program.ok_or(ProgramError::InvalidArgument)?,
        })
    }
}

pub struct Behavior;

macro_rules! impl_token_sweep_behavior {
    ($wrapper:ty) => {
        impl AccountBehavior<$wrapper> for Behavior {
            type Args<'a> = Args<'a>;
            const RUN_CHECK: bool = false;
            const RUN_EXIT: bool = true;

            #[inline(always)]
            fn exit<'a>(account: &mut $wrapper, args: &Args<'a>) -> Result<(), ProgramError> {
                crate::exit::sweep_token_account(
                    args.token_program,
                    account.to_account_view(),
                    args.mint,
                    args.receiver,
                    args.authority,
                )
            }
        }
    };
}

impl_token_sweep_behavior!(Account<crate::token::Token>);
impl_token_sweep_behavior!(Account<crate::token_2022::Token2022>);
impl_token_sweep_behavior!(InterfaceAccount<crate::token::Token>);
