//! Token close behavior module.
//!
//! Provides exit behavior for closing token accounts via CPI.
//!
//! ```rust,ignore
//! use quasar_spl::accounts::token_close;
//! #[account(mut, token_close(dest = receiver, authority = authority, token_program = token_program))]
//! pub vault: Account<Token>,
//! ```

use quasar_lang::prelude::*;

pub struct Args<'a> {
    pub dest: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: &'a AccountView,
}

pub struct ArgsBuilder<'a> {
    dest: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            dest: None,
            authority: None,
            token_program: None,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
    #[inline(always)]
    pub fn dest(mut self, v: &'a AccountView) -> Self {
        self.dest = Some(v);
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

    #[inline(always)]
    pub fn build_check(self) -> Result<Args<'a>, ProgramError> {
        self.build_exit()
    }

    #[inline(always)]
    pub fn build_init(self) -> Result<Args<'a>, ProgramError> {
        self.build_exit()
    }

    #[inline(always)]
    pub fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            dest: self.dest.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program.ok_or(ProgramError::InvalidArgument)?,
        })
    }
}

pub struct Behavior;

macro_rules! impl_token_close_behavior {
    ($wrapper:ty) => {
        impl AccountBehavior<$wrapper> for Behavior {
            type Args<'a> = Args<'a>;
            const RUN_CHECK: bool = false;
            const RUN_EXIT: bool = true;

            #[inline(always)]
            fn exit<'a>(account: &mut $wrapper, args: &Args<'a>) -> Result<(), ProgramError> {
                // SAFETY: The exit hook has exclusive access to the loaded account wrapper.
                let view = unsafe { <$wrapper as AccountLoad>::to_account_view_mut(account) };
                crate::exit::close_token_account(
                    args.token_program,
                    view,
                    args.dest,
                    args.authority,
                )
            }
        }
    };
}

impl_token_close_behavior!(Account<crate::token::Token>);
impl_token_close_behavior!(Account<crate::token_2022::Token2022>);
impl_token_close_behavior!(InterfaceAccount<crate::token::Token>);
