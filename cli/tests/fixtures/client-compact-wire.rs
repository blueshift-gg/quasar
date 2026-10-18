use client_regressions_client::instructions::{SubmitInstruction, decode_instruction, ProgramInstruction};
use solana_address::Address;
use solana_instruction::Instruction;

#[test]
fn optional_dynamic_values_encode_tags_before_tail_payloads() {
    for name in [None, Some(""), Some("é")] {
        for addresses in [None, Some(vec![]), Some(vec![Address::new_from_array([42; 32])])] {
            let mut expected = vec![7, u8::from(name.is_some()), u8::from(addresses.is_some())];
            if let Some(name) = name {
                expected.push(name.len() as u8);
                expected.extend(name.as_bytes());
            }
            if let Some(addresses) = &addresses {
                expected.extend((addresses.len() as u16).to_le_bytes());
                for address in addresses { expected.extend(address.as_ref()); }
            }
            let ix: Instruction = SubmitInstruction {
                authority: Address::default(),
                maybe_name: name.map(Into::into),
                maybe_addrs: addresses.clone().map(Into::into),
            }.into();
            assert_eq!(ix.data, expected);
            let Some(ProgramInstruction::Submit { maybe_name, maybe_addrs }) = decode_instruction(&ix.data) else { panic!("submit did not decode") };
            assert_eq!(maybe_name.as_ref().map(|v| v.as_bytes()), name.map(str::as_bytes));
            assert_eq!(maybe_addrs.as_ref().map(|v| v.0.as_slice()), addresses.as_deref());
        }
    }
}
