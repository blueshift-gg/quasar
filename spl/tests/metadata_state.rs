#![cfg(feature = "metadata")]
#![allow(dead_code)]

use std::mem::size_of;

use quasar_core::__internal::{
    AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE, NOT_BORROWED,
};
use quasar_core::traits::{AccountCheck, CheckOwner, ZeroCopyDeref};
use quasar_spl::metadata::{
    MasterEditionAccount, MasterEditionPrefix, MetadataAccount, MetadataPrefix, MetadataProgram,
    METADATA_PROGRAM_ID,
};
use solana_program_error::ProgramError;
use solana_address::Address;

// ---------------------------------------------------------------------------
// Test helpers (same pattern as core/tests/)
// ---------------------------------------------------------------------------

struct AccountBuffer {
    inner: Vec<u64>,
}

impl AccountBuffer {
    fn new(data_len: usize) -> Self {
        let byte_len =
            size_of::<RuntimeAccount>() + data_len + MAX_PERMITTED_DATA_INCREASE + size_of::<u64>();
        let u64_count = byte_len.div_ceil(8);
        Self {
            inner: vec![0; u64_count],
        }
    }

    fn raw(&mut self) -> *mut RuntimeAccount {
        self.inner.as_mut_ptr() as *mut RuntimeAccount
    }

    fn init(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = 0;
            (*raw).resize_delta = 0;
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    fn init_executable(
        &mut self,
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        data_len: u64,
        is_signer: bool,
        is_writable: bool,
    ) {
        let raw = self.raw();
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = 1;
            (*raw).resize_delta = 0;
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = lamports;
            (*raw).data_len = data_len;
        }
    }

    unsafe fn view(&mut self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }

    fn write_data(&mut self, data: &[u8]) {
        let data_start = size_of::<RuntimeAccount>();
        let dst = unsafe {
            let ptr = (self.inner.as_mut_ptr() as *mut u8).add(data_start);
            std::slice::from_raw_parts_mut(ptr, data.len())
        };
        dst.copy_from_slice(data);
    }
}

fn metadata_program_bytes() -> [u8; 32] {
    [
        11, 112, 101, 177, 227, 209, 124, 69, 56, 157, 82, 127, 107, 4, 195, 205, 88, 184, 108,
        115, 26, 160, 253, 181, 73, 182, 209, 188, 3, 248, 41, 70,
    ]
}

// ---------------------------------------------------------------------------
// MetadataPrefix — layout and accessors
// ---------------------------------------------------------------------------

#[test]
fn metadata_prefix_size_is_65() {
    assert_eq!(size_of::<MetadataPrefix>(), 65);
}

#[test]
fn metadata_prefix_alignment_is_1() {
    assert_eq!(std::mem::align_of::<MetadataPrefix>(), 1);
}

#[test]
fn metadata_prefix_len_constant() {
    assert_eq!(MetadataPrefix::LEN, 65);
}

#[test]
fn metadata_prefix_accessors() {
    let mut bytes = [0u8; 65];
    bytes[0] = 4; // key = MetadataV1
    bytes[1..33].copy_from_slice(&[0xAA; 32]); // update_authority
    bytes[33..65].copy_from_slice(&[0xBB; 32]); // mint

    let prefix = unsafe { &*(bytes.as_ptr() as *const MetadataPrefix) };
    assert_eq!(prefix.key(), 4);
    assert_eq!(prefix.update_authority(), &Address::new_from_array([0xAA; 32]));
    assert_eq!(prefix.mint(), &Address::new_from_array([0xBB; 32]));
}

// ---------------------------------------------------------------------------
// MasterEditionPrefix — layout and accessors
// ---------------------------------------------------------------------------

#[test]
fn master_edition_prefix_size_is_18() {
    assert_eq!(size_of::<MasterEditionPrefix>(), 18);
}

#[test]
fn master_edition_prefix_alignment_is_1() {
    assert_eq!(std::mem::align_of::<MasterEditionPrefix>(), 1);
}

