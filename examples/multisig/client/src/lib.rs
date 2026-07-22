use solana_address::Address;

pub const ID: Address = solana_address::address!("44444444444444444444444444444444444444444444");

pub mod errors;
pub mod instructions;
pub mod pda;
pub mod state;

pub use errors::*;
pub use instructions::*;
pub use pda::*;
pub use state::*;
