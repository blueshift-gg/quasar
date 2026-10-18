//! IDL client generation and interface compatibility checks.
//!
//! The runtime emits its interface through `idl().to_json()`. `read` adapts
//! that interface to the existing public schema used by the client generators.
pub mod codegen;
pub mod lint;
mod runtime;
pub mod types;
pub use runtime::read;
