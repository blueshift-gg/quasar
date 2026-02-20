extern crate std;

use alloc::vec;
use alloc::vec::Vec;
use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};

use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;
use solana_program_pack::Pack;
use spl_token_interface::state::Account as TokenAccount;

use crate::client::{MakeInstruction, RefundInstruction, TakeInstruction};

fn setup() -> Mollusk {
    let mut mollusk = Mollusk::new(&crate::ID, "../../target/deploy/quasar_escrow");
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
    let mut data = vec![0u8; 138]; // 1 disc + 32*4 + 8 + 1
    data[0] = 1; // EscrowAccount discriminator
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
    let (escrow, escrow_bump) =
        Address::find_program_address(&[b"escrow", maker.as_ref()], &crate::ID);
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

    let instruction: Instruction = MakeInstruction {
        maker,
        escrow,
        maker_ta_a,
        maker_ta_b,
        vault_ta_a,
        rent,
        token_program,
        system_program,
        deposit: 1337,
        receive: 1337,
    }
    .into();

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

    // Validate escrow state was written correctly
    let escrow_data = &result.resulting_accounts[1].1.data;
    assert_eq!(escrow_data.len(), 138, "escrow data length");
    assert_eq!(escrow_data[0], 1, "discriminator");
    assert_eq!(&escrow_data[1..33], maker.as_ref(), "maker");
    assert_eq!(&escrow_data[33..65], mint_a.as_ref(), "mint_a");
    assert_eq!(&escrow_data[65..97], mint_b.as_ref(), "mint_b");
    assert_eq!(&escrow_data[97..129], maker_ta_b.as_ref(), "maker_ta_b");
    assert_eq!(&escrow_data[129..137], &1337u64.to_le_bytes(), "receive");
    assert_eq!(escrow_data[137], escrow_bump, "bump");

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
        Address::find_program_address(&[b"escrow", maker.as_ref()], &crate::ID);
    let maker_ta_b = Address::new_unique();
    let escrow_account = Account {
        lamports: 2_000_000,
        data: build_escrow_data(maker, mint_a, mint_b, maker_ta_b, 1337, escrow_bump),
        owner: crate::ID,
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

    let instruction: Instruction = TakeInstruction {
        taker,
        escrow,
        maker,
        taker_ta_a,
        taker_ta_b,
        maker_ta_b,
        vault_ta_a,
        token_program,
    }
    .into();

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
        Address::find_program_address(&[b"escrow", maker.as_ref()], &crate::ID);
    let maker_ta_b = Address::new_unique();
    let escrow_account = Account {
        lamports: 2_000_000,
        data: build_escrow_data(maker, mint_a, mint_b, maker_ta_b, 1337, escrow_bump),
        owner: crate::ID,
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

    let instruction: Instruction = RefundInstruction {
        maker,
        escrow,
        maker_ta_a,
        vault_ta_a,
        token_program,
    }
    .into();

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
