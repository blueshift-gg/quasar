//! Metadata account behavior module.
//!
//! Provides check and init behavior for metadata account fields.
//!
//! ```rust,ignore
//! use quasar_metadata::accounts::metadata;
//! #[account(metadata(program = mp, mint = mint, mint_authority = auth, ...))]
//! pub metadata: Account<MetadataAccount>,
//! ```
//!
//! # Field ordering
//!
//! When using `#[account(init, metadata(...))]`, the metadata field must appear
//! before any `master_edition` field in the struct; the derive processes init
//! in declaration order and `create_master_edition_v3` requires the metadata
//! account to exist.

use quasar_lang::prelude::*;

pub struct Args<'a> {
    pub program: &'a AccountView,
    pub mint: &'a AccountView,
    pub mint_authority: Option<&'a AccountView>,
    pub update_authority: Option<&'a AccountView>,
    pub system_program: Option<&'a AccountView>,
    pub rent: Option<&'a AccountView>,
    pub name: Option<&'a str>,
    pub symbol: Option<&'a str>,
    pub uri: Option<&'a str>,
    pub seller_fee_basis_points: Option<u16>,
    pub is_mutable: Option<bool>,
}

pub struct ArgsBuilder<'a> {
    program: Option<&'a AccountView>,
    mint: Option<&'a AccountView>,
    mint_authority: Option<&'a AccountView>,
    update_authority: Option<&'a AccountView>,
    system_program: Option<&'a AccountView>,
    rent: Option<&'a AccountView>,
    name: Option<&'a str>,
    symbol: Option<&'a str>,
    uri: Option<&'a str>,
    seller_fee_basis_points: Option<u16>,
    is_mutable: Option<bool>,
}

impl<'a> Args<'a> {
    pub fn builder() -> ArgsBuilder<'a> {
        ArgsBuilder {
            program: None,
            mint: None,
            mint_authority: None,
            update_authority: None,
            system_program: None,
            rent: None,
            name: None,
            symbol: None,
            uri: None,
            seller_fee_basis_points: None,
            is_mutable: None,
        }
    }
}

impl<'a> ArgsBuilder<'a> {
    #[inline(always)]
    pub fn program(mut self, v: &'a AccountView) -> Self {
        self.program = Some(v);
        self
    }

    #[inline(always)]
    pub fn mint(mut self, v: &'a AccountView) -> Self {
        self.mint = Some(v);
        self
    }

    #[inline(always)]
    pub fn mint_authority(mut self, v: &'a AccountView) -> Self {
        self.mint_authority = Some(v);
        self
    }

    #[inline(always)]
    pub fn update_authority(mut self, v: &'a AccountView) -> Self {
        self.update_authority = Some(v);
        self
    }

    #[inline(always)]
    pub fn system_program(mut self, v: &'a AccountView) -> Self {
        self.system_program = Some(v);
        self
    }

    #[inline(always)]
    pub fn rent(mut self, v: &'a AccountView) -> Self {
        self.rent = Some(v);
        self
    }

    #[inline(always)]
    pub fn name(mut self, v: &'a str) -> Self {
        self.name = Some(v);
        self
    }

    #[inline(always)]
    pub fn symbol(mut self, v: &'a str) -> Self {
        self.symbol = Some(v);
        self
    }

    #[inline(always)]
    pub fn uri(mut self, v: &'a str) -> Self {
        self.uri = Some(v);
        self
    }

    #[inline(always)]
    pub fn seller_fee_basis_points(mut self, v: u16) -> Self {
        self.seller_fee_basis_points = Some(v);
        self
    }

    #[inline(always)]
    pub fn is_mutable(mut self, v: bool) -> Self {
        self.is_mutable = Some(v);
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
            mint_authority: self.mint_authority,
            update_authority: self.update_authority,
            system_program: self.system_program,
            rent: self.rent,
            name: self.name,
            symbol: self.symbol,
            uri: self.uri,
            seller_fee_basis_points: self.seller_fee_basis_points,
            is_mutable: self.is_mutable,
        })
    }

    /// Build args for the init phase. All CPI-relevant fields required.
    #[inline(always)]
    fn build_init(self) -> Result<Args<'a>, ProgramError> {
        Ok(Args {
            program: self.program.ok_or(ProgramError::InvalidArgument)?,
            mint: self.mint.ok_or(ProgramError::InvalidArgument)?,
            mint_authority: Some(self.mint_authority.ok_or(ProgramError::InvalidArgument)?),
            update_authority: Some(self.update_authority.ok_or(ProgramError::InvalidArgument)?),
            system_program: Some(self.system_program.ok_or(ProgramError::InvalidArgument)?),
            rent: Some(self.rent.ok_or(ProgramError::InvalidArgument)?),
            name: Some(self.name.ok_or(ProgramError::InvalidArgument)?),
            symbol: Some(self.symbol.ok_or(ProgramError::InvalidArgument)?),
            uri: Some(self.uri.ok_or(ProgramError::InvalidArgument)?),
            seller_fee_basis_points: Some(self.seller_fee_basis_points.unwrap_or(0)),
            is_mutable: Some(self.is_mutable.unwrap_or(true)),
        })
    }

    #[inline(always)]
    fn build_exit(self) -> Result<Args<'a>, ProgramError> {
        self.build_check()
    }
}

pub struct Behavior;

impl AccountBehavior<Account<crate::MetadataAccount>> for Behavior {
    type Args<'a> = Args<'a>;
    const SETS_INIT_PARAMS: bool = true;
    const VALIDATES_ACCOUNT_DATA: bool = true;

    #[inline(always)]
    fn set_init_param<'a>(
        params: &mut <Account<crate::MetadataAccount> as AccountInit>::InitParams<'a>,
        args: &Args<'a>,
    ) -> Result<(), ProgramError> {
        *params = crate::MetadataInitParams::Create {
            program: args.program,
            mint: args.mint,
            mint_authority: args.mint_authority.ok_or(ProgramError::InvalidArgument)?,
            update_authority: args.update_authority.ok_or(ProgramError::InvalidArgument)?,
            system_program: args.system_program.ok_or(ProgramError::InvalidArgument)?,
            rent: args.rent.ok_or(ProgramError::InvalidArgument)?,
            name: args.name.ok_or(ProgramError::InvalidArgument)?,
            symbol: args.symbol.ok_or(ProgramError::InvalidArgument)?,
            uri: args.uri.ok_or(ProgramError::InvalidArgument)?,
            seller_fee_basis_points: args.seller_fee_basis_points.unwrap_or(0),
            is_mutable: args.is_mutable.unwrap_or(true),
        };
        Ok(())
    }

    #[inline(always)]
    fn check<'a>(
        account: &Account<crate::MetadataAccount>,
        args: &Args<'a>,
    ) -> Result<(), ProgramError> {
        crate::validate::validate_metadata_program(args.program)?;
        let view = account.to_account_view();
        crate::validate::validate_metadata_account(
            view,
            args.mint.address(),
            args.update_authority.map(|ua| ua.address()),
        )?;
        crate::pda::verify_metadata_address(view.address(), args.mint.address())?;
        Ok(())
    }
}
