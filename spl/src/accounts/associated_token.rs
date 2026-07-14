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

use quasar_lang::prelude::*;

/// Resolved arguments for associated-token validation or initialization.
pub struct Args<'a> {
    /// Token mint account.
    pub mint: &'a AccountView,
    /// Wallet or PDA that owns the associated token account.
    pub authority: &'a AccountView,
    /// Token program used during initialization, when applicable.
    pub token_program: Option<&'a AccountView>,
    /// System Program used during initialization, when applicable.
    pub system_program: Option<&'a AccountView>,
    /// Associated Token Program used during initialization, when applicable.
    pub ata_program: Option<&'a AccountView>,
}

/// Builder for associated-token behavior arguments.
pub struct ArgsBuilder<'a> {
    mint: Option<&'a AccountView>,
    authority: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
    system_program: Option<&'a AccountView>,
    ata_program: Option<&'a AccountView>,
}

impl<'a> Args<'a> {
    /// Starts an empty argument builder.
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
    /// Sets the token mint account.
    #[inline(always)]
    pub fn mint(mut self, v: &'a AccountView) -> Self {
        self.mint = Some(v);
        self
    }

    /// Sets the associated account authority.
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

    /// Sets the System Program account.
    #[inline(always)]
    pub fn system_program(mut self, v: &'a AccountView) -> Self {
        self.system_program = Some(v);
        self
    }

    /// Sets the Associated Token Program account.
    #[inline(always)]
    pub fn ata_program(mut self, v: &'a AccountView) -> Self {
        self.ata_program = Some(v);
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
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: self.token_program,
            system_program: None,
            ata_program: None,
        })
    }

    #[inline(always)]
    fn build_init(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            authority: self.authority.ok_or(ProgramError::InvalidArgument)?,
            token_program: Some(self.token_program.ok_or(ProgramError::InvalidArgument)?),
            system_program: Some(self.system_program.ok_or(ProgramError::InvalidArgument)?),
            ata_program: Some(self.ata_program.ok_or(ProgramError::InvalidArgument)?),
        })
    }

    #[inline(always)]
    fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        self.build_check()
    }
}

/// Associated-token account behavior implementation marker.
pub struct Behavior;

const ATA_PROGRAM_ARG: u64 = quasar_lang::account_behavior::behavior_arg_key_hash("ata_program");
const SYSTEM_PROGRAM_ARG: u64 =
    quasar_lang::account_behavior::behavior_arg_key_hash("system_program");
const TOKEN_PROGRAM_ARG: u64 =
    quasar_lang::account_behavior::behavior_arg_key_hash("token_program");

macro_rules! impl_ata_behavior {
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
                !(PHASE == quasar_lang::account_behavior::ARG_PHASE_CHECK
                    && (KEY == ATA_PROGRAM_ARG
                        || KEY == SYSTEM_PROGRAM_ARG
                        || (!$check_token_program && KEY == TOKEN_PROGRAM_ARG)))
            }

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
                let view = account.to_account_view();
                let token_program = if $check_token_program {
                    args.token_program
                        .map(|program| program.address())
                        .unwrap_or_else(|| view.owner())
                } else {
                    view.owner()
                };
                crate::validate::validate_ata(
                    view,
                    args.authority.address(),
                    args.mint.address(),
                    token_program,
                )
            }
        }
    };
}

impl_ata_behavior!(
    Account<crate::token::Token>,
    check_token_program = false,
    validates_account_data = true
);
impl_ata_behavior!(
    Account<crate::token_2022::Token2022>,
    check_token_program = false,
    validates_account_data = true
);
impl_ata_behavior!(
    InterfaceAccount<crate::token::Token>,
    check_token_program = true,
    validates_account_data = false
);

/// Check-only behavior for `InterfaceAccount<TokenInterface>`.
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