#[test]
fn master_edition_prefix_len_constant() {
    assert_eq!(MasterEditionPrefix::LEN, 18);
}

#[test]
fn master_edition_prefix_supply() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6; // key = MasterEditionV2
    bytes[1..9].copy_from_slice(&42u64.to_le_bytes()); // supply = 42

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.key(), 6);
    assert_eq!(prefix.supply(), 42);
}

#[test]
fn master_edition_prefix_max_supply_some() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6;
    bytes[1..9].copy_from_slice(&10u64.to_le_bytes()); // supply
    bytes[9] = 1; // max_supply_flag = Some
    bytes[10..18].copy_from_slice(&100u64.to_le_bytes()); // max_supply = 100

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.supply(), 10);
    assert_eq!(prefix.max_supply(), Some(100));
}

#[test]
fn master_edition_prefix_max_supply_none() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6;
    bytes[1..9].copy_from_slice(&5u64.to_le_bytes());
    bytes[9] = 0; // max_supply_flag = None

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.supply(), 5);
    assert_eq!(prefix.max_supply(), None);
}

#[test]
fn master_edition_prefix_max_supply_zero_value() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6;
    bytes[9] = 1; // Some
    bytes[10..18].copy_from_slice(&0u64.to_le_bytes()); // max_supply = 0

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.max_supply(), Some(0));
}

#[test]
fn master_edition_prefix_max_supply_u64_max() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6;
    bytes[9] = 1;
    bytes[10..18].copy_from_slice(&u64::MAX.to_le_bytes());

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.max_supply(), Some(u64::MAX));
}

// ---------------------------------------------------------------------------
// MetadataAccount — AccountCheck
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_check_valid() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
    let mut data = [0u8; 65];
    data[0] = 4; // KEY_METADATA_V1
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(MetadataAccount::check(&view).is_ok());
}

#[test]
fn metadata_account_check_too_small() {
    let mut buf = AccountBuffer::new(64);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 64, false, false);
    let mut data = [0u8; 64];
    data[0] = 4;
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

#[test]
fn metadata_account_check_wrong_key_zero() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
    buf.write_data(&[0u8; 65]); // key = 0, not 4
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn metadata_account_check_wrong_key_other() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
    let mut data = [0u8; 65];
    data[0] = 6; // MasterEditionV2, not MetadataV1
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

// ---------------------------------------------------------------------------
// MetadataAccount — CheckOwner
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_owner_correct() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
    let view = unsafe { buf.view() };
    assert!(MetadataAccount::check_owner(&view).is_ok());
}

#[test]
fn metadata_account_owner_wrong() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], [0u8; 32], 1000, 65, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check_owner(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

#[test]
fn metadata_account_owner_single_byte_diff() {
    let mut owner = metadata_program_bytes();
    owner[31] ^= 0x01; // flip one bit
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], owner, 1000, 65, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check_owner(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

// ---------------------------------------------------------------------------
// MetadataAccount — ZeroCopyDeref
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_deref_reads_fields() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
    let mut data = [0u8; 65];
    data[0] = 4;
    data[1..33].copy_from_slice(&[0x11; 32]); // update_authority
    data[33..65].copy_from_slice(&[0x22; 32]); // mint
    buf.write_data(&data);
    let view = unsafe { buf.view() };

    let prefix = MetadataAccount::deref_from(&view);
    assert_eq!(prefix.key(), 4);
    assert_eq!(prefix.update_authority(), &Address::new_from_array([0x11; 32]));
    assert_eq!(prefix.mint(), &Address::new_from_array([0x22; 32]));
}

#[test]
fn metadata_account_deref_larger_data() {
    let mut buf = AccountBuffer::new(512);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 512, false, false);
    let mut data = [0u8; 512];
    data[0] = 4;
    data[1..33].copy_from_slice(&[0xCC; 32]);
    data[33..65].copy_from_slice(&[0xDD; 32]);
    buf.write_data(&data);
    let view = unsafe { buf.view() };

    let prefix = MetadataAccount::deref_from(&view);
    assert_eq!(prefix.key(), 4);
    assert_eq!(prefix.update_authority(), &Address::new_from_array([0xCC; 32]));
    assert_eq!(prefix.mint(), &Address::new_from_array([0xDD; 32]));
}

