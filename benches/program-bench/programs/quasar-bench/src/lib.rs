//! Consolidated CU benchmark program.
//!
//! One instruction per discriminator, covering the success-path surface of the
//! quasar framework so the driver can record compute units for each. Ranges:
//!   0..=9    canonical (cross-framework comparable, mirrors typhoon)
//!   10..=33  PDA / dynamic accounts / lifecycle
//!   40..=51  SPL token (init, transfer, mint, burn, approve, revoke, close, T22, interface)
//!   80..=108 account validation / events / heap / sysvar / CPI
//!
//! Instruction bodies are lifted from the crates under `tests/programs/*`;
//! account and event discriminators are reassigned here to stay unique within
//! one program.
#![no_std]
#![allow(dead_code, clippy::too_many_arguments, clippy::result_unit_err)]

use quasar_lang::{cpi::InstructionAccount, prelude::*, sysvars::Sysvar as _};
use quasar_spl::prelude::*;

declare_id!("Bench111111111111111111111111111111111111111");

/// Self marker so CPI instructions can target this program.
pub struct SelfProgram;
impl Id for SelfProgram {
    const ID: Address = crate::ID;
}

pub const RETURN_U64_VALUE: u64 = 777;

#[error_code]
pub enum BenchError {
    RequireEqFailed,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[account(discriminator = 1)]
#[seeds(b"pda")]
pub struct Data {
    pub byte: u8,
    pub bump: u8,
}

#[account(discriminator = 10, set_inner)]
#[seeds(b"simple", authority: Address)]
pub struct SimpleAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = 11, set_inner)]
#[seeds(b"spacetest", authority: Address)]
pub struct SpaceTestAccount {
    pub authority: Address,
    pub value: u64,
    pub bump: u8,
}

#[account(discriminator = 12, set_inner)]
pub struct DynamicAccount {
    pub name: String<8>,
    pub tags: Vec<Address, 2>,
}

#[account(discriminator = 13, set_inner)]
pub struct TwoDynArgsAccount {
    pub tag: u64,
    pub a_len: u8,
    pub a: u64,
    pub b_len: u8,
    pub b: u64,
}

#[account(discriminator = 14)]
pub struct DynStrAccount {
    pub authority: Address,
    pub label: String<255>,
}

#[account(discriminator = 15)]
pub struct DynBytesAccount {
    pub authority: Address,
    pub data: Vec<u8, 1024>,
}

#[account(discriminator = 16, set_inner)]
pub struct ErrorTestAccount {
    pub authority: Address,
    pub value: u64,
}

#[account(discriminator = 17, set_inner)]
#[seeds(b"clock")]
pub struct ClockSnapshot {
    pub slot: u64,
    pub unix_timestamp: i64,
}

#[account(discriminator = 18, set_inner)]
#[seeds(b"rent")]
pub struct RentSnapshot {
    pub min_balance_100: u64,
}

#[account(discriminator = 19, set_inner)]
#[seeds(b"rent_calc")]
pub struct RentCalcSnapshot {
    pub min_balance: u64,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event(discriminator = 1)]
pub struct SimpleEvent {
    pub value: u64,
}

#[event(discriminator = 4)]
pub struct MultiEvent {
    pub a: u64,
    pub b: u64,
    pub c: Address,
}

#[event(discriminator = 5)]
pub struct EmptyEvent {}

#[event(discriminator = 6)]
pub struct LargeEvent {
    pub a: u64,
    pub b: u64,
    pub c: u64,
    pub d: u64,
    pub e: Address,
    pub f: Address,
    pub g: u128,
    pub h: u128,
}

#[event(discriminator = 7)]
pub struct SecondSimpleEvent {
    pub value: u64,
}

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

#[program]
mod quasar_bench {
    use super::*;

