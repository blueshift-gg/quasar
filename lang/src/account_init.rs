use {
    crate::{
        cpi::{system, Signer},
        sysvars::rent::Rent,
    },
    solana_account_view::AccountView,
    solana_address::Address,
    solana_program_error::ProgramResult,
};

/// Context for account initialization CPI.
pub struct InitCtx<'a> {
    pub payer: &'a AccountView,
    pub target: &'a mut AccountView,
    pub program_id: &'a Address,
    pub space: u64,
    pub signers: &'a [Signer<'a, 'a>],
    pub rent: &'a Rent,
}

/// Initialization behavior for account types.
///
/// Implemented on the behavior target (Token, Mint, `#[account]` types),
/// NOT on wrapper types (`Account<T>`, `InterfaceAccount<T>`).
///
/// The `derive(Accounts)` macro calls this via:
/// ```text
/// type __Target = <FieldTy as AccountLoad>::BehaviorTarget;
/// <__Target as AccountInit>::init(ctx, &params)?;
/// ```
pub trait AccountInit {
    type InitParams<'a>: Default;

    fn init<'a>(ctx: InitCtx<'a>, params: &Self::InitParams<'a>) -> ProgramResult;
}

/// Create account via system program + write discriminator.
#[inline(always)]
pub fn init_account(
    payer: &AccountView,
    account: &mut AccountView,
    space: u64,
    owner: &Address,
    signers: &[Signer],
    rent: &Rent,
    discriminator: &[u8],
) -> ProgramResult {
    system::init_account_with_rent(payer, account, space, owner, signers, rent)?;
    system::write_discriminator(account, discriminator)?;
    Ok(())
}
