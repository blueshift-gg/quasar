use solana_address::Address;

pub const ID: Address = solana_address::address!("33333333333333333333333333333333333333333333");

pub mod errors;
pub mod instructions;
pub mod pda;

pub use errors::*;
pub use instructions::*;
pub use pda::*;
