#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
#[seeds(b"iface-snapshot", namespace: u32)]
pub struct InterfaceSnapshot {
    pub namespace: u32,
    pub bump: u8,
}

#[repr(C)]
pub struct ExternalConfigData {
    pub namespace: u32,
    pub bump: u8,
}

pub struct ExternalConfig;

impl Owners for ExternalConfig {
    fn owners() -> &'static [Address] {
        static OWNERS: [Address; 1] = [ID];
        &OWNERS
    }
}

impl AccountCheck for ExternalConfig {
    type Params = ();

    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}

impl ZeroCopyDeref for ExternalConfig {
    type Target = ExternalConfigData;

    unsafe fn deref_from(view: &AccountView) -> &Self::Target {
        &*(view.data_ptr() as *const Self::Target)
    }

    unsafe fn deref_from_mut(view: &mut AccountView) -> &mut Self::Target {
        &mut *(view.data_ptr() as *mut Self::Target)
    }
}

#[derive(Accounts)]
pub struct Good {
    #[account(mut)]
    pub payer: Signer,
    pub config: InterfaceAccount<ExternalConfig>,
    #[account(
        mut,
        init,
        payer = payer,
        seeds = InterfaceSnapshot::seeds(config.namespace),
        bump
    )]
    pub snapshot: Account<InterfaceSnapshot>,
    pub system_program: Program<System>,
}

fn main() {}
