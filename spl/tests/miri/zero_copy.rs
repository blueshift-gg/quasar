use super::*;

#[test]
fn zero_copy_deref_from_token() {
    let (mut buf, _data) = token_account_buffer(12345);
    let view = unsafe { buf.view() };

    let state = unsafe { <Token as ZeroCopyDeref>::deref_from(&view) };
    assert_eq!(state.amount(), 12345);
    assert_eq!(state.mint(), &Address::new_from_array([0xAA; 32]));
    assert_eq!(state.owner(), &Address::new_from_array([0xBB; 32]));
}

#[test]
fn zero_copy_deref_from_mut_token() {
    let (mut buf, _data) = token_account_buffer(500);
    let mut view = unsafe { buf.view() };

    let state = unsafe { <Token as ZeroCopyDeref>::deref_from_mut(&mut view) };

    // Read
    assert_eq!(state.amount(), 500);

    // Write through mut reference
    unsafe {
        let amount_ptr = (state as *mut TokenDataZc as *mut u8).add(64);
        let new_amount: u64 = 777;
        core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
    }
    assert_eq!(state.amount(), 777);
}

#[test]
fn zero_copy_deref_from_mint() {
    let (mut buf, _data) = mint_account_buffer(1_000_000, 6);
    let view = unsafe { buf.view() };

    let state = unsafe { <Mint as ZeroCopyDeref>::deref_from(&view) };
    assert_eq!(state.supply(), 1_000_000);
    assert_eq!(state.decimals(), 6);
}

#[test]
fn zero_copy_deref_from_mut_mint() {
    let (mut buf, _data) = mint_account_buffer(100, 9);
    let mut view = unsafe { buf.view() };

    let state = unsafe { <Mint as ZeroCopyDeref>::deref_from_mut(&mut view) };
    assert_eq!(state.supply(), 100);

    // Write supply
    unsafe {
        let supply_ptr = (state as *mut MintDataZc as *mut u8).add(36);
        let new_supply: u64 = 42;
        core::ptr::copy_nonoverlapping(new_supply.to_le_bytes().as_ptr(), supply_ptr, 8);
    }
    assert_eq!(state.supply(), 42);
}

#[test]
fn zero_copy_deref_from_exact_boundary() {
    // Exactly 165 bytes tests boundary alignment of the cast.
    let data = build_simple_token_data(u64::MAX);
    let mut buf = AccountBuffer::new(165);
    buf.init([1u8; 32], SPL_TOKEN_OWNER, 1_000_000, 165, false, true);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let state = unsafe { <Token as ZeroCopyDeref>::deref_from(&view) };
    assert_eq!(state.amount(), u64::MAX);
}

#[test]
fn zero_copy_deref_aliased_read_after_mut() {
    // Get &mut via deref_from_mut, write, drop it, then get & via deref_from.
    let (mut buf, _data) = token_account_buffer(300);
    let mut view = unsafe { buf.view() };

    {
        let state_mut = unsafe { <Token as ZeroCopyDeref>::deref_from_mut(&mut view) };
        assert_eq!(state_mut.amount(), 300);

        // Write through mut
        unsafe {
            let amount_ptr = (state_mut as *mut TokenDataZc as *mut u8).add(64);
            let new_amount: u64 = 600;
            core::ptr::copy_nonoverlapping(new_amount.to_le_bytes().as_ptr(), amount_ptr, 8);
        }
    }

    // Read through a fresh deref_from to check that the write is visible.
    let state_shared = unsafe { <Token as ZeroCopyDeref>::deref_from(&view) };
    assert_eq!(state_shared.amount(), 600);
}
