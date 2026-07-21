//! Safe downstream code must not select a bit-pattern-restricted Rust type as
//! an instruction decoder's zero-copy representation.
use quasar_lang::prelude::*;

struct Evil;

impl InstructionArg for Evil {
    type Zc = bool;

    fn from_zc(_zc: &Self::Zc) -> Self {
        Self
    }

    fn to_zc(&self) -> Self::Zc {
        false
    }
}

fn main() {}
