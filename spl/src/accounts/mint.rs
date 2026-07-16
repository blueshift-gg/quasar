//! Mint account behavior module.
//!
//! Provides check and init behavior for mint account fields.
//!
//! ```rust,ignore
//! use quasar_spl::accounts::mint;
//! #[account(mint(authority = authority, decimals = 6, token_program = token_program))]
//! pub my_mint: Account<Mint>,
//! ```

use quasar_lang::prelude::*;

/// Resolved arguments for mint validation or initialization.
pub struct Args<'a> {
    /// Expected mint authority, or `None` to skip that check.
    pub authority: Option<&'a AccountView>,
    /// Expected decimals, or `None` to skip that check.
    pub decimals: Option<u8>,
    /// Expected freeze-authority policy.
    pub freeze_authority: FreezeAuthorityArg<'a>,
    /// Token program used during initialization or explicit validation.
    pub token_program: Option<&'a AccountView>,
}

/// Freeze authority specification for the behavior arg.
pub enum FreezeAuthorityArg<'a> {
    /// Not specified; skip check.
    Unset,
    /// Explicitly `None`; assert no freeze authority.
    AssertNone,
    /// Explicitly `Some(field)`; assert matches.
    AssertEquals(&'a AccountView),
}

/// Builder for mint behavior arguments.
pub struct ArgsBuilder<'a> {
    authority: Option<&'a AccountView>,
    decimals: Option<u8>,
    freeze_authority: FreezeAuthorityArg<'a>,
    token_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    /// Starts an empty argument builder.
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            authority: None,
            decimals: None,
            freeze_authority: FreezeAuthorityArg::Unset,
            token_program: None,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
    /// Sets the expected mint authority.
    #[inline(always)]
    pub fn authority(mut self, v: &'a AccountView) -> Self {
        self.authority = Some(v);
        self
    }

    /// Sets the expected decimal precision.
    #[inline(always)]
    pub fn decimals(mut self, v: u8) -> Self {
        self.decimals = Some(v);
        self
    }

    /// Sets the expected optional freeze authority.
    #[inline(always)]
    pub fn freeze_authority(mut self, v: Option<&'a AccountView>) -> Self {
        self.freeze_authority = match v {
            None => FreezeAuthorityArg::AssertNone,
            Some(view) => FreezeAuthorityArg::AssertEquals(view),
        };
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
        Ok(Args {
            authority: self.authority,
            decimals: self.decimals,
            freeze_authority: self.freeze_authority,
            token_program: self.token_program,
        })
    }

    #[inline(always)]
    fn build_init(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            authority: Some(self.authority.ok_or(ProgramError::InvalidArgument)?),
            decimals: self.decimals,
            freeze_authority: self.freeze_authority,
            token_program: Some(self.token_program.ok_or(ProgramError::InvalidArgument)?),
        })
    }

    #[inline(always)]
    fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        self.build_check()
    }
}

/// Mint account behavior implementation marker.
pub struct Behavior;

const TOKEN_PROGRAM_ARG: u64 =
    quasar_lang::account_behavior::behavior_arg_key_hash("token_program");

macro_rules! impl_mint_behavior {
    (
        $wrapper:ty,
        check_token_program = $check_token_program:literal,
        validates_account_data = $validates_account_data:literal
    ) => {
        impl AccountBehavior<$wrapper> for Behavior {
            type Args<'a> = Args<'a>;
            const SETS_INIT_PARAMS: bool = true;
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
                let freeze = match &args.freeze_authority {
                    FreezeAuthorityArg::Unset | FreezeAuthorityArg::AssertNone => None,
                    FreezeAuthorityArg::AssertEquals(view) => Some(view.address()),
                };
                *params = crate::token::MintInitParams::Mint {
                    decimals: args.decimals.unwrap_or(6),
                    authority: args
                        .authority
                        .ok_or(ProgramError::InvalidArgument)?
                        .address(),
                    freeze_authority: freeze,
                    token_program: args.token_program.ok_or(ProgramError::InvalidArgument)?,
                };
                Ok(())
            }

            #[inline(always)]
            fn check<'a>(account: &$wrapper, args: &Args<'a>) -> Result<(), ProgramError> {
                let freeze = match &args.freeze_authority {
                    FreezeAuthorityArg::Unset => crate::validate::FreezeCheck::Skip,
                    FreezeAuthorityArg::AssertNone => crate::validate::FreezeCheck::AssertNone,
                    FreezeAuthorityArg::AssertEquals(view) => {
                        crate::validate::FreezeCheck::AssertEquals(view.address())
                    }
                };
                crate::validate::validate_mint_constraints(
                    account.to_account_view(),
                    args.authority.map(AccountView::address),
                    args.decimals,
                    freeze,
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

impl_mint_behavior!(
    Account<crate::token::Mint>,
    check_token_program = false,
    validates_account_data = true
);
impl_mint_behavior!(
    Account<crate::token_2022::Mint2022>,
    check_token_program = false,
    validates_account_data = true
);
impl_mint_behavior!(
    InterfaceAccount<crate::token::Mint>,
    check_token_program = true,
    validates_account_data = false
);
