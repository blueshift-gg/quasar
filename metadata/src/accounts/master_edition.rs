//! Master edition account behavior module.
//!
//! Provides check and init behavior for master edition account fields.
//!
//! ```rust,ignore
//! use quasar_metadata::accounts::master_edition;
//! #[account(master_edition(program = mp, mint = mint, ...))]
//! pub master_edition: Account<MasterEditionAccount>,
//! ```
//!
//! # Field ordering
//!
//! The `metadata` field must be declared before `master_edition` in the struct.
//! The `create_master_edition_v3` CPI requires the metadata account, and the
//! derive processes init in declaration order.

use quasar_lang::prelude::*;

/// Max supply specification for the master edition behavior arg.
pub enum MaxSupplyArg {
    /// Not specified; defaults to `Some(0)` (unique 1/1 NFT).
    Unset,
    /// Unlimited editions (max_supply = None).
    Unlimited,
    /// Limited to N editions (max_supply = Some(N)).
    Limited(u64),
}

/// Resolved arguments for master-edition validation or initialization.
pub struct Args<'a> {
    /// Token Metadata program account.
    pub program: &'a AccountView,
    /// Mint associated with the edition.
    pub mint: &'a AccountView,
    /// Metadata update authority, required for initialization.
    pub update_authority: Option<&'a AccountView>,
    /// Mint authority, required for initialization.
    pub mint_authority: Option<&'a AccountView>,
    /// Metadata account, required for initialization.
    pub metadata: Option<&'a AccountView>,
    /// SPL Token program, required for initialization.
    pub token_program: Option<&'a AccountView>,
    /// System Program, required for initialization.
    pub system_program: Option<&'a AccountView>,
    /// Rent sysvar, required for initialization.
    pub rent: Option<&'a AccountView>,
    /// Maximum printable-edition policy.
    pub max_supply: MaxSupplyArg,
}

/// Builder for master-edition behavior arguments.
pub struct ArgsBuilder<'a> {
    program: Option<&'a AccountView>,
    mint: Option<&'a AccountView>,
    update_authority: Option<&'a AccountView>,
    mint_authority: Option<&'a AccountView>,
    metadata: Option<&'a AccountView>,
    token_program: Option<&'a AccountView>,
    system_program: Option<&'a AccountView>,
    rent: Option<&'a AccountView>,
    max_supply: MaxSupplyArg,
}

impl<'a> Args<'a> {
    /// Starts an empty argument builder.
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            program: None,
            mint: None,
            update_authority: None,
            mint_authority: None,
            metadata: None,
            token_program: None,
            system_program: None,
            rent: None,
            max_supply: MaxSupplyArg::Unset,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
    /// Sets the Token Metadata program account.
    #[inline(always)]
    pub fn program(mut self, v: &'a AccountView) -> Self {
        self.program = Some(v);
        self
    }

    /// Sets the edition mint account.
    #[inline(always)]
    pub fn mint(mut self, v: &'a AccountView) -> Self {
        self.mint = Some(v);
        self
    }

    /// Sets the metadata update authority.
    #[inline(always)]
    pub fn update_authority(mut self, v: &'a AccountView) -> Self {
        self.update_authority = Some(v);
        self
    }

    /// Sets the mint authority.
    #[inline(always)]
    pub fn mint_authority(mut self, v: &'a AccountView) -> Self {
        self.mint_authority = Some(v);
        self
    }

    /// Sets the metadata account.
    #[inline(always)]
    pub fn metadata(mut self, v: &'a AccountView) -> Self {
        self.metadata = Some(v);
        self
    }

    /// Sets the SPL Token program account.
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

    /// Sets the Rent sysvar account.
    #[inline(always)]
    pub fn rent(mut self, v: &'a AccountView) -> Self {
        self.rent = Some(v);
        self
    }

    /// Sets an optional maximum printable-edition supply.
    #[inline(always)]
    pub fn max_supply(mut self, v: Option<u64>) -> Self {
        self.max_supply = match v {
            None => MaxSupplyArg::Unlimited,
            Some(n) => MaxSupplyArg::Limited(n),
        };
        self
    }
}

impl<'a> quasar_lang::account_behavior::BehaviorArgsBuilder for ArgsBuilder<'a> {
    type Init = Args<'a>;
    type Check = Args<'a>;
    type Exit = Args<'a>;

    /// Build args for the check phase. Only `program` and `mint` are required.
    #[inline(always)]
    fn build_check(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            program: self.program.ok_or(ProgramError::InvalidArgument)?,
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            update_authority: self.update_authority,
            mint_authority: self.mint_authority,
            metadata: self.metadata,
            token_program: self.token_program,
            system_program: self.system_program,
            rent: self.rent,
            max_supply: self.max_supply,
        })
    }

    /// Build args for the init phase. All CPI-relevant fields required.
    #[inline(always)]
    fn build_init(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            program: self.program.ok_or(ProgramError::InvalidArgument)?,
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            update_authority: Some(self.update_authority.ok_or(ProgramError::InvalidArgument)?),
            mint_authority: Some(self.mint_authority.ok_or(ProgramError::InvalidArgument)?),
            metadata: Some(self.metadata.ok_or(ProgramError::InvalidArgument)?),
            token_program: Some(self.token_program.ok_or(ProgramError::InvalidArgument)?),
            system_program: Some(self.system_program.ok_or(ProgramError::InvalidArgument)?),
            rent: Some(self.rent.ok_or(ProgramError::InvalidArgument)?),
            max_supply: self.max_supply,
        })
    }

    #[inline(always)]
    fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        self.build_check()
    }
}

/// Master-edition account behavior implementation marker.
pub struct Behavior;

impl AccountBehavior<Account<crate::MasterEditionAccount>> for Behavior {
    type Args<'a> = Args<'a>;
    const SETS_INIT_PARAMS: bool = true;
    const VALIDATES_ACCOUNT_DATA: bool = true;

    #[inline(always)]
    fn set_init_param<'a>(
        params: &mut <Account<crate::MasterEditionAccount> as AccountInit>::InitParams<'a>,
        args: &Args<'a>,
    ) -> Result<(), ProgramError> {
        *params = crate::MasterEditionInitParams::Create {
            program: args.program,
            mint: args.mint,
            update_authority: args.update_authority.ok_or(ProgramError::InvalidArgument)?,
            mint_authority: args.mint_authority.ok_or(ProgramError::InvalidArgument)?,
            metadata: args.metadata.ok_or(ProgramError::InvalidArgument)?,
            token_program: args.token_program.ok_or(ProgramError::InvalidArgument)?,
            system_program: args.system_program.ok_or(ProgramError::InvalidArgument)?,
            rent: args.rent.ok_or(ProgramError::InvalidArgument)?,
            max_supply: match &args.max_supply {
                MaxSupplyArg::Unset => Some(0),
                MaxSupplyArg::Unlimited => None,
                MaxSupplyArg::Limited(n) => Some(*n),
            },
        };
        Ok(())
    }

    #[inline(always)]
    fn check<'a>(
        account: &Account<crate::MasterEditionAccount>,
        args: &Args<'a>,
    ) -> Result<(), ProgramError> {
        crate::validate::validate_metadata_program(args.program)?;
        crate::validate::validate_master_edition_account(
            account.to_account_view(),
            Some(args.mint.address()),
        )?;
        // Validate metadata account when provided (owner, key, mint, and PDA).
        if let Some(metadata) = args.metadata {
            crate::validate::validate_metadata_account(metadata, args.mint.address(), None)?;
            crate::pda::verify_metadata_address(metadata.address(), args.mint.address())?;
        }
        Ok(())
    }
}
