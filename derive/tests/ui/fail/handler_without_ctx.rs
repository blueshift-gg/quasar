use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[instruction(0)]
pub fn greet(amount: u64) -> Result {
    let _ = amount;
    Ok(())
}

fn main() {}
