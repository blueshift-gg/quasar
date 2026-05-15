use {
    super::*,
    crate::cpi::{AccountBuffer, MIN_ACCOUNT_BUF},
};

#[kani::proof]
fn transfer_data_layout() {
    let lamports: u64 = kani::any();

    let mut buf_from = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf_from.init([1; 32], [0; 32], 0, true, true, false);
    let from = unsafe { buf_from.view() };

    let mut buf_to = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf_to.init([2; 32], [0; 32], 0, false, true, false);
    let to = unsafe { buf_to.view() };

    let cpi = transfer(&from, &to, lamports);
    let data = cpi.instruction_data();
    assert!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]) == IX_TRANSFER as u32);
    assert!(
        u64::from_le_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]) == lamports
    );
}

#[kani::proof]
#[kani::unwind(33)]
fn assign_data_layout() {
    let owner_bytes: [u8; 32] = kani::any();
    let owner = solana_address::Address::new_from_array(owner_bytes);

    let mut buf = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf.init([1; 32], [0; 32], 0, true, true, false);
    let acct = unsafe { buf.view() };

    let cpi = assign(&acct, &owner);
    let data = cpi.instruction_data();
    assert!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]) == IX_ASSIGN as u32);

    let mut i = 0;
    while i < 32 {
        assert!(data[4 + i] == owner_bytes[i]);
        i += 1;
    }
}

#[kani::proof]
fn allocate_data_layout() {
    let space: u64 = kani::any();

    let mut buf = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf.init([1; 32], [0; 32], 0, true, true, false);
    let acct = unsafe { buf.view() };

    let cpi = allocate(&acct, space);
    let data = cpi.instruction_data();
    assert!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]) == IX_ALLOCATE as u32);
    assert!(
        u64::from_le_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]) == space
    );
}

#[kani::proof]
#[kani::unwind(33)]
fn create_account_data_layout() {
    let lamports: u64 = kani::any();
    let space: u64 = kani::any();
    let owner_bytes: [u8; 32] = kani::any();
    let owner = solana_address::Address::new_from_array(owner_bytes);

    let mut buf_from = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf_from.init([1; 32], [0; 32], 0, true, true, false);
    let from = unsafe { buf_from.view() };

    let mut buf_to = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    buf_to.init([2; 32], [0; 32], 0, true, true, false);
    let to = unsafe { buf_to.view() };

    let cpi = create_account(&from, &to, lamports, space, &owner);
    let data = cpi.instruction_data();

    assert!(u32::from_le_bytes([data[0], data[1], data[2], data[3]]) == IX_CREATE_ACCOUNT as u32);
    assert!(
        u64::from_le_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ]) == lamports
    );
    assert!(
        u64::from_le_bytes([
            data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
        ]) == space
    );

    let mut i = 0;
    while i < 32 {
        assert!(data[20 + i] == owner_bytes[i]);
        i += 1;
    }
}
