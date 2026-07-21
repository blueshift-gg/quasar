//! Token sweep behavior module.
//!
//! Provides exit behavior for sweeping all tokens out before close.
//!
//! ```text
//! use quasar_spl::accounts::token_sweep;
//! #[account(mut, token_sweep(
//!     receiver = receiver, mint = mint,
//!     authority = authority, token_program = token_program,
//! ))]
//! pub vault: Account<Token>,
//! ```

use quasar_lang::prelude::*;

/// Resolved arguments for a token-account sweep epilogue.
pub struct Args<'a> {
    /// Token account receiving the swept balance.
    pub receiver: &'a AccountView,
    /// Mint shared by the source and receiver accounts.
    pub mint: &'a AccountView,
    /// Authority allowed to transfer from the source account.
    pub authority: &'a AccountView,
    /// Token program that owns the accounts.
    pub token_program: &'a AccountView,
}

/// Builder for token-sweep behavior arguments.
pub struct ArgsBuilder<'a> {
    receiver: Option<&'a AccountView>,
    mint: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    /// Starts an empty argument builder.
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
    /// Sets the destination token account.
    #[inline(always)]
    pub fn receiver(mut self, v: &'a AccountView) -> Self {
        self.receiver = Some(v);
        self
    }

    /// Sets the token mint account.
    #[inline(always)]
    pub fn mint(mut self, v: &'a AccountView) -> Self {
        self.mint = Some(v);
        self
    }

    /// Sets the transfer authority.
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
            receiver: self.receiver.ok_or(ProgramError::InvalidArgument)?,
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program.ok_or(ProgramError::InvalidArgument)?,
        })
    }
}

/// Token-sweep behavior implementation marker.
pub struct Behavior;

macro_rules! impl_token_sweep_behavior {
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
                crate::exit::sweep_token_account(
                    args.token_program,
                    account.to_account_view(),
                    args.mint,
                    args.receiver,
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
                crate::exit::sweep_token_account_signed(
                    args.token_program,
                    account.to_account_view(),
                    args.mint,
                    args.receiver,
                    args.authority,
                    signer,
                )
            }
        }
    };
}

impl_token_sweep_behavior!(Account<crate::token::Token>);
impl_token_sweep_behavior!(Account<crate::token_2022::Token2022>);
impl_token_sweep_behavior!(InterfaceAccount<crate::token::Token>);
