use solana_address::Address;

pub const ID: Address = solana_address::address!("Bench111111111111111111111111111111111111111");

pub mod instructions;
pub mod state;
pub mod events;
pub mod errors;
pub mod pda;

pub use instructions::*;
pub use state::*;
pub use events::*;
pub use errors::*;
pub use pda::*;
