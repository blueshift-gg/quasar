use quasar_lang::prelude::*;

#[event(discriminator = 1)]
pub struct BytesEvent {
    pub hash: [u8; 32],
    pub amount: u64,
}

fn main() {}
