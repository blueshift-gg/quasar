//! IDL generation for Quasar programs.
//!
//! IDL fragments are emitted at compile time by derive macros (`#[account]`,
//! `#[event]`, `#[error_code]`, `#[derive(QuasarSerialize)]`) and assembled by
//! the `#[program]` macro into a complete IDL JSON via the `idl-build` feature.
//!
//! This crate provides:
//! - **Codegen**: TypeScript and Rust client generators from IDL JSON
//! - **Types**: re-export of `quasar-idl-schema` types
//!
//! Programs compile with `--features idl-build` to produce IDL.
pub mod codegen;
pub mod types;
