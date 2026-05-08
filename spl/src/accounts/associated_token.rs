//! Associated token account behavior module.
//!
//! Provides check and init behavior for ATA fields.
//!
//! ```rust,ignore
//! use quasar_spl::accounts::associated_token;
//! #[account(init, associated_token(
//!     mint = mint, authority = authority,
//!     token_program = token_program, system_program = system_program,
//!     ata_program = ata_program,
//! ))]
//! pub ata: Account<Token>,
//! ```

use {
    crate::ops::{capabilities::AssociatedTokenCheck, ctx::AssociatedTokenCheckCtx},
    quasar_lang::prelude::*,
};

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

pub struct Args<'a> {
    pub mint: &'a AccountView,
    pub authority: &'a AccountView,
    pub token_program: Option<&'a AccountView>,
    pub system_program: Option<&'a AccountView>,
    pub ata_program: Option<&'a AccountView>,
}

pub struct ArgsBuilder<'a> {
    mint: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
    system_program: Option<&'a AccountView>,
    ata_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            mint: None,
            authority: None,
            token_program: None,
            system_program: None,
            ata_program: None,
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
    pub fn system_program(mut self, v: &'a AccountView) -> Self {
        self.system_program = Some(v);
        self
    }

    #[inline(always)]
    pub fn ata_program(mut self, v: &'a AccountView) -> Self {
        self.ata_program = Some(v);
        self
    }

    #[inline(always)]
    pub fn build_check(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program,
            system_program: None,
            ata_program: None,
        })
    }

    #[inline(always)]
    pub fn build_init(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: Some(self.token_program.ok_or(ProgramError::InvalidArgument)?),
            system_program: Some(self.system_program.ok_or(ProgramError::InvalidArgument)?),
            ata_program: Some(self.ata_program.ok_or(ProgramError::InvalidArgument)?),
        })
    }

    #[inline(always)]
    pub fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        self.build_check()
    }
}

// ---------------------------------------------------------------------------
// Behavior — concrete impls per wrapper type
// ---------------------------------------------------------------------------

pub struct Behavior;

macro_rules! impl_ata_behavior {
    ($wrapper:ty, check_token_program = $check_token_program:literal) => {
        impl AccountBehavior<$wrapper> for Behavior {
            type Args<'a> = Args<'a>;
            const SETS_INIT_PARAMS: bool = true;
            const INIT_SATISFIES_CHECK: bool = true;

            #[inline(always)]
            fn set_init_param<'a>(
                params: &mut <$wrapper as AccountInit>::InitParams<'a>,
                args: &Args<'a>,
            ) -> Result<(), ProgramError> {
                let tp = args.token_program.ok_or(ProgramError::InvalidArgument)?;
                let sp = args.system_program.ok_or(ProgramError::InvalidArgument)?;
                let ap = args.ata_program.ok_or(ProgramError::InvalidArgument)?;
                *params = crate::token::TokenInitKind::AssociatedToken {
                    mint: args.mint,
                    authority: args.authority,
                    token_program: tp,
                    system_program: sp,
                    ata_program: ap,
                    idempotent: false,
                };
                Ok(())
            }

            #[inline(always)]
            fn check<'a>(account: &$wrapper, args: &Args<'a>) -> Result<(), ProgramError> {
                <$wrapper as AssociatedTokenCheck>::check_associated_token_view(
                    account.to_account_view(),
                    AssociatedTokenCheckCtx {
                        mint: args.mint,
                        authority: args.authority,
                        token_program: if $check_token_program {
                            args.token_program
                        } else {
                            None
                        },
                    },
                )
            }
        }
    };
}

impl_ata_behavior!(Account<crate::token::Token>, check_token_program = false);
impl_ata_behavior!(
    Account<crate::token_2022::Token2022>,
    check_token_program = false
);
impl_ata_behavior!(
    InterfaceAccount<crate::token::Token>,
    check_token_program = true
);

/// Check-only behavior for InterfaceAccount<TokenInterface>.
impl AccountBehavior<InterfaceAccount<crate::interface::TokenInterface>> for Behavior {
    type Args<'a> = Args<'a>;

    #[inline(always)]
    fn check<'a>(
        account: &InterfaceAccount<crate::interface::TokenInterface>,
        args: &Args<'a>,
    ) -> Result<(), ProgramError> {
        let tp = args
            .token_program
            .map(|p| p.address())
            .unwrap_or_else(|| account.to_account_view().owner());
        crate::validate::validate_ata(
            account.to_account_view(),
            args.authority.address(),
            args.mint.address(),
            tp,
        )
    }
}
