extern crate std;
use {
    alloc::vec,
    quasar_escrow_client::*,
    quasar_svm::{Account, Instruction, Pubkey, QuasarSvm},
    spl_token_interface::state::{Account as TokenAccount, AccountState, Mint},
    std::println,
};

// Deterministic addresses — avoids Pubkey::new_unique() whose global counter
// produces different values depending on test binary layout / discovery order.
const MAKER: Pubkey = Pubkey::new_from_array([1; 32]);
const TAKER: Pubkey = Pubkey::new_from_array([2; 32]);
const MINT_A: Pubkey = Pubkey::new_from_array([3; 32]);
const MINT_B: Pubkey = Pubkey::new_from_array([4; 32]);
const MAKER_TA_A: Pubkey = Pubkey::new_from_array([5; 32]);
const MAKER_TA_B: Pubkey = Pubkey::new_from_array([6; 32]);
const VAULT_TA_A: Pubkey = Pubkey::new_from_array([7; 32]);
const TAKER_TA_A: Pubkey = Pubkey::new_from_array([8; 32]);
const TAKER_TA_B: Pubkey = Pubkey::new_from_array([9; 32]);
const WRONG_OWNER: Pubkey = Pubkey::new_from_array([10; 32]);

fn setup() -> QuasarSvm {
    let elf = std::fs::read("../../target/deploy/quasar_escrow.so").unwrap();
    QuasarSvm::new()
        .with_program(&crate::ID, &elf)
        .with_token_program()
}

fn signer(address: Pubkey) -> Account {
    quasar_svm::token::create_keyed_system_account(&address, 1_000_000_000)
}

fn empty(address: Pubkey) -> Account {
    Account {
        address,
        lamports: 0,
        data: vec![],
        owner: quasar_svm::system_program::ID,
        executable: false,
    }
}

fn mint(address: Pubkey, authority: Pubkey) -> Account {
    quasar_svm::token::create_keyed_mint_account(
        &address,
        &Mint {
            mint_authority: Some(authority).into(),
            supply: 1_000_000_000,
            decimals: 9,
            is_initialized: true,
            freeze_authority: None.into(),
        },
    )
}

fn token(address: Pubkey, mint: Pubkey, owner: Pubkey, amount: u64) -> Account {
    quasar_svm::token::create_keyed_token_account(
        &address,
        &TokenAccount {
            mint,
            owner,
            amount,
            state: AccountState::Initialized,
            ..TokenAccount::default()
        },
    )
}

fn escrow_account(
    address: Pubkey,
    maker: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    maker_ta_b: Pubkey,
    receive: u64,
    bump: u8,
) -> Account {
    let escrow = Escrow {
        maker,
        mint_a,
        mint_b,
        maker_ta_b,
        receive,
        bump,
    };
    Account {
        address,
        lamports: 2_000_000,
        data: wincode::serialize(&escrow).unwrap(),
        owner: crate::ID,
        executable: false,
    }
}

/// Mark specific account indices as signers on an instruction.
fn with_signers(mut ix: Instruction, indices: &[usize]) -> Instruction {
    for &i in indices {
        ix.accounts[i].is_signer = true;
    }
    ix
}

#[test]
fn test_make_cu() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction = with_signers(
        MakeInstruction {
            maker: MAKER,
            escrow,
            mint_a: MINT_A,
            mint_b: MINT_B,
            maker_ta_a: MAKER_TA_A,
            maker_ta_b: MAKER_TA_B,
            vault_ta_a: VAULT_TA_A,
            rent,
            token_program,
            system_program,
            deposit: 1337,
            receive: 1337,
        }
        .into(),
        &[5, 6],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(MAKER),
            empty(escrow),
            mint(MINT_A, MAKER),
            mint(MINT_B, MAKER),
            token(MAKER_TA_A, MINT_A, MAKER, 1_000_000),
            empty(MAKER_TA_B),
            empty(VAULT_TA_A),
        ],
    );

    assert!(result.is_ok(), "make failed: {:?}", result.raw_result);

    let escrow_data = &result.account(&escrow).unwrap().data;
    assert_eq!(escrow_data[0], 1, "discriminator");
    assert_eq!(&escrow_data[1..33], MAKER.as_ref(), "maker");
    assert_eq!(&escrow_data[129..137], &1337u64.to_le_bytes(), "receive");
    assert_eq!(escrow_data[137], escrow_bump, "bump");

    println!("  MAKE CU: {}", result.compute_units_consumed);
}

#[test]
fn test_take_cu() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction = with_signers(
        TakeInstruction {
            taker: TAKER,
            escrow,
            maker: MAKER,
            mint_a: MINT_A,
            mint_b: MINT_B,
            taker_ta_a: TAKER_TA_A,
            taker_ta_b: TAKER_TA_B,
            maker_ta_b: MAKER_TA_B,
            vault_ta_a: VAULT_TA_A,
            rent,
            token_program,
            system_program,
        }
        .into(),
        &[5, 7],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(TAKER),
            escrow_account(escrow, MAKER, MINT_A, MINT_B, MAKER_TA_B, 1337, escrow_bump),
            signer(MAKER),
            mint(MINT_A, MAKER),
            mint(MINT_B, MAKER),
            empty(TAKER_TA_A),
            token(TAKER_TA_B, MINT_B, TAKER, 10_000),
            empty(MAKER_TA_B),
            token(VAULT_TA_A, MINT_A, escrow, 1337),
        ],
    );

    assert!(result.is_ok(), "take failed: {:?}", result.raw_result);
    println!("  TAKE CU: {}", result.compute_units_consumed);
}

