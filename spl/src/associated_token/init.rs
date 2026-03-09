use quasar_core::prelude::*;

use super::instructions::{create, create_idempotent};
use super::AssociatedTokenProgram;
use crate::helpers::init::validate_token_account;
use crate::instructions::TokenCpi;

/// Extension trait for associated token account initialization.
///
/// Unlike [`InitToken`](crate::InitToken) which chains `create_account + initialize_account3`,
/// this delegates to the ATA program which handles creation + initialization in a single CPI.
///
/// ```ignore
/// self.new_ata.init(
///     self.payer,
///     self.wallet,
///     self.mint,
///     self.system_program,
///     self.token_program,
///     self.ata_program,
/// )?;
/// ```
pub trait InitAssociatedToken: AsAccountView + Sized {
    /// Create an associated token account via the ATA program.
    ///
    /// Fails if the account already exists.
    #[inline(always)]
    fn init(
        &self,
        payer: &impl AsAccountView,
        wallet: &impl AsAccountView,
        mint: &impl AsAccountView,
        system_program: &Program<System>,
        token_program: &impl TokenCpi,
        ata_program: &AssociatedTokenProgram,
    ) -> Result<(), ProgramError> {
        create(
            ata_program,
            payer,
            self.to_account_view(),
            wallet,
            mint,
            system_program,
            token_program,
        )
        .invoke()
    }

    /// Create an associated token account if it doesn't already exist.
    ///
    /// Uses `CreateIdempotent` — no-ops if the account is already initialized.
    /// When the account exists, validates mint and authority match.
    #[inline(always)]
    fn init_if_needed(
        &self,
        payer: &impl AsAccountView,
        wallet: &impl AsAccountView,
        mint: &impl AsAccountView,
        system_program: &Program<System>,
        token_program: &impl TokenCpi,
        ata_program: &AssociatedTokenProgram,
    ) -> Result<(), ProgramError> {
        let view = self.to_account_view();
        if quasar_core::is_system_program(unsafe { view.owner() }) {
            create_idempotent(
                ata_program,
                payer,
                view,
                wallet,
                mint,
                system_program,
                token_program,
            )
            .invoke()
        } else {
            validate_token_account(
                view,
                mint.to_account_view().address(),
                wallet.to_account_view().address(),
            )
        }
    }
}
