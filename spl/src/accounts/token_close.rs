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

/// Resolved arguments for a token-account close epilogue.
pub struct Args<'a> {
    /// Account receiving reclaimed lamports.
    pub dest: &'a AccountView,
    /// Authority allowed to close the token account.
    pub authority: &'a AccountView,
    /// Token program that owns the account.
    pub token_program: &'a AccountView,
}

/// Builder for token-close behavior arguments.
pub struct ArgsBuilder<'a> {
    dest: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    /// Starts an empty argument builder.
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            dest: None,
            authority: None,
            token_program: None,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
    /// Sets the lamport destination.
    #[inline(always)]
    pub fn dest(mut self, v: &'a AccountView) -> Self {
        self.dest = Some(v);
        self
    }

    /// Sets the close authority.
    #[inline(always)]
    pub fn authority(mut self, v: &'a AccountView) -> Self {
        self.authority = Some(v);
        self
    }

    /// Sets the Token or Token-2022 program account.
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
            dest: self.dest.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program.ok_or(ProgramError::InvalidArgument)?,
        })
    }
}

/// Token-close behavior implementation marker.
pub struct Behavior;

macro_rules! impl_token_close_behavior {
    ($wrapper:ty) => {
        impl AccountBehavior<$wrapper> for Behavior {
            type Args<'a> = Args<'a>;
            const RUN_CHECK: bool = false;
            const RUN_EXIT: bool = true;

            #[inline(always)]
            fn uses_exit_signer_arg<const KEY: u64>() -> bool {
                KEY == quasar_lang::account_behavior::behavior_arg_key_hash("authority")
            }

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

            #[inline(always)]
            fn exit_signed<'a, S>(
                account: &mut $wrapper,
                args: &Args<'a>,
                signer: &S,
            ) -> Result<(), ProgramError>
            where
                S: quasar_lang::cpi::CpiSignerSeeds + ?Sized,
            {
                // SAFETY: The exit hook has exclusive access to the loaded account wrapper.
                let view = unsafe { <$wrapper as AccountLoad>::to_account_view_mut(account) };
                crate::exit::close_token_account_signed(
                    args.token_program,
                    view,
                    args.dest,
                    args.authority,
                    signer,
                )
            }
        }
    };
}

impl_token_close_behavior!(Account<crate::token::Token>);
impl_token_close_behavior!(Account<crate::token_2022::Token2022>);
impl_token_close_behavior!(InterfaceAccount<crate::token::Token>);
