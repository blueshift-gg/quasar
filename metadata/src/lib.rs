//! Metaplex Token Metadata program integration.
//!
//! Provides zero-copy account types ([`MetadataAccount`],
//! [`MasterEditionAccount`]), CPI methods ([`MetadataCpi`]), and initialization
//! helpers ([`InitMetadata`], [`InitMasterEdition`]) for the Metaplex Token
//! Metadata program.

#![no_std]
#![warn(missing_docs)]

mod account_init;
pub mod accounts;
mod codec;
mod constants;
mod init;
/// Token Metadata CPI instruction builders.
pub mod instructions;
pub mod pda;
pub mod prelude;
mod program;
mod state;
/// Validation helpers for Token Metadata accounts and program identities.
pub mod validate;

pub use {
    account_init::{MasterEditionInitParams, MetadataInitParams},
    constants::METADATA_PROGRAM_ID,
    init::{InitMasterEdition, InitMetadata},
    instructions::MetadataCpi,
    program::MetadataProgram,
    state::{
        MasterEditionAccount, MasterEditionPrefix, MasterEditionPrefixZc, MetadataAccount,
        MetadataPrefix, MetadataPrefixZc,
    },
};
