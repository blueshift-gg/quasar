//! Metaplex Token Metadata program integration.
//!
//! Provides zero-copy account types ([`MetadataAccount`],
//! [`MasterEditionAccount`]), CPI methods ([`MetadataCpi`]), and initialization
//! helpers ([`InitMetadata`], [`InitMasterEdition`]) for the Metaplex Token
//! Metadata program.

#![no_std]

mod codec;
mod constants;
mod init;
pub mod instructions;
mod program;
mod state;

pub use {
    constants::METADATA_PROGRAM_ID,
    init::{InitMasterEdition, InitMetadata},
    instructions::MetadataCpi,
    program::MetadataProgram,
    state::{MasterEditionAccount, MasterEditionPrefix, MetadataAccount, MetadataPrefix},
};
