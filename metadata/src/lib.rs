//! Metaplex Token Metadata accounts, constraints, initialization and CPI calls.
//! Account prefixes use `InterfaceAccount`: Metaplex may append data to them.
#![no_std]

pub mod accounts;
mod codec;
mod init;
pub mod instructions;
pub mod pda;
pub mod state;
pub mod validate;

pub use init::{InitMasterEdition, InitMetadata};
pub use state::{MasterEdition, MasterEditionAccount, Metadata, MetadataAccount};

use quasar_lang::{Address, wrapper::ProgramId};

/// Metaplex Token Metadata (`metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`).
pub const METADATA_PROGRAM_ID: Address =
    Address::from_str_const("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// Use as `Program<MetadataProgram>`.
pub struct MetadataProgram;

impl ProgramId for MetadataProgram {
    const ID: Address = METADATA_PROGRAM_ID;
}

pub mod prelude {
    pub use crate::{
        InitMasterEdition, InitMetadata, MasterEdition, MasterEditionAccount, Metadata,
        MetadataAccount, MetadataProgram,
        accounts::{master_edition, metadata},
        instructions, pda,
    };
}
