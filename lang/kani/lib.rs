use {super::*, solana_address::Address};

/// Prove that `keys_eq` is equivalent to byte-wise equality for all
/// possible 32-byte address pairs.
#[kani::proof]
fn keys_eq_equivalence() {
    let a_bytes: [u8; 32] = kani::any();
    let b_bytes: [u8; 32] = kani::any();
    let a = Address::new_from_array(a_bytes);
    let b = Address::new_from_array(b_bytes);
    assert!(
        keys_eq(&a, &b) == (a_bytes == b_bytes),
        "keys_eq must be equivalent to byte-wise equality"
    );
}

/// Prove that `is_system_program` is true iff all 32 bytes are zero.
#[kani::proof]
fn is_system_program_equivalence() {
    let bytes: [u8; 32] = kani::any();
    let addr = Address::new_from_array(bytes);
    assert!(
        is_system_program(&addr) == (bytes == [0u8; 32]),
        "is_system_program must be true iff address is all-zero"
    );
}

/// Prove that `decode_header_error` returns `AccountBorrowFailed` when
/// the borrow byte does not match (duplicate account detection).
#[kani::proof]
fn decode_header_dup_returns_borrow_failed() {
    let header: u32 = kani::any();
    let expected: u32 = kani::any();
    let required_mask: u32 = kani::any();

    let h_bytes = header.to_le_bytes();
    let e_bytes = expected.to_le_bytes();

    // Borrow bytes differ; dup detection path.
    kani::assume(h_bytes[0] != e_bytes[0]);

    let result = decode_header_error(header, expected, required_mask);
    let borrow_failed = u64::from(solana_program_error::ProgramError::AccountBorrowFailed);
    assert!(
        result == borrow_failed,
        "borrow mismatch must return AccountBorrowFailed"
    );
}

/// Prove that `decode_header_error` returns 0 (accept) when the borrow
/// byte matches and all required flags are present (superset is OK).
#[kani::proof]
fn decode_header_accepts_superset() {
    let header: u32 = kani::any();
    let expected: u32 = kani::any();
    let required_mask: u32 = kani::any();

    let h_bytes = header.to_le_bytes();
    let e_bytes = expected.to_le_bytes();

    // Borrow bytes match.
    kani::assume(h_bytes[0] == e_bytes[0]);
    // All required flags present.
    kani::assume((header & required_mask) == (expected & required_mask));

    let result = decode_header_error(header, expected, required_mask);
    assert!(result == 0, "superset flags must be accepted (return 0)");
}

/// Prove that when the borrow byte matches, mask check fails, and
/// expected signer is nonzero but actual signer is zero, we get
/// `MissingRequiredSignature`.
#[kani::proof]
fn decode_header_missing_signer() {
    let header: u32 = kani::any();
    let expected: u32 = kani::any();
    let required_mask: u32 = kani::any();

    let h_bytes = header.to_le_bytes();
    let e_bytes = expected.to_le_bytes();

    // Borrow bytes match.
    kani::assume(h_bytes[0] == e_bytes[0]);
    // Mask check fails (not a superset).
    kani::assume((header & required_mask) != (expected & required_mask));
    // Expected signer nonzero, actual signer zero.
    kani::assume(e_bytes[1] != 0);
    kani::assume(h_bytes[1] == 0);

    let result = decode_header_error(header, expected, required_mask);
    let missing_sig = u64::from(solana_program_error::ProgramError::MissingRequiredSignature);
    assert!(
        result == missing_sig,
        "missing signer must return MissingRequiredSignature"
    );
}

/// Prove that when signer is OK but writable is missing, we get
/// `Immutable`.
#[kani::proof]
fn decode_header_missing_writable() {
    let header: u32 = kani::any();
    let expected: u32 = kani::any();
    let required_mask: u32 = kani::any();

    let h_bytes = header.to_le_bytes();
    let e_bytes = expected.to_le_bytes();

    // Borrow bytes match.
    kani::assume(h_bytes[0] == e_bytes[0]);
    // Mask check fails.
    kani::assume((header & required_mask) != (expected & required_mask));
    // Signer check passes (either not required or present).
    kani::assume(e_bytes[1] == 0 || h_bytes[1] != 0);
    // Expected writable nonzero, actual writable zero.
    kani::assume(e_bytes[2] != 0);
    kani::assume(h_bytes[2] == 0);

    let result = decode_header_error(header, expected, required_mask);
    let immutable = u64::from(solana_program_error::ProgramError::Immutable);
    assert!(
        result == immutable,
        "missing writable must return Immutable"
    );
}

/// Prove that when signer and writable are both OK but mask still
/// fails, we get `InvalidAccountData` (the executable fallthrough).
#[kani::proof]
fn decode_header_fallthrough_invalid_data() {
    let header: u32 = kani::any();
    let expected: u32 = kani::any();
    let required_mask: u32 = kani::any();

    let h_bytes = header.to_le_bytes();
    let e_bytes = expected.to_le_bytes();

    // Borrow bytes match.
    kani::assume(h_bytes[0] == e_bytes[0]);
    // Mask check fails.
    kani::assume((header & required_mask) != (expected & required_mask));
    // Signer check passes.
    kani::assume(e_bytes[1] == 0 || h_bytes[1] != 0);
    // Writable check passes.
    kani::assume(e_bytes[2] == 0 || h_bytes[2] != 0);

    let result = decode_header_error(header, expected, required_mask);
    let invalid_data = u64::from(solana_program_error::ProgramError::InvalidAccountData);
    assert!(
        result == invalid_data,
        "fallthrough must return InvalidAccountData"
    );
}
