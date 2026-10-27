//! Account seed declarations must survive without an instruction seed constraint.
use quasar_idl::{
    codegen::{model::ProgramModel, rust},
    types::IdlResolver,
};
use quasar_lang::prelude::*;

declare_id!("FaNXqQYPePivF3hVJ3ZdM74zzMa9P4QU7HX7t6RrDygg");

#[account(discriminator = 1)]
#[seeds(b"position", market: Address, authority: Address, nonce: u64, tag: [u8; 4])]
pub struct YieldPosition {
    pub market: Address,
    pub authority: Address,
}

#[account(discriminator = 2)]
#[seeds(b"config\"\\")]
pub struct Config {
    pub value: u8,
}

#[account(discriminator = 3)]
#[seeds()]
pub struct EmptySeeds {
    pub value: u8,
}

const LEN: usize = 4;

#[account(discriminator = 4)]
#[seeds(b"const-length", tag: [u8; LEN])]
pub struct ConstLength {
    pub value: u8,
}

#[account(discriminator = 5)]
#[seeds(b"qualified", who: quasar_lang::Address)]
pub struct Qualified {
    pub value: u8,
}

#[derive(Accounts)]
pub struct Update {
    pub market: UncheckedAccount,
    pub authority: Signer,
    #[account(mut, has_one(market), has_one(authority))]
    pub yield_position: Account<YieldPosition>,
    pub config: Account<Config>,
    pub empty: Account<EmptySeeds>,
    pub const_length: Account<ConstLength>,
    pub qualified: Account<Qualified>,
}

#[test]
fn unsupported_seed_types_omit_only_their_accounts_helpers() {
    let idl = quasar_idl::read(&idl().to_json()).unwrap();
    for name in ["ConstLength", "Qualified"] {
        assert!(idl.accounts.iter().find(|account| account.name == name).unwrap().seeds.is_none());
        assert!(idl.types.iter().any(|ty| ty.name == name));
    }
    let files = rust::generate_client(&idl);
    let source = &files.iter().find(|(path, _)| path == "pda.rs").unwrap().1;
    assert!(!source.contains("find_const_length_address"));
    assert!(!source.contains("find_qualified_address"));
    assert!(source.contains("find_yield_position_address"));
    assert!(source.contains("find_config_address"));
    for path in ["state/const_length.rs", "state/qualified.rs", "instructions/update.rs"] {
        assert!(files.iter().any(|(name, _)| name == path));
    }
}

#[program]
mod positions {
    use super::*;

    #[instruction(0)]
    pub fn update(_ctx: Ctx<Update>) -> Result {
        Ok(())
    }
}

#[test]
fn generates_helpers_from_account_declarations_with_only_has_one_constraints() {
    let idl = quasar_idl::read(&idl().to_json()).unwrap();
    assert!(
        idl.instructions[0].accounts.iter().all(|a| matches!(a.resolver, IdlResolver::Input {}))
    );
    assert_eq!(idl.accounts[0].seeds.as_ref().unwrap().len(), 5);
    // Public-schema serialization must retain the declaration too.
    let idl = quasar_idl::read(&serde_json::to_string(&idl).unwrap()).unwrap();
    assert!(rust::has_pdas(&idl));
    assert!(rust::generate_cargo_toml_for_program(&ProgramModel::new(&idl)).contains("curve25519"));
    let files = rust::generate_client(&idl);
    let source = &files.iter().find(|(path, _)| path == "pda.rs").unwrap().1;
    for fragment in [
        "pub fn find_yield_position_address(market: &Address, authority: &Address, nonce: u64, tag: [u8; 4], program_id: &Address)",
        "let nonce_seed = nonce.to_le_bytes();",
        "&[b\"position\", market.as_ref(), authority.as_ref(), nonce_seed.as_ref(), tag.as_ref()]",
        "pub fn find_config_address(program_id: &Address)",
        "&[b\"config\\\"\\\\\"]",
        "pub fn find_empty_seeds_address(program_id: &Address)",
        "Address::find_program_address(&[], program_id)",
    ] {
        assert!(source.contains(fragment), "missing {fragment}:\n{source}");
    }
    assert!(files.iter().find(|(path, _)| path == "lib.rs").unwrap().1.contains("pub mod pda;"));
}

#[test]
fn preserves_existing_instruction_helper_without_duplicate_names() {
    let mut idl = quasar_idl::read(&idl().to_json()).unwrap();
    idl.instructions[0].accounts[2].resolver = IdlResolver::Pda {
        program: quasar_idl::types::IdlPdaProgram::ProgramId {},
        seeds: vec![quasar_idl::types::IdlPdaSeed::Account { path: "authority".into() }],
        bump: None,
    };
    let files = rust::generate_client(&idl);
    let source = &files.iter().find(|(path, _)| path == "pda.rs").unwrap().1;
    assert_eq!(source.matches("pub fn find_yield_position_address(").count(), 1);
    assert!(
        source.contains("find_yield_position_address(authority: &Address, program_id: &Address)")
    );
    assert!(source.contains("pub fn find_yield_position_2_address(market: &Address, authority: &Address, nonce: u64, tag: [u8; 4], program_id: &Address)"));
    assert!(source.contains(
        "&[b\"position\", market.as_ref(), authority.as_ref(), nonce_seed.as_ref(), tag.as_ref()]"
    ));
}

#[test]
fn identical_seed_declarations_reuse_the_existing_helper() {
    let mut idl = quasar_idl::read(&idl().to_json()).unwrap();
    idl.instructions[0].accounts[2].resolver = IdlResolver::Pda {
        program: quasar_idl::types::IdlPdaProgram::ProgramId {},
        seeds: idl.accounts[0].seeds.clone().unwrap(),
        bump: None,
    };
    let files = rust::generate_client(&idl);
    let source = &files.iter().find(|(path, _)| path == "pda.rs").unwrap().1;
    assert_eq!(source.matches("pub fn find_yield_position_address(").count(), 1);
    assert!(!source.contains("find_yield_position_2_address"));
}

#[test]
fn conflicting_helper_names_skip_occupied_suffixes() {
    let mut idl = quasar_idl::read(&idl().to_json()).unwrap();
    for (index, name) in ["config", "config_2"].into_iter().enumerate() {
        idl.instructions[0].accounts[index].name = name.into();
        idl.instructions[0].accounts[index].resolver = IdlResolver::Pda {
            program: quasar_idl::types::IdlPdaProgram::ProgramId {},
            seeds: vec![quasar_idl::types::IdlPdaSeed::Const { value: name.as_bytes().to_vec() }],
            bump: None,
        };
    }
    let files = rust::generate_client(&idl);
    let source = &files.iter().find(|(path, _)| path == "pda.rs").unwrap().1;
    for name in ["config", "config_2", "config_3"] {
        assert_eq!(source.matches(&format!("pub fn find_{name}_address(")).count(), 1);
    }
    assert!(source.contains("&[b\"config\\\"\\\\\"]"));
}
