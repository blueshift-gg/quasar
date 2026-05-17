#![allow(unexpected_cfgs)]

use quasar_derive::Seeds;

#[derive(Seeds)]
#[seeds(
    b"many",
    s0: u8,
    s1: u8,
    s2: u8,
    s3: u8,
    s4: u8,
    s5: u8,
    s6: u8,
    s7: u8,
    s8: u8,
    s9: u8,
    s10: u8,
    s11: u8,
    s12: u8,
    s13: u8,
    s14: u8,
    s15: u8
)]
pub struct TooManySeeds;

fn main() {}
