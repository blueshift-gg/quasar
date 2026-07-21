use super::*;

#[test]
fn mint_deref_reads_all_fields() {
    let (mut buf, _data) = mint_account_buffer(1_000_000_000, 9);
    let view = unsafe { buf.view() };

    <Mint as CheckOwner>::check_owner(&view).unwrap();
    <Mint as quasar_lang::account_load::AccountLoad>::check(&view).unwrap();
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintDataZc = &*account;

    assert!(state.mint_authority().is_some());
    assert_eq!(
        state.mint_authority().unwrap(),
        &Address::new_from_array([0xCC; 32])
    );
    assert_eq!(state.supply(), 1_000_000_000);
    assert_eq!(state.decimals(), 9);
    assert!(state.is_initialized());
    assert!(state.freeze_authority().is_none());
}

#[test]
fn mint_exact_size_buffer() {
    let data = build_simple_mint_data(0, 6);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 500_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    assert_eq!(account.decimals(), 6);
    assert_eq!(account.supply(), 0);
}

#[test]
fn mint_deref_mut_write() {
    let (mut buf, _data) = mint_account_buffer(500, 6);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Mint>::from_account_view_unchecked_mut(&mut view) };
    assert_eq!(account.supply(), 500);

    // Write supply through raw pointer. Supply is at offset 36 (flag=4,
    // authority=32)
    let state: &mut MintDataZc = &mut *account;
    unsafe {
        let supply_ptr = (state as *mut MintDataZc as *mut u8).add(36);
        let new_supply: u64 = 999_999;
        core::ptr::copy_nonoverlapping(new_supply.to_le_bytes().as_ptr(), supply_ptr, 8);
    }
    assert_eq!(account.supply(), 999_999);
}

#[test]
fn mint_all_flags_set() {
    let data = build_mint_data(true, [0xAA; 32], u64::MAX, 18, true, true, [0xBB; 32]);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintDataZc = &*account;

    assert!(state.mint_authority().is_some());
    assert_eq!(state.supply(), u64::MAX);
    assert_eq!(state.decimals(), 18);
    assert!(state.is_initialized());
    assert!(state.freeze_authority().is_some());
    assert_eq!(
        state.freeze_authority().unwrap(),
        &Address::new_from_array([0xBB; 32])
    );
}

#[test]
fn mint_no_authorities() {
    let data = build_mint_data(false, [0; 32], 0, 0, false, false, [0; 32]);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintDataZc = &*account;

    assert!(state.mint_authority().is_none());
    assert!(state.freeze_authority().is_none());
    assert!(!state.is_initialized());
}
