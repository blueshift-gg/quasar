use super::*;

#[test]
fn intrinsic_load_for_concrete_token_skips_generic_zeropod_validation() {
    let mut data = build_simple_token_data(42);
    data[72] = 2; // invalid COption tag for delegate
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Account<Token> as AccountLoad>::load(&view)
            .map(|_| ())
            .unwrap_err(),
        ProgramError::InvalidAccountData
    );
    assert!(unsafe { <Account<Token> as AccountLoad>::load_intrinsic(&view) }.is_ok());
}

#[test]
fn intrinsic_load_for_concrete_token_keeps_owner_validation() {
    let mut data = build_simple_token_data(42);
    data[72] = 2; // invalid COption tag would be skipped by the behavior fast-load
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], TOKEN_2022_OWNER, 1_000_000, 165, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert_eq!(
        unsafe { <Account<Token> as AccountLoad>::load_intrinsic(&view) }
            .map(|_| ())
            .unwrap_err(),
        ProgramError::IllegalOwner
    );
}

#[test]
fn intrinsic_load_for_interface_token_keeps_generic_zeropod_validation() {
    let mut data = build_simple_token_data(42);
    data[72] = 2; // invalid COption tag for delegate
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert_eq!(
        unsafe { <InterfaceAccount<Token> as AccountLoad>::load_intrinsic(&view) }
            .map(|_| ())
            .unwrap_err(),
        ProgramError::InvalidAccountData
    );
}

#[test]
fn token_behavior_validation_rejects_invalid_coption_tags_after_fast_load() {
    let mut data = build_simple_token_data(42);
    data[72] = 2; // invalid COption tag for delegate
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(unsafe { <Account<Token> as AccountLoad>::load_intrinsic(&view) }.is_ok());
    assert_eq!(
        validate_token_account(
            &view,
            &Address::new_from_array([0xAA; 32]),
            &Address::new_from_array([0xBB; 32]),
            None,
        ),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn token_behavior_validation_rejects_invalid_state_after_fast_load() {
    let mut data = build_simple_token_data(42);
    data[108] = 3; // invalid AccountState
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(unsafe { <Account<Token> as AccountLoad>::load_intrinsic(&view) }.is_ok());
    assert_eq!(
        validate_token_account(
            &view,
            &Address::new_from_array([0xAA; 32]),
            &Address::new_from_array([0xBB; 32]),
            None,
        ),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn mint_behavior_validation_rejects_invalid_coption_tags_after_fast_load() {
    let mut data = build_simple_mint_data(500, 6);
    data[46] = 2; // invalid COption tag for freeze authority
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(unsafe { <Account<Mint> as AccountLoad>::load_intrinsic(&view) }.is_ok());
    assert_eq!(
        validate_mint_with_freeze(
            &view,
            &Address::new_from_array([0xCC; 32]),
            Some(6),
            FreezeCheck::Skip,
            None,
        ),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn mint_behavior_validation_rejects_invalid_initialized_flag_after_fast_load() {
    let mut data = build_simple_mint_data(500, 6);
    data[45] = 2; // invalid bool-like initialized flag
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, false);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert!(unsafe { <Account<Mint> as AccountLoad>::load_intrinsic(&view) }.is_ok());
    assert_eq!(
        validate_mint_with_freeze(
            &view,
            &Address::new_from_array([0xCC; 32]),
            Some(6),
            FreezeCheck::Skip,
            None,
        ),
        Err(ProgramError::InvalidAccountData)
    );
}
