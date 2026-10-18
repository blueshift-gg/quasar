use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
pub struct State {
    pub value: u64,
}

#[derive(Accounts)]
pub struct Create {
    #[account(init())]
    pub state: Account<State>,
    pub system_program: Program<SystemProgram>,
}

fn main() {}
