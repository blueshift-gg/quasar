use solana_address::Address;

pub const ID: Address = solana_address::address!("22222222222222222222222222222222222222222222");

pub mod events;
pub mod instructions;
pub mod pda;
pub mod state;

pub use {events::*, instructions::*, pda::*, state::*};
