use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// Enum without `#[repr(u8)]` — must be rejected because `*self as u8`
// is only well-defined when the enum is fixed to a 1-byte primitive.
#[derive(QuasarSerialize)]
pub enum Side {
    Bid,
    Ask,
}

fn main() {}
