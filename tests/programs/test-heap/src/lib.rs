#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
pub mod events;
declare_id!("33333333333333333333333333333333333333333333");

#[program]
mod quasar_test_heap {
    use super::*;

    /// Non-heap instruction with no allocation.
    #[instruction(discriminator = 0)]
    pub fn no_heap_ok(ctx: Ctx<NoHeapOk>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Heap-enabled instruction that allocates.
    #[instruction(discriminator = 1, heap)]
    pub fn heap_vec_ok(ctx: Ctx<HeapVecOk>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Non-heap instruction that attempts allocation.
    #[instruction(discriminator = 2)]
    pub fn no_heap_alloc_attempt(ctx: Ctx<NoHeapAllocAttempt>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Event emission in a program with at least one heap-enabled instruction.
    #[instruction(discriminator = 3)]
    pub fn emit_event_ok(ctx: Ctx<EmitEventOk>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    /// Heap-enabled instruction allocating past the 256 KiB cap: must abort.
    #[instruction(discriminator = 4, heap)]
    pub fn heap_alloc_beyond_cap(ctx: Ctx<HeapAllocBeyondCap>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}
