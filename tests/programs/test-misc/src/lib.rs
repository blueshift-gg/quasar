#![no_std]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod instructions;
use instructions::*;
pub mod errors;
pub mod state;
declare_id!("44444444444444444444444444444444444444444444");

pub const EXPECTED_ADDRESS: Address = Address::new_from_array([42u8; 32]);

#[program]
mod quasar_test_misc {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Ctx<InitializeSimple>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    #[instruction(discriminator = 1)]
    pub fn close_account(ctx: Ctx<CloseAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 2)]
    pub fn update_has_one(ctx: Ctx<UpdateHasOne>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 3)]
    pub fn update_address(ctx: Ctx<UpdateAddress>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 4)]
    pub fn signer_check(ctx: Ctx<SignerCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 5)]
    pub fn owner_check(ctx: Ctx<OwnerCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 6)]
    pub fn mut_check(ctx: Ctx<MutCheck>, new_value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_value)
    }

    #[instruction(discriminator = 7)]
    pub fn init_if_needed(ctx: Ctx<InitIfNeeded>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    #[instruction(discriminator = 8)]
    pub fn system_account_check(ctx: Ctx<SystemAccountCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 9)]
    pub fn transfer_test(ctx: Ctx<TransferTest>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }

    #[instruction(discriminator = 10)]
    pub fn assign_test(ctx: Ctx<AssignTest>, owner: Address) -> Result<(), ProgramError> {
        ctx.accounts.handler(owner)
    }

    #[instruction(discriminator = 11)]
    pub fn create_account_test(
        ctx: Ctx<CreateAccountTest>,
        lamports: u64,
        space: u64,
        owner: Address,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(lamports, space, owner)
    }

    #[instruction(discriminator = 12)]
    pub fn check_multi_disc(ctx: Ctx<CheckMultiDisc>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 13)]
    pub fn constraint_check(ctx: Ctx<ConstraintCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 14)]
    pub fn realloc_check(ctx: Ctx<ReallocCheck>, _new_space: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 15)]
    pub fn optional_account(ctx: Ctx<OptionalAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 16)]
    pub fn remaining_accounts_check(
        ctx: CtxWithRemaining<RemainingAccountsCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(ctx.remaining_accounts())
    }

    #[instruction(discriminator = 44)]
    pub fn remaining_accounts_passthrough_check(
        ctx: CtxWithRemaining<RemainingAccountsCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(ctx.remaining_accounts_passthrough())
    }

    #[instruction(discriminator = 20)]
    pub fn dynamic_account_check(ctx: Ctx<DynamicAccountCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 21)]
    pub fn dynamic_instruction_check(
        ctx: Ctx<DynamicInstructionCheck>,
        name: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(name)
    }

    #[instruction(discriminator = 22)]
    pub fn mixed_account_check(ctx: Ctx<MixedAccountCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 23)]
    pub fn small_prefix_check(ctx: Ctx<SmallPrefixCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 24)]
    pub fn dynamic_readback(
        ctx: Ctx<DynamicReadback>,
        expected_name_len: u8,
        expected_tags_count: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_name_len, expected_tags_count)
    }

    #[instruction(discriminator = 26)]
    pub fn dynamic_mutate(
        ctx: Ctx<DynamicMutate>,
        new_name: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_name)
    }

    #[instruction(discriminator = 17)]
    pub fn space_override(ctx: Ctx<SpaceOverride>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    #[instruction(discriminator = 18)]
    pub fn explicit_payer(ctx: Ctx<ExplicitPayer>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    #[instruction(discriminator = 19)]
    pub fn optional_has_one(ctx: Ctx<OptionalHasOne>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 27)]
    pub fn mutate_then_readback(
        ctx: Ctx<MutateThenReadback>,
        expected_tags_count: u8,
        new_name: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_tags_count, new_name)
    }

    #[instruction(discriminator = 28)]
    pub fn dyn_str_check(ctx: Ctx<DynStrCheck>, expected_len: u8) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_len)
    }

    #[instruction(discriminator = 29)]
    pub fn dyn_bytes_check(ctx: Ctx<DynBytesCheck>, expected_len: u8) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_len)
    }

    #[instruction(discriminator = 30)]
    pub fn signer_and_mut_check(
        ctx: Ctx<SignerAndMutCheck>,
        new_value: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_value)
    }

    #[instruction(discriminator = 31)]
    pub fn has_one_and_owner_check(ctx: Ctx<HasOneAndOwnerCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 32)]
    pub fn constraint_custom_error(ctx: Ctx<ConstraintCustomError>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 41)]
    pub fn double_mut_check(ctx: Ctx<DoubleMutCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 42)]
    pub fn init_no_disc(ctx: Ctx<InitNoDisc>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    #[instruction(discriminator = 43)]
    pub fn read_no_disc(ctx: Ctx<ReadNoDisc>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 45)]
    pub fn return_u64(ctx: Ctx<ReturnU64>) -> Result<u64, ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 46)]
    pub fn return_payload(
        ctx: Ctx<ReturnPayloadInstruction>,
    ) -> Result<state::ReturnPayload, ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 47)]
    pub fn plain_ok(ctx: Ctx<PlainOk>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 48)]
    pub fn cpi_invoke_with_return(ctx: Ctx<CpiInvokeWithReturn>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 49)]
    pub fn cpi_invoke_struct_return(ctx: Ctx<CpiInvokeStructReturn>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 50)]
    pub fn cpi_invoke_ignore_return(ctx: Ctx<CpiInvokeIgnoreReturn>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 51)]
    pub fn cpi_invoke_missing_return(ctx: Ctx<CpiInvokeMissingReturn>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 52)]
    pub fn option_u64_some(
        ctx: Ctx<OptionU64Some>,
        value: Option<u64>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(value)
    }

    #[instruction(discriminator = 53)]
    pub fn option_u64_none(
        ctx: Ctx<OptionU64None>,
        value: Option<u64>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(value)
    }

    #[instruction(discriminator = 54)]
    pub fn option_address_some(
        ctx: Ctx<OptionAddressSome>,
        addr: Option<Address>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(addr)
    }

    #[instruction(discriminator = 55)]
    pub fn option_address_none(
        ctx: Ctx<OptionAddressNone>,
        addr: Option<Address>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(addr)
    }

    #[instruction(discriminator = 56)]
    pub fn interface_migration_check(
        ctx: Ctx<InterfaceMigrationCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 57)]
    pub fn dynamic_stack_cache(
        ctx: Ctx<DynamicStackCache>,
        new_name: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_name)
    }

    #[instruction(discriminator = 58)]
    pub fn dynamic_view_mut(
        ctx: Ctx<DynamicViewMut>,
        new_name: String<8>,
        new_tags: Vec<Address, 2>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_name, new_tags)
    }

    #[instruction(discriminator = 59)]
    pub fn dynamic_view_mut_missing_field(
        ctx: Ctx<DynamicViewMutMissingField>,
        new_name: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_name)
    }

    #[instruction(discriminator = 60)]
    pub fn cpi_mut_readback(ctx: Ctx<CpiMutReadback>, new_value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_value)
    }
}
