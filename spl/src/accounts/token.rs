//! Token account behavior module.
//!
//! Provides check and init behavior for token account fields.
//!
//! ```rust,ignore
//! use quasar_spl::accounts::token;
//! #[account(token(mint = mint, authority = authority, token_program = token_program))]
//! pub vault: Account<Token>,
//! ```

use quasar_lang::prelude::*;

pub struct Args<'a> {
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: Option<&'a AccountView>,
}

pub struct ArgsBuilder<'a> {
    mint: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            mint: None,
            authority: None,
            token_program: None,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
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

    #[inline(always)]
    pub fn build_check(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program,
        })
    }

    #[inline(always)]
    pub fn build_init(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: Some(self.token_program.ok_or(ProgramError::InvalidArgument)?),
        })
    }

    #[inline(always)]
    pub fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        self.build_check()
    }
}

pub struct Behavior;

const TOKEN_PROGRAM_ARG: u64 =
    quasar_lang::account_behavior::behavior_arg_key_hash("token_program");

/// Implement token behavior for a concrete token wrapper type.
macro_rules! impl_token_behavior {
    (
        $wrapper:ty,
        check_token_program = $check_token_program:literal,
        validates_account_data = $validates_account_data:literal
    ) => {
        impl AccountBehavior<$wrapper> for Behavior {
            type Args<'a> = Args<'a>;
            const SETS_INIT_PARAMS: bool = true;
            const INIT_SATISFIES_CHECK: bool = true;
            const VALIDATES_ACCOUNT_DATA: bool = $validates_account_data;

            #[inline(always)]
            fn uses_arg<const PHASE: u8, const KEY: u64>() -> bool {
                !(!$check_token_program
                    && PHASE == quasar_lang::account_behavior::ARG_PHASE_CHECK
                    && KEY == TOKEN_PROGRAM_ARG)
            }

            #[inline(always)]
            fn set_init_param<'a>(
                params: &mut <$wrapper as AccountInit>::InitParams<'a>,
                args: &Args<'a>,
            ) -> Result<(), ProgramError> {
                *params = crate::token::TokenInitKind::Token {
                    mint: args.mint,
                    authority: args.authority.address(),
                    token_program: args.token_program.ok_or(ProgramError::InvalidArgument)?,
                };
                Ok(())
            }

            #[inline(always)]
            fn check<'a>(account: &$wrapper, args: &Args<'a>) -> Result<(), ProgramError> {
                crate::validate::validate_token_account(
                    account.to_account_view(),
                    args.mint.address(),
                    args.authority.address(),
                    if $check_token_program {
                        args.token_program.map(|program| program.address())
                    } else {
                        None
                    },
                )
            }
        }
    };
}

impl_token_behavior!(
    Account<crate::token::Token>,
    check_token_program = false,
    validates_account_data = true
);
impl_token_behavior!(
    Account<crate::token_2022::Token2022>,
    check_token_program = false,
    validates_account_data = true
);
impl_token_behavior!(
    InterfaceAccount<crate::token::Token>,
    check_token_program = true,
    validates_account_data = false
);

/// Check-only behavior for InterfaceAccount<TokenInterface>.
/// `TokenInterface` has no single fixed token layout, so validation dispatches
/// from the actual owner.
impl AccountBehavior<InterfaceAccount<crate::interface::TokenInterface>> for Behavior {
    type Args<'a> = Args<'a>;

    #[inline(always)]
    fn check<'a>(
        account: &InterfaceAccount<crate::interface::TokenInterface>,
        args: &Args<'a>,
    ) -> Result<(), ProgramError> {
        crate::validate::validate_token_account(
            account.to_account_view(),
            args.mint.address(),
            args.authority.address(),
            args.token_program.map(|tp| tp.address()),
        )
    }
}
