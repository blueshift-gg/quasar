use quasar_core::cpi::system::SYSTEM_PROGRAM_ID;
use quasar_core::prelude::*;

use crate::cpi::TokenCpi;
use crate::interface::InterfaceMintAccount;
use crate::interface::InterfaceTokenAccount;
use crate::state::{MintAccountState, TokenAccountState};
use crate::token::{MintAccount, TokenAccount};
use crate::token_2022::{Mint2022Account, Token2022Account};

/// Extension trait providing `.init()` on `Initialize<T>` for token account types.
///
/// Chains `SystemProgram::create_account` → `InitializeAccount3` in two CPIs.
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
    /// Chains `SystemProgram::create_account` → `InitializeAccount3` in two CPIs.
    /// The account must not already exist.
    #[inline(always)]
    fn init(
        &self,
        system_program: &SystemProgram,
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
        system_program: &SystemProgram,
        payer: &impl AsAccountView,
        token_program: &impl TokenCpi,
        mint: &impl AsAccountView,
        owner: &Address,
        rent: Option<&Rent>,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if view.owned_by(&SYSTEM_PROGRAM_ID) {
            self.init(system_program, payer, token_program, mint, owner, rent)
        } else {
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

impl InitToken for Initialize<TokenAccount> {}
impl InitToken for Initialize<Token2022Account> {}
impl InitToken for Initialize<InterfaceTokenAccount> {}

/// Extension trait providing `.init()` on `Initialize<T>` for mint account types.
///
/// Chains `SystemProgram::create_account` → `InitializeMint2` in two CPIs.
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
    /// Chains `SystemProgram::create_account` → `InitializeMint2` in two CPIs.
    /// The account must not already exist.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn init(
        &self,
        system_program: &SystemProgram,
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
        system_program: &SystemProgram,
        payer: &impl AsAccountView,
        token_program: &impl TokenCpi,
        decimals: u8,
        mint_authority: &Address,
        freeze_authority: Option<&Address>,
        rent: Option<&Rent>,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if view.owned_by(&SYSTEM_PROGRAM_ID) {
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

impl InitMint for Initialize<MintAccount> {}
impl InitMint for Initialize<Mint2022Account> {}
impl InitMint for Initialize<InterfaceMintAccount> {}
