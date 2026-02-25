extern crate std;

use {
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_pack::Pack,
    spl_token_interface::state::Account as TokenAccount,
    std::{vec, vec::Vec},
};

fn program_id() -> Address {
    Address::new_from_array(crate::ID_BYTES)
}

fn setup() -> Mollusk {
    let mut mollusk = Mollusk::new(&program_id(), "../../target/deploy/pinocchio_escrow");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk
}

fn pack_token(mint: Address, owner: Address, amount: u64) -> Vec<u8> {
    let token = TokenAccount {
        mint,
        owner,
        amount,
        delegate: None.into(),
        state: spl_token_interface::state::AccountState::Initialized,
        is_native: None.into(),
        delegated_amount: 0,
        close_authority: None.into(),
    };
    let mut data = vec![0u8; TokenAccount::LEN];
    Pack::pack(token, &mut data).unwrap();
    data
}

fn build_escrow_data(
    maker: Address,
    mint_a: Address,
    mint_b: Address,
    maker_ta_b: Address,
    receive: u64,
    bump: u8,
) -> Vec<u8> {
    let mut data = vec![0u8; 138];
    data[0] = 1; // discriminator
    data[1..33].copy_from_slice(maker.as_ref());
    data[33..65].copy_from_slice(mint_a.as_ref());
    data[65..97].copy_from_slice(mint_b.as_ref());
    data[97..129].copy_from_slice(maker_ta_b.as_ref());
    data[129..137].copy_from_slice(&receive.to_le_bytes());
    data[137] = bump;
    data
}

#[test]
fn test_make_cu() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let maker_account = Account::new(1_000_000_000, 0, &system_program);
    let (escrow, _) = Address::find_program_address(&[b"escrow", maker.as_ref()], &program_id());
    let escrow_account = Account::default();

    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, maker, 1_000_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let maker_ta_b = Address::new_unique();
    let maker_ta_b_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_b, maker, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, escrow, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    // Instruction data: [disc=0, deposit=1337u64, receive=1337u64]
    let mut ix_data = vec![0u8];
    ix_data.extend_from_slice(&1337u64.to_le_bytes());
    ix_data.extend_from_slice(&1337u64.to_le_bytes());

    let instruction = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(escrow, false),
            AccountMeta::new(maker_ta_a, false),
            AccountMeta::new_readonly(maker_ta_b, false),
            AccountMeta::new(vault_ta_a, false),
            AccountMeta::new_readonly(rent, false),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: ix_data,
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (maker_ta_a, maker_ta_a_account),
            (maker_ta_b, maker_ta_b_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "make failed: {:?}",
        result.program_result
    );
    std::println!("\n========================================");
    std::println!("  MAKE CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_take_cu() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, _) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let taker = Address::new_unique();
    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let (escrow, escrow_bump) =
        Address::find_program_address(&[b"escrow", maker.as_ref()], &program_id());
    let maker_ta_b = Address::new_unique();
    let escrow_account = Account {
        lamports: 2_000_000,
        data: build_escrow_data(maker, mint_a, mint_b, maker_ta_b, 1337, escrow_bump),
        owner: program_id(),
        executable: false,
        rent_epoch: 0,
    };

    let maker_account = Account::new(1_000_000, 0, &system_program);
    let taker_account = Account::new(1_000_000_000, 0, &system_program);

    let taker_ta_a = Address::new_unique();
    let taker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, taker, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let taker_ta_b = Address::new_unique();
    let taker_ta_b_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_b, taker, 10_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let maker_ta_b_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_b, maker, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, escrow, 1337),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Instruction data: [disc=1]
    let instruction = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(taker, true),
            AccountMeta::new(escrow, false),
            AccountMeta::new(maker, false),
            AccountMeta::new(taker_ta_a, false),
            AccountMeta::new(taker_ta_b, false),
            AccountMeta::new(maker_ta_b, false),
            AccountMeta::new(vault_ta_a, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: vec![1],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (taker, taker_account),
            (escrow, escrow_account),
            (maker, maker_account),
            (taker_ta_a, taker_ta_a_account),
            (taker_ta_b, taker_ta_b_account),
            (maker_ta_b, maker_ta_b_account),
            (vault_ta_a, vault_ta_a_account),
            (token_program, token_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "take failed: {:?}",
        result.program_result
    );
    std::println!("\n========================================");
    std::println!("  TAKE CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}

#[test]
fn test_refund_cu() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, _) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let (escrow, escrow_bump) =
        Address::find_program_address(&[b"escrow", maker.as_ref()], &program_id());
    let maker_ta_b = Address::new_unique();
    let escrow_account = Account {
        lamports: 2_000_000,
        data: build_escrow_data(maker, mint_a, mint_b, maker_ta_b, 1337, escrow_bump),
        owner: program_id(),
        executable: false,
        rent_epoch: 0,
    };

    let maker_account = Account::new(1_000_000_000, 0, &system_program);

    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, maker, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, escrow, 1337),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Instruction data: [disc=2]
    let instruction = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(escrow, false),
            AccountMeta::new(maker_ta_a, false),
            AccountMeta::new(vault_ta_a, false),
            AccountMeta::new_readonly(token_program, false),
        ],
        data: vec![2],
    };

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (maker_ta_a, maker_ta_a_account),
            (vault_ta_a, vault_ta_a_account),
            (token_program, token_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "refund failed: {:?}",
        result.program_result
    );
    std::println!("\n========================================");
    std::println!("  REFUND CU: {}", result.compute_units_consumed);
    std::println!("========================================\n");
}
