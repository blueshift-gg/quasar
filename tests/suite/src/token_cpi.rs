use mollusk_svm::Mollusk;
use quasar_spl::get_associated_token_address_const;
use quasar_test_token_cpi::client::*;
use solana_account::Account;
use solana_address::Address;
use solana_instruction::Instruction;
use solana_program_pack::Pack;
use spl_token_interface::state::Account as TokenAccount;
use spl_token_interface::state::AccountState;
use spl_token_interface::state::Mint;

fn setup() -> Mollusk {
    let mut mollusk = Mollusk::new(
        &quasar_test_token_cpi::ID,
        "../../target/deploy/quasar_test_token_cpi",
    );
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk
}

fn pack_token(mint: Address, owner: Address, amount: u64) -> Vec<u8> {
    let token = TokenAccount {
        mint,
        owner,
        amount,
        delegate: None.into(),
        state: AccountState::Initialized,
        is_native: None.into(),
        delegated_amount: 0,
        close_authority: None.into(),
    };
    let mut data = vec![0u8; TokenAccount::LEN];
    Pack::pack(token, &mut data).unwrap();
    data
}

fn pack_token_with_delegate(
    mint: Address,
    owner: Address,
    amount: u64,
    delegate: Address,
    delegated_amount: u64,
) -> Vec<u8> {
    let token = TokenAccount {
        mint,
        owner,
        amount,
        delegate: Some(delegate).into(),
        state: AccountState::Initialized,
        is_native: None.into(),
        delegated_amount,
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

fn token_program_account() -> (Address, Account) {
    mollusk_svm_programs_token::token::keyed_account()
}

#[test]
fn test_transfer_checked_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let to = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(authority, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 500),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = TransferCheckedInstruction {
        authority,
        from,
        mint,
        to,
        token_program,
        amount: 200,
        decimals: 9,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (mint, mint_account),
            (to, to_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "transfer_checked failed: {:?}",
        result.program_result
    );
    let from_data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    let to_data: TokenAccount = Pack::unpack(&result.resulting_accounts[3].1.data).unwrap();
    assert_eq!(from_data.amount, 300, "from balance should be 300");
    assert_eq!(to_data.amount, 200, "to balance should be 200");
    println!(
        "  transfer_checked: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_transfer_checked_wrong_decimals() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let to = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(authority, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 500),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = TransferCheckedInstruction {
        authority,
        from,
        mint,
        to,
        token_program,
        amount: 200,
        decimals: 6,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (mint, mint_account),
            (to, to_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "transfer_checked should fail with wrong decimals"
    );
}

#[test]
fn test_approve_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let source = Address::new_unique();
    let delegate = Address::new_unique();
    let source_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 1000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let delegate_account = Account::new(1_000_000, 0, &Address::default());
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = ApproveInstruction {
        authority,
        source,
        delegate,
        token_program,
        amount: 500,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (source, source_account),
            (delegate, delegate_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "approve failed: {:?}",
        result.program_result
    );
    let source_data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(
        Option::<Address>::from(source_data.delegate),
        Some(delegate),
        "delegate should be set"
    );
    assert_eq!(
        source_data.delegated_amount, 500,
        "delegated_amount should be 500"
    );
    println!("  approve: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_revoke_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let source = Address::new_unique();
    let delegate = Address::new_unique();
    let source_account = Account {
        lamports: 1_000_000,
        data: pack_token_with_delegate(mint, authority, 1000, delegate, 500),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = RevokeInstruction {
        authority,
        source,
        token_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (source, source_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "revoke failed: {:?}",
        result.program_result
    );
    let source_data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(
        Option::<Address>::from(source_data.delegate),
        None,
        "delegate should be cleared"
    );
    println!("  revoke: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_mint_to_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let to = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(authority, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = MintToInstruction {
        authority,
        mint,
        to,
        token_program,
        amount: 5000,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (mint, mint_account),
            (to, to_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "mint_to failed: {:?}",
        result.program_result
    );
    let to_data: TokenAccount = Pack::unpack(&result.resulting_accounts[2].1.data).unwrap();
    let mint_data: Mint = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(to_data.amount, 5000, "to balance should be 5000");
    assert_eq!(
        mint_data.supply,
        1_000_000_000 + 5000,
        "supply should increase by 5000"
    );
    println!("  mint_to: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_mint_to_wrong_authority() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let real_authority = Address::new_unique();
    let fake_authority = Address::new_unique();
    let mint = Address::new_unique();
    let to = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(real_authority, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, real_authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let fake_authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = MintToInstruction {
        authority: fake_authority,
        mint,
        to,
        token_program,
        amount: 5000,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (fake_authority, fake_authority_account),
            (mint, mint_account),
            (to, to_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "mint_to should fail with wrong authority"
    );
}

#[test]
fn test_burn_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(authority, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 1000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = BurnInstruction {
        authority,
        from,
        mint,
        token_program,
        amount: 300,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (mint, mint_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "burn failed: {:?}",
        result.program_result
    );
    let from_data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    let mint_data: Mint = Pack::unpack(&result.resulting_accounts[2].1.data).unwrap();
    assert_eq!(from_data.amount, 700, "from balance should be 700");
    assert_eq!(
        mint_data.supply,
        1_000_000_000 - 300,
        "supply should decrease by 300"
    );
    println!("  burn: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_burn_insufficient_balance() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(authority, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = BurnInstruction {
        authority,
        from,
        mint,
        token_program,
        amount: 500,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (mint, mint_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "burn should fail with insufficient balance"
    );
}

#[test]
fn test_close_account_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let account = Address::new_unique();
    let token_data = pack_token(mint, authority, 0);
    let account_lamports = 2_000_000u64;
    let account_acct = Account {
        lamports: account_lamports,
        data: token_data,
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_lamports = 1_000_000u64;
    let authority_account = Account::new(authority_lamports, 0, &Address::default());
    let instruction: Instruction = CloseTokenAccountInstruction {
        authority,
        account,
        destination: authority,
        token_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (account, account_acct),
            (
                authority,
                Account::new(authority_lamports, 0, &Address::default()),
            ),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "close_account failed: {:?}",
        result.program_result
    );
    let closed = &result.resulting_accounts[1].1;
    assert_eq!(closed.lamports, 0, "closed account should have 0 lamports");
    assert!(
        closed.data.iter().all(|&b| b == 0),
        "closed account data should be zeroed"
    );
    println!(
        "  close_account: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_close_account_nonzero_balance_fails() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let account = Address::new_unique();
    let account_acct = Account {
        lamports: 2_000_000,
        data: pack_token(mint, authority, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = CloseTokenAccountInstruction {
        authority,
        account,
        destination: authority,
        token_program,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (account, account_acct),
            (authority, Account::new(1_000_000, 0, &Address::default())),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "close_account should fail with nonzero token balance"
    );
}

#[test]
fn test_interface_transfer_spl_token() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let to = Address::new_unique();
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 1000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = InterfaceTransferInstruction {
        authority,
        from,
        to,
        token_program,
        amount: 400,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (to, to_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "interface_transfer (SPL Token) failed: {:?}",
        result.program_result
    );
    let from_data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    let to_data: TokenAccount = Pack::unpack(&result.resulting_accounts[2].1.data).unwrap();
    assert_eq!(from_data.amount, 600, "from balance should be 600");
    assert_eq!(to_data.amount, 400, "to balance should be 400");
    println!(
        "  interface_transfer (SPL Token): OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_interface_account_wrong_owner() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let to = Address::new_unique();
    let wrong_program = Address::new_unique();
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 1000),
        owner: wrong_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let instruction: Instruction = InterfaceTransferInstruction {
        authority,
        from,
        to,
        token_program,
        amount: 400,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (to, to_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "interface_transfer should fail with wrong owner on from account"
    );
}

#[test]
fn test_interface_wrong_program() {
    let mollusk = setup();
    let (token_program, _token_program_account) = token_program_account();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let to = Address::new_unique();
    let fake_program = Address::new_unique();
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 1000),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());
    let fake_program_account = Account {
        lamports: 1_000_000,
        data: vec![],
        owner: Address::default(),
        executable: true,
        rent_epoch: 0,
    };
    let instruction: Instruction = InterfaceTransferInstruction {
        authority,
        from,
        to,
        token_program: fake_program,
        amount: 400,
    }
    .into();
    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (to, to_account),
            (fake_program, fake_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "interface_transfer should fail with non-token program"
    );
}

// ---------------------------------------------------------------------------
// ATA validation tests (associated_token::mint + associated_token::authority)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_ata_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let wallet = Address::new_unique();
    let mint = Address::new_unique();
    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);

    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(wallet, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let ata_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, wallet, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_addr,
        mint,
        wallet,
        token_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (ata_addr, ata_account),
            (mint, mint_account),
            (wallet, wallet_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_ok(),
        "validate_ata should pass with correct ATA: {:?}",
        result.program_result
    );
}

#[test]
fn test_validate_ata_wrong_address() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let wallet = Address::new_unique();
    let mint = Address::new_unique();
    let wrong_ata = Address::new_unique();

    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(wallet, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let ata_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, wallet, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: wrong_ata,
        mint,
        wallet,
        token_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (wrong_ata, ata_account),
            (mint, mint_account),
            (wallet, wallet_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "validate_ata should fail with wrong ATA address"
    );
}

#[test]
fn test_validate_ata_wrong_mint() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let wallet = Address::new_unique();
    let mint = Address::new_unique();
    let wrong_mint = Address::new_unique();
    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);

    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(wallet, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let ata_account = Account {
        lamports: 1_000_000,
        data: pack_token(wrong_mint, wallet, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_addr,
        mint,
        wallet,
        token_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (ata_addr, ata_account),
            (mint, mint_account),
            (wallet, wallet_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "validate_ata should fail when token account has wrong mint"
    );
}

#[test]
fn test_validate_ata_wrong_authority() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let wallet = Address::new_unique();
    let wrong_wallet = Address::new_unique();
    let mint = Address::new_unique();
    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);

    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(wallet, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let ata_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, wrong_wallet, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let instruction: Instruction = ValidateAtaCheckInstruction {
        ata: ata_addr,
        mint,
        wallet,
        token_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (ata_addr, ata_account),
            (mint, mint_account),
            (wallet, wallet_account),
            (token_program, token_program_account),
        ],
    );
    assert!(
        result.program_result.is_err(),
        "validate_ata should fail when token account has wrong authority"
    );
}

// ---------------------------------------------------------------------------
// Init token account (#[account(init, token::mint = ..., token::authority = ...)])
// ---------------------------------------------------------------------------

#[test]
fn test_init_token_account_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let token_account = Address::new_unique();
    let token_account_obj = Account::default();

    let mut instruction: Instruction = InitTokenAccountInstruction {
        payer,
        token_account,
        mint,
        token_program,
        system_program,
    }
    .into();

    // create_account requires the new account to be a signer
    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (token_account, token_account_obj),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_token_account should succeed: {:?}",
        result.program_result
    );

    let data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(data.mint, mint, "token account mint should match");
    assert_eq!(data.owner, payer, "token account authority should be payer");
    assert_eq!(data.amount, 0, "token account balance should be 0");
    assert_eq!(
        result.resulting_accounts[1].1.owner, token_program,
        "token account owner should be token program"
    );
    println!(
        "  init_token_account: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_init_token_account_already_initialized() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    // Token account that already exists (owned by token program)
    let token_account = Address::new_unique();
    let token_account_obj = Account {
        lamports: 1_000_000,
        data: pack_token(mint, payer, 100),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = InitTokenAccountInstruction {
        payer,
        token_account,
        mint,
        token_program,
        system_program,
    }
    .into();

    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (token_account, token_account_obj),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_token_account should fail when account already initialized"
    );
}

// ---------------------------------------------------------------------------
// Executable check (checks::Executable negative test)
// ---------------------------------------------------------------------------

#[test]
fn test_executable_check_non_executable_program() {
    let mollusk = setup();
    let authority = Address::new_unique();
    let mint = Address::new_unique();
    let from = Address::new_unique();
    let to = Address::new_unique();

    let (real_token_program, _) = token_program_account();

    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(authority, 9),
        owner: real_token_program,
        executable: false,
        rent_epoch: 0,
    };
    let from_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 500),
        owner: real_token_program,
        executable: false,
        rent_epoch: 0,
    };
    let to_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, authority, 0),
        owner: real_token_program,
        executable: false,
        rent_epoch: 0,
    };
    let authority_account = Account::new(1_000_000, 0, &Address::default());

    // Pass the correct token program address but with executable = false
    let non_executable_program = Account {
        lamports: 1_000_000,
        data: vec![],
        owner: Address::default(),
        executable: false, // NOT executable
        rent_epoch: 0,
    };

    let instruction: Instruction = TransferCheckedInstruction {
        authority,
        from,
        mint,
        to,
        token_program: real_token_program,
        amount: 200,
        decimals: 9,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (authority, authority_account),
            (from, from_account),
            (mint, mint_account),
            (to, to_account),
            (real_token_program, non_executable_program),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "should fail when token program account is not executable"
    );
}

// ---------------------------------------------------------------------------
// Init-if-needed token account (#[account(init_if_needed, token::mint, token::authority)])
// ---------------------------------------------------------------------------

fn setup_with_ata() -> Mollusk {
    let mut mollusk = Mollusk::new(
        &quasar_test_token_cpi::ID,
        "../../target/deploy/quasar_test_token_cpi",
    );
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::associated_token::add_program(&mut mollusk);
    mollusk
}

#[test]
fn test_init_if_needed_token_new_account() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let token_account = Address::new_unique();
    let token_account_obj = Account::default();

    let mut instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account,
        mint,
        token_program,
        system_program,
    }
    .into();

    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (token_account, token_account_obj),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_if_needed should succeed for new account: {:?}",
        result.program_result
    );

    let data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(data.mint, mint);
    assert_eq!(data.owner, payer);
    assert_eq!(data.amount, 0);
    println!(
        "  init_if_needed_token (new): OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_init_if_needed_token_existing_valid() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let token_account = Address::new_unique();
    let token_account_obj = Account {
        lamports: 1_000_000,
        data: pack_token(mint, payer, 500),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account,
        mint,
        token_program,
        system_program,
    }
    .into();

    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (token_account, token_account_obj),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_if_needed should pass for existing valid account: {:?}",
        result.program_result
    );
    println!("  init_if_needed_token (existing valid): OK");
}

#[test]
fn test_init_if_needed_token_wrong_mint() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint = Address::new_unique();
    let wrong_mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let token_account = Address::new_unique();
    let token_account_obj = Account {
        lamports: 1_000_000,
        data: pack_token(wrong_mint, payer, 500),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = InitIfNeededTokenInstruction {
        payer,
        token_account,
        mint,
        token_program,
        system_program,
    }
    .into();

    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (token_account, token_account_obj),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_if_needed should fail when existing account has wrong mint"
    );
}

// ---------------------------------------------------------------------------
// Init ATA (#[account(init, associated_token::mint, associated_token::authority)])
// ---------------------------------------------------------------------------

fn ata_program_account() -> (Address, Account) {
    mollusk_svm_programs_token::associated_token::keyed_account()
}

#[test]
fn test_init_ata_success() {
    let mollusk = setup_with_ata();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();
    let (ata_program, ata_program_account) = ata_program_account();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let wallet = Address::new_unique();
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);
    let ata_account = Account::default();

    let instruction: Instruction = InitAtaInstruction {
        payer,
        ata: ata_addr,
        wallet,
        mint,
        token_program,
        system_program,
        ata_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (ata_addr, ata_account),
            (wallet, wallet_account),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
            (ata_program, ata_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_ata should succeed: {:?}",
        result.program_result
    );

    let data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(data.mint, mint, "ATA mint should match");
    assert_eq!(data.owner, wallet, "ATA owner should be wallet");
    assert_eq!(data.amount, 0, "ATA balance should be 0");
    println!("  init_ata: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_init_ata_already_initialized() {
    let mollusk = setup_with_ata();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();
    let (ata_program, ata_program_account) = ata_program_account();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let wallet = Address::new_unique();
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);
    let ata_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, wallet, 0),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitAtaInstruction {
        payer,
        ata: ata_addr,
        wallet,
        mint,
        token_program,
        system_program,
        ata_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (ata_addr, ata_account),
            (wallet, wallet_account),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
            (ata_program, ata_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_ata should fail when ATA already exists"
    );
}

// ---------------------------------------------------------------------------
// Init-if-needed ATA (#[account(init_if_needed, associated_token::...)])
// ---------------------------------------------------------------------------

#[test]
fn test_init_if_needed_ata_new() {
    let mollusk = setup_with_ata();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();
    let (ata_program, ata_program_account) = ata_program_account();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let wallet = Address::new_unique();
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);
    let ata_account = Account::default();

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_addr,
        wallet,
        mint,
        token_program,
        system_program,
        ata_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (ata_addr, ata_account),
            (wallet, wallet_account),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
            (ata_program, ata_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_if_needed_ata should succeed for new account: {:?}",
        result.program_result
    );

    let data: TokenAccount = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(data.mint, mint);
    assert_eq!(data.owner, wallet);
    println!(
        "  init_if_needed_ata (new): OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_init_if_needed_ata_existing_valid() {
    let mollusk = setup_with_ata();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();
    let (ata_program, ata_program_account) = ata_program_account();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let wallet = Address::new_unique();
    let wallet_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(payer, 9),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let (ata_addr, _) = get_associated_token_address_const(&wallet, &mint);
    let ata_account = Account {
        lamports: 1_000_000,
        data: pack_token(mint, wallet, 200),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let instruction: Instruction = InitIfNeededAtaInstruction {
        payer,
        ata: ata_addr,
        wallet,
        mint,
        token_program,
        system_program,
        ata_program,
    }
    .into();

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (ata_addr, ata_account),
            (wallet, wallet_account),
            (mint, mint_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
            (ata_program, ata_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_if_needed_ata should pass for existing valid ATA: {:?}",
        result.program_result
    );
    println!("  init_if_needed_ata (existing valid): OK");
}

// ---------------------------------------------------------------------------
// Mint init (#[account(init, mint::decimals, mint::authority)])
// ---------------------------------------------------------------------------

#[test]
fn test_init_mint_success() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint_authority = Address::new_unique();
    let mint_authority_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account::default();

    let mut instruction: Instruction = InitMintAccountInstruction {
        payer,
        mint,
        mint_authority,
        token_program,
        system_program,
    }
    .into();

    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (mint, mint_account),
            (mint_authority, mint_authority_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_mint should succeed: {:?}",
        result.program_result
    );

    let data: Mint = Pack::unpack(&result.resulting_accounts[1].1.data).unwrap();
    assert_eq!(data.decimals, 6, "mint decimals should be 6");
    assert_eq!(
        data.mint_authority,
        Some(mint_authority).into(),
        "mint authority should match"
    );
    assert!(data.is_initialized, "mint should be initialized");
    assert_eq!(
        result.resulting_accounts[1].1.owner, token_program,
        "mint owner should be token program"
    );
    println!("  init_mint: OK (CU: {})", result.compute_units_consumed);
}

#[test]
fn test_init_mint_already_initialized() {
    let mollusk = setup();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint_authority = Address::new_unique();
    let mint_authority_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account {
        lamports: 1_000_000,
        data: pack_mint(mint_authority, 6),
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    };

    let mut instruction: Instruction = InitMintAccountInstruction {
        payer,
        mint,
        mint_authority,
        token_program,
        system_program,
    }
    .into();

    instruction.accounts[1].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (mint, mint_account),
            (mint_authority, mint_authority_account),
            (token_program, token_program_account),
            (system_program, system_program_account),
        ],
    );

    assert!(
        result.program_result.is_err(),
        "init_mint should fail when mint already initialized"
    );
}

// ---------------------------------------------------------------------------
// Mint init + metadata CPI (#[account(init, mint::*, metadata::*)])
// ---------------------------------------------------------------------------

fn setup_with_metadata() -> Mollusk {
    let mut mollusk = Mollusk::new(
        &quasar_test_token_cpi::ID,
        "../../target/deploy/quasar_test_token_cpi",
    );
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk.add_program(
        &quasar_spl::metadata::METADATA_PROGRAM_ID,
        "../fixtures/mpl_token_metadata",
    );
    mollusk
}

fn metadata_pda(mint: &Address) -> (Address, u8) {
    Address::find_program_address(
        &[
            b"metadata",
            quasar_spl::metadata::METADATA_PROGRAM_ID.as_ref(),
            mint.as_ref(),
        ],
        &quasar_spl::metadata::METADATA_PROGRAM_ID,
    )
}

#[test]
fn test_init_mint_with_metadata_success() {
    let mollusk = setup_with_metadata();
    let (token_program, token_program_account) = token_program_account();
    let (system_program, system_program_account) =
        mollusk_svm::program::keyed_account_for_system_program();

    let payer = Address::new_unique();
    let payer_account = Account::new(10_000_000_000, 0, &system_program);

    let mint_authority = Address::new_unique();
    let mint_authority_account = Account::new(1_000_000, 0, &Address::default());

    let mint = Address::new_unique();
    let mint_account = Account::default();

    let (metadata_addr, _) = metadata_pda(&mint);
    let metadata_account = Account::default();

    let metadata_program_account = mollusk_svm::program::create_program_account_loader_v3(
        &quasar_spl::metadata::METADATA_PROGRAM_ID,
    );

    let (rent_sysvar, rent_sysvar_account) = mollusk.sysvars.keyed_account_for_rent_sysvar();

    let mut instruction: Instruction = InitMintWithMetadataInstruction {
        payer,
        mint_authority,
        mint,
        metadata: metadata_addr,
        metadata_program: quasar_spl::metadata::METADATA_PROGRAM_ID,
        token_program,
        system_program,
        rent: rent_sysvar,
    }
    .into();

    // create_account requires the new mint to be a signer
    instruction.accounts[2].is_signer = true;

    let result = mollusk.process_instruction(
        &instruction,
        &[
            (payer, payer_account),
            (mint_authority, mint_authority_account),
            (mint, mint_account),
            (metadata_addr, metadata_account),
            (
                quasar_spl::metadata::METADATA_PROGRAM_ID,
                metadata_program_account,
            ),
            (token_program, token_program_account),
            (system_program, system_program_account),
            (rent_sysvar, rent_sysvar_account),
        ],
    );

    assert!(
        result.program_result.is_ok(),
        "init_mint_with_metadata should succeed: {:?}",
        result.program_result
    );

    // Verify mint was initialized
    let mint_data: Mint = Pack::unpack(&result.resulting_accounts[2].1.data).unwrap();
    assert_eq!(mint_data.decimals, 0, "NFT mint decimals should be 0");
    assert!(mint_data.is_initialized, "mint should be initialized");

    // Verify metadata account was created (owned by metadata program)
    assert_eq!(
        result.resulting_accounts[3].1.owner,
        quasar_spl::metadata::METADATA_PROGRAM_ID,
        "metadata account should be owned by metadata program"
    );
    assert!(
        !result.resulting_accounts[3].1.data.is_empty(),
        "metadata account should have data"
    );

    println!(
        "  init_mint_with_metadata: OK (CU: {})",
        result.compute_units_consumed
    );
}
