use super::*;

/// Prove `check_address_match` returns `Ok(())` when addresses are equal.
#[kani::proof]
fn check_address_match_equal_returns_ok() {
    let bytes: [u8; 32] = kani::any();
    let a = Address::new_from_array(bytes);
    let b = Address::new_from_array(bytes);
    assert!(check_address_match(&a, &b, ProgramError::InvalidArgument) == Ok(()));
}

/// Prove `check_address_match` returns the caller's exact error when
/// addresses differ.
#[kani::proof]
fn check_address_match_unequal_returns_exact_error() {
    let a_bytes: [u8; 32] = kani::any();
    let b_bytes: [u8; 32] = kani::any();
    kani::assume(a_bytes != b_bytes);
    let a = Address::new_from_array(a_bytes);
    let b = Address::new_from_array(b_bytes);
    let code: u32 = kani::any();
    let error = ProgramError::Custom(code);
    assert!(check_address_match(&a, &b, error) == Err(ProgramError::Custom(code)));
}

/// Prove `check_constraint` returns `Ok(())` when condition is true.
#[kani::proof]
fn check_constraint_true_returns_ok() {
    let code: u32 = kani::any();
    let error = ProgramError::Custom(code);
    assert!(check_constraint(true, error) == Ok(()));
}

/// Prove `check_constraint` returns the caller's exact error when condition
/// is false.
#[kani::proof]
fn check_constraint_false_returns_exact_error() {
    let code: u32 = kani::any();
    let error = ProgramError::Custom(code);
    assert!(check_constraint(false, error) == Err(ProgramError::Custom(code)));
}
