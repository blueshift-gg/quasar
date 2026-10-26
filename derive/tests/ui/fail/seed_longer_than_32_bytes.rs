use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[derive(Seeds)]
#[seeds(b"a seed that is longer than thirty-two bytes")]
pub struct TooLong;

fn main() {}
