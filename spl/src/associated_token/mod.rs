use {
    crate::{
        helpers::constants::{ATA_PROGRAM_BYTES, SPL_TOKEN_ID},
        state::TokenAccountState,
    },
    quasar_core::{prelude::*, traits::Id},
};

mod address;
pub mod init;
pub mod instructions;
mod validate;

pub use {
    address::{
        get_associated_token_address, get_associated_token_address_const,
        get_associated_token_address_with_program, get_associated_token_address_with_program_const,
    },
    init::InitAssociatedToken,
    instructions::{create, create_idempotent},
    validate::validate_ata,
};

// ---------------------------------------------------------------------------
// AssociatedTokenProgram — program account type
// ---------------------------------------------------------------------------

quasar_core::define_account!(pub struct AssociatedTokenProgram => [checks::Executable, checks::Address]);

impl Id for AssociatedTokenProgram {
    const ID: Address = Address::new_from_array(ATA_PROGRAM_BYTES);
}

// ---------------------------------------------------------------------------
// AssociatedToken — account marker type
// ---------------------------------------------------------------------------

/// Associated token account view — validates owner is SPL Token program.
///
/// Use as `Account<AssociatedToken>` for SPL Token-only ATAs, or
/// `InterfaceAccount<AssociatedToken>` for both SPL Token and Token-2022.
///
/// The derive macro recognizes this type and auto-derives the ATA address
/// from `associated_token::mint` + `associated_token::authority` attributes.
#[repr(transparent)]
pub struct AssociatedToken {
    __view: AccountView,
}
impl_single_owner!(AssociatedToken, SPL_TOKEN_ID, TokenAccountState);
