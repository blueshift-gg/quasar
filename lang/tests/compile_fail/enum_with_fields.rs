use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

// Enum with non-unit variants cannot be encoded as a single discriminant
// byte, so the derive must reject it rather than silently dropping the
// payload.
#[repr(u8)]
#[derive(QuasarSerialize)]
pub enum Command {
    Ping,
    Transfer { amount: u64 },
}

fn main() {}
