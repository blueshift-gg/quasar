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
    use quasar_lang::idl_build::__reexport::{
        serde_json, Idl, IdlValidationPlan, VALIDATION_EXTENSION_KEY,
    };

    let json = crate::__quasar_build_idl();
    let idl: Idl = serde_json::from_str(&json).expect("generated IDL should deserialize");
    let instruction = idl
        .instructions
        .first()
        .expect("instruction should be emitted");
    let accounts = &instruction.accounts;

    let required = accounts
        .iter()
        .find(|account| account.name == "required")
        .expect("required account should be emitted");
    assert!(!required.optional);

    let maybe_state = accounts
        .iter()
        .find(|account| account.name == "maybeState")
        .expect("optional account should be emitted");
    assert!(maybe_state.optional);

    let extensions = idl
        .extensions
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .expect("IDL extensions");
    let plan: IdlValidationPlan = serde_json::from_value(
        extensions
            .get(VALIDATION_EXTENSION_KEY)
            .expect("compiler validation plan")
            .clone(),
    )
    .expect("validation plan should deserialize");
    let instruction_plan = plan
        .instructions
        .get("use_optional")
        .expect("instruction validation plan");
    let maybe_state = instruction_plan
        .accounts
        .iter()
        .find(|account| account.name == "maybeState")
        .expect("optional account validation");
    assert!(maybe_state.optional);
    assert_eq!(maybe_state.wrapper, "Account");
    assert_eq!(maybe_state.load, "Fixed(validates=[])");
}

#[cfg(feature = "idl-build")]
#[test]
fn generated_validation_extension_is_hash_covered_but_not_abi_covered() {
    use quasar_lang::idl_build::__reexport::{
        compute_abi_hash, compute_idl_hash, serde_json, Idl, VALIDATION_EXTENSION_KEY,
    };

    let json = crate::__quasar_build_idl();
    let mut idl: Idl = serde_json::from_str(&json).expect("generated IDL should deserialize");
    let hashes = idl.hashes.clone().expect("generated hashes");
    assert_eq!(compute_idl_hash(&idl), hashes.idl);
    assert_eq!(compute_abi_hash(&idl), hashes.abi);

    idl.extensions
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
        .expect("IDL extensions")
        .get_mut(VALIDATION_EXTENSION_KEY)
        .and_then(serde_json::Value::as_object_mut)
        .expect("validation plan")
        .insert("version".to_string(), serde_json::json!(999));

    assert_ne!(compute_idl_hash(&idl), hashes.idl);
    assert_eq!(compute_abi_hash(&idl), hashes.abi);
}