// ---------------------------------------------------------------------------
// MasterEditionAccount — AccountCheck
// ---------------------------------------------------------------------------

#[test]
fn master_edition_account_check_valid() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    let mut data = [0u8; 18];
    data[0] = 6; // KEY_MASTER_EDITION_V2
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(MasterEditionAccount::check(&view).is_ok());
}

#[test]
fn master_edition_account_check_too_small() {
    let mut buf = AccountBuffer::new(17);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 17, false, false);
    let mut data = [0u8; 17];
    data[0] = 6;
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

#[test]
fn master_edition_account_check_wrong_key_zero() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    buf.write_data(&[0u8; 18]);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn master_edition_account_check_wrong_key_metadata() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    let mut data = [0u8; 18];
    data[0] = 4; // MetadataV1, not MasterEditionV2
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

// ---------------------------------------------------------------------------
// MasterEditionAccount — CheckOwner
// ---------------------------------------------------------------------------

#[test]
fn master_edition_account_owner_correct() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    let view = unsafe { buf.view() };
    assert!(MasterEditionAccount::check_owner(&view).is_ok());
}

#[test]
fn master_edition_account_owner_wrong() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], [0xFF; 32], 1000, 18, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check_owner(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

// ---------------------------------------------------------------------------
// MasterEditionAccount — ZeroCopyDeref
// ---------------------------------------------------------------------------

#[test]
fn master_edition_account_deref_reads_fields() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    let mut data = [0u8; 18];
    data[0] = 6;
    data[1..9].copy_from_slice(&777u64.to_le_bytes());
    data[9] = 1; // Some
    data[10..18].copy_from_slice(&1000u64.to_le_bytes());
    buf.write_data(&data);
    let view = unsafe { buf.view() };

    let prefix = MasterEditionAccount::deref_from(&view);
    assert_eq!(prefix.key(), 6);
    assert_eq!(prefix.supply(), 777);
    assert_eq!(prefix.max_supply(), Some(1000));
}

#[test]
fn master_edition_account_deref_unlimited() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    let mut data = [0u8; 18];
    data[0] = 6;
    data[1..9].copy_from_slice(&0u64.to_le_bytes());
    data[9] = 0; // None (unlimited)
    buf.write_data(&data);
    let view = unsafe { buf.view() };

    let prefix = MasterEditionAccount::deref_from(&view);
    assert_eq!(prefix.supply(), 0);
    assert_eq!(prefix.max_supply(), None);
}

// ---------------------------------------------------------------------------
// MetadataProgram — address validation
// ---------------------------------------------------------------------------

