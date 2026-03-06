//! Metaplex Token Metadata program integration.
//!
//! Provides zero-copy account types ([`MetadataAccount`], [`MasterEditionAccount`]),
//! CPI methods ([`MetadataCpi`]), and initialization helpers ([`InitMetadata`],
//! [`InitMasterEdition`]) for the Metaplex Token Metadata program.

mod constants;
pub mod instructions;
mod init;
mod program;
mod state;

pub use constants::METADATA_PROGRAM_ID;
pub use instructions::MetadataCpi;
pub use init::{InitMasterEdition, InitMetadata};
pub use program::MetadataProgram;
pub use state::{MasterEditionAccount, MasterEditionPrefix, MetadataAccount, MetadataPrefix};
