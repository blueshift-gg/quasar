use multisig_client::{instructions::*, types::ArrayString32, state::ProgramAccount};
use quasar_lang::{Address, zeropod};
use solana_instruction::Instruction;

#[test]
fn generated_instruction_matches_the_runtime_builder() {
    let signer = Address::new_from_array([7; 32]);
    let config = Address::new_from_array([8; 32]);
    let system = Address::new_from_array([0; 32]);
    let expected = multisig::set_label::instruction(
        (signer, config, system),
        multisig::Label { label: zeropod::ArrayString::try_from("hello").unwrap() },
    );
    let mut storage = [0; 32];
    storage[..5].copy_from_slice(b"hello");
    let actual: Instruction = SetLabelInstruction {
        creator: signer, config, system_program: system,
        label: ArrayString32 { length: 5, storage },
    }.into();
    assert_eq!(actual, expected);
    assert!(matches!(decode_instruction(&actual.data), Some(ProgramInstruction::SetLabel { .. })));
}

#[test]
fn generated_account_decoder_reads_zeropod_bytes() {
    let creator = Address::new_from_array([7; 32]);
    let state = multisig::MultisigConfig {
        creator, threshold: 2, bump: 255, label: "hello".into(),
        members: vec![creator, Address::new_from_array([8; 32])],
    };
    let mut bytes = vec![1];
    bytes.extend(zeropod::to_vec(&state).unwrap());
    let Some(ProgramAccount::MultisigConfig(decoded)) = multisig_client::state::decode_account(&bytes) else {
        panic!("account did not decode");
    };
    assert_eq!(decoded.creator, state.creator);
    assert_eq!(decoded.label, state.label);
    assert_eq!(decoded.members, state.members);
    assert_eq!(decoded.bump, 255);
}
