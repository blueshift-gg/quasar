use {
    crate::{
        cpi::{system, Signer},
        ops::RentAccess,
        sysvars::rent::Rent,
    },
    solana_account_view::AccountView,
    solana_address::Address,
    solana_program_error::ProgramResult,
};

/// Context for account initialization CPI.
pub struct InitCtx<'a, R: RentAccess> {
    /// Account paying rent and system-program create costs.
    pub payer: &'a AccountView,
    /// Raw account slot being initialized.
    pub target: &'a mut AccountView,
    /// Owner assigned to the newly created account.
    pub program_id: &'a Address,
    /// Data length to allocate.
    pub space: u64,
    /// PDA signer seeds for the create CPI, empty for non-PDA init.
    pub signers: &'a [Signer<'a, 'a>],
    /// Rent source for the current instruction.
    pub rent: &'a R,
}

/// Initialization behavior for account types.
///
/// Implemented on the behavior target (Token, Mint, `#[account]` types), not
/// on wrapper types (`Account<T>`, `InterfaceAccount<T>`).
///
/// The `derive(Accounts)` macro calls this via:
/// ```text
/// <FieldTy as AccountInit>::init(ctx, &params)?;
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be initialized with `init`",
    label = "`init` requires `Account<T>` or `InterfaceAccount<T>`",
    note = "only program-owned data accounts (or SPL Token/Mint) implement `AccountInit`"
)]
pub trait AccountInit {
    /// Params assembled by behavior groups before init.
    type InitParams<'a>: Default;

    /// Whether `Default` init params are valid (i.e., the account can be
    /// created without any behavior filling the params). Program-owned
    /// accounts with `InitParams = ()` set this to `true`. Protocol accounts
    /// like Token/Mint set this to `false`; their `Unset` default is a
    /// runtime error if no behavior fills the params.
    const DEFAULT_INIT_PARAMS_VALID: bool = true;

    /// Initialize `ctx.target` using the supplied params.
    fn init<'a, R: RentAccess>(ctx: InitCtx<'a, R>, params: &Self::InitParams<'a>)
        -> ProgramResult;
}

/// Create a program-owned account and write its discriminator.
#[inline(always)]
pub fn init_account(
    payer: &AccountView,
    target: &mut AccountView,
    space: u64,
    owner: &Address,
    signers: &[Signer],
    rent: &Rent,
    discriminator: &[u8],
) -> ProgramResult {
    system::init_account_with_rent(payer, target, space, owner, signers, rent)?;
    system::write_discriminator(target, discriminator)?;
    Ok(())
}
