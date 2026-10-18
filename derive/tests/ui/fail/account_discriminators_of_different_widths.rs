use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
pub struct Short {
    pub value: u64,
}

#[account(discriminator = [1, 2])]
pub struct Long {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Both {
    pub short: Account<Short>,
    pub long: Account<Long>,
}
#[program]
mod widths {
    use super::*;

    #[instruction(0)]
    pub fn both(_ctx: Ctx<Both>) -> Result<()> {
        Ok(())
    }
}

fn main() {}
