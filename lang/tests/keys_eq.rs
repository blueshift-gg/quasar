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
