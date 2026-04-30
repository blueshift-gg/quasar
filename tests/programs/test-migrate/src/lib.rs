#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
pub mod state;

declare_id!("MiGR8NhJhroY6hma5mfg5xM1EG6A5FNyKoDE3aTr3Sq");

#[program]
mod quasar_test_migrate {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn migrate_config(ctx: Ctx<MigrateConfig>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
