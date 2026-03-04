use quasar_core::prelude::*;

use crate::constants::{SPL_TOKEN_ID, TOKEN_2022_ID};
use crate::cpi::TokenCpi;
use crate::interface::InterfaceAccount;
use crate::state::{MintAccountState, TokenAccountState};
use crate::token::{Mint, Token};
use crate::token_2022::{Mint2022, Token2022};

#[inline(always)]
fn is_token_program_owner(view: &AccountView) -> bool {
    let owner = unsafe { view.owner() };
    quasar_core::keys_eq(owner, &SPL_TOKEN_ID) || quasar_core::keys_eq(owner, &TOKEN_2022_ID)
}

/// Extension trait providing `.init()` on `Initialize<T>` for token account types.
///
/// Chains `System::create_account` → `InitializeAccount3` in two CPIs.
/// The account is allocated with 165 bytes and assigned to the given token program.
///
/// Pass `Some(&rent)` to reuse an already-fetched Rent sysvar, or `None`
/// to fetch it via syscall (`Rent::get()`):
///
/// ```ignore
/// self.new_token.init(
///     self.system_program,
///     self.payer,
///     self.token_program,
///     self.mint,
///     self.owner.address(),
///     None,
/// )?;
/// ```
pub trait InitToken: AsAccountView + Sized {
    /// Create and initialize a token account.
    ///
    /// Chains `System::create_account` → `InitializeAccount3` in two CPIs.
    /// The account must not already exist.
    #[inline(always)]
    fn init(
        &self,
        system_program: &Program<System>,
        payer: &impl AsAccountView,
        token_program: &impl TokenCpi,
        mint: &impl AsAccountView,
        owner: &Address,
        rent: Option<&Rent>,
    ) -> Result<(), ProgramError> {
        system_program
            .create_account_with_minimum_balance(
                payer,
                self,
                TokenAccountState::LEN as u64,
                token_program.address(),
                rent,
            )?
            .invoke()?;

        token_program
            .initialize_account3(self, mint, owner)
            .invoke()
    }

    /// Create and initialize a token account if it doesn't already exist.
    ///
    /// Checks `owner == system_program` to determine if the account needs
    /// initialization. When the account already exists, validates that its
    /// mint and authority match the expected values.
    #[inline(always)]
    fn init_if_needed(
        &self,
        system_program: &Program<System>,
        payer: &impl AsAccountView,
        token_program: &impl TokenCpi,
        mint: &impl AsAccountView,
        owner: &Address,
        rent: Option<&Rent>,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if quasar_core::is_system_program(unsafe { view.owner() }) {
            self.init(system_program, payer, token_program, mint, owner, rent)
        } else {
            // Validate that the account is owned by a token program.
            // Without this check, an attacker could pass an account owned by
            // an arbitrary program with crafted data matching expected offsets.
            if !is_token_program_owner(view) {
                return Err(ProgramError::IllegalOwner);
            }
            if view.data_len() < TokenAccountState::LEN {
                return Err(ProgramError::InvalidAccountData);
            }
            // SAFETY: data_len >= 165 checked above, TokenAccountState is
            // #[repr(C)] with alignment 1, pointer is to account data start.
            let state = unsafe { &*(view.data_ptr() as *const TokenAccountState) };
            if !state.is_initialized() {
                return Err(ProgramError::UninitializedAccount);
            }
            if state.mint() != mint.to_account_view().address() {
                return Err(ProgramError::InvalidAccountData);
            }
            if state.owner() != owner {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(())
        }
    }
}

impl InitToken for Initialize<Token> {}
impl InitToken for Initialize<Token2022> {}
impl<T: AccountCheck + ZeroCopyDeref<Target = TokenAccountState>> InitToken
    for Initialize<InterfaceAccount<T>>
{
}

/// Extension trait providing `.init()` on `Initialize<T>` for mint account types.
///
/// Chains `System::create_account` → `InitializeMint2` in two CPIs.
/// The account is allocated with 82 bytes and assigned to the given token program.
///
/// ```ignore
/// self.new_mint.init(
///     self.system_program,
///     self.payer,
///     self.token_program,
///     6, // decimals
///     self.authority.address(),
///     None, // no freeze authority
///     None, // const rent calculation
/// )?;
/// ```
pub trait InitMint: AsAccountView + Sized {
    /// Create and initialize a mint.
    ///
    /// Chains `System::create_account` → `InitializeMint2` in two CPIs.
    /// The account must not already exist.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init(
        &self,
        system_program: &Program<System>,
        payer: &impl AsAccountView,
        token_program: &impl TokenCpi,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
        rent: Option<&Rent>,
    ) -> Result<(), ProgramError> {
        system_program
            .create_account_with_minimum_balance(
                payer,
                self,
                MintAccountState::LEN as u64,
                token_program.address(),
                rent,
            )?
            .invoke()?;

        token_program
            .initialize_mint2(self, decimals, mint_authority, freeze_authority)
            .invoke()
    }

