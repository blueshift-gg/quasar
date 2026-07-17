//! Metaplex Token Metadata CPI instruction builders.
//!
//! One module per instruction (or per closely related pair). The
//! [`MetadataCpi`] trait in `cpi` dispatches to these builders, each of
//! which returns a ready-to-invoke CPI call.

mod approve_collection;
mod burn;
mod cpi;
pub(crate) mod create_master_edition;
pub(crate) mod create_metadata;
mod data;
mod freeze_thaw;
mod mint_edition;
mod remove_creator;
mod revoke_collection;
mod set_and_verify_collection;
mod set_collection_size;
mod set_token_standard;
mod sign_metadata;
mod unverify_collection;
mod update_metadata;
mod update_primary_sale;
mod utilize;
mod verify_collection;

#[cfg(kani)]
#[path = "../../kani/instructions.rs"]
mod kani_proofs;

pub use cpi::MetadataCpi;
use data::{option_u64_data, u64_data, MAX_NAME_LEN, MAX_SYMBOL_LEN, MAX_URI_LEN};
