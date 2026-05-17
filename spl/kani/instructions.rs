use quasar_lang::prelude::Address;

fn assert_amount_data<const DISCRIMINATOR: u8>(data: [u8; 9], amount: u64) {
    assert!(data[0] == DISCRIMINATOR);
    let amount_bytes = amount.to_le_bytes();
    let mut i: usize = 0;
    while i < 8 {
        assert!(data[1 + i] == amount_bytes[i]);
        i += 1;
    }
}

fn assert_address_at<const OFFSET: usize>(data: &[u8], expected: &[u8; 32]) {
    let mut i: usize = 0;
    while i < 32 {
        assert!(data[OFFSET + i] == expected[i]);
        i += 1;
    }
}

/// Prove that the `transfer` instruction data layout is correct for all
/// possible `amount` values.
#[kani::proof]
fn transfer_instruction_layout() {
    let amount: u64 = kani::any();

    let data = super::amount_data::<3>(amount);
    assert_amount_data::<3>(data, amount);
}

/// Prove that the `mint_to` instruction data layout is correct for all
/// possible `amount` values.
#[kani::proof]
fn mint_to_instruction_layout() {
    let amount: u64 = kani::any();

    let data = super::amount_data::<7>(amount);
    assert_amount_data::<7>(data, amount);
}

/// Prove that the `burn` instruction data layout is correct for all
/// possible `amount` values.
#[kani::proof]
fn burn_instruction_layout() {
    let amount: u64 = kani::any();

    let data = super::amount_data::<8>(amount);
    assert_amount_data::<8>(data, amount);
}

/// Prove that the `approve` instruction data layout is correct for all
/// possible `amount` values.
#[kani::proof]
fn approve_instruction_layout() {
    let amount: u64 = kani::any();

    let data = super::amount_data::<4>(amount);
    assert_amount_data::<4>(data, amount);
}

/// Prove that the `transfer_checked` instruction data layout is correct
/// for all possible `amount` and `decimals` values.
#[kani::proof]
fn transfer_checked_instruction_layout() {
    let amount: u64 = kani::any();
    let decimals: u8 = kani::any();

    let data = super::checked_amount_data::<12>(amount, decimals);

    assert!(data[0] == 12u8);
    let amount_bytes = amount.to_le_bytes();
    let mut i: usize = 0;
    while i < 8 {
        assert!(data[1 + i] == amount_bytes[i]);
        i += 1;
    }
    assert!(data[9] == decimals);
}

/// Prove that the `initialize_account3` instruction data layout is
/// correct for all possible owner addresses.
#[kani::proof]
fn initialize_account3_instruction_layout() {
    let owner: [u8; 32] = kani::any();
    let owner_address = Address::new_from_array(owner);

    let data = super::initialize_account3_data(&owner_address);

    assert!(data[0] == 18u8);
    assert_address_at::<1>(&data, &owner);
}

/// Prove that the `initialize_mint2` instruction data layout is correct
/// when a freeze authority is provided.
#[kani::proof]
fn initialize_mint2_instruction_layout_with_freeze() {
    let decimals: u8 = kani::any();
    let mint_authority: [u8; 32] = kani::any();
    let freeze_authority: [u8; 32] = kani::any();
    let mint_authority_address = Address::new_from_array(mint_authority);
    let freeze_authority_address = Address::new_from_array(freeze_authority);

    let data = super::initialize_mint2_data(
        decimals,
        &mint_authority_address,
        Some(&freeze_authority_address),
    );

    assert!(data[0] == 20u8);
    assert!(data[1] == decimals);
    assert_address_at::<2>(&data, &mint_authority);
    assert!(data[34] == 1u8);
    assert_address_at::<35>(&data, &freeze_authority);
}

/// Prove that the `initialize_mint2` instruction data layout is correct
/// when no freeze authority is provided.
#[kani::proof]
fn initialize_mint2_instruction_layout_without_freeze() {
    let decimals: u8 = kani::any();
    let mint_authority: [u8; 32] = kani::any();
    let mint_authority_address = Address::new_from_array(mint_authority);

    let data = super::initialize_mint2_data(decimals, &mint_authority_address, None);

    assert!(data[0] == 20u8);
    assert!(data[1] == decimals);
    assert_address_at::<2>(&data, &mint_authority);
    assert!(data[34] == 0u8);
    let mut i: usize = 0;
    while i < 32 {
        assert!(data[35 + i] == 0u8);
        i += 1;
    }
}
