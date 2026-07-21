use super::*;

#[test]
fn transfer_data_all_bytes_initialized() {
    let amount: u64 = 1_000_000;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 3u8); // TRANSFER opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 3);
    assert_eq!(
        u64::from_le_bytes(data[1..9].try_into().unwrap()),
        1_000_000
    );
}

#[test]
fn transfer_boundary_amounts() {
    for &amount in &[0u64, 1, u64::MAX] {
        let data = unsafe {
            let mut buf = MaybeUninit::<[u8; 9]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 3u8);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            buf.assume_init()
        };
        assert_eq!(data[0], 3);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), amount);
    }
}

#[test]
fn mint_to_data_initialized() {
    let amount: u64 = 999_999;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 7u8); // MINT_TO opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 7);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 999_999);
}

#[test]
fn approve_data_initialized() {
    let amount: u64 = 500;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 4u8); // APPROVE opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 4);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 500);
}

#[test]
fn burn_data_initialized() {
    let amount: u64 = u64::MAX;
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 9]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 8u8); // BURN opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        buf.assume_init()
    };
    assert_eq!(data[0], 8);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), u64::MAX);
}

#[test]
fn revoke_data_initialized() {
    // Revoke is a single byte, so no MaybeUninit is needed.
    let data: [u8; 1] = [5u8]; // REVOKE opcode
    assert_eq!(data[0], 5);
}

#[test]
fn close_account_data_initialized() {
    // Close account is a single byte
    let data: [u8; 1] = [9u8]; // CLOSE_ACCOUNT opcode
    assert_eq!(data[0], 9);
}

#[test]
fn initialize_account_data_initialized() {
    let owner = Address::new_from_array([0xDD; 32]);
    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 33]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 18u8); // INITIALIZE_ACCOUNT3 opcode
        core::ptr::copy_nonoverlapping(owner.as_ref().as_ptr(), ptr.add(1), 32);
        buf.assume_init()
    };
    assert_eq!(data[0], 18);
    assert_eq!(&data[1..33], &[0xDD; 32]);
}

#[test]
fn initialize_mint_data_with_freeze_authority() {
    let mint_authority = Address::new_from_array([0xAA; 32]);
    let freeze_authority = Address::new_from_array([0xBB; 32]);
    let decimals: u8 = 9;

    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20u8); // INITIALIZE_MINT2 opcode
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        // With freeze authority
        core::ptr::write(ptr.add(34), 1u8); // COption::Some
        core::ptr::copy_nonoverlapping(freeze_authority.as_ref().as_ptr(), ptr.add(35), 32);
        buf.assume_init()
    };
    assert_eq!(data[0], 20);
    assert_eq!(data[1], 9);
    assert_eq!(&data[2..34], &[0xAA; 32]);
    assert_eq!(data[34], 1); // COption::Some tag
    assert_eq!(&data[35..67], &[0xBB; 32]);
}

#[test]
fn initialize_mint_data_without_freeze_authority() {
    let mint_authority = Address::new_from_array([0xAA; 32]);
    let decimals: u8 = 6;

    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 67]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 20u8);
        core::ptr::write(ptr.add(1), decimals);
        core::ptr::copy_nonoverlapping(mint_authority.as_ref().as_ptr(), ptr.add(2), 32);
        // Without freeze authority, zero the remaining 33 bytes.
        core::ptr::write_bytes(ptr.add(34), 0, 33);
        buf.assume_init()
    };
    assert_eq!(data[0], 20);
    assert_eq!(data[1], 6);
    assert_eq!(&data[2..34], &[0xAA; 32]);
    // All zeros for COption::None + padding
    assert!(data[34..67].iter().all(|&b| b == 0));
}

#[test]
fn transfer_checked_data_initialized() {
    let amount: u64 = 42_000;
    let decimals: u8 = 9;

    let data = unsafe {
        let mut buf = MaybeUninit::<[u8; 10]>::uninit();
        let ptr = buf.as_mut_ptr() as *mut u8;
        core::ptr::write(ptr, 12u8); // transfer-checked opcode
        core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
        core::ptr::write(ptr.add(9), decimals);
        buf.assume_init()
    };
    assert_eq!(data[0], 12);
    assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), 42_000);
    assert_eq!(data[9], 9);
}

#[test]
fn transfer_checked_boundary_values() {
    for &(amount, decimals) in &[(0u64, 0u8), (u64::MAX, 255), (1, 18)] {
        let data = unsafe {
            let mut buf = MaybeUninit::<[u8; 10]>::uninit();
            let ptr = buf.as_mut_ptr() as *mut u8;
            core::ptr::write(ptr, 12u8);
            core::ptr::copy_nonoverlapping(amount.to_le_bytes().as_ptr(), ptr.add(1), 8);
            core::ptr::write(ptr.add(9), decimals);
            buf.assume_init()
        };
        assert_eq!(data[0], 12);
        assert_eq!(u64::from_le_bytes(data[1..9].try_into().unwrap()), amount);
        assert_eq!(data[9], decimals);
    }
}

#[test]
fn sync_native_data_initialized() {
    let data: [u8; 1] = [17u8]; // SYNC_NATIVE opcode
    assert_eq!(data[0], 17);
}