    /// Create and initialize a mint if it doesn't already exist.
    ///
    /// Checks `owner == system_program` to determine if the account needs
    /// initialization. When the account already exists, validates that its
    /// mint authority matches the expected value.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init_if_needed(
        &self,
        system_program: &Program<System>,
        payer: &impl AsAccountView,
        token_program: &impl TokenCpi,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
        rent: Option<&Rent>,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if quasar_core::is_system_program(unsafe { view.owner() }) {
            self.init(
                system_program,
                payer,
                token_program,
                decimals,
                mint_authority,
                freeze_authority,
                rent,
            )
        } else {
            if !is_token_program_owner(view) {
                return Err(ProgramError::IllegalOwner);
            }
            if view.data_len() < MintAccountState::LEN {
                return Err(ProgramError::InvalidAccountData);
            }
            // SAFETY: data_len >= 82 checked above, MintAccountState is
            // #[repr(C)] with alignment 1, pointer is to account data start.
            let state = unsafe { &*(view.data_ptr() as *const MintAccountState) };
            if !state.is_initialized() {
                return Err(ProgramError::UninitializedAccount);
            }
            if !state.has_mint_authority() || state.mint_authority_unchecked() != mint_authority {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(())
        }
    }
}

impl InitMint for Initialize<Mint> {}
impl InitMint for Initialize<Mint2022> {}
impl<T: AccountCheck + ZeroCopyDeref<Target = MintAccountState>> InitMint
    for Initialize<InterfaceAccount<T>>
{
}

/// Validate that an existing token account has the expected mint and authority.
///
/// Used by generated `#[account(init_if_needed, token::...)]` code when the
/// account is already initialized.
#[inline(always)]
pub fn validate_token_account(
    view: &AccountView,
    mint: &Address,
    authority: &Address,
) -> Result<(), ProgramError> {
    if !is_token_program_owner(view) {
        return Err(ProgramError::IllegalOwner);
    }
    if view.data_len() < TokenAccountState::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: data_len >= 165 checked above, TokenAccountState is
    // #[repr(C)] with alignment 1, pointer is to account data start.
    let state = unsafe { &*(view.data_ptr() as *const TokenAccountState) };
    if !state.is_initialized() {
        return Err(ProgramError::UninitializedAccount);
    }
    if state.mint() != mint {
        return Err(ProgramError::InvalidAccountData);
    }
    if state.owner() != authority {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Validate that an existing mint account has the expected authority.
///
/// Used by generated `#[account(init_if_needed, mint::...)]` code when the
/// account is already initialized.
#[inline(always)]
pub fn validate_mint(view: &AccountView, mint_authority: &Address) -> Result<(), ProgramError> {
    if !is_token_program_owner(view) {
        return Err(ProgramError::IllegalOwner);
    }
    if view.data_len() < MintAccountState::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    // SAFETY: data_len >= 82 checked above, MintAccountState is
    // #[repr(C)] with alignment 1, pointer is to account data start.
    let state = unsafe { &*(view.data_ptr() as *const MintAccountState) };
    if !state.is_initialized() {
        return Err(ProgramError::UninitializedAccount);
    }
    if !state.has_mint_authority() || state.mint_authority_unchecked() != mint_authority {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}