    // --- 0..=9 canonical ---
    #[instruction(discriminator = 0)]
    pub fn ping(_ctx: Ctx<Empty>) -> Result<(), ProgramError> {
        Ok(())
    }
    #[instruction(discriminator = 1)]
    pub fn log(_ctx: Ctx<Empty>) -> Result<(), ProgramError> {
        quasar_lang::prelude::log("Instruction: Log");
        Ok(())
    }
    #[instruction(discriminator = 2)]
    pub fn create_account(ctx: Ctx<CreateAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 3)]
    pub fn transfer(ctx: Ctx<Transfer>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }
    #[instruction(discriminator = 4)]
    pub fn unchecked_accounts(_ctx: Ctx<UncheckedAccounts>) -> Result<(), ProgramError> {
        Ok(())
    }
    #[instruction(discriminator = 5)]
    pub fn accounts(_ctx: Ctx<AccountsC>) -> Result<(), ProgramError> {
        Ok(())
    }
    #[instruction(discriminator = 6)]
    pub fn create_pda_static(ctx: Ctx<CreatePdaStatic>) -> Result<(), ProgramError> {
        let bump = ctx.bumps.account;
        ctx.accounts.handler(bump)
    }
    #[instruction(discriminator = 7)]
    pub fn verify_pda_static(_ctx: Ctx<VerifyPdaStatic>) -> Result<(), ProgramError> {
        Ok(())
    }
    #[instruction(discriminator = 8)]
    pub fn create_pda_dynamic(ctx: Ctx<CreatePdaDynamic>) -> Result<(), ProgramError> {
        let bump = ctx.bumps.account;
        ctx.accounts.handler(bump)
    }
    #[instruction(discriminator = 9)]
    pub fn verify_pda_dynamic(_ctx: Ctx<VerifyPdaDynamic>) -> Result<(), ProgramError> {
        Ok(())
    }

    // --- 10..=21 PDA ---

    // --- 22..=27 dynamic ---
    #[instruction(discriminator = 22)]
    pub fn dynamic_readback(
        ctx: Ctx<DynamicReadback>,
        expected_name_len: u8,
        expected_tags_count: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_name_len, expected_tags_count)
    }
    #[instruction(discriminator = 23)]
    pub fn dynamic_mutate(
        ctx: Ctx<DynamicMutate>,
        new_name: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_name)
    }
    #[instruction(discriminator = 24)]
    pub fn dynamic_view_mut(
        ctx: Ctx<DynamicViewMut>,
        new_name: String<8>,
        new_tags: Vec<Address, 2>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_name, new_tags)
    }
    #[instruction(discriminator = 25)]
    pub fn two_dyn(
        ctx: Ctx<TwoDyn>,
        tag: u64,
        a: String<8>,
        b: String<8>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(tag, a, b)
    }
    #[instruction(discriminator = 26)]
    pub fn dyn_bytes_check(ctx: Ctx<DynBytesCheck>, expected_len: u8) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_len)
    }
    #[instruction(discriminator = 27)]
    pub fn dyn_str_check(ctx: Ctx<DynStrCheck>, expected_len: u8) -> Result<(), ProgramError> {
        ctx.accounts.handler(expected_len)
    }

    // --- 28..=33 lifecycle ---
    #[instruction(discriminator = 28)]
    pub fn create_account_cpi(
        ctx: Ctx<CreateAccountCpi>,
        lamports: u64,
        space: u64,
        owner: Address,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(lamports, space, owner)
    }
    #[instruction(discriminator = 29)]
    pub fn close_account(ctx: Ctx<CloseAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 30)]
    pub fn realloc_check(ctx: Ctx<ReallocCheck>, new_space: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_space)
    }
    #[instruction(discriminator = 32)]
    pub fn assign_test(ctx: Ctx<AssignTest>, owner: Address) -> Result<(), ProgramError> {
        ctx.accounts.handler(owner)
    }
    #[instruction(discriminator = 33)]
    pub fn space_override(ctx: Ctx<SpaceOverride>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value, &ctx.bumps)
    }

    // --- 40..=51 token ---
    #[instruction(discriminator = 40)]
    pub fn init_mint(ctx: Ctx<InitMint>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 41)]
    pub fn init_token_account(ctx: Ctx<InitToken>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 42)]
    pub fn init_ata(ctx: Ctx<InitAta>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 43)]
    pub fn transfer_checked(
        ctx: Ctx<TransferChecked>,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount, decimals)
    }
    #[instruction(discriminator = 44)]
    pub fn mint_to(ctx: Ctx<MintTo>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }
    #[instruction(discriminator = 45)]
    pub fn burn(ctx: Ctx<Burn>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }
    #[instruction(discriminator = 46)]
    pub fn approve(ctx: Ctx<Approve>, amount: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount)
    }
    #[instruction(discriminator = 47)]
    pub fn revoke(ctx: Ctx<Revoke>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 48)]
    pub fn close_token_account(ctx: Ctx<CloseTokenAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 49)]
    pub fn sweep_and_close(ctx: Ctx<SweepAndClose>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 50)]
    pub fn init_mint_t22(ctx: Ctx<InitMintT22>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 51)]
    pub fn transfer_checked_interface(
        ctx: Ctx<TransferCheckedInterface>,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount, decimals)
    }

    // --- 80..=92 validation ---
    #[instruction(discriminator = 81)]
    pub fn header_nodup_signer(ctx: Ctx<HeaderNoDupSigner>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 83)]
    pub fn header_dup_signer(ctx: Ctx<HeaderDupSigner>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 85)]
    pub fn two_accounts_check(ctx: Ctx<TwoAccountsCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 87)]
    pub fn has_one_default(ctx: Ctx<HasOneDefault>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 88)]
    pub fn owner_check(ctx: Ctx<OwnerCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 89)]
    pub fn program_check(ctx: Ctx<ProgramCheck>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 91)]
    pub fn require_eq_check(ctx: Ctx<RequireEqCheck>, a: u64, b: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(a, b)
    }
    #[instruction(discriminator = 92)]
    pub fn remaining_typed_check(
        ctx: CtxWithRemaining<RemainingTypedCheck>,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(ctx.remaining_accounts())
    }

    // --- 93..=98 events ---
    #[instruction(discriminator = 93)]
    pub fn emit_empty_event(ctx: Ctx<EmitEmptyEvent>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 94)]
    pub fn emit_u64_event(ctx: Ctx<EmitU64Event>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value)
    }
    #[instruction(discriminator = 95)]
    pub fn emit_multi_field(
        ctx: Ctx<EmitMultiField>,
        a: u64,
        b: u64,
        c: Address,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(a, b, c)
    }
    #[instruction(discriminator = 96)]
    pub fn emit_large_event(
        ctx: Ctx<EmitLargeEvent>,
        a: u64,
        b: u64,
        c: u64,
        d: u64,
        e: Address,
        f: Address,
        g: u128,
        h: u128,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(a, b, c, d, e, f, g, h)
    }
    #[instruction(discriminator = 97)]
    pub fn emit_two_events(
        ctx: Ctx<EmitTwoEvents>,
        first: u64,
        second: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts.handler(first, second)
    }
    #[instruction(discriminator = 98)]
    pub fn emit_via_cpi(ctx: Ctx<EmitViaCpi>, value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(value)
    }

    // --- 99..=100 heap ---
    #[instruction(discriminator = 100, heap)]
    pub fn heap_vec_ok(ctx: Ctx<HeapVecOk>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    // --- 101..=104 sysvar ---
    #[instruction(discriminator = 101)]
    pub fn read_clock(ctx: Ctx<ReadClock>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 102)]
    pub fn read_clock_from_account(ctx: Ctx<ReadClockFromAccount>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 103)]
    pub fn read_rent(ctx: Ctx<ReadRent>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 104)]
    pub fn read_rent_calc(ctx: Ctx<ReadRentCalc>, data_len: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(data_len)
    }

    // --- 105..=108 CPI ---
    #[instruction(discriminator = 105)]
    pub fn return_u64(ctx: Ctx<ReturnU64>) -> Result<u64, ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 106)]
    pub fn cpi_invoke_with_return(ctx: Ctx<CpiInvokeWithReturn>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 107)]
    pub fn cpi_invoke_ignore_return(ctx: Ctx<CpiInvokeIgnoreReturn>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
    #[instruction(discriminator = 108)]
    pub fn cpi_mut_readback(ctx: Ctx<CpiMutReadback>, new_value: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(new_value)
    }
}

