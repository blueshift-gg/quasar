use quasar_lang::prelude::*;

const HASH_LEN: usize = 32;

#[event(discriminator = [1])]
pub struct Bad {
    pub hash: [u8; HASH_LEN],
}

fn main() {}
