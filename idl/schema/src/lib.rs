//! Public IDL JSON schema types for Quasar.
//!
//! This crate defines the canonical `quasar-idl/1.0.0` schema contract.
//! All client generators, CLI tools, and external tooling depend on these
//! types.
//!
//! Readers accept strict, stable `quasar-idl/1.x.y` SemVer strings, including
//! build metadata but not prerelease versions. Later compatible 1.x producers
//! must place additive data below the opaque `extensions` field; unknown root
//! and leaf fields are rejected.
//!
//! `hashes.idl` is a Quasar-producer integrity hash over every admitted field,
//! not a language-neutral JSON canonicalization standard. Verify it with this
//! crate's [`compute_idl_hash`] or with `quasar idl verify`.

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
