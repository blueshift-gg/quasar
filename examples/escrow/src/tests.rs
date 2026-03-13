extern crate std;

use {
    crate::idl_client::{MakeInstruction, RefundInstruction, TakeInstruction},
    alloc::{vec, vec::Vec},
    mollusk_svm::{program::keyed_account_for_system_program, Mollusk},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::Instruction,
    solana_program_pack::Pack,
    spl_token_interface::state::{Account as TokenAccount, Mint},
};

/// Mark specific account indices as signers on an instruction.
/// Simulates a client including keypairs in the transaction for accounts
/// that need creation (init_if_needed with uninitialized accounts).
fn with_signers(mut ix: Instruction, indices: &[usize]) -> Instruction {
    for &i in indices {
        ix.accounts[i].is_signer = true;
    }
    ix
}

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

fn pack_mint(authority: Address, decimals: u8) -> Vec<u8> {
    let mint = Mint {
        mint_authority: Some(authority).into(),
        supply: 1_000_000_000,
        decimals,
        is_initialized: true,
        freeze_authority: None.into(),
    };
    let mut data = vec![0u8; Mint::LEN];
    Pack::pack(mint, &mut data).unwrap();
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
    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mint_b = Address::new_unique();
    let mint_b_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, maker, 1_000_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Uninitialized — init_if_needed will create + InitializeAccount3
    let maker_ta_b = Address::new_unique();
    let maker_ta_b_account = Account::new(0, 0, &system_program);

    // Uninitialized — init_if_needed will create + InitializeAccount3
    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account::new(0, 0, &system_program);

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    // Client builder marks maker as signer (client.rs:23)
    // Mark uninitialized accounts (maker_ta_b=5, vault_ta_a=6) as signers for
    // create_account CPI
    let instruction = with_signers(
        MakeInstruction {
            maker,
            escrow,
            mint_a,
            mint_b,
            maker_ta_a,
            maker_ta_b,
            vault_ta_a,
            rent,
            token_program,
            system_program,
            deposit: 1337,
            receive: 1337,
        }
        .into(),
        &[5, 6],
    );

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (mint_a, mint_a_account),
            (mint_b, mint_b_account),
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
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let taker = Address::new_unique();
    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mint_b_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

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

    // Uninitialized — init_if_needed will create + InitializeAccount3
    let taker_ta_a = Address::new_unique();
    let taker_ta_a_account = Account::new(0, 0, &system_program);

    let taker_ta_b = Address::new_unique();
    let taker_ta_b_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_b, taker, 10_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Uninitialized — init_if_needed will create + InitializeAccount3
    let maker_ta_b_account = Account::new(0, 0, &system_program);

    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, escrow, 1337),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    // Client builder marks taker as signer (client.rs:63)
    // Mark uninitialized accounts (taker_ta_a=5, maker_ta_b=7) as signers for
    // create_account CPI
    let instruction = with_signers(
        TakeInstruction {
            taker,
            escrow,
            maker,
            mint_a,
            mint_b,
            taker_ta_a,
            taker_ta_b,
            maker_ta_b,
            vault_ta_a,
            rent,
            token_program,
            system_program,
        }
        .into(),
        &[5, 7],
    );

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (taker, taker_account),
            (escrow, escrow_account),
            (maker, maker_account),
            (mint_a, mint_a_account),
            (mint_b, mint_b_account),
            (taker_ta_a, taker_ta_a_account),
            (taker_ta_b, taker_ta_b_account),
            (maker_ta_b, maker_ta_b_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
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
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

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

    // Uninitialized — init_if_needed will create + InitializeAccount3
    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account::new(0, 0, &system_program);

    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, escrow, 1337),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    // Client builder marks maker as signer (client.rs:99)
    // Mark uninitialized account (maker_ta_a=3) as signer for create_account CPI
    let instruction = with_signers(
        RefundInstruction {
            maker,
            escrow,
            mint_a,
            maker_ta_a,
            vault_ta_a,
            rent,
            token_program,
            system_program,
        }
        .into(),
        &[3],
    );

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (mint_a, mint_a_account),
            (maker_ta_a, maker_ta_a_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
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

// ---------------------------------------------------------------------------
// init_if_needed: pre-existing token accounts (validation branch)
// ---------------------------------------------------------------------------

#[test]
fn test_make_existing_token_accounts() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let maker_account = Account::new(1_000_000_000, 0, &system_program);
    let (escrow, _) = Address::find_program_address(&[b"escrow", maker.as_ref()], &crate::ID);
    let escrow_account = Account::default();

    let mint_a = Address::new_unique();
    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mint_b = Address::new_unique();
    let mint_b_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, maker, 1_000_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Already initialized — init_if_needed validates mint + owner, skips create
    let maker_ta_b = Address::new_unique();
    let maker_ta_b_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_b, maker, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Already initialized — init_if_needed validates mint + owner, skips create
    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_a, escrow, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let mut instruction: Instruction = MakeInstruction {
        maker,
        escrow,
        mint_a,
        mint_b,
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
    instruction.accounts[0].is_signer = true; // maker

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (mint_a, mint_a_account),
            (mint_b, mint_b_account),
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
        "make with existing token accounts failed: {:?}",
        result.program_result
    );
    std::println!(
        "  make with existing token accounts: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_make_existing_maker_ta_b_wrong_mint() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let maker_account = Account::new(1_000_000_000, 0, &system_program);
    let (escrow, _) = Address::find_program_address(&[b"escrow", maker.as_ref()], &crate::ID);
    let escrow_account = Account::default();

    let mint_a = Address::new_unique();
    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mint_b = Address::new_unique();
    let mint_b_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, maker, 1_000_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Wrong mint — should be mint_b but is mint_a
    let wrong_mint = mint_a;
    let maker_ta_b = Address::new_unique();
    let maker_ta_b_account = Account {
        lamports: 2_039_280,
        data: pack_token(wrong_mint, maker, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Pre-existing so the test reaches maker_ta_b validation
    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_a, escrow, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let mut instruction: Instruction = MakeInstruction {
        maker,
        escrow,
        mint_a,
        mint_b,
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
    instruction.accounts[0].is_signer = true; // maker

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (mint_a, mint_a_account),
            (mint_b, mint_b_account),
            (maker_ta_a, maker_ta_a_account),
            (maker_ta_b, maker_ta_b_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "make should fail with wrong mint on maker_ta_b"
    );
    std::println!("  make with wrong mint rejected: OK");
}

#[test]
fn test_make_existing_maker_ta_b_wrong_owner() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let maker_account = Account::new(1_000_000_000, 0, &system_program);
    let (escrow, _) = Address::find_program_address(&[b"escrow", maker.as_ref()], &crate::ID);
    let escrow_account = Account::default();

    let mint_a = Address::new_unique();
    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mint_b = Address::new_unique();
    let mint_b_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint_a, maker, 1_000_000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Wrong token owner — should be maker but is someone else
    let wrong_owner = Address::new_unique();
    let maker_ta_b = Address::new_unique();
    let maker_ta_b_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_b, wrong_owner, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Pre-existing so the test reaches maker_ta_b validation
    let vault_ta_a = Address::new_unique();
    let vault_ta_a_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_a, escrow, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let mut instruction: Instruction = MakeInstruction {
        maker,
        escrow,
        mint_a,
        mint_b,
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
    instruction.accounts[0].is_signer = true; // maker

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (mint_a, mint_a_account),
            (mint_b, mint_b_account),
            (maker_ta_a, maker_ta_a_account),
            (maker_ta_b, maker_ta_b_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "make should fail with wrong owner on maker_ta_b"
    );
    std::println!("  make with wrong owner rejected: OK");
}

#[test]
fn test_take_existing_token_accounts() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let taker = Address::new_unique();
    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mint_b_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

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

    // Already initialized — taker's mint_a token account
    let taker_ta_a = Address::new_unique();
    let taker_ta_a_account = Account {
        lamports: 2_039_280,
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

    // Already initialized — maker's mint_b token account
    let maker_ta_b_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_b, maker, 500),
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

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let mut instruction: Instruction = TakeInstruction {
        taker,
        escrow,
        maker,
        mint_a,
        mint_b,
        taker_ta_a,
        taker_ta_b,
        maker_ta_b,
        vault_ta_a,
        rent,
        token_program,
        system_program,
    }
    .into();
    instruction.accounts[0].is_signer = true; // taker

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (taker, taker_account),
            (escrow, escrow_account),
            (maker, maker_account),
            (mint_a, mint_a_account),
            (mint_b, mint_b_account),
            (taker_ta_a, taker_ta_a_account),
            (taker_ta_b, taker_ta_b_account),
            (maker_ta_b, maker_ta_b_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "take with existing token accounts failed: {:?}",
        result.program_result
    );
    std::println!(
        "  take with existing token accounts: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_refund_existing_maker_ta_a() {
    let mollusk = setup();

    let (token_program, token_program_account) = mollusk_svm_programs_token::token::keyed_account();
    let (system_program, system_program_account) = keyed_account_for_system_program();

    let maker = Address::new_unique();
    let mint_a = Address::new_unique();
    let mint_b = Address::new_unique();

    let mint_a_account = Account {
        lamports: 1_000_000,
        data: pack_mint(maker, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

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

    // Already initialized — maker's mint_a token account
    let maker_ta_a = Address::new_unique();
    let maker_ta_a_account = Account {
        lamports: 2_039_280,
        data: pack_token(mint_a, maker, 5_000),
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

    let (rent, rent_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let mut instruction: Instruction = RefundInstruction {
        maker,
        escrow,
        mint_a,
        maker_ta_a,
        vault_ta_a,
        rent,
        token_program,
        system_program,
    }
    .into();
    instruction.accounts[0].is_signer = true; // maker

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (maker, maker_account),
            (escrow, escrow_account),
            (mint_a, mint_a_account),
            (maker_ta_a, maker_ta_a_account),
            (vault_ta_a, vault_ta_a_account),
            (rent, rent_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "refund with existing maker_ta_a failed: {:?}",
        result.program_result
    );
    std::println!(
        "  refund with existing maker_ta_a: OK (CU: {})",
        result.compute_units_consumed
    );
}
