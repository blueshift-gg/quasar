use super::*;

#[test]
fn adversarial_all_zero_buffer() {
    let exact = size_of::<RuntimeAccount>();
    let mut buf = AccountBuffer::exact(exact);
    // Leave everything zero -- borrow_state=0 means mutably borrowed.
    let view = unsafe { buf.view() };

    assert_eq!(view.data_len(), 0);
    assert_eq!(view.lamports(), 0);
    assert!(!view.is_signer());
    assert!(!view.is_writable());
    assert!(!view.executable());
}

#[test]
fn adversarial_max_lamports() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], u64::MAX, 0, true, true);
    let view = unsafe { buf.view() };
    assert_eq!(view.lamports(), u64::MAX);
    set_lamports(&view, u64::MAX);
    assert_eq!(view.lamports(), u64::MAX);
    set_lamports(&view, 0);
    assert_eq!(view.lamports(), 0);
}

#[test]
fn adversarial_interleaved_close_write_read() {
    let data_len = 16usize;
    let mut src_buf = AccountBuffer::new(data_len);
    src_buf.init(
        [1u8; 32],
        TEST_OWNER.to_bytes(),
        500_000,
        data_len as u64,
        false,
        true,
    );
    let mut data = vec![0u8; data_len];
    data[0] = 0x01;
    src_buf.write_data(&data);

    let mut other_buf = AccountBuffer::new(16);
    other_buf.init([3u8; 32], TEST_OWNER.to_bytes(), 999, 16, false, true);

    let mut dst_buf = AccountBuffer::new(0);
    dst_buf.init([2u8; 32], [0u8; 32], 100, 0, false, true);

    let mut src_view = unsafe { src_buf.view() };
    let other_view = unsafe { other_buf.view() };
    let dst_view = unsafe { dst_buf.view() };

    let account =
        unsafe { Account::<TestCloseableType>::from_account_view_unchecked_mut(&mut src_view) };
    account.close(&dst_view).unwrap();

    assert_eq!(src_view.lamports(), 0);
    assert_eq!(other_view.lamports(), 999);
    set_lamports(&other_view, 888);
    assert_eq!(other_view.lamports(), 888);
}

#[test]
fn adversarial_remaining_zero_data_len_all() {
    let entries: Vec<_> = (0..8).map(|i| MultiAccountEntry::account(i, 0)).collect();
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 8);
    for v in &views {
        assert_eq!(v.data_len(), 0);
    }
}

#[test]
fn adversarial_remaining_all_duplicates_preserved() {
    let mut entries = vec![MultiAccountEntry::account(0x01, 8)];
    for _ in 0..7 {
        entries.push(MultiAccountEntry::duplicate(0));
    }
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 8);
    let first_addr = *views[0].address();
    for v in &views[1..] {
        assert_eq!(v.address(), &first_addr);
    }
}

#[test]
fn adversarial_remaining_all_duplicates_preserve_order() {
    let mut entries = vec![MultiAccountEntry::account(0x01, 8)];
    for _ in 0..7 {
        entries.push(MultiAccountEntry::duplicate(0));
    }
    let mut buf = MultiAccountBuffer::new(&entries);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 8);
    let first_addr = *views[0].address();
    for v in &views[1..] {
        assert_eq!(v.address(), &first_addr);
    }
}

#[test]
fn adversarial_pod_alignment_is_one() {
    assert_eq!(align_of::<PodU64>(), 1);
    assert_eq!(align_of::<PodU32>(), 1);
    assert_eq!(align_of::<PodU16>(), 1);
    assert_eq!(align_of::<PodU128>(), 1);
    assert_eq!(align_of::<PodI64>(), 1);
    assert_eq!(align_of::<PodI32>(), 1);
    assert_eq!(align_of::<PodI16>(), 1);
    assert_eq!(align_of::<PodI128>(), 1);
    assert_eq!(align_of::<PodBool>(), 1);
}

#[test]
fn adversarial_transparent_wrapper_sizes() {
    assert_eq!(
        size_of::<Account<TestAccountType>>(),
        size_of::<AccountView>()
    );
    assert_eq!(
        align_of::<Account<TestAccountType>>(),
        align_of::<AccountView>()
    );
}

