#![allow(unexpected_cfgs)]
//! A keypair-init account (`init` without an `address`, i.e. a non-PDA account
//! created from a freshly generated keypair) must be reported as a signer in
//! the IDL accounts-meta fragment -- matching the generated client, which also
//! requires the caller to sign for it.

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct Vault {
    pub value: u64,
}

#[derive(Accounts)]
pub struct CreateVault {
    #[account(mut, init, payer = payer)]
    pub vault: Account<Vault>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<SystemProgram>,
}

#[program(no_entrypoint)]
pub mod idl_keypair_init_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn create_vault(ctx: Ctx<CreateVault>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.vault;
        Ok(())
    }
}

#[cfg(feature = "idl-build")]
#[test]
fn keypair_init_account_is_signer() {
    use quasar_lang::idl_build::__reexport::serde_json;

    let json = crate::__quasar_build_idl();
    let idl: serde_json::Value =
        serde_json::from_str(&json).expect("generated IDL should deserialize");
    let instruction = idl["instructions"]
        .as_array()
        .expect("instructions should be an array")
        .iter()
        .next()
        .expect("instruction should be emitted");
    let accounts = instruction["accounts"]
        .as_array()
        .expect("instruction accounts should be an array");

    let vault = accounts
        .iter()
        .find(|account| account["name"] == "vault")
        .expect("keypair-init account should be emitted");
    assert_eq!(
        vault.get("signer").and_then(|value| value.as_bool()),
        Some(true),
        "keypair-init account must be a signer in the IDL accounts meta"
    );
}
