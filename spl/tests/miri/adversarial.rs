use super::*;

#[test]
fn all_zero_token_data() {
    // 165 bytes of all zeros; state=0 means uninitialized.
    let data = [0u8; 165];
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenDataZc = &*account;

    // All fields should be zero/default
    assert_eq!(state.amount(), 0);
    assert!(!state.is_initialized());
    assert!(!state.is_frozen());
    assert!(state.delegate().is_none());
    assert!(state.native().is_none());
    assert!(state.close_authority().is_none());
    assert_eq!(state.delegated_amount(), 0);
}

#[test]
fn all_zero_mint_data() {
    let data = [0u8; 82];
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Mint>::from_account_view_unchecked(&view) };
    let state: &MintDataZc = &*account;

    assert_eq!(state.supply(), 0);
    assert_eq!(state.decimals(), 0);
    assert!(!state.is_initialized());
    assert!(state.mint_authority().is_none());
    assert!(state.freeze_authority().is_none());
}

#[test]
fn all_ff_token_data() {
    // All 0xFF bytes test maximum field values.
    // Note: flag fields check byte[0] == 1 (not != 0), so 0xFF flags are "false".
    let data = [0xFF; 165];
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };
    let state: &TokenDataZc = &*account;

    assert_eq!(state.amount(), u64::MAX);
    // 0xFF != 1, so flags are NOT set despite all bytes being 0xFF
    assert!(state.delegate().is_none());
    assert!(state.native().is_none());
    assert_eq!(state.delegated_amount(), u64::MAX);
    assert!(state.close_authority().is_none());
    // The option is semantically None, but the payload bytes are still present.
    assert_eq!(
        state.delegate.value_unchecked(),
        &Address::new_from_array([0xFF; 32])
    );
    assert_eq!(
        state.close_authority.value_unchecked(),
        &Address::new_from_array([0xFF; 32])
    );
}

#[test]
fn max_amount_values() {
    let data = build_token_data(
        [0xFF; 32],
        [0xFF; 32],
        u64::MAX,
        true,
        [0xFF; 32],
        1,
        true,
        u64::MAX,
        u64::MAX,
        true,
        [0xFF; 32],
    );
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, u64::MAX, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let account = unsafe { Account::<Token>::from_account_view_unchecked(&view) };

    assert_eq!(account.amount(), u64::MAX);
    assert_eq!(account.native_amount().unwrap(), u64::MAX);
    assert_eq!(account.delegated_amount(), u64::MAX);
    assert_eq!(view.lamports(), u64::MAX);
}

#[test]
fn rapid_deref_mut_cycling() {
    // 50 mut/shared cycles on the same view
    let (mut buf, _data) = token_account_buffer(0);
    let mut view = unsafe { buf.view() };

    for i in 0u64..50 {
        // Mutable deref scoped so the borrow is released before shared deref.
        {
            let state_mut = unsafe { <Token as ZeroCopyDeref>::deref_from_mut(&mut view) };
            unsafe {
                let amount_ptr = (state_mut as *mut TokenDataZc as *mut u8).add(64);
                core::ptr::copy_nonoverlapping(i.to_le_bytes().as_ptr(), amount_ptr, 8);
            }
        }

        // Shared deref
        let state_shared = unsafe { <Token as ZeroCopyDeref>::deref_from(&view) };
        assert_eq!(state_shared.amount(), i);
    }
}

#[test]
fn rapid_interface_account_cycling() {
    // Repeated from_account_view / from_account_view_mut cycles
    let data = build_simple_token_data(0);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };

    for _ in 0..30 {
        let shared = InterfaceAccount::<Token>::from_account_view(&view).unwrap();
        let _ = shared.amount();

        let mutable = InterfaceAccount::<Token>::from_account_view_mut(&mut view).unwrap();
        let _ = mutable.amount();
    }
}

#[test]
fn token_account_size_assertion() {
    // Compile-time assertion is in the source, but let's verify at runtime too
    assert_eq!(core::mem::size_of::<TokenDataZc>(), 165);
    assert_eq!(core::mem::align_of::<TokenDataZc>(), 1);
}

#[test]
fn mint_account_size_assertion() {
    assert_eq!(core::mem::size_of::<MintDataZc>(), 82);
    assert_eq!(core::mem::align_of::<MintDataZc>(), 1);
}

#[test]
fn token_deref_then_lamport_write_then_reread() {
    // Lifecycle: read token data, write lamports, re-read token data
    let (mut buf, _data) = token_account_buffer(42);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Read token state
    assert_eq!(account.amount(), 42);

    // Modify lamports (different region of RuntimeAccount)
    set_lamports(account.to_account_view(), 0);

    // Re-read token state; it should be unaffected.
    assert_eq!(account.amount(), 42);
    assert_eq!(account.to_account_view().lamports(), 0);
}

#[test]
fn maybeunit_init_then_read_every_byte_transfer() {
    // Verify every byte of the 9-byte transfer buffer is deterministic
    let amount: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 3u8);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    // Read every byte individually
    for i in 0..9 {
        let _ = data[i];
    }
    assert_eq!(data[0], 3);
    let amount_bytes = amount.to_le_bytes();
    for i in 0..8 {
        assert_eq!(data[i + 1], amount_bytes[i]);
    }
}