#[test]
fn metadata_program_correct_address() {
    let mut buf = AccountBuffer::new(0);
    buf.init_executable(metadata_program_bytes(), [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(MetadataProgram::from_account_view(&view).is_ok());
}

#[test]
fn metadata_program_wrong_address() {
    let mut buf = AccountBuffer::new(0);
    buf.init_executable([0xFF; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataProgram::from_account_view(&view),
        Err(ProgramError::IncorrectProgramId)
    ));
}

#[test]
fn metadata_program_id_matches_expected_bytes() {
    let expected = metadata_program_bytes();
    assert_eq!(*METADATA_PROGRAM_ID.as_ref(), expected);
}

// ===========================================================================
// Adversarial tests — corruption, boundary, and exhaustive validation
// ===========================================================================

// ---------------------------------------------------------------------------
// Exhaustive key validation — only the correct key byte should pass
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_rejects_all_keys_except_4() {
    for key in 0u8..=255 {
        let mut buf = AccountBuffer::new(65);
        buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
        let mut data = [0u8; 65];
        data[0] = key;
        buf.write_data(&data);
        let view = unsafe { buf.view() };
        if key == 4 {
            assert!(MetadataAccount::check(&view).is_ok(), "key=4 should pass");
        } else {
            assert!(
                matches!(MetadataAccount::check(&view), Err(ProgramError::InvalidAccountData)),
                "key={key} should be rejected"
            );
        }
    }
}

#[test]
fn master_edition_account_rejects_all_keys_except_6() {
    for key in 0u8..=255 {
        let mut buf = AccountBuffer::new(18);
        buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
        let mut data = [0u8; 18];
        data[0] = key;
        buf.write_data(&data);
        let view = unsafe { buf.view() };
        if key == 6 {
            assert!(MasterEditionAccount::check(&view).is_ok(), "key=6 should pass");
        } else {
            assert!(
                matches!(MasterEditionAccount::check(&view), Err(ProgramError::InvalidAccountData)),
                "key={key} should be rejected"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Boundary data lengths — exact boundary and off-by-one
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_zero_length_data() {
    let mut buf = AccountBuffer::new(0);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

#[test]
fn metadata_account_one_byte_data_with_correct_key() {
    let mut buf = AccountBuffer::new(1);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 1, false, false);
    buf.write_data(&[4]); // correct key but way too small
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

#[test]
fn master_edition_account_zero_length_data() {
    let mut buf = AccountBuffer::new(0);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

#[test]
fn master_edition_account_one_byte_with_correct_key() {
    let mut buf = AccountBuffer::new(1);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 1, false, false);
    buf.write_data(&[6]); // correct key but too small
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::AccountDataTooSmall)
    ));
}

// ---------------------------------------------------------------------------
// All-0xFF adversarial data — attacker fills every byte with 0xFF
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_all_ff_data_rejected() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], metadata_program_bytes(), 1000, 65, false, false);
    buf.write_data(&[0xFF; 65]); // key=0xFF, not 4
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn master_edition_account_all_ff_data_rejected() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], metadata_program_bytes(), 1000, 18, false, false);
    buf.write_data(&[0xFF; 18]); // key=0xFF, not 6
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

// ---------------------------------------------------------------------------
// All-zero (uninitialized) account attack
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_all_zero_uninitialized_rejected() {
    let mut buf = AccountBuffer::new(65);
    buf.init([0u8; 32], metadata_program_bytes(), 0, 65, false, false);
    buf.write_data(&[0u8; 65]);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

#[test]
fn master_edition_account_all_zero_uninitialized_rejected() {
    let mut buf = AccountBuffer::new(18);
    buf.init([0u8; 32], metadata_program_bytes(), 0, 18, false, false);
    buf.write_data(&[0u8; 18]);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MasterEditionAccount::check(&view),
        Err(ProgramError::InvalidAccountData)
    ));
}

// ---------------------------------------------------------------------------
// Owner checks — all-0xFF owner, each byte position difference
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_owner_all_ff_rejected() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], [0xFF; 32], 1000, 65, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataAccount::check_owner(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

#[test]
fn metadata_account_owner_each_byte_position_differs() {
    let correct = metadata_program_bytes();
    for i in 0..32 {
        let mut owner = correct;
        owner[i] ^= 0x01; // flip one bit in each position
        let mut buf = AccountBuffer::new(65);
        buf.init([10u8; 32], owner, 1000, 65, false, false);
        let view = unsafe { buf.view() };
        assert!(
            matches!(MetadataAccount::check_owner(&view), Err(ProgramError::IllegalOwner)),
            "owner byte {i} flipped should fail"
        );
    }
}

#[test]
fn master_edition_account_owner_each_byte_position_differs() {
    let correct = metadata_program_bytes();
    for i in 0..32 {
        let mut owner = correct;
        owner[i] ^= 0x80; // flip high bit
        let mut buf = AccountBuffer::new(18);
        buf.init([20u8; 32], owner, 1000, 18, false, false);
        let view = unsafe { buf.view() };
        assert!(
            matches!(MasterEditionAccount::check_owner(&view), Err(ProgramError::IllegalOwner)),
            "owner byte {i} flipped should fail"
        );
    }
}

// ---------------------------------------------------------------------------
// MasterEdition max_supply_flag adversarial values
// ---------------------------------------------------------------------------

#[test]
fn master_edition_max_supply_flag_2_returns_none() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6;
    bytes[1..9].copy_from_slice(&10u64.to_le_bytes());
    bytes[9] = 2; // invalid Borsh Option tag
    bytes[10..18].copy_from_slice(&999u64.to_le_bytes());

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.max_supply(), None, "flag=2 should be treated as None");
}

