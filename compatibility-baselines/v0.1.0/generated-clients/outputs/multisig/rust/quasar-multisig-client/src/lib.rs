use solana_address::Address;

pub const ID: Address = solana_address::address!("44444444444444444444444444444444444444444444");

pub mod instructions;
pub mod state;
pub mod errors;
pub mod pda;

pub use instructions::*;
pub use state::*;
pub use errors::*;
pub use pda::*;
