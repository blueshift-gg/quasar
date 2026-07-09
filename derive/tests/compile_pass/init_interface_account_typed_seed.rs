#![allow(unexpected_cfgs)]
extern crate alloc;
use quasar_derive::Accounts;
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
// External account wrapper with manual AccountLoad.
// Real users would typically use define_account! with a ZeroPod schema.
quasar_lang::define_account!(pub struct ExternalConfig => []);
impl quasar_lang::account_load::AccountLoad for ExternalConfig {
    fn check(_view: &AccountView) -> Result<(), ProgramError> {
        Ok(())
    }
}
impl Owners for ExternalConfig {
    fn check_owner(view: &AccountView) -> Result<(), ProgramError> {
        if quasar_lang::keys_eq(view.owner(), &ID) {
            Ok(())
        } else {
            Err(ProgramError::IllegalOwner)
        }
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
#[instruction(namespace: u32)]
pub struct Good {
    #[account(mut)]
    pub payer: Signer,
    pub config: InterfaceAccount<ExternalConfig>,
    #[account(
        mut,
        init,
        address = InterfaceSnapshot::seeds(namespace)
    )]
    pub snapshot: Account<InterfaceSnapshot>,
    pub system_program: Program<SystemProgram>,
}
fn main() {}