#[test]
fn adversarial_resize_to_max_permitted_data_increase() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 100, 0, false, true);
    let mut view = unsafe { buf.view() };

    resize(&mut view, MAX_PERMITTED_DATA_INCREASE).unwrap();
    assert_eq!(view.data_len(), MAX_PERMITTED_DATA_INCREASE);

    let result = resize(&mut view, MAX_PERMITTED_DATA_INCREASE + 1);
    assert!(result.is_err());
}

#[test]
fn adversarial_resize_ping_pong() {
    let mut buf = AccountBuffer::new(0);
    buf.init([1u8; 32], [0u8; 32], 100, 0, false, true);
    let mut view = unsafe { buf.view() };

    for _ in 0..20 {
        resize(&mut view, 100).unwrap();
        assert_eq!(view.data_len(), 100);
        let data = unsafe { view.borrow_unchecked() };
        assert!(data.iter().all(|&b| b == 0));
        resize(&mut view, 0).unwrap();
        assert_eq!(view.data_len(), 0);
    }
}

#[test]
fn adversarial_write_all_data_bytes_then_verify() {
    for &data_len in &[1usize, 7, 8, 15, 16, 31, 32, 64, 128, 255] {
        let mut buf = AccountBuffer::new(data_len);
        buf.init([1u8; 32], [0u8; 32], 100, data_len as u64, false, true);
        let mut view = unsafe { buf.view() };

        {
            let data = unsafe { view.borrow_unchecked_mut() };
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }
        }

        {
            let data = unsafe { view.borrow_unchecked() };
            assert_eq!(data.len(), data_len);
            for (i, &byte) in data.iter().enumerate() {
                assert_eq!(byte, (i % 256) as u8);
            }
        }
    }
}

#[test]
fn adversarial_remaining_iterator_varied_data_lengths() {
    let mut buf = MultiAccountBuffer::new(&[
        MultiAccountEntry::Full {
            address: [0x01; 32],
            owner: [0xAA; 32],
            lamports: 100,
            data_len: 3,
            data: Some(vec![0xFF; 3]),
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x02; 32],
            owner: [0xBB; 32],
            lamports: 200,
            data_len: 0,
            data: None,
            is_signer: false,
            is_writable: true,
        },
        MultiAccountEntry::Full {
            address: [0x03; 32],
            owner: [0xCC; 32],
            lamports: 300,
            data_len: 15,
            data: Some(vec![0xDD; 15]),
            is_signer: false,
            is_writable: true,
        },
    ]);
    let remaining = unsafe { RemainingAccounts::new(buf.as_mut_ptr(), buf.boundary(), &[]) };

    let views: Vec<_> = remaining.iter().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(views.len(), 3);
    assert_eq!(views[0].data_len(), 3);
    assert_eq!(views[1].data_len(), 0);
    assert_eq!(views[2].data_len(), 15);
}

#[test]
fn adversarial_cpi_create_account_boundary_space() {
    for &space in &[0u64, 1, u64::MAX] {
        let mut from_buf = AccountBuffer::new(0);
        from_buf.init([1u8; 32], [0u8; 32], 1_000_000, 0, true, true);
        let mut to_buf = AccountBuffer::new(0);
        to_buf.init([2u8; 32], [0u8; 32], 0, 0, true, true);
        let from = unsafe { from_buf.view() };
        let to = unsafe { to_buf.view() };
        let owner = Address::new_from_array([0xAA; 32]);

        let call = quasar_lang::cpi::system::create_account(&from, &to, 1u64, space, &owner);
        let data = call.instruction_data();
        assert_eq!(u64::from_le_bytes(data[12..20].try_into().unwrap()), space);
    }
}

#[test]
fn adversarial_dynamic_header_only_no_tail() {
    let mut buf = make_dyn_buffer_exact(b"", &[]);
    let view = unsafe { buf.view() };
    let data = unsafe { view.borrow_unchecked() };

    assert_eq!(data.len(), DYN_HEADER_SIZE + 4 + 4);

    let mut offset = DYN_HEADER_SIZE;
    let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(name_len, 0);

    let s = unsafe { core::str::from_utf8_unchecked(&data[offset..offset]) };
    assert_eq!(s, "");

    let tags_count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
    offset += 4;
    assert_eq!(tags_count, 0);

    let slice: &[Address] =
        unsafe { core::slice::from_raw_parts(data[offset..].as_ptr() as *const Address, 0) };
    assert_eq!(slice.len(), 0);
}
