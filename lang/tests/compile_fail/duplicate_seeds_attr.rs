#![allow(unexpected_cfgs)]

use quasar_derive::Seeds;

#[derive(Seeds)]
#[seeds(b"first")]
#[seeds(b"second")]
pub struct DuplicateSeeds;

fn main() {}
