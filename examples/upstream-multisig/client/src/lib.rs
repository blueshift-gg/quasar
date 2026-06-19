use solana_address::Address;

pub const ID: Address = solana_address::address!("44444444444444444444444444444444444444444444");

pub mod instructions;
pub mod pda;
pub mod state;

pub use {instructions::*, pda::*, state::*};