#[test]
fn maybeunit_init_then_read_every_byte_initialize_mint() {
    // The largest MaybeUninit buffer: 67 bytes for initialize_mint2
    let mint_auth = [0xAA; 32];
    let freeze_auth = [0xBB; 32];

    // With freeze authority
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20u8);
        core::ptr::write(ptr.add(1), 9u8);
        core::ptr::copy_nonoverlapping(mint_auth.as_ptr(), ptr.add(2), 32);
        core::ptr::write(ptr.add(34), 1u8);
        core::ptr::copy_nonoverlapping(freeze_auth.as_ptr(), ptr.add(35), 32);
        buf.assume_init()
    };
    // Read every byte; Miri will flag any uninitialized byte.
    for i in 0..67 {
        let _ = data[i];
    }
}

#[test]
fn maybeunit_init_then_read_every_byte_initialize_account() {
    let owner = [0xCC; 32];
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 18u8);
        core::ptr::copy_nonoverlapping(owner.as_ptr(), ptr.add(1), 32);
        buf.assume_init()
    };
    // Read every byte
    for i in 0..33 {
        let _ = data[i];
    }
}

#[test]
fn maybeunit_init_then_read_every_byte_transfer_checked() {
    let amount: u64 = 0x0102030405060708;
    let decimals: u8 = 18;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 10]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 12u8);
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        core::ptr::write(ptr.add(9), decimals);
        buf.assume_init()
    };
    // Read every byte
    for i in 0..10 {
        let _ = data[i];
    }
    assert_eq!(data[9], 18);
}

#[test]
fn keys_eq_spl_token_id() {
    // Verify the SPL_TOKEN_ID constant matches expected bytes
    assert!(quasar_lang::keys_eq(
        &SPL_TOKEN_ID,
        &Address::new_from_array(SPL_TOKEN_BYTES)
    ));
    assert!(!quasar_lang::keys_eq(
        &SPL_TOKEN_ID,
        &Address::new_from_array(TOKEN_2022_BYTES)
    ));
}

#[test]
fn keys_eq_token_2022_id() {
    assert!(quasar_lang::keys_eq(
        &TOKEN_2022_ID,
        &Address::new_from_array(TOKEN_2022_BYTES)
    ));
    assert!(!quasar_lang::keys_eq(
        &TOKEN_2022_ID,
        &Address::new_from_array(SPL_TOKEN_BYTES)
    ));
}

#[test]
fn multiple_interface_accounts_from_different_buffers() {
    // Two separate buffers, two separate InterfaceAccount views
    let data1 = build_simple_token_data(111);
    let data2 = build_simple_token_data(222);

    let mut buf1 = AccountBuffer::new(165);
    buf1.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf1.write_data(&data1);

    let mut buf2 = AccountBuffer::new(165);
    buf2.init([2u8; 32], TOKEN_2022_OWNER, 2_000_000, 165, false, true);
    buf2.write_data(&data2);

    let view1 = unsafe { buf1.view() };
    let view2 = unsafe { buf2.view() };

    let iface1 = InterfaceAccount::<Token>::from_account_view(&view1).unwrap();
    let iface2 = InterfaceAccount::<Token>::from_account_view(&view2).unwrap();

    assert_eq!(iface1.amount(), 111);
    assert_eq!(iface2.amount(), 222);

    // Cross-read doesn't interfere
    assert_eq!(iface1.amount(), 111);
    assert_eq!(iface2.amount(), 222);
}

#[test]
fn account_view_owner_read_for_interface_check() {
    // Tests the view.owner() call path used in InterfaceAccount::from_account_view
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };

    // Explicitly test the owner read
    let owner = view.owner();
    assert!(quasar_lang::keys_eq(owner, &SPL_TOKEN_ID));
}

#[test]
fn account_view_owner_read_token_2022() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], TOKEN_2022_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let owner = view.owner();
    assert!(quasar_lang::keys_eq(owner, &TOKEN_2022_ID));
}

#[test]
fn token_deref_after_lamport_drain() {
    // Simulate closing: drain lamports, then read token data
    let (mut buf, _data) = token_account_buffer(42);
    let mut view = unsafe { buf.view() };

    let account = unsafe { Account::<Token>::from_account_view_unchecked_mut(&mut view) };

    // Drain lamports
    set_lamports(account.to_account_view(), 0);

    // Token data should still be readable (account data region unchanged)
    assert_eq!(account.amount(), 42);
}

#[test]
fn interleaved_token_and_mint_deref() {
    // Create both a token and mint buffer, interleave reads
    let (mut token_buf, _) = token_account_buffer(500);
    let (mut mint_buf, _) = mint_account_buffer(1_000_000, 6);

    let token_view = unsafe { token_buf.view() };
    let mint_view = unsafe { mint_buf.view() };

    let token_acct = unsafe { Account::<Token>::from_account_view_unchecked(&token_view) };
    let mint_acct = unsafe { Account::<Mint>::from_account_view_unchecked(&mint_view) };

    // Interleave reads between the two
    for _ in 0..20 {
        assert_eq!(token_acct.amount(), 500);
        assert_eq!(mint_acct.supply(), 1_000_000);
        assert_eq!(mint_acct.decimals(), 6);
        let _ = token_acct.mint();
        let _ = mint_acct.mint_authority();
    }
}

#[test]
fn spl_token_id_and_token_2022_id_differ() {
    // Verify the two program IDs are distinct (last byte differs)
    assert!(!quasar_lang::keys_eq(&SPL_TOKEN_ID, &TOKEN_2022_ID));
    // Verify specific byte difference
    assert_ne!(SPL_TOKEN_BYTES[31], TOKEN_2022_BYTES[31]);
}
