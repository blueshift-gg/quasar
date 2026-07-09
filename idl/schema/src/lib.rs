//! Public IDL JSON schema types for Quasar.
//!
//! This crate defines the canonical `quasar-idl/1.0.0` schema contract.
//! All client generators, CLI tools, and external tooling depend on these
//! types.

pub mod account;
pub mod canonical;
pub mod codec;
pub mod error;
pub mod event;
pub mod instruction;
pub mod layout;
pub mod root;
pub mod space;
pub mod types;

pub use {
    account::*, canonical::*, codec::*, error::*, event::*, instruction::*, layout::*, root::*,
    space::*, types::*,
};