#[test]
fn master_edition_max_supply_flag_255_returns_none() {
    let mut bytes = [0u8; 18];
    bytes[0] = 6;
    bytes[1..9].copy_from_slice(&10u64.to_le_bytes());
    bytes[9] = 255; // invalid Borsh Option tag
    bytes[10..18].copy_from_slice(&999u64.to_le_bytes());

    let prefix = unsafe { &*(bytes.as_ptr() as *const MasterEditionPrefix) };
    assert_eq!(prefix.max_supply(), None, "flag=255 should be treated as None");
}

// ---------------------------------------------------------------------------
// Correct data + wrong owner: combined check order matters
// ---------------------------------------------------------------------------

#[test]
fn metadata_account_correct_data_wrong_owner_fails_owner_check() {
    let mut buf = AccountBuffer::new(65);
    buf.init([10u8; 32], [0u8; 32], 1000, 65, false, false); // zero owner
    let mut data = [0u8; 65];
    data[0] = 4; // correct key
    data[1..33].copy_from_slice(&[0xAA; 32]);
    data[33..65].copy_from_slice(&[0xBB; 32]);
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    // Data check passes
    assert!(MetadataAccount::check(&view).is_ok());
    // Owner check fails
    assert!(matches!(
        MetadataAccount::check_owner(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

#[test]
fn master_edition_correct_data_wrong_owner_fails_owner_check() {
    let mut buf = AccountBuffer::new(18);
    buf.init([20u8; 32], [0u8; 32], 1000, 18, false, false);
    let mut data = [0u8; 18];
    data[0] = 6;
    data[1..9].copy_from_slice(&42u64.to_le_bytes());
    buf.write_data(&data);
    let view = unsafe { buf.view() };
    assert!(MasterEditionAccount::check(&view).is_ok());
    assert!(matches!(
        MasterEditionAccount::check_owner(&view),
        Err(ProgramError::IllegalOwner)
    ));
}

// ---------------------------------------------------------------------------
// MetadataProgram — adversarial address variants
// ---------------------------------------------------------------------------

#[test]
fn metadata_program_all_zero_address_rejected() {
    let mut buf = AccountBuffer::new(0);
    buf.init_executable([0u8; 32], [0u8; 32], 0, 0, false, false);
    let view = unsafe { buf.view() };
    assert!(matches!(
        MetadataProgram::from_account_view(&view),
        Err(ProgramError::IncorrectProgramId)
    ));
}

#[test]
fn metadata_program_each_byte_position_differs() {
    let correct = metadata_program_bytes();
    for i in 0..32 {
        let mut addr = correct;
        addr[i] ^= 0x01;
        let mut buf = AccountBuffer::new(0);
        buf.init_executable(addr, [0u8; 32], 0, 0, false, false);
        let view = unsafe { buf.view() };
        assert!(
            matches!(MetadataProgram::from_account_view(&view), Err(ProgramError::IncorrectProgramId)),
            "program address byte {i} flipped should fail"
        );
    }
}
