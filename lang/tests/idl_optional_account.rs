#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[account(discriminator = 1)]
pub struct OptionalState {
    pub value: u64,
}

#[derive(Accounts)]
pub struct UseOptional {
    pub required: Account<OptionalState>,
    pub maybe_state: Option<Account<OptionalState>>,
}

#[program(no_entrypoint)]
pub mod idl_optional_account_program {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn use_optional(ctx: Ctx<UseOptional>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.required;
        let _ = &ctx.accounts.maybe_state;
        Ok(())
    }
}

#[cfg(feature = "idl-build")]
#[test]
fn optional_accounts_emit_optional_flag() {
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

    let required = accounts
        .iter()
        .find(|account| account["name"] == "required")
        .expect("required account should be emitted");
    assert_eq!(required.get("optional"), None);

    let maybe_state = accounts
        .iter()
        .find(|account| account["name"] == "maybeState")
        .expect("optional account should be emitted");
    assert_eq!(
        maybe_state
            .get("optional")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
}
