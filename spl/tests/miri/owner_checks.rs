use super::*;

#[test]
fn check_owner_spl_token_passes() {
    let (mut buf, _data) = token_account_buffer(100);
    let view = unsafe { buf.view() };
    assert!(<Token as CheckOwner>::check_owner(&view).is_ok());
}

#[test]
fn check_owner_wrong_owner_fails() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], [0xFF; 32], 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Token as CheckOwner>::check_owner(&view).unwrap_err(),
        ProgramError::IllegalOwner
    );
}

#[test]
fn account_check_data_too_small() {
    let mut buf = AccountBuffer::new(100);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 100, false, true);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Token as quasar_lang::account_load::AccountLoad>::check(&view).unwrap_err(),
        ProgramError::AccountDataTooSmall
    );
}

#[test]
fn mint_check_owner_passes() {
    let (mut buf, _data) = mint_account_buffer(100, 6);
    let view = unsafe { buf.view() };
    assert!(<Mint as CheckOwner>::check_owner(&view).is_ok());
}

#[test]
fn mint_account_check_data_too_small() {
    let mut buf = AccountBuffer::new(50);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 50, false, true);

    let view = unsafe { buf.view() };
    assert_eq!(
        <Mint as quasar_lang::account_load::AccountLoad>::check(&view).unwrap_err(),
        ProgramError::AccountDataTooSmall
    );
}
