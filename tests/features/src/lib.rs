//! One program that uses every wrapper, constraint and effect, so each can be
//! tested on the SBF build.
#![no_std]

use callee::{Payload, Purse, Tally, program::Callee};
use quasar_lang::{cpi::CpiAccount, effects::set_return_data, prelude::*};

declare_id!("Ac2HBsUjsR3CXTHaLswEAUPYEJmb8A3a5sv3cR6Bvqje");

#[account(discriminator = 1)]
#[seeds(b"counter", authority: Address)]
pub struct Counter {
    pub count: u64,
    pub authority: Address,
}

#[account(discriminator = 2)]
#[seeds(b"vault", depositor: Address)]
pub struct Vault {
    pub depositor: Address,
    pub bump: u8,
    pub amount: u64,
}

#[derive(Seeds)]
#[seeds(b"config")]
pub struct Config;

/// The first layout of a vault, before `VaultV2` added a lock.
#[account(discriminator = 5)]
pub struct VaultV1 {
    pub depositor: Address,
    pub amount: u64,
}

#[account(discriminator = 6)]
pub struct VaultV2 {
    pub depositor: Address,
    pub amount: u64,
    pub locked_until: i64,
}

impl From<VaultV1> for VaultV2 {
    fn from(old: VaultV1) -> Self {
        Self { depositor: old.depositor, amount: old.amount, locked_until: 0 }
    }
}

#[account(discriminator = 4)]
pub struct Profile {
    #[max_len(16)]
    pub name: String,
    #[max_len(4)]
    pub members: Vec<Address>,
}

/// A counter at an address its creator picks with a seed.
#[account(discriminator = 3)]
#[seeds(b"seeded", creator: Address, seed: u64)]
pub struct Seeded {
    pub seed: u64,
}

/// The PDA that signs calls to the callee.
#[derive(Seeds)]
#[seeds(b"signer")]
pub struct CallSigner;

#[event(discriminator = 9)]
pub struct Paid {
    pub amount: u64,
    pub to: Address,
}

#[error_code]
pub enum FeatureError {
    #[msg("the vault is empty")]
    Empty,
    #[msg("the vault belongs to someone else")]
    NotDepositor,
}

/// An address `Pinned` requires, for `address =` without seeds.
pub const PINNED: Address = Address::new_from_array([5; 32]);

#[derive(Accounts)]
pub struct Greet {
    pub user: Signer,
}

#[derive(Accounts)]
pub struct Increment {
    #[account(mut)]
    pub counter: Account<Counter>,
}

#[derive(Accounts)]
pub struct Probe {
    pub accounts: Remaining<UncheckedAccount>,
}

#[derive(Accounts)]
pub struct TouchBoth {
    #[account(mut)]
    pub first: Account<Counter>,
    pub between: UncheckedAccount,
    #[account(mut)]
    pub last: Account<Counter>,
}

#[derive(Accounts)]
pub struct Maybe {
    pub first: Option<Signer>,
    pub second: Option<Signer>,
    /// Loaded after the entries that mark accounts absent.
    pub last: Signer,
}

#[derive(Accounts)]
pub struct Owned {
    pub depositor: Signer,
    #[account(
        has_one(depositor @ FeatureError::NotDepositor),
        seeds(depositor, bump = vault.bump())
    )]
    pub vault: Pda<Account<Vault>>,
}

/// `vault`'s check names `depositor`, declared after it, so it runs once
/// `depositor` has loaded.
#[derive(Accounts)]
pub struct OwnedLater {
    #[account(seeds(depositor, bump = vault.bump()))]
    pub vault: Pda<Account<Vault>>,
    pub depositor: Signer,
}

