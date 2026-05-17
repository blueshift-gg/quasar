#![allow(unexpected_cfgs)]

use quasar_lang::prelude::*;

solana_address::declare_id!("11111111111111111111111111111112");

#[derive(Accounts)]
pub struct Initialize {
    pub signer: Signer,
}

#[program(no_entrypoint)]
pub mod auto_instruction_discriminator_program {
    use super::*;

    #[instruction]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(discriminator = 7)]
    pub fn pinned(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }

    #[instruction(raw)]
    pub fn raw(ctx: Context) -> Result<(), ProgramError> {
        let _ = ctx.data;
        Ok(())
    }

    #[instruction]
    pub fn close(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        let _ = &ctx.accounts.signer;
        Ok(())
    }
}

#[cfg(feature = "idl-build")]
#[test]
fn auto_instruction_discriminators_are_compact_and_marked() {
    use quasar_lang::idl_build::__reexport::{serde_json, Idl};

    let json = crate::__quasar_build_idl();
    let idl: Idl = serde_json::from_str(&json).expect("generated IDL should deserialize");
    let discriminator_sources = idl
        .metadata
        .extra
        .get("quasar:instructionDiscriminatorSource")
        .and_then(serde_json::Value::as_object)
        .expect("auto discriminator source metadata");

    let initialize = idl
        .instructions
        .iter()
        .find(|instruction| instruction.name == "initialize")
        .expect("initialize instruction");
    assert_eq!(initialize.discriminator, vec![0]);
    assert_eq!(
        discriminator_sources
            .get("initialize")
            .and_then(serde_json::Value::as_str),
        Some("auto")
    );

    let pinned = idl
        .instructions
        .iter()
        .find(|instruction| instruction.name == "pinned")
        .expect("pinned instruction");
    assert_eq!(pinned.discriminator, vec![7]);
    assert!(!discriminator_sources.contains_key("pinned"));

    let raw = idl
        .instructions
        .iter()
        .find(|instruction| instruction.name == "raw")
        .expect("raw instruction");
    assert_eq!(raw.discriminator, vec![1]);
    assert_eq!(
        discriminator_sources
            .get("raw")
            .and_then(serde_json::Value::as_str),
        Some("auto")
    );

    let close = idl
        .instructions
        .iter()
        .find(|instruction| instruction.name == "close")
        .expect("close instruction");
    assert_eq!(close.discriminator, vec![2]);
    assert_eq!(
        discriminator_sources
            .get("close")
            .and_then(serde_json::Value::as_str),
        Some("auto")
    );
}
