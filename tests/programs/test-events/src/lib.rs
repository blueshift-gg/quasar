#![no_std]
#![allow(dead_code)]

use quasar_core::prelude::*;

mod instructions;
use instructions::*;
pub mod events;
declare_id!("66666666666666666666666666666666666666666666");

#[program]
mod quasar_test_events {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn emit_u64_event(ctx: Ctx<EmitU64Event>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value)
    }

    #[instruction(discriminator = 1)]
    pub fn emit_address_event(
        ctx: Ctx<EmitAddressEvent>,
        addr: Address,
        value: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(addr, value)
    }

    #[instruction(discriminator = 2)]
    pub fn emit_bool_event(ctx: Ctx<EmitBoolEvent>, flag: bool) -> Result<(), ProgramError> {
        ctx.accounts.handler(flag)
    }

    #[instruction(discriminator = 3)]
    pub fn emit_multi_field(
        ctx: Ctx<EmitMultiField>,
        a: u64,
        b: u64,
        c: Address,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(a, b, c)
    }

    #[instruction(discriminator = 4)]
    pub fn emit_via_cpi(ctx: Ctx<EmitViaCpi>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value)
    }
}