// ---------------------------------------------------------------------------
// Accounts + handlers
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Empty {}

#[derive(Accounts)]
pub struct CreateAccount {
    #[account(mut)]
    pub admin: Signer,
    #[account(init, payer = admin)]
    pub account: Account<Data>,
    pub system_program: Program<SystemProgram>,
}
impl CreateAccount {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        self.account.byte = 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Transfer {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub account: SystemAccount,
    pub system_program: Program<SystemProgram>,
}
impl Transfer {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.system_program
            .transfer(&self.payer, &self.account, amount)
            .invoke()
    }
}

#[derive(Seeds)]
#[seeds(b"pda", authority: Address)]
pub struct DataPdaDyn;

#[derive(Accounts)]
pub struct CreatePdaStatic {
    #[account(mut)]
    pub admin: Signer,
    #[account(init, payer = admin, address = Data::seeds())]
    pub account: Account<Data>,
    pub system_program: Program<SystemProgram>,
}
impl CreatePdaStatic {
    #[inline(always)]
    pub fn handler(&mut self, bump: u8) -> Result<(), ProgramError> {
        self.account.byte = 1;
        self.account.bump = bump;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct VerifyPdaStatic {
    #[account(address = Data::seeds())]
    pub account: Account<Data>,
}

#[derive(Accounts)]
pub struct CreatePdaDynamic {
    #[account(mut)]
    pub admin: Signer,
    /// CHECK: only used as a PDA seed.
    pub authority: UncheckedAccount,
    #[account(init, payer = admin, address = DataPdaDyn::seeds(authority.address()))]
    pub account: Account<Data>,
    pub system_program: Program<SystemProgram>,
}
impl CreatePdaDynamic {
    #[inline(always)]
    pub fn handler(&mut self, bump: u8) -> Result<(), ProgramError> {
        self.account.byte = 1;
        self.account.bump = bump;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct VerifyPdaDynamic {
    /// CHECK: only used as a PDA seed.
    pub authority: UncheckedAccount,
    #[account(address = DataPdaDyn::seeds(authority.address()))]
    pub account: Account<Data>,
}

#[derive(Accounts)]
pub struct UncheckedAccounts {
    pub account1: UncheckedAccount,
    pub account2: UncheckedAccount,
    pub account3: UncheckedAccount,
    pub account4: UncheckedAccount,
    pub account5: UncheckedAccount,
    pub account6: UncheckedAccount,
    pub account7: UncheckedAccount,
    pub account8: UncheckedAccount,
    pub account9: UncheckedAccount,
    pub account10: UncheckedAccount,
}

#[derive(Accounts)]
pub struct AccountsC {
    pub account1: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account2: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account3: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account4: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account5: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account6: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account7: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account8: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account9: Account<Data>,
    /// CHECK: benchmark reuses one initialized account for all slots.
    #[account(dup)]
    pub account10: Account<Data>,
}

// --- PDA ---

// --- dynamic ---

#[derive(Accounts)]
pub struct DynamicReadback {
    pub account: Account<DynamicAccount>,
}
impl DynamicReadback {
    #[inline(always)]
    pub fn handler(
        &self,
        expected_name_len: u8,
        expected_tags_count: u8,
    ) -> Result<(), ProgramError> {
        let name = self.account.name();
        if name.len() != expected_name_len as usize {
            return Err(ProgramError::Custom(1));
        }
        let tags = self.account.tags();
        if tags.len() != expected_tags_count as usize {
            return Err(ProgramError::Custom(2));
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DynamicMutate {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}
impl DynamicMutate {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str) -> Result<(), ProgramError> {
        let mut guard = self.account.as_mut(self.payer.to_account_view());
        if !guard.name.set(new_name) {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DynamicViewMut {
    #[account(mut)]
    pub account: Account<DynamicAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}
impl DynamicViewMut {
    #[inline(always)]
    pub fn handler(&mut self, new_name: &str, new_tags: &[Address]) -> Result<(), ProgramError> {
        {
            let mut guard = self.account.as_mut(self.payer.to_account_view());
            if !guard.name.set(new_name) {
                return Err(ProgramError::InvalidInstructionData);
            }
            if !guard.tags.set_from_slice(new_tags) {
                return Err(ProgramError::InvalidInstructionData);
            }
        }
        if self.account.name() != new_name {
            return Err(ProgramError::Custom(13));
        }
        if self.account.tags() != new_tags {
            return Err(ProgramError::Custom(14));
        }
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(tag: u64, a: String<8>, b: String<8>)]
pub struct TwoDyn {
    #[account(mut, constraints(tag != 0 && a.len() == b.len()))]
    pub account: Account<TwoDynArgsAccount>,
}
impl TwoDyn {
    #[inline(always)]
    pub fn handler(&mut self, tag: u64, a: &str, b: &str) -> Result<(), ProgramError> {
        let mut a_buf = [0u8; 8];
        a_buf[..a.len()].copy_from_slice(a.as_bytes());
        let mut b_buf = [0u8; 8];
        b_buf[..b.len()].copy_from_slice(b.as_bytes());
        self.account.set_inner(TwoDynArgsAccountInner {
            tag,
            a_len: a.len() as u8,
            a: u64::from_le_bytes(a_buf),
            b_len: b.len() as u8,
            b: u64::from_le_bytes(b_buf),
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DynBytesCheck {
    pub account: Account<DynBytesAccount>,
}
impl DynBytesCheck {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let data = self.account.data();
        if data.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DynStrCheck {
    pub account: Account<DynStrAccount>,
}
impl DynStrCheck {
    #[inline(always)]
    pub fn handler(&self, expected_len: u8) -> Result<(), ProgramError> {
        let label = self.account.label();
        if label.len() != expected_len as usize {
            return Err(ProgramError::Custom(1));
        }
        Ok(())
    }
}

// --- lifecycle ---

#[derive(Accounts)]
pub struct CreateAccountCpi {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub new_account: Signer,
    pub system_program: Program<SystemProgram>,
}
impl CreateAccountCpi {
    #[inline(always)]
    pub fn handler(&self, lamports: u64, space: u64, owner: Address) -> Result<(), ProgramError> {
        self.system_program
            .create_account(&self.payer, &self.new_account, lamports, space, &owner)
            .invoke()
    }
}

#[derive(Accounts)]
pub struct CloseAccount {
    #[account(mut)]
    pub authority: Signer,
    #[account(mut, has_one(authority), close(dest = authority), address = SimpleAccount::seeds(authority.address()))]
    pub account: Account<SimpleAccount>,
}
impl CloseAccount {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ReallocCheck {
    #[account(mut)]
    pub account: Account<SimpleAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}
impl ReallocCheck {
    #[inline(always)]
    pub fn handler(&mut self, new_space: u64) -> Result<(), ProgramError> {
        self.account
            .realloc(new_space as usize, self.payer.to_account_view(), None)
    }
}

#[derive(Accounts)]
pub struct AssignTest {
    #[account(mut)]
    pub account: Signer,
    pub system_program: Program<SystemProgram>,
}
impl AssignTest {
    #[inline(always)]
    pub fn handler(&self, owner: Address) -> Result<(), ProgramError> {
        self.system_program.assign(&self.account, &owner).invoke()
    }
}

#[derive(Accounts)]
pub struct SpaceOverride {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, address = SpaceTestAccount::seeds(payer.address()))]
    pub account: Account<SpaceTestAccount>,
    pub system_program: Program<SystemProgram>,
}
impl SpaceOverride {
    #[inline(always)]
    pub fn handler(&mut self, value: u64, bumps: &SpaceOverrideBumps) -> Result<(), ProgramError> {
        self.account.set_inner(SpaceTestAccountInner {
            authority: *self.payer.address(),
            value,
            bump: bumps.account,
        });
        Ok(())
    }
}

// --- token ---

#[derive(Accounts)]
pub struct InitMint {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, mint(decimals = 6, authority = mint_authority, freeze_authority = None, token_program = token_program))]
    pub mint: Account<Mint>,
    pub mint_authority: Signer,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}
impl InitMint {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitToken {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, token(mint = mint, authority = payer, token_program = token_program))]
    pub token_account: Account<Token>,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
}
impl InitToken {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitAta {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, associated_token(authority = wallet, mint = mint, token_program = token_program))]
    pub ata: Account<Token>,
    pub wallet: Signer,
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
    pub system_program: Program<SystemProgram>,
    pub ata_program: Program<AssociatedTokenProgram>,
}
impl InitAta {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferChecked {
    pub authority: Signer,
    #[account(mut)]
    pub from: Account<Token>,
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<Token>,
    pub token_program: Program<TokenProgram>,
}
impl TransferChecked {
    #[inline(always)]
    pub fn handler(&self, amount: u64, decimals: u8) -> Result<(), ProgramError> {
        self.token_program
            .transfer_checked(
                &self.from,
                &self.mint,
                &self.to,
                &self.authority,
                amount,
                decimals,
            )
            .invoke()
    }
}

#[derive(Accounts)]
pub struct MintTo {
    pub authority: Signer,
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(mut)]
    pub to: Account<Token>,
    pub token_program: Program<TokenProgram>,
}
impl MintTo {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .mint_to(&self.mint, &self.to, &self.authority, amount)
            .invoke()
    }
}

#[derive(Accounts)]
pub struct Burn {
    pub authority: Signer,
    #[account(mut)]
    pub from: Account<Token>,
    #[account(mut)]
    pub mint: Account<Mint>,
    pub token_program: Program<TokenProgram>,
}
impl Burn {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .burn(&self.from, &self.mint, &self.authority, amount)
            .invoke()
    }
}

#[derive(Accounts)]
pub struct Approve {
    pub authority: Signer,
    #[account(mut)]
    pub source: Account<Token>,
    pub delegate: UncheckedAccount,
    pub token_program: Program<TokenProgram>,
}
impl Approve {
    #[inline(always)]
    pub fn handler(&self, amount: u64) -> Result<(), ProgramError> {
        self.token_program
            .approve(&self.source, &self.delegate, &self.authority, amount)
            .invoke()
    }
}

#[derive(Accounts)]
pub struct Revoke {
    pub authority: Signer,
    #[account(mut)]
    pub source: Account<Token>,
    pub token_program: Program<TokenProgram>,
}
impl Revoke {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .revoke(&self.source, &self.authority)
            .invoke()
    }
}

#[derive(Accounts)]
pub struct CloseTokenAccount {
    #[account(mut)]
    pub account: Account<Token>,
    #[account(mut)]
    pub destination: Signer,
    /// CHECK: duplicate signer used when authority and destination alias.
    #[account(dup)]
    pub authority: Signer,
    pub token_program: Program<TokenProgram>,
}
impl CloseTokenAccount {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        self.token_program
            .close_account(&self.account, &self.destination, &self.authority)
            .invoke()
    }
}

#[derive(Accounts)]
pub struct SweepAndClose {
    pub authority: Signer,
    #[account(
        mut,
        token(mint = mint, authority = authority, token_program = token_program),
        token_sweep(receiver = receiver, mint = mint, authority = authority, token_program = token_program),
        token_close(dest = destination, authority = authority, token_program = token_program)
    )]
    pub source: Account<Token>,
    #[account(mut)]
    pub receiver: Account<Token>,
    pub mint: Account<Mint>,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub token_program: Program<TokenProgram>,
}
impl SweepAndClose {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitMintT22 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, mint(decimals = 6, authority = mint_authority, freeze_authority = None, token_program = token_program))]
    pub mint: Account<Mint2022>,
    pub mint_authority: Signer,
    pub token_program: Program<Token2022Program>,
    pub system_program: Program<SystemProgram>,
}
impl InitMintT22 {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TransferCheckedInterface {
    pub authority: Signer,
    #[account(mut)]
    pub from: InterfaceAccount<Token>,
    pub mint: InterfaceAccount<Mint>,
    #[account(mut)]
    pub to: InterfaceAccount<Token>,
    pub token_program: Interface<TokenInterface>,
}
impl TransferCheckedInterface {
    #[inline(always)]
    pub fn handler(&self, amount: u64, decimals: u8) -> Result<(), ProgramError> {
        self.token_program
            .transfer_checked(
                &self.from,
                &self.mint,
                &self.to,
                &self.authority,
                amount,
                decimals,
            )
            .invoke()
    }
}

// --- validation ---

#[derive(Accounts)]
pub struct HeaderNoDupSigner {
    pub account: Signer,
}
impl HeaderNoDupSigner {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct HeaderDupSigner {
    #[account(mut)]
    pub payer: Signer,
    /// CHECK: benchmark aliases payer and authority to the same signer.
    #[account(dup)]
    pub authority: Signer,
}
impl HeaderDupSigner {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TwoAccountsCheck {
    pub first: Account<ErrorTestAccount>,
    pub second: Account<ErrorTestAccount>,
}
impl TwoAccountsCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct HasOneDefault {
    pub authority: Signer,
    #[account(has_one(authority))]
    pub account: Account<ErrorTestAccount>,
}
impl HasOneDefault {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct OwnerCheck {
    pub account: Account<SimpleAccount>,
}
impl OwnerCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProgramCheck {
    pub program: Program<SystemProgram>,
}
impl ProgramCheck {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct RequireEqCheck {
    pub signer: Signer,
}
impl RequireEqCheck {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64) -> Result<(), ProgramError> {
        require_eq!(a, b, BenchError::RequireEqFailed);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct RemainingTypedCheck {
    pub authority: Signer,
}
impl RemainingTypedCheck {
    #[inline(always)]
    pub fn handler(&self, remaining: RemainingAccounts<'_>) -> Result<(), ProgramError> {
        let parsed = remaining.parse::<Account<ErrorTestAccount>, 4>()?;
        if core::hint::black_box(parsed.iter().count()) > 4 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}

// --- events ---

#[derive(Accounts)]
pub struct EmitEmptyEvent {
    pub signer: Signer,
}
impl EmitEmptyEvent {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        emit!(EmptyEvent {});
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitU64Event {
    pub signer: Signer,
}
impl EmitU64Event {
    #[inline(always)]
    pub fn handler(&self, value: u64) -> Result<(), ProgramError> {
        emit!(SimpleEvent { value });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitMultiField {
    pub signer: Signer,
}
impl EmitMultiField {
    #[inline(always)]
    pub fn handler(&self, a: u64, b: u64, c: Address) -> Result<(), ProgramError> {
        emit!(MultiEvent { a, b, c });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitLargeEvent {
    pub signer: Signer,
}
impl EmitLargeEvent {
    #[inline(always)]
    pub fn handler(
        &self,
        a: u64,
        b: u64,
        c: u64,
        d: u64,
        e: Address,
        f: Address,
        g: u128,
        h: u128,
    ) -> Result<(), ProgramError> {
        emit!(LargeEvent {
            a,
            b,
            c,
            d,
            e,
            f,
            g,
            h
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitTwoEvents {
    pub signer: Signer,
}
impl EmitTwoEvents {
    #[inline(always)]
    pub fn handler(&self, first: u64, second: u64) -> Result<(), ProgramError> {
        emit!(SimpleEvent { value: first });
        emit!(SecondSimpleEvent { value: second });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitViaCpi {
    pub signer: Signer,
    pub event_authority: EventAuthority,
    pub program: Program<SelfProgram>,
}
impl EmitViaCpi {
    #[inline(always)]
    pub fn handler(&self, value: u64) -> Result<(), ProgramError> {
        emit_cpi!(SimpleEvent { value })?;
        Ok(())
    }
}

// --- heap ---

#[derive(Accounts)]
pub struct HeapVecOk {
    pub signer: Signer,
}
impl HeapVecOk {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        use alloc::vec;
        #[allow(clippy::useless_vec)]
        let v = vec![1u8; 64];
        if core::hint::black_box(v.len()) != 64 {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}

// --- sysvar ---

#[derive(Accounts)]
pub struct ReadClock {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, address = ClockSnapshot::seeds())]
    pub snapshot: Account<ClockSnapshot>,
    pub system_program: Program<SystemProgram>,
}
impl ReadClock {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = Clock::get()?;
        self.snapshot.set_inner(ClockSnapshotInner {
            slot: clock.slot.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ReadClockFromAccount {
    pub _payer: Signer,
    #[account(mut)]
    pub snapshot: Account<ClockSnapshot>,
    pub clock: Sysvar<Clock>,
}
impl ReadClockFromAccount {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let clock = &self.clock;
        self.snapshot.set_inner(ClockSnapshotInner {
            slot: clock.slot.get(),
            unix_timestamp: clock.unix_timestamp.get(),
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ReadRent {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, address = RentSnapshot::seeds())]
    pub snapshot: Account<RentSnapshot>,
    pub system_program: Program<SystemProgram>,
}
impl ReadRent {
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(100);
        self.snapshot.set_inner(RentSnapshotInner {
            min_balance_100: min_balance,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ReadRentCalc {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, init, address = RentCalcSnapshot::seeds())]
    pub snapshot: Account<RentCalcSnapshot>,
    pub system_program: Program<SystemProgram>,
}
impl ReadRentCalc {
    #[inline(always)]
    pub fn handler(&mut self, data_len: u64) -> Result<(), ProgramError> {
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance_unchecked(data_len as usize);
        self.snapshot
            .set_inner(RentCalcSnapshotInner { min_balance });
        Ok(())
    }
}

// --- CPI ---

#[derive(Accounts)]
pub struct ReturnU64 {
    pub program: Program<SelfProgram>,
}
impl ReturnU64 {
    #[inline(always)]
    pub fn handler(&self) -> Result<u64, ProgramError> {
        Ok(RETURN_U64_VALUE)
    }
}

#[derive(Accounts)]
pub struct CpiInvokeWithReturn {
    pub program: Program<SelfProgram>,
}
impl CpiInvokeWithReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        let ret = CpiCall::<1, 1>::new(
            &crate::ID,
            [InstructionAccount::readonly(self.program.address())],
            [self.program.to_account_view()],
            [105],
        )
        .invoke_with_return()?;
        let expected = <u64 as InstructionArg>::to_zc(&RETURN_U64_VALUE);
        // SAFETY: reading the zero-copy repr of a stack `u64` as bytes.
        let expected_bytes = unsafe {
            core::slice::from_raw_parts(
                &expected as *const <u64 as InstructionArg>::Zc as *const u8,
                core::mem::size_of::<<u64 as InstructionArg>::Zc>(),
            )
        };
        if ret.as_slice() != expected_bytes {
            return Err(ProgramError::InvalidInstructionData);
        }
        if ret.decode::<u64>()? != RETURN_U64_VALUE {
            return Err(ProgramError::InvalidInstructionData);
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CpiInvokeIgnoreReturn {
    pub program: Program<SelfProgram>,
}
impl CpiInvokeIgnoreReturn {
    #[inline(always)]
    pub fn handler(&self) -> Result<(), ProgramError> {
        CpiCall::<1, 1>::new(
            &crate::ID,
            [InstructionAccount::readonly(self.program.address())],
            [self.program.to_account_view()],
            [105],
        )
        .invoke()
    }
}

#[derive(Accounts)]
pub struct CpiMutReadback {
    #[account(mut)]
    pub account: Account<SimpleAccount>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}
impl CpiMutReadback {
    #[inline(always)]
    pub fn handler(&mut self, new_value: u64) -> Result<(), ProgramError> {
        let authority = self.account.authority;
        let bump = self.account.bump;
        let initial_lamports = self.account.to_account_view().lamports();
        self.account.set_inner(SimpleAccountInner {
            authority,
            value: new_value,
            bump,
        });
        if self.account.value != new_value {
            return Err(ProgramError::Custom(1));
        }
        self.system_program
            .transfer(&self.payer, &self.account, 1_000u64)
            .invoke()?;
        if self.account.to_account_view().lamports() != initial_lamports + 1_000 {
            return Err(ProgramError::Custom(2));
        }
        if self.account.value != new_value {
            return Err(ProgramError::Custom(3));
        }
        let second_value = new_value.wrapping_add(1);
        self.account.set_inner(SimpleAccountInner {
            authority,
            value: second_value,
            bump,
        });
        if self.account.value != second_value {
            return Err(ProgramError::Custom(4));
        }
        Ok(())
    }
}