#[derive(Accounts)]
pub struct Pinned {
    #[account(address = PINNED)]
    pub account: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Wallet {
    pub wallet: SystemAccount,
}

#[derive(Accounts)]
pub struct Signers {
    #[account(mut)]
    pub signers: Remaining<Signer>,
}

#[derive(Accounts)]
#[data(Seeding)]
pub struct CreateSeeded {
    #[account(mut)]
    pub creator: Signer,
    #[account(init(payer = creator), seeds(creator, data.seed))]
    pub seeded: Pda<Account<Seeded>>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct CreateProfile {
    #[account(mut)]
    pub owner: Signer,
    #[account(init(payer = owner))]
    pub profile: Account<Profile>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Drain {
    #[account(mut)]
    pub from: UncheckedAccount,
    #[account(mut)]
    pub to: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ReadV2 {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, migrate(from = VaultV1, payer = payer))]
    pub vault: Account<VaultV2>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct ReadSysvars {
    pub clock: Sysvar<Clock>,
    pub rent: Sysvar<Rent>,
}

#[derive(Accounts)]
pub struct CreateNote {
    #[account(mut)]
    pub payer: Signer,
    #[account(init(payer = payer, idempotent))]
    pub note: Account<Counter>,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct Funded {
    #[account(check(vault.amount() > 0 @ FeatureError::Empty), check(vault.amount() < 1_000))]
    pub vault: Account<Vault>,
}

#[derive(Accounts)]
pub struct Global {
    #[account(seeds())]
    pub config: Pda<Account<Counter>, Config>,
}

#[derive(Accounts)]
pub struct Ordered {
    pub first: Signer,
    pub second: Account<Counter>,
}

#[derive(Accounts)]
pub struct Pay {
    #[account(mut)]
    pub vault: Account<Vault>,
    #[account(mut)]
    pub to: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CloseVault {
    #[account(mut, close(to = to))]
    pub vault: Account<Vault>,
    #[account(mut)]
    pub to: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Resize {
    #[account(mut)]
    pub data: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CallRecord {
    pub callee: Program<Callee>,
    #[account(mut)]
    pub record: UncheckedAccount,
    pub user: Signer,
    #[account(seeds())]
    pub pda: Pda<UncheckedAccount, CallSigner>,
}

#[derive(Accounts)]
pub struct CallBump {
    pub callee: Program<Callee>,
    #[account(mut)]
    pub tally: Account<Tally>,
}

#[derive(Accounts)]
pub struct CallKeepingOwn {
    pub callee: Program<Callee>,
    #[account(mut)]
    pub counter: Account<Counter>,
}

#[derive(Accounts)]
pub struct CallSelf {
    pub features: Program<program::Features>,
    #[account(mut)]
    pub vault: Account<Vault>,
    #[account(mut)]
    pub to: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CreateCounter {
    #[account(mut)]
    pub payer: Signer,
    #[account(init(payer = payer), seeds(payer))]
    pub counter: Pda<Account<Counter>>,
    pub system_program: Program<SystemProgram>,
}

/// The callee checks the purse and its holder as it loads them, so this
/// program's same checks are left to it (`cpi::Vouches`).
#[derive(Accounts)]
pub struct CallTouch {
    pub callee: Program<Callee>,
    pub holder: Signer,
    #[account(mut, has_one(holder))]
    pub purse: Account<Purse>,
}

#[derive(Accounts)]
pub struct Fund {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub to: UncheckedAccount,
    pub system_program: Program<SystemProgram>,
}

#[derive(Accounts)]
pub struct WithdrawOwned {
    #[account(mut)]
    pub depositor: Signer,
    #[account(mut, has_one(depositor), seeds(depositor, bump = vault.bump()))]
    pub vault: Pda<Account<Vault>>,
}

/// Only the depositor can take lamports out of their vault.
#[property(features::withdraw_owned)]
fn only_the_depositor_withdraws(
    before: &WithdrawOwned,
    after: &WithdrawOwned,
    _data: &Amount,
) -> bool {
    after.vault.lamports() >= before.vault.lamports()
        || before.vault.depositor() == before.depositor.address()
}

/// An amount of lamports, or of a vault's balance.
#[derive(InstructionData)]
pub struct Amount {
    pub amount: u64,
}

/// What `increment` adds.
#[derive(InstructionData)]
pub struct By {
    pub by: u64,
}

/// The first value `probe` reports.
#[derive(InstructionData)]
pub struct Tag {
    pub tag: u64,
}

#[derive(InstructionData)]
pub struct Len {
    pub len: u64,
}

/// The seed a `Seeded` account's address takes.
#[derive(InstructionData)]
pub struct Seeding {
    pub seed: u64,
}
#[program]
mod features {
    use super::*;

    #[instruction(0)]
    pub fn greet(_ctx: Ctx<Greet>) -> Result<()> {
        Ok(())
    }

    #[instruction(1)]
    pub fn increment(mut ctx: Ctx<Increment, By>) -> Result<()> {
        let count = ctx.accounts.counter.count();
        let count = count.checked_add(ctx.data.by).ok_or(ProgramError::ArithmeticOverflow)?;
        ctx.accounts.counter.set_count(count)?;
        Ok(())
    }

    /// Reports, through return data, `tag` and then what the input held for
    /// each remaining account: key, owner, lamports, data length, the three
    /// flags, and the first 16 bytes of data.
    #[instruction(2)]
    pub fn probe(ctx: Ctx<Probe, Tag>) -> Result<()> {
        let mut report = [0u8; 1024];
        let mut end = 0;
        let mut put = |bytes: &[u8]| {
            report[end..end + bytes.len()].copy_from_slice(bytes);
            end += bytes.len();
        };
        put(&ctx.data.tag.to_le_bytes());
        for account in ctx.accounts.accounts {
            let account = account?;
            let (data, entry) = (account.data(), account.entry());
            put(account.address().as_array());
            put(account.owner().as_array());
            put(&account.lamports().to_le_bytes());
            put(&(data.len() as u64).to_le_bytes());
            put(&[entry.is_signer(), entry.is_writable(), entry.executable()].map(u8::from));
            put(&data[..data.len().min(16)]);
        }
        set_return_data(&report[..end]);
        Ok(())
    }

    #[instruction(3)]
    pub fn touch_both(mut ctx: Ctx<TouchBoth>) -> Result<()> {
        for counter in [&mut ctx.accounts.first, &mut ctx.accounts.last] {
            let count = counter.count() + 1;
            counter.set_count(count)?;
        }
        Ok(())
    }

    /// Reports, through return data, which accounts are present.
    #[instruction(4)]
    pub fn maybe(ctx: Ctx<Maybe>) -> Result<()> {
        let Maybe { first, second, .. } = &ctx.accounts;
        set_return_data(&[first.is_some(), second.is_some()].map(u8::from));
        Ok(())
    }

    #[instruction(5)]
    pub fn owned(_ctx: Ctx<Owned>) -> Result<()> {
        Ok(())
    }

    #[instruction(6)]
    pub fn funded(_ctx: Ctx<Funded>) -> Result<()> {
        Ok(())
    }

    #[instruction(7)]
    pub fn global(_ctx: Ctx<Global>) -> Result<()> {
        Ok(())
    }

    #[instruction(8)]
    pub fn ordered(_ctx: Ctx<Ordered>) -> Result<()> {
        Ok(())
    }

    #[instruction(9)]
    pub fn pay(mut ctx: Ctx<Pay, Amount>) -> Result<()> {
        let amount = ctx.data.amount;
        ctx.accounts.vault.sub_lamports(amount)?;
        ctx.accounts.to.add_lamports(amount)?;
        emit!(Paid { amount, to: *ctx.accounts.to.address() });
        Ok(())
    }

    #[instruction(10)]
    pub fn close(_ctx: Ctx<CloseVault>) -> Result<()> {
        Ok(())
    }

    #[instruction(11)]
    pub fn resize(mut ctx: Ctx<Resize, Len>) -> Result<()> {
        ctx.accounts.data.resize(ctx.data.len as usize)
    }

    #[instruction(12)]
    pub fn call_record(mut ctx: Ctx<CallRecord, Payload>) -> Result<()> {
        let CallRecord { callee, record, user, pda } = &mut ctx.accounts;
        let signer = CallSigner::seeds().with_bump(pda.bump());
        let accounts = [
            CpiAccount::writable(record),
            CpiAccount::readonly(user).signer(),
            CpiAccount::readonly(pda).signer(),
        ];
        callee::record::cpi(callee, accounts, ctx.data).invoke_signed(&signer)
    }

    /// The callee changes an account it owns; the account is checked again
    /// after the call, and its new value read.
    #[instruction(13)]
    pub fn call_bump(mut ctx: Ctx<CallBump>) -> Result<u64> {
        let CallBump { callee, tally } = &mut ctx.accounts;
        callee::bump::cpi(callee, callee::Bump::cpi_accounts(tally), ()).invoke()?;
        tally.reload()?;
        Ok(tally.count())
    }

    /// A counter this program owns stays usable across the call.
    #[instruction(14)]
    pub fn call_keeping_own(mut ctx: Ctx<CallKeepingOwn>) -> Result<()> {
        let CallKeepingOwn { callee, counter } = &mut ctx.accounts;
        callee::noop::cpi(callee, [CpiAccount::writable_to(callee, counter)], ()).invoke()?;
        let count = counter.count() + 1;
        counter.set_count(count)?;
        Ok(())
    }

    #[instruction(15)]
    pub fn create_counter(mut ctx: Ctx<CreateCounter>) -> Result<()> {
        let authority = *ctx.accounts.payer.address();
        let count = u64::from(ctx.accounts.counter.bump());
        ctx.accounts.counter.set_inner(&Counter { count, authority })?;
        Ok(())
    }

    #[instruction(16)]
    pub fn fund(mut ctx: Ctx<Fund, Amount>) -> Result<()> {
        let Fund { from, to, system_program } = &mut ctx.accounts;
        system_program.transfer(from, to, ctx.data.amount).invoke()
    }

    #[instruction(17)]
    pub fn withdraw_owned(mut ctx: Ctx<WithdrawOwned, Amount>) -> Result<()> {
        let amount = ctx.data.amount;
        let WithdrawOwned { depositor, vault } = &mut ctx.accounts;
        let left = vault.amount().checked_sub(amount).ok_or(ProgramError::InsufficientFunds)?;
        vault.set_amount(left)?;
        vault.sub_lamports(amount)?;
        depositor.add_lamports(amount)
    }

    /// Closes the vault by calling this program's `close`, and reads it no
    /// more.
    #[instruction(18)]
    pub fn call_self(mut ctx: Ctx<CallSelf>) -> Result<()> {
        let CallSelf { features, vault, to } = &mut ctx.accounts;
        close::cpi(features, CloseVault::cpi_accounts(vault, to), ()).invoke()
    }

    #[instruction(19)]
    pub fn pinned(_ctx: Ctx<Pinned>) -> Result<()> {
        Ok(())
    }

    #[instruction(20)]
    pub fn wallet(_ctx: Ctx<Wallet>) -> Result<()> {
        Ok(())
    }

    /// Returns how many signers were passed.
    #[instruction(21)]
    pub fn signers(ctx: Ctx<Signers>) -> Result<u8> {
        let mut count = 0;
        for signer in ctx.accounts.signers {
            signer?;
            count += 1;
        }
        Ok(count)
    }

    #[instruction(22)]
    pub fn create_note(_ctx: Ctx<CreateNote>) -> Result<()> {
        Ok(())
    }

    /// Reports, through return data, the clock as its account holds it, then
    /// whether `get` reads the same clock and rent, then the rent-exempt balance
    /// for 41 bytes.
    #[instruction(23)]
    pub fn sysvars(ctx: Ctx<ReadSysvars>) -> Result<()> {
        let ReadSysvars { clock, rent } = &ctx.accounts;
        let (clock_bytes, rent) = (clock.as_ref().as_slice(), rent.to_owned());
        let same = Clock::get()? == clock.to_owned() && Rent::get()? == rent;
        let mut report = [0; 49];
        report[..40].copy_from_slice(clock_bytes);
        report[40] = u8::from(same);
        report[41..].copy_from_slice(&rent.minimum_balance(41).to_le_bytes());
        set_return_data(&report);
        Ok(())
    }

    #[instruction(24)]
    pub fn create_seeded(mut ctx: Ctx<CreateSeeded, Seeding>) -> Result<()> {
        ctx.accounts.seeded.set_seed(ctx.data.seed)?;
        Ok(())
    }

    /// Names the profile and makes its owner the only member.
    #[instruction(25)]
    pub fn create_profile(mut ctx: Ctx<CreateProfile>) -> Result<()> {
        let owner = *ctx.accounts.owner.address();
        let profile = &mut ctx.accounts.profile;
        profile.set_name("quasar")?;
        profile.set_members(&[owner])?;
        Ok(())
    }

    #[instruction(26)]
    pub fn drain(mut ctx: Ctx<Drain, Amount>) -> Result<()> {
        ctx.accounts.from.sub_lamports(ctx.data.amount)?;
        ctx.accounts.to.add_lamports(ctx.data.amount)
    }

    #[instruction(27)]
    pub fn owned_later(_ctx: Ctx<OwnedLater>) -> Result<()> {
        Ok(())
    }

    /// Returns the vault's amount, whichever layout it held.
    #[instruction(28)]
    pub fn read_v2(ctx: Ctx<ReadV2>) -> Result<u64> {
        Ok(ctx.accounts.vault.amount())
    }

    /// Reads the vault after the call that closed it, without `reload()`.
    #[instruction(30)]
    pub fn read_after_call(mut ctx: Ctx<CallSelf>) -> Result<u64> {
        let CallSelf { features, vault, to } = &mut ctx.accounts;
        close::cpi(features, CloseVault::cpi_accounts(vault, to), ()).invoke()?;
        Ok(vault.amount())
    }

    /// Checks the vault again after the call that closed it.
    #[instruction(31)]
    pub fn reload_after_call(mut ctx: Ctx<CallSelf>) -> Result<u64> {
        let CallSelf { features, vault, to } = &mut ctx.accounts;
        close::cpi(features, CloseVault::cpi_accounts(vault, to), ()).invoke()?;
        vault.reload()?;
        Ok(vault.amount())
    }

    #[instruction(29)]
    pub fn call_touch(mut ctx: Ctx<CallTouch>) -> Result<()> {
        let CallTouch { callee, holder, purse } = &mut ctx.accounts;
        callee::touch::cpi(callee, callee::Touch::cpi_accounts(holder, purse), ()).invoke()
    }
}