#[test]
fn test_refund_cu() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction = with_signers(
        RefundInstruction {
            maker: MAKER,
            escrow,
            mint_a: MINT_A,
            maker_ta_a: MAKER_TA_A,
            vault_ta_a: VAULT_TA_A,
            rent,
            token_program,
            system_program,
        }
        .into(),
        &[3],
    );

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(MAKER),
            escrow_account(escrow, MAKER, MINT_A, MINT_B, MAKER_TA_B, 1337, escrow_bump),
            mint(MINT_A, MAKER),
            empty(MAKER_TA_A),
            token(VAULT_TA_A, MINT_A, escrow, 1337),
        ],
    );

    assert!(result.is_ok(), "refund failed: {:?}", result.raw_result);
    println!("  REFUND CU: {}", result.compute_units_consumed);
}

// ---------------------------------------------------------------------------
// init_if_needed: pre-existing token accounts
// ---------------------------------------------------------------------------

#[test]
fn test_make_existing_token_accounts() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, _) = Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction: Instruction = MakeInstruction {
        maker: MAKER,
        escrow,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        rent,
        token_program,
        system_program,
        deposit: 1337,
        receive: 1337,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(MAKER),
            empty(escrow),
            mint(MINT_A, MAKER),
            mint(MINT_B, MAKER),
            token(MAKER_TA_A, MINT_A, MAKER, 1_000_000),
            token(MAKER_TA_B, MINT_B, MAKER, 0),
            token(VAULT_TA_A, MINT_A, escrow, 0),
        ],
    );

    assert!(
        result.is_ok(),
        "make with existing token accounts failed: {:?}",
        result.raw_result
    );
    println!(
        "  make with existing token accounts: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_make_existing_maker_ta_b_wrong_mint() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, _) = Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction: Instruction = MakeInstruction {
        maker: MAKER,
        escrow,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        rent,
        token_program,
        system_program,
        deposit: 1337,
        receive: 1337,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(MAKER),
            empty(escrow),
            mint(MINT_A, MAKER),
            mint(MINT_B, MAKER),
            token(MAKER_TA_A, MINT_A, MAKER, 1_000_000),
            token(MAKER_TA_B, MINT_A, MAKER, 0), // wrong mint
            token(VAULT_TA_A, MINT_A, escrow, 0),
        ],
    );

    assert!(
        result.is_err(),
        "make should fail with wrong mint on maker_ta_b"
    );
}

#[test]
fn test_make_existing_maker_ta_b_wrong_owner() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, _) = Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction: Instruction = MakeInstruction {
        maker: MAKER,
        escrow,
        mint_a: MINT_A,
        mint_b: MINT_B,
        maker_ta_a: MAKER_TA_A,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        rent,
        token_program,
        system_program,
        deposit: 1337,
        receive: 1337,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(MAKER),
            empty(escrow),
            mint(MINT_A, MAKER),
            mint(MINT_B, MAKER),
            token(MAKER_TA_A, MINT_A, MAKER, 1_000_000),
            token(MAKER_TA_B, MINT_B, WRONG_OWNER, 0), // wrong owner
            token(VAULT_TA_A, MINT_A, escrow, 0),
        ],
    );

    assert!(
        result.is_err(),
        "make should fail with wrong owner on maker_ta_b"
    );
}

#[test]
fn test_take_existing_token_accounts() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction: Instruction = TakeInstruction {
        taker: TAKER,
        escrow,
        maker: MAKER,
        mint_a: MINT_A,
        mint_b: MINT_B,
        taker_ta_a: TAKER_TA_A,
        taker_ta_b: TAKER_TA_B,
        maker_ta_b: MAKER_TA_B,
        vault_ta_a: VAULT_TA_A,
        rent,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(TAKER),
            escrow_account(escrow, MAKER, MINT_A, MINT_B, MAKER_TA_B, 1337, escrow_bump),
            signer(MAKER),
            mint(MINT_A, MAKER),
            mint(MINT_B, MAKER),
            token(TAKER_TA_A, MINT_A, TAKER, 0),
            token(TAKER_TA_B, MINT_B, TAKER, 10_000),
            token(MAKER_TA_B, MINT_B, MAKER, 500),
            token(VAULT_TA_A, MINT_A, escrow, 1337),
        ],
    );

    assert!(
        result.is_ok(),
        "take with existing token accounts failed: {:?}",
        result.raw_result
    );
    println!(
        "  take with existing token accounts: OK (CU: {})",
        result.compute_units_consumed
    );
}

#[test]
fn test_refund_existing_maker_ta_a() {
    let mut svm = setup();

    let token_program = quasar_svm::SPL_TOKEN_PROGRAM_ID;
    let system_program = quasar_svm::system_program::ID;
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[b"escrow", MAKER.as_ref()], &crate::ID);
    let rent = quasar_svm::solana_sdk_ids::sysvar::rent::ID;

    let instruction: Instruction = RefundInstruction {
        maker: MAKER,
        escrow,
        mint_a: MINT_A,
        maker_ta_a: MAKER_TA_A,
        vault_ta_a: VAULT_TA_A,
        rent,
        token_program,
        system_program,
    }
    .into();

    let result = svm.process_instruction(
        &instruction,
        &[
            signer(MAKER),
            escrow_account(escrow, MAKER, MINT_A, MINT_B, MAKER_TA_B, 1337, escrow_bump),
            mint(MINT_A, MAKER),
            token(MAKER_TA_A, MINT_A, MAKER, 5_000),
            token(VAULT_TA_A, MINT_A, escrow, 1337),
        ],
    );

    assert!(
        result.is_ok(),
        "refund with existing maker_ta_a failed: {:?}",
        result.raw_result
    );
    println!(
        "  refund with existing maker_ta_a: OK (CU: {})",
        result.compute_units_consumed
    );
}
