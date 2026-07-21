use super::*;

#[test]
fn interface_account_cast_spl_token_owner() {
    let data = build_simple_token_data(42);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view(&view).unwrap();

    // Deref through InterfaceAccount -> TokenDataZc
    assert_eq!(iface.amount(), 42);
    assert_eq!(iface.mint(), &Address::new_from_array([0xAA; 32]));
}

#[test]
fn interface_account_cast_token_2022_owner() {
    let data = build_simple_token_data(77);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], TOKEN_2022_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view(&view).unwrap();

    assert_eq!(iface.amount(), 77);
}

#[test]
fn interface_account_mut_cast() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, true, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view_mut(&mut view).unwrap();

    // Read through &mut InterfaceAccount
    assert_eq!(iface.amount(), 100);

    // Write through DerefMut -> &mut TokenDataZc
    let state: &mut TokenDataZc = &mut *iface;
    unsafe {
        let amount_ptr = (state as *mut TokenDataZc as *mut u8).add(64);
        let new_amount: u64 = 200;
        core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
    }

    assert_eq!(iface.amount(), 200);
}

#[test]
fn interface_account_aliasing() {
    // &mut view -> &mut InterfaceAccount<Token>, interleaved R/W
    let data = build_simple_token_data(50);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Token>::from_account_view_mut(&mut view).unwrap();

    // Interleaved access uses iface.to_account_view() to avoid
    // reborrowing `view` while `iface` holds a mutable borrow.
    assert_eq!(iface.amount(), 50);
    assert_eq!(iface.to_account_view().lamports(), 1_000_000);

    set_lamports(iface.to_account_view(), 2_000_000);
    assert_eq!(iface.to_account_view().lamports(), 2_000_000);

    // Rapid interleaving through the wrapper
    for _ in 0..20 {
        let _ = iface.to_account_view().lamports();
        let _ = iface.amount();
        let _ = iface.to_account_view().data_len();
        let _ = iface.mint();
    }
}

#[test]
fn interface_account_wrong_owner_rejected() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    // Use a random owner that is neither SPL Token nor Token-2022
    buf.init([1u8; 32], [0xFF; 32], 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let result = InterfaceAccount::<Token>::from_account_view(&view);
    match result {
        Err(e) => assert_eq!(e, ProgramError::IllegalOwner),
        Ok(_) => panic!("expected IllegalOwner"),
    }
}

#[test]
fn interface_account_immutable_rejected() {
    let data = build_simple_token_data(100);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, false); // NOT writable
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let result = InterfaceAccount::<Token>::from_account_view_mut(&mut view);
    match result {
        Err(e) => assert_eq!(e, ProgramError::Immutable),
        Ok(_) => panic!("expected Immutable"),
    }
}

#[test]
fn interface_account_data_too_small() {
    // Only 100 bytes of data, but TokenDataZc needs 165
    let mut buf = AccountBuffer::new(100);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 100, false, true);

    let view = unsafe { buf.view() };
    let result = InterfaceAccount::<Token>::from_account_view(&view);
    match result {
        Err(e) => assert_eq!(e, ProgramError::AccountDataTooSmall),
        Ok(_) => panic!("expected AccountDataTooSmall"),
    }
}

#[test]
fn interface_account_mint_spl_token() {
    let data = build_simple_mint_data(1_000_000, 6);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], SPL_TOKEN_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Mint>::from_account_view(&view).unwrap();
    assert_eq!(iface.supply(), 1_000_000);
    assert_eq!(iface.decimals(), 6);
}

#[test]
fn interface_account_mint_token_2022() {
    let data = build_simple_mint_data(999, 9);
    let mut buf = AccountBuffer::new(82);
    buf.init([2u8; 32], TOKEN_2022_OWNER, 1_000_000, 82, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = InterfaceAccount::<Mint>::from_account_view(&view).unwrap();
    assert_eq!(iface.supply(), 999);
    assert_eq!(iface.decimals(), 9);
}

#[test]
fn interface_account_unchecked_cast() {
    let data = build_simple_token_data(77);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let iface = unsafe { InterfaceAccount::<Token>::from_account_view_unchecked(&view) };
    assert_eq!(iface.amount(), 77);
}

#[test]
fn interface_account_unchecked_mut_cast() {
    let data = build_simple_token_data(88);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let mut view = unsafe { buf.view() };
    let iface = unsafe { InterfaceAccount::<Token>::from_account_view_unchecked_mut(&mut view) };
    assert_eq!(iface.amount(), 88);
}
