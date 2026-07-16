use {
    quasar_lang::{is_system_program, keys_eq},
    solana_address::Address,
};

#[test]
fn keys_eq_identical() {
    let a = Address::new_from_array([0xAB; 32]);
    assert!(keys_eq(&a, &a));
}

#[test]
fn keys_eq_first_word_mismatch() {
    let a = Address::new_from_array([0xFF; 32]);
    let mut b_bytes = [0xFF; 32];
    b_bytes[0] = 0x00;
    let b = Address::new_from_array(b_bytes);
    assert!(!keys_eq(&a, &b));
}

#[test]
fn keys_eq_last_word_mismatch() {
    let a = Address::new_from_array([0xFF; 32]);
    let mut b_bytes = [0xFF; 32];
    b_bytes[31] = 0x00;
    let b = Address::new_from_array(b_bytes);
    assert!(!keys_eq(&a, &b));
}

#[test]
fn keys_eq_all_zero() {
    let a = Address::new_from_array([0; 32]);
    let b = Address::new_from_array([0; 32]);
    assert!(keys_eq(&a, &b));
}

#[test]
fn is_system_program_zero() {
    let addr = Address::new_from_array([0; 32]);
    assert!(is_system_program(&addr));
}

#[test]
fn is_system_program_nonzero() {
    let mut bytes = [0u8; 32];
    bytes[16] = 1;
    let addr = Address::new_from_array(bytes);
    assert!(!is_system_program(&addr));
}

// AddressVerify for plain Address values: exact match yields bump 0, any
// mismatch yields AddressMismatch, and &Address delegates. (This is the
// terminal address check behind `address = expr` constraints.)

#[test]
fn address_verify_accepts_exact_match() {
    use quasar_lang::{address::AddressVerify, prelude::ProgramError};
    let expected = solana_address::Address::new_from_array([7; 32]);
    let actual = solana_address::Address::new_from_array([7; 32]);
    assert_eq!(expected.verify(&actual, &expected), Ok(0));
    let _ = ProgramError::Custom(0);
}

#[test]
fn address_verify_rejects_any_corrupted_word() {
    use quasar_lang::{
        address::AddressVerify,
        prelude::{ProgramError, QuasarError},
    };
    let expected = solana_address::Address::new_from_array([7; 32]);
    for pos in [0usize, 7, 8, 15, 16, 23, 24, 31] {
        let mut bytes = [7u8; 32];
        bytes[pos] ^= 0x01;
        let actual = solana_address::Address::new_from_array(bytes);
        assert_eq!(
            expected.verify(&actual, &expected),
            Err(ProgramError::Custom(QuasarError::AddressMismatch as u32)),
            "corrupted byte {pos} must be detected"
        );
    }
}

#[test]
fn address_verify_reference_delegates() {
    use quasar_lang::{
        address::AddressVerify,
        prelude::{ProgramError, QuasarError},
    };
    let expected = solana_address::Address::new_from_array([9; 32]);
    let matching = solana_address::Address::new_from_array([9; 32]);
    let mismatched = solana_address::Address::new_from_array([8; 32]);
    assert_eq!((&expected).verify(&matching, &expected), Ok(0));
    assert_eq!(
        (&expected).verify(&mismatched, &expected),
        Err(ProgramError::Custom(QuasarError::AddressMismatch as u32))
    );
}
