use super::*;

#[test]
fn token_deref_reads_all_fields() {
    let (mut buf, _data) = token_account_buffer(1_000_000);
    let view = unsafe { buf.view() };

    // Use Account<Token> to exercise Deref into TokenDataZc.
    <Token as CheckOwner>::check_owner(&view).unwrap();
    <Token as quasar_lang::account_load::AccountLoad>::check(&view).unwrap();
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenDataZc = &*account;

    assert_eq!(state.mint(), &Address::new_from_array([0xAA; 32]));
    assert_eq!(state.owner(), &Address::new_from_array([0xBB; 32]));
    assert_eq!(state.amount(), 1_000_000);
    assert!(state.delegate().is_none());
    assert!(state.is_initialized());
    assert!(!state.is_frozen());
    assert!(state.native().is_none());
    assert!(state.native_amount().is_none());
    assert_eq!(state.delegated_amount(), 0);
    assert!(state.close_authority().is_none());
}

#[test]
fn token_deref_exact_size_buffer() {
    // Allocate exactly 165 bytes of data, with no slack.
    let data = build_simple_token_data(42);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 500_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenDataZc = &*account;
    assert_eq!(state.amount(), 42);
}

#[test]
fn token_deref_mut_writes_amount() {
    let (mut buf, _data) = token_account_buffer(100);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Read initial amount
    assert_eq!(account.amount(), 100);

    // Write new amount through DerefMut
    let state: &mut TokenDataZc = &mut *account;
    // TokenDataZc fields are private, so we write through raw pointer
    // to the amount field at offset 64 (mint=32, owner=32)
    unsafe {
        let amount_ptr = (state as *mut TokenDataZc as *mut u8).add(64);
        let new_amount: u64 = 999;
        core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
    }

    // Verify write took effect
    assert_eq!(account.amount(), 999);
}

#[test]
fn token_deref_mut_aliasing_stress() {
    // &view and &mut Account<Token>, interleaved reads/writes
    let (mut buf, _data) = token_account_buffer(500);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Read through &mut Account
    assert_eq!(account.amount(), 500);

    // Read lamports through the account's view
    assert_eq!(account.to_account_view().lamports(), 1_000_000);

    // Write lamports through the account's view (interior mutability)
    set_lamports(account.to_account_view(), 2_000_000);

    // Read back through &mut Account
    assert_eq!(account.to_account_view().lamports(), 2_000_000);

    // Read token data through &mut
    assert_eq!(account.amount(), 500);

    // Interleave: read account view, read account, repeat
    for _ in 0..10 {
        let _ = account.to_account_view().lamports();
        let _ = account.amount();
        let _ = account.to_account_view().data_len();
        let _ = account.mint();
    }
}

#[test]
fn token_deref_various_flag_patterns() {
    // All flags set: delegate, is_native, close_authority
    let data = build_token_data(
        [0x11; 32], // mint
        [0x22; 32], // owner
        5_000_000,  // amount
        true,       // delegate_flag
        [0x33; 32], // delegate
        2,          // state = Frozen
        true,       // is_native
        100_000,    // native_amount
        3_000_000,  // delegated_amount
        true,       // close_authority_flag
        [0x44; 32], // close_authority
    );
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenDataZc = &*account;

    assert!(state.delegate().is_some());
    assert_eq!(
        state.delegate().unwrap(),
        &Address::new_from_array([0x33; 32])
    );
    assert!(state.is_frozen());
    assert!(state.is_initialized());
    assert!(state.native().is_some());
    assert_eq!(state.native_amount().unwrap(), 100_000);
    assert_eq!(state.delegated_amount(), 3_000_000);
    assert!(state.close_authority().is_some());
    assert_eq!(
        state.close_authority().unwrap(),
        &Address::new_from_array([0x44; 32])
    );
}

#[test]
fn token_deref_no_flags_set() {
    // All optional flags off
    let data = build_token_data(
        [0x11; 32], [0x22; 32], 0,     // zero amount
        false, // no delegate
        [0; 32], 0,     // state = Uninitialized
        false, // not native
        0, 0, false, // no close authority
        [0; 32],
    );
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenDataZc = &*account;

    assert!(state.delegate().is_none());
    assert!(state.native().is_none());
    assert!(state.native_amount().is_none());
    assert!(state.close_authority().is_none());
    assert!(!state.is_initialized());
    assert!(!state.is_frozen());
}
