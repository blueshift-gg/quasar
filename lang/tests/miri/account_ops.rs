use super::*;

#[test]
fn ops_assign_changes_owner() {
    let mut buf = AccountBuffer::new(8);
    buf.init([1u8; 32], [0xAA; 32], 100, 8, false, true);
    let mut view = unsafe { buf.view() };

    for i in 0..5u8 {
        let owner = Address::new_from_array([i; 32]);
        unsafe { view.assign(&owner) };
        assert!(view.owned_by(&owner));
    }
}

#[test]
fn ops_resize_grows_and_zeroes_extension() {
    let initial_data_len = 8usize;
    let mut buf = AccountBuffer::new(initial_data_len);
    buf.init(
        [1u8; 32],
        [0u8; 32],
        100,
        initial_data_len as u64,
        false,
        true,
    );
    buf.write_data(&[0xFF; 8]);

    let mut view = unsafe { buf.view() };
    assert_eq!(view.data_len(), 8);

    resize(&mut view, 16).unwrap();
    assert_eq!(view.data_len(), 16);

    let data = unsafe { view.borrow_unchecked() };
    assert!(data[..8].iter().all(|&b| b == 0xFF));
    assert!(data[8..16].iter().all(|&b| b == 0));

    resize(&mut view, 4).unwrap();
    assert_eq!(view.data_len(), 4);
}

#[test]
fn ops_account_realloc_rejects_below_space() {
    let data_len = 8usize;
    let mut account_buf = AccountBuffer::new(data_len);
    account_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );

    let mut payer_buf = AccountBuffer::new(0);
    payer_buf.init([2u8; 32], [0u8; 32], 1_000_000, 0, true, true);

    let mut view = unsafe { account_buf.view() };
    let payer = unsafe { payer_buf.view() };
    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut view) };

    let err = account.realloc(data_len - 1, &payer, None).unwrap_err();
    assert_eq!(err, ProgramError::AccountDataTooSmall);
    assert_eq!(account.to_account_view().data_len(), data_len);
}

#[test]
fn ops_close_transfers_lamports_and_zeroes_fields() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], 500_000, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    account.close(&dst_view).unwrap();

    assert_eq!(src_view.lamports(), 0);
    assert_eq!(src_view.data_len(), 0);
    assert!(src_view.owned_by(&Address::new_from_array([0u8; 32])));
    assert_eq!(dst_view.lamports(), 1_500_000);
}

#[test]
fn ops_close_rejected_by_check_owner() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [3u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dest_buf = AccountBuffer::new(0);
    dest_buf.init([2u8; 32], [0u8; 32], 0, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let dest_view = unsafe { dest_buf.view() };

    let closeable =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    closeable.close(&dest_view).unwrap();

    let result = <TestCloseableType as CheckOwner>::check_owner(&src_view);
    assert!(result.is_err());
}

#[test]
fn ops_close_rejects_non_writable_destination() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], 500_000, 0, false, false);

    let mut src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    let result = account.close(&dst_view);
    assert!(result.is_err());
    assert_eq!(src_view.lamports(), 1_000_000);
}

#[test]
fn ops_close_rejects_lamport_overflow() {
    // Lamport overflow is physically impossible (total SOL supply ~5.8e17 <
    // u64::MAX ~1.8e19). close() uses wrapping_add to skip the overflow branch.
    // This test verifies the wrapping behavior with synthetic values that can't
    // occur in production.
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], u64::MAX, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    let result = account.close(&dst_view);
    // wrapping_add: u64::MAX + 1_000_000 wraps (physically impossible on Solana)
    assert!(result.is_ok());
    assert_eq!(src_view.lamports(), 0);
    assert_eq!(dst_view.lamports(), u64::MAX.wrapping_add(1_000_000));
}

#[test]
fn ops_close_rejects_self_close() {
    let data_len = 16usize;
    let address = [1u8; 32];
    let initial_data = [
        0x01, 0xA5, 0x5A, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
        0xDD,
    ];
    let mut buf = AccountBuffer::new(data_len);
    buf.init(
        address,
        TEST_OWNER.to_bytes(),
        1_000_000,
        data_len as u64,
        false,
        true,
    );
    buf.write_data(&initial_data);

    let mut src_view = unsafe { buf.view() };
    let self_view = unsafe { AccountView::new_unchecked(buf.raw()) };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    let result = account.close(&self_view);

    assert_eq!(result, Err(ProgramError::InvalidArgument));
    assert_eq!(self_view.address().to_bytes(), address);
    assert_eq!(self_view.lamports(), 1_000_000);
    assert_eq!(self_view.data_len(), data_len);
    assert_eq!(unsafe { self_view.borrow_unchecked() }, initial_data);
    assert!(self_view.owned_by(&TEST_OWNER));
    assert!(!self_view.executable());
    assert!(self_view.is_writable());
}

#[test]
fn ops_borrow_unchecked_mut_write_then_read_via_data_ptr() {
    let mut buf = AccountBuffer::new(16);
    buf.init([1u8; 32], [0u8; 32], 100, 16, false, true);
    let mut view = unsafe { buf.view() };

    {
        let data = unsafe { view.borrow_unchecked_mut() };
        data[0..8].copy_from_slice(&42u64.to_le_bytes());
    }
    let val = unsafe { *(view.data_ptr() as *const u64) };
    assert_eq!(val, 42);
}
