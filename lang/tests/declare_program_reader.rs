#![cfg(feature = "declare-program")]

use {
    quasar_lang::prelude::*,
    solana_account_view::{RuntimeAccount, NOT_BORROWED},
};

declare_program!(external_fixed, "tests/fixtures/external_fixed.idl.json");

struct AccountBuffer {
    words: std::vec::Vec<u64>,
}

impl AccountBuffer {
    fn new(owner: [u8; 32], data: &[u8]) -> Self {
        let bytes = core::mem::size_of::<RuntimeAccount>() + data.len();
        let mut buffer = Self {
            words: vec![0; bytes.div_ceil(core::mem::size_of::<u64>())],
        };
        let raw = buffer.words.as_mut_ptr() as *mut RuntimeAccount;
        // SAFETY: `words` is 8-byte aligned and was allocated for the complete
        // runtime header plus `data`.
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = 0;
            (*raw).is_writable = 1;
            (*raw).executable = 0;
            (*raw).padding = [0; 4];
            (*raw).address = Address::new_from_array([5; 32]);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = 1;
            (*raw).data_len = data.len() as u64;
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                (raw as *mut u8).add(core::mem::size_of::<RuntimeAccount>()),
                data.len(),
            );
        }
        buffer
    }

    fn view(&mut self) -> AccountView {
        // SAFETY: `new` initialized the header and contiguous data allocation.
        unsafe { AccountView::new_unchecked(self.words.as_mut_ptr() as *mut RuntimeAccount) }
    }
}

fn push_u16(data: &mut std::vec::Vec<u8>, value: u16) {
    data.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(data: &mut std::vec::Vec<u8>, value: u32) {
    data.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(data: &mut std::vec::Vec<u8>, value: u64) {
    data.extend_from_slice(&value.to_le_bytes());
}

fn valid_account_data() -> std::vec::Vec<u8> {
    let mut data = vec![3, 4];
    data.extend_from_slice(&[11; 32]);
    for value in [10, 20, 30] {
        push_u32(&mut data, value);
    }
    push_u64(&mut data, 40);
    data.push(1);
    push_u64(&mut data, 50);
    data.push(0);
    for value in [60, 70, 80, 90] {
        push_u16(&mut data, value);
    }
    assert_eq!(data.len(), external_fixed::ForeignState::ACCOUNT_DATA_LEN);
    data
}

#[test]
fn fixed_account_reader_validates_and_decodes_nested_arrays() {
    let mut buffer = AccountBuffer::new([0; 32], &valid_account_data());
    let state = external_fixed::ForeignState::read_account(&buffer.view()).unwrap();

    assert_eq!(state.authority, Address::new_from_array([11; 32]));
    assert_eq!(state.counts, [10, 20, 30]);
    assert_eq!(
        state.records,
        [
            external_fixed::Record {
                amount: 40,
                active: true,
            },
            external_fixed::Record {
                amount: 50,
                active: false,
            },
        ]
    );
    assert_eq!(state.matrix, [[60, 70], [80, 90]]);
}

#[test]
fn fixed_account_reader_rejects_owner_length_discriminator_and_bool() {
    let data = valid_account_data();
    let mut wrong_owner = AccountBuffer::new([1; 32], &data);
    assert_eq!(
        external_fixed::ForeignState::read_account(&wrong_owner.view()),
        Err(ProgramError::IllegalOwner)
    );

    let mut short = AccountBuffer::new([0; 32], &data[..data.len() - 1]);
    assert_eq!(
        external_fixed::ForeignState::read_account(&short.view()),
        Err(ProgramError::AccountDataTooSmall)
    );

    let mut bad_discriminator = data.clone();
    bad_discriminator[0] = 99;
    let mut bad_discriminator = AccountBuffer::new([0; 32], &bad_discriminator);
    assert_eq!(
        external_fixed::ForeignState::read_account(&bad_discriminator.view()),
        Err(ProgramError::InvalidAccountData)
    );

    let mut bad_bool = data;
    bad_bool[54] = 2;
    let mut bad_bool = AccountBuffer::new([0; 32], &bad_bool);
    assert_eq!(
        external_fixed::ForeignState::read_account(&bad_bool.view()),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn cpi_encoder_writes_direct_defined_and_pubkey_arrays() {
    let mut program = AccountBuffer::new([0; 32], &[]);
    let mut state = AccountBuffer::new([0; 32], &valid_account_data());
    let owners = [
        Address::new_from_array([7; 32]),
        Address::new_from_array([8; 32]),
    ];
    let program_view = program.view();
    let state_view = state.view();
    let call = external_fixed::write(
        &program_view,
        &state_view,
        [1, 2],
        [
            external_fixed::Record {
                amount: 3,
                active: true,
            },
            external_fixed::Record {
                amount: 4,
                active: false,
            },
        ],
        owners,
    );

    let mut expected = vec![9];
    push_u16(&mut expected, 1);
    push_u16(&mut expected, 2);
    push_u64(&mut expected, 3);
    expected.push(1);
    push_u64(&mut expected, 4);
    expected.push(0);
    expected.extend_from_slice(&[7; 32]);
    expected.extend_from_slice(&[8; 32]);
    assert_eq!(call.instruction_data(), expected);
}
