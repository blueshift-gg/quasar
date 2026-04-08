//! IDL generation for Quasar programs.
//!
//! Parses Quasar program source files and produces:
//! - **IDL JSON** — machine-readable program interface (instructions, accounts,
//!   events, errors, types)
//! - **TypeScript client** — typed instruction builders and account decoders
//! - **Rust client** — off-chain instruction construction
//!
//! The parser reads Rust AST via `syn` to extract `#[program]`, `#[account]`,
//! `#[instruction]`, `#[event]`, and `#[error_code]` definitions. Discriminator
//! collision detection prevents runtime ID clashes across all types.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p quasar-idl -- path/to/program/crate
//! ```
pub mod codegen;
pub mod lint;
pub mod parser;
pub mod types;
